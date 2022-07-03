#[cfg(target_os = "macos")]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

use async_trait::async_trait;
use parcel_css::bundler::{BundleErrorKind, Bundler, FileProvider, SourceProvider};
use parcel_css::css_modules::{CssModuleExports, CssModuleReferences, PatternParseError};
use parcel_css::dependencies::Dependency;
use parcel_css::error::{Error, ErrorLocation, MinifyErrorKind, ParserError, PrinterErrorKind};
use parcel_css::stylesheet::{
  MinifyOptions, ParserOptions, PrinterOptions, PseudoClasses, StyleAttribute, StyleSheet,
};
use parcel_css::targets::Browsers;
use parcel_sourcemap::SourceMap;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::runtime::Runtime;

use callback_future::CallbackFuture;

// ---------------------------------------------

#[cfg(target_arch = "wasm32")]
use serde_wasm_bindgen::{from_value, Serializer};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn transform(config_val: JsValue) -> Result<JsValue, JsValue> {
  let config: Config = from_value(config_val).map_err(JsValue::from)?;
  let code = unsafe { std::str::from_utf8_unchecked(&config.code) };
  let res = compile(code, &config)?;
  let serializer = Serializer::new().serialize_maps_as_objects(true);
  res.serialize(&serializer).map_err(JsValue::from)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "transformStyleAttribute")]
pub fn transform_style_attribute(config_val: JsValue) -> Result<JsValue, JsValue> {
  let config: AttrConfig = from_value(config_val).map_err(JsValue::from)?;
  let code = unsafe { std::str::from_utf8_unchecked(&config.code) };
  let res = compile_attr(code, &config)?;
  let serializer = Serializer::new().serialize_maps_as_objects(true);
  res.serialize(&serializer).map_err(JsValue::from)
}

// ---------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
use napi::threadsafe_function::{
  ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
#[cfg(not(target_arch = "wasm32"))]
use napi::{CallContext, JsFunction, JsObject, JsUnknown};
#[cfg(not(target_arch = "wasm32"))]
use napi_derive::{js_function, module_exports};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceMapJson<'a> {
  version: u8,
  mappings: String,
  sources: &'a Vec<String>,
  sources_content: &'a Vec<String>,
  names: &'a Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TransformResult {
  #[serde(with = "serde_bytes")]
  code: Vec<u8>,
  #[serde(with = "serde_bytes")]
  map: Option<Vec<u8>>,
  exports: Option<CssModuleExports>,
  references: Option<CssModuleReferences>,
  dependencies: Option<Vec<Dependency>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl TransformResult {
  fn into_js(self, ctx: CallContext) -> napi::Result<JsUnknown> {
    // Manually construct buffers so we avoid a copy and work around
    // https://github.com/napi-rs/napi-rs/issues/1124.
    let mut obj = ctx.env.create_object()?;
    let buf = ctx.env.create_buffer_with_data(self.code)?;
    obj.set_named_property("code", buf.into_raw())?;
    obj.set_named_property(
      "map",
      if let Some(map) = self.map {
        let buf = ctx.env.create_buffer_with_data(map)?;
        buf.into_raw().into_unknown()
      } else {
        ctx.env.get_null()?.into_unknown()
      },
    )?;
    obj.set_named_property("exports", ctx.env.to_js_value(&self.exports)?)?;
    obj.set_named_property("references", ctx.env.to_js_value(&self.references)?)?;
    obj.set_named_property("dependencies", ctx.env.to_js_value(&self.dependencies)?)?;
    Ok(obj.into_unknown())
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[js_function(1)]
fn transform(ctx: CallContext) -> napi::Result<JsUnknown> {
  let opts = ctx.get::<JsObject>(0)?;
  let config: Config = ctx.env.from_js_value(opts)?;
  let code = unsafe { std::str::from_utf8_unchecked(&config.code) };
  let res = compile(code, &config);

  match res {
    Ok(res) => res.into_js(ctx),
    Err(err) => err.throw(ctx, Some(code)),
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[js_function(1)]
fn transform_style_attribute(ctx: CallContext) -> napi::Result<JsUnknown> {
  let opts = ctx.get::<JsObject>(0)?;
  let config: AttrConfig = ctx.env.from_js_value(opts)?;
  let code = unsafe { std::str::from_utf8_unchecked(&config.code) };
  let res = compile_attr(code, &config);

  match res {
    Ok(res) => res.into_js(ctx),
    Err(err) => err.throw(ctx, Some(code)),
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[js_function(1)]
fn bundle(ctx: CallContext) -> napi::Result<JsUnknown> {
  let opts = ctx.get::<JsObject>(0)?;
  let config: BundleConfig = ctx.env.from_js_value(&opts)?;

  // Throw early error if user mistakenly passes `resolver` into synchronous `bundle()`.
  let resolver = opts.get::<&str, JsUnknown>("resolver")?;
  if resolver.is_some() {
    return Err(napi::Error::new(
      napi::Status::InvalidArg,
      "`bundle()` doesn't support custom JS resolvers but received a `resolver` property. Use `bundleAsync()` instead.".to_owned()),
    );
  }

  let fs = FileProvider::new();
  let async_runtime = Runtime::new()?;
  let res = async_runtime.block_on(compile_bundle(&fs, &config));

  match res {
    Ok(res) => res.into_js(ctx),
    Err(err) => {
      let code = match &err {
        CompileError::ParseError(Error {
          loc: Some(ErrorLocation { filename, .. }),
          ..
        })
        | CompileError::PrinterError(Error {
          loc: Some(ErrorLocation { filename, .. }),
          ..
        })
        | CompileError::MinifyError(Error {
          loc: Some(ErrorLocation { filename, .. }),
          ..
        })
        | CompileError::BundleError(Error {
          loc: Some(ErrorLocation { filename, .. }),
          ..
        }) => Some(async_runtime.block_on(fs.read(Path::new(filename)))?),
        _ => None,
      };
      err.throw(ctx, code)
    }
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[js_function(1)]
fn bundle_async(ctx: CallContext) -> napi::Result<JsObject> {
  // Parse JS arguments into Rust values so they can be used in the `Future`.
  let opts = ctx.get::<JsObject>(0)?;
  let config: BundleConfig = ctx.env.from_js_value(&opts)?;

  // Get `read()` and `resolve()` JS functions.
  let maybe_resolver = &opts.get::<&str, JsObject>("resolver")?;
  let maybe_unsafe_read = match &maybe_resolver {
    None => None,
    Some(resolver) => match resolver.get::<&str, JsUnknown>("read")? {
      None => None,
      Some(read) => {
        let read_type = read.get_type().unwrap();
        if read_type != napi::ValueType::Function {
          return Err(napi::Error::new(
            napi::Status::FunctionExpected,
            format!(
              "Expected `resolver.read` to be of type `{}` but was of type `{}`",
              napi::ValueType::Function,
              read_type
            ),
          ));
        }

        Some(unsafe { read.cast::<JsFunction>() })
      }
    },
  };
  let maybe_unsafe_resolve = match &maybe_resolver {
    None => None,
    Some(resolver) => match resolver.get::<&str, JsUnknown>("resolve")? {
      None => None,
      Some(resolve) => {
        let resolve_type = resolve.get_type().unwrap();
        if resolve_type != napi::ValueType::Function {
          return Err(napi::Error::new(
            napi::Status::FunctionExpected,
            format!(
              "Expected `resolver.resolve` to be of type `{}` but was of type `{}`.",
              napi::ValueType::Function,
              resolve_type
            ),
          ));
        }

        Some(unsafe { resolve.cast::<JsFunction>() })
      }
    },
  };

  // Map `read()` and `resolve()` to thread-safe N-API functions.
  let maybe_read: Option<ThreadsafeFunction<ReadArgs, ErrorStrategy::Fatal>> = match maybe_unsafe_read {
    None => None,
    Some(unsafe_read) => Some(unsafe_read.create_threadsafe_function(
      0, /* max_queue_size */
      // On the main thread, convert Rust args struct into JS arguments.
      |ctx: ThreadSafeCallContext<ReadArgs>| {
        Ok(vec![
          ctx.env.create_string(&ctx.value.file.to_str().unwrap())?.into_unknown(),
          ctx
            .env
            .create_function_from_closure("callback", ctx.value.callback)?
            .into_unknown(),
        ])
      },
    )?),
  };
  let maybe_resolve = match maybe_unsafe_resolve {
    None => None,
    Some(unsafe_resolve) => Some(unsafe_resolve.create_threadsafe_function(
      0, /* max_queue_size */
      // On the main thread, convert Rust args struct into JS arguments.
      |ctx: ThreadSafeCallContext<ResolveArgs>| {
        Ok(vec![
          ctx.env.create_string(&ctx.value.specifier)?.into_unknown(),
          ctx
            .env
            .create_string(ctx.value.originating_file.to_str().unwrap())?
            .into_unknown(),
          ctx
            .env
            .create_function_from_closure("callback", ctx.value.callback)?
            .into_unknown(),
        ])
      },
    )?),
  };

  // Execute asynchronous operation and return a `Promise` to JS.
  ctx.env.execute_tokio_future(
    // Perform asynchronous work, *cannot* access JS data from here.
    async {
      let source_provider = JsSourceProvider::new(FileProvider::new(), maybe_read, maybe_resolve);
      let res = compile_bundle(&source_provider, &config).await;
      drop(config);

      match res {
        Ok(transform_result) => Ok(transform_result),
        Err(compile_error) => Err(napi::Error::new(
          napi::Status::GenericFailure,
          compile_error.to_string(),
        )),
      }
    },
    // Convert the result from Rust data to JS data.
    |&mut env, transform_result| env.to_js_value(&transform_result),
  )
}

/// Arguments passed to the JS custom read function.
struct ReadArgs {
  file: PathBuf,
  callback: Box<dyn Fn(CallContext) -> napi::Result<napi::JsUndefined> + Send>,
}

/// Arguments passed to the JS custom resolve function.
struct ResolveArgs {
  specifier: String,
  originating_file: PathBuf,
  callback: Box<dyn Fn(CallContext) -> napi::Result<napi::JsUndefined> + Send>,
}

/// Buffer containing cached source inputs. This wrapper struct is only necessary to be
/// marked `Send` so it can be used with a `static` lifetime as required by
/// `ThreadsafeFunction`. Since `JsSourceProvider` uses this in an `Arc<Mutex<Buffer>>`
/// This can outlive the `JsSourceProvider` if necessary.
struct InputCache {
  inputs: Vec<*mut String>,
}

unsafe impl Send for InputCache {}

impl Drop for InputCache {
  fn drop(&mut self) {
    for ptr in self.inputs.iter() {
      std::mem::drop(unsafe { Box::from_raw(*ptr) });
    }
  }
}

/// A `SourceProvider` implementation which uses JS implementations where given, falling
/// back to a wrapped `SourceProvider` where not given.
struct JsSourceProvider<P>
where
  P: SourceProvider,
{
  /// Buffer with cached source files.
  cache: Arc<Mutex<InputCache>>,

  /// Fallback provider to use when no custom JS implementation is available for an
  /// operation.
  fallback_provider: P,

  /// Custom JS `read()` function.
  maybe_read: Option<ThreadsafeFunction<ReadArgs, ErrorStrategy::Fatal>>,

  /// Custom JS `resolve()` function.
  maybe_resolve: Option<ThreadsafeFunction<ResolveArgs, ErrorStrategy::Fatal>>,
}

impl<P: SourceProvider> JsSourceProvider<P> {
  /// Creates a new `JsSourceProvider`.
  fn new(
    fallback_provider: P,
    maybe_read: Option<ThreadsafeFunction<ReadArgs, ErrorStrategy::Fatal>>,
    maybe_resolve: Option<ThreadsafeFunction<ResolveArgs, ErrorStrategy::Fatal>>,
  ) -> JsSourceProvider<P> {
    JsSourceProvider {
      cache: Arc::new(Mutex::new(InputCache { inputs: Vec::new() })),
      fallback_provider,
      maybe_read,
      maybe_resolve,
    }
  }
}

// JS implementations use thread-safe functions, so `JsSourceProvider` is thread-safe as
// long as the fallback provider is also thread-safe.
unsafe impl<P: SourceProvider + Send> Send for JsSourceProvider<P> {}
unsafe impl<P: SourceProvider + Sync> Sync for JsSourceProvider<P> {}

#[async_trait]
impl<P: SourceProvider> SourceProvider for JsSourceProvider<P> {
  async fn read<'a>(&'a self, file: &Path) -> std::io::Result<&'a str> {
    let cache = self.cache.clone();
    match self.maybe_read.clone() {
      // Use fallback provider if no JS `read()` implementation exists.
      None => self.fallback_provider.read(file).await,
      // Use JS `read()` implementation.
      Some(read) => {
        let file = file.to_owned();

        // Wait until JS calls back with the result.
        CallbackFuture::<std::io::Result<&str>>::new(move |complete| {
          // `complete` can only be called once, but JS could invoke this callback
          // multiple times. Use a `Cell` to hide the mutability and restrict it to
          // only be called once.
          let complete = Cell::new(Some(complete));

          // Invoke the JS `read()` function.
          read.call(ReadArgs {
            file,
            // JS invokes this callback with the result. It follows Node async
            // conventions (`cb(error, result)`).
            callback: Box::new(move |ctx| {
              // Get completion callback. This function can only be called once by JS
              // so any subsequent executions should error immediately.
              let complete = complete.take().ok_or(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Callback invoked twice.",
              ))?;

              // Validate no errors were thrown.
              let error = ctx.get::<JsUnknown>(0)?;
              let error_type = error.get_type()?;
              if error_type != napi::ValueType::Null && error_type != napi::ValueType::Undefined {
                complete(Err(std::io::Error::new(std::io::ErrorKind::Other, format!(
                  "`read()` threw error:\n{}",
                  error.coerce_to_string()?.into_utf8()?.as_str()?.to_owned(),
                ))));
                return ctx.env.get_undefined();
              }

              // Validate that result was a string.
              let result = ctx.get::<JsUnknown>(1)?;
              let result_type = result.get_type()?;
              if result_type != napi::ValueType::String {
                complete(Err(std::io::Error::new(std::io::ErrorKind::Other, format!(
                  "Expected `read()` to return a value of type `{}`, but it returned a value of type `{}` instead.", napi::ValueType::String, result_type,
                ))));
                return ctx.env.get_undefined();
              }

              // Get file contents and add to the source file cache.
              let result = result.coerce_to_string()?.into_utf8()?.as_str()?.to_owned();
              let ptr = Box::into_raw(Box::new(result));
              cache.lock().unwrap().inputs.push(ptr);

              // SAFETY: this is safe because the pointer is not dropped until the
              // `JsSourceProvider` is, and we never remove from the list of pointers
              // stored in the vector.
              complete(Ok(unsafe { &*ptr }));

              ctx.env.get_undefined()
            }),
          }, ThreadsafeFunctionCallMode::Blocking);
        }).await
      }
    }
  }

  async fn resolve(&self, specifier: &str, originating_file: &Path) -> Result<PathBuf, Error<BundleErrorKind>> {
    match self.maybe_resolve.clone() {
      // Use fallback provider if no JS `resolve()` implementation exists.
      None => self.fallback_provider.resolve(specifier, originating_file).await,
      // Use JS `resolve()` implementation.
      Some(resolve) => {
        let specifier = specifier.to_owned();
        let originating_file = originating_file.to_path_buf().to_owned();

        // Wait until JS calls back with the result.
        CallbackFuture::<Result<PathBuf, Error<BundleErrorKind>>>::new(move |complete| {
          // `complete` can only be called once, but JS could invoke this callback
          // multiple times. Use a `Cell` to hide the mutability and restrict it to
          // only be called once.
          let complete = Cell::new(Some(complete));

          // Invoke the JS `resolve()` function.
          resolve.call(ResolveArgs {
            specifier,
            originating_file,
            // JS invokes this callback with the result.
            callback: Box::new(move |ctx| {
              // Get completion callback. This function can only be called once by JS
              // so any subsequent executions should error immediately.
              let complete = complete.take().ok_or(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Callback invoked twice.",
              ))?;

              // Check for error.
              let error = ctx.get::<JsUnknown>(0)?;
              let error_type = error.get_type()?;
              if error_type != napi::ValueType::Null && error_type != napi::ValueType::Undefined {
                complete(Err(Error {
                  kind: BundleErrorKind::IOError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("`resolve()` threw error:\n{}", error.coerce_to_string()?.into_utf8()?.as_str()?.to_owned()),
                  )),
                  loc: None,
                }));
                return ctx.env.get_undefined();
              }

              // Validate that result was a string.
              let result = ctx.get::<JsUnknown>(1)?;
              let result_type = result.get_type()?;
              if result_type != napi::ValueType::String {
                complete(Err(Error {
                  kind: BundleErrorKind::IOError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Expected `resolve()` to return a value of type `{}`, but it returned a value of type `{}` instead.", napi::ValueType::String, result_type),
                  )),
                  loc: None,
                }));
                return ctx.env.get_undefined();
              }

              // Convert JS string into a Rust `PathBuf`.
              let resolved = result.coerce_to_string()?.into_utf8()?.as_str()?.to_owned();
              let resolved = Path::new(&resolved).to_path_buf();
              complete(Ok(resolved));

              ctx.env.get_undefined()
            }),
          }, ThreadsafeFunctionCallMode::Blocking);
        }).await
      }
    }
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[module_exports]
fn init(mut exports: JsObject) -> napi::Result<()> {
  exports.create_named_method("transform", transform)?;
  exports.create_named_method("transformStyleAttribute", transform_style_attribute)?;
  exports.create_named_method("bundle", bundle)?;
  exports.create_named_method("bundleAsync", bundle_async)?;

  Ok(())
}

// ---------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
  pub filename: String,
  #[serde(with = "serde_bytes")]
  pub code: Vec<u8>,
  pub targets: Option<Browsers>,
  pub minify: Option<bool>,
  pub source_map: Option<bool>,
  pub drafts: Option<Drafts>,
  pub css_modules: Option<CssModulesOption>,
  pub analyze_dependencies: Option<bool>,
  pub pseudo_classes: Option<OwnedPseudoClasses>,
  pub unused_symbols: Option<HashSet<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CssModulesOption {
  Bool(bool),
  Config(CssModulesConfig),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CssModulesConfig {
  pattern: Option<String>,
  #[serde(default)]
  dashed_idents: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleConfig {
  pub filename: String,
  pub targets: Option<Browsers>,
  pub minify: Option<bool>,
  pub source_map: Option<bool>,
  pub drafts: Option<Drafts>,
  pub css_modules: Option<CssModulesOption>,
  pub analyze_dependencies: Option<bool>,
  pub pseudo_classes: Option<OwnedPseudoClasses>,
  pub unused_symbols: Option<HashSet<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnedPseudoClasses {
  pub hover: Option<String>,
  pub active: Option<String>,
  pub focus: Option<String>,
  pub focus_visible: Option<String>,
  pub focus_within: Option<String>,
}

impl<'a> Into<PseudoClasses<'a>> for &'a OwnedPseudoClasses {
  fn into(self) -> PseudoClasses<'a> {
    PseudoClasses {
      hover: self.hover.as_deref(),
      active: self.active.as_deref(),
      focus: self.focus.as_deref(),
      focus_visible: self.focus_visible.as_deref(),
      focus_within: self.focus_within.as_deref(),
    }
  }
}

#[derive(Serialize, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Drafts {
  #[serde(default)]
  nesting: bool,
  #[serde(default)]
  custom_media: bool,
}

fn compile<'i>(code: &'i str, config: &Config) -> Result<TransformResult, CompileError<'i>> {
  let drafts = config.drafts.as_ref();
  let mut stylesheet = StyleSheet::parse(
    &config.filename,
    &code,
    ParserOptions {
      nesting: matches!(drafts, Some(d) if d.nesting),
      custom_media: matches!(drafts, Some(d) if d.custom_media),
      css_modules: if let Some(css_modules) = &config.css_modules {
        match css_modules {
          CssModulesOption::Bool(true) => Some(parcel_css::css_modules::Config::default()),
          CssModulesOption::Bool(false) => None,
          CssModulesOption::Config(c) => Some(parcel_css::css_modules::Config {
            pattern: if let Some(pattern) = c.pattern.as_ref() {
              match parcel_css::css_modules::Pattern::parse(pattern) {
                Ok(p) => p,
                Err(e) => return Err(CompileError::PatternError(e)),
              }
            } else {
              Default::default()
            },
            dashed_idents: c.dashed_idents,
          }),
        }
      } else {
        None
      },
      source_index: 0,
    },
  )?;
  stylesheet.minify(MinifyOptions {
    targets: config.targets,
    unused_symbols: config.unused_symbols.clone().unwrap_or_default(),
  })?;

  let mut source_map = if config.source_map.unwrap_or(false) {
    let mut sm = SourceMap::new("/");
    sm.add_source(&config.filename);
    sm.set_source_content(0, code)?;
    Some(sm)
  } else {
    None
  };

  let res = stylesheet.to_css(PrinterOptions {
    minify: config.minify.unwrap_or(false),
    source_map: source_map.as_mut(),
    targets: config.targets,
    analyze_dependencies: config.analyze_dependencies.unwrap_or(false),
    pseudo_classes: config.pseudo_classes.as_ref().map(|p| p.into()),
  })?;

  let map = if let Some(mut source_map) = source_map {
    Some(source_map_to_json(&mut source_map)?)
  } else {
    None
  };

  Ok(TransformResult {
    code: res.code.into_bytes(),
    map,
    exports: res.exports,
    references: res.references,
    dependencies: res.dependencies,
  })
}

async fn compile_bundle<'i, P: SourceProvider>(
  fs: &'i P,
  config: &BundleConfig,
) -> Result<TransformResult, CompileError<'i>> {
  let mut source_map = if config.source_map.unwrap_or(false) {
    Some(SourceMap::new("/"))
  } else {
    None
  };

  let drafts = config.drafts.as_ref();
  let parser_options = ParserOptions {
    nesting: matches!(drafts, Some(d) if d.nesting),
    custom_media: matches!(drafts, Some(d) if d.custom_media),
    css_modules: if let Some(css_modules) = &config.css_modules {
      match css_modules {
        CssModulesOption::Bool(true) => Some(parcel_css::css_modules::Config::default()),
        CssModulesOption::Bool(false) => None,
        CssModulesOption::Config(c) => Some(parcel_css::css_modules::Config {
          pattern: if let Some(pattern) = c.pattern.as_ref() {
            match parcel_css::css_modules::Pattern::parse(pattern) {
              Ok(p) => p,
              Err(e) => return Err(CompileError::PatternError(e)),
            }
          } else {
            Default::default()
          },
          dashed_idents: c.dashed_idents,
        }),
      }
    } else {
      None
    },
    ..ParserOptions::default()
  };

  let mut bundler = Bundler::new(fs, source_map.as_mut(), parser_options);
  let mut stylesheet = bundler.bundle(Path::new(&config.filename)).await?;

  stylesheet.minify(MinifyOptions {
    targets: config.targets,
    unused_symbols: config.unused_symbols.clone().unwrap_or_default(),
  })?;

  let res = stylesheet.to_css(PrinterOptions {
    minify: config.minify.unwrap_or(false),
    source_map: source_map.as_mut(),
    targets: config.targets,
    analyze_dependencies: config.analyze_dependencies.unwrap_or(false),
    pseudo_classes: config.pseudo_classes.as_ref().map(|p| p.into()),
  })?;

  let map = if let Some(source_map) = &mut source_map {
    Some(source_map_to_json(source_map)?)
  } else {
    None
  };

  Ok(TransformResult {
    code: res.code.into_bytes(),
    map,
    exports: res.exports,
    references: res.references,
    dependencies: res.dependencies,
  })
}

#[inline]
fn source_map_to_json<'i>(source_map: &mut SourceMap) -> Result<Vec<u8>, CompileError<'i>> {
  let mut vlq_output: Vec<u8> = Vec::new();
  source_map.write_vlq(&mut vlq_output)?;

  let sm = SourceMapJson {
    version: 3,
    mappings: unsafe { String::from_utf8_unchecked(vlq_output) },
    sources: source_map.get_sources(),
    sources_content: source_map.get_sources_content(),
    names: source_map.get_names(),
  };

  Ok(serde_json::to_vec(&sm).unwrap())
}

#[derive(Serialize, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttrConfig {
  #[serde(with = "serde_bytes")]
  pub code: Vec<u8>,
  pub targets: Option<Browsers>,
  pub minify: Option<bool>,
  pub analyze_dependencies: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AttrResult {
  #[serde(with = "serde_bytes")]
  code: Vec<u8>,
  dependencies: Option<Vec<Dependency>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl AttrResult {
  fn into_js(self, ctx: CallContext) -> napi::Result<JsUnknown> {
    // Manually construct buffers so we avoid a copy and work around
    // https://github.com/napi-rs/napi-rs/issues/1124.
    let mut obj = ctx.env.create_object()?;
    let buf = ctx.env.create_buffer_with_data(self.code)?;
    obj.set_named_property("code", buf.into_raw())?;
    obj.set_named_property("dependencies", ctx.env.to_js_value(&self.dependencies)?)?;
    Ok(obj.into_unknown())
  }
}

fn compile_attr<'i>(code: &'i str, config: &AttrConfig) -> Result<AttrResult, CompileError<'i>> {
  let mut attr = StyleAttribute::parse(&code)?;
  attr.minify(MinifyOptions {
    targets: config.targets,
    ..MinifyOptions::default()
  });
  let res = attr.to_css(PrinterOptions {
    minify: config.minify.unwrap_or(false),
    source_map: None,
    targets: config.targets,
    analyze_dependencies: config.analyze_dependencies.unwrap_or(false),
    pseudo_classes: None,
  })?;
  Ok(AttrResult {
    code: res.code.into_bytes(),
    dependencies: res.dependencies,
  })
}

enum CompileError<'i> {
  ParseError(Error<ParserError<'i>>),
  MinifyError(Error<MinifyErrorKind>),
  PrinterError(Error<PrinterErrorKind>),
  SourceMapError(parcel_sourcemap::SourceMapError),
  BundleError(Error<BundleErrorKind<'i>>),
  PatternError(PatternParseError),
}

impl<'i> std::fmt::Display for CompileError<'i> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      CompileError::ParseError(err) => err.kind.fmt(f),
      CompileError::MinifyError(err) => err.kind.fmt(f),
      CompileError::PrinterError(err) => err.kind.fmt(f),
      CompileError::BundleError(err) => err.kind.fmt(f),
      CompileError::PatternError(err) => err.fmt(f),
      CompileError::SourceMapError(err) => write!(f, "{}", err.to_string()), // TODO: switch to `fmt::Display` once parcel_sourcemap supports this
    }
  }
}

impl<'i> CompileError<'i> {
  #[cfg(not(target_arch = "wasm32"))]
  fn throw(self, ctx: CallContext, code: Option<&str>) -> napi::Result<JsUnknown> {
    let reason = self.to_string();
    let data = match &self {
      CompileError::ParseError(Error { kind, .. }) => ctx.env.to_js_value(kind)?,
      CompileError::PrinterError(Error { kind, .. }) => ctx.env.to_js_value(kind)?,
      CompileError::MinifyError(Error { kind, .. }) => ctx.env.to_js_value(kind)?,
      CompileError::BundleError(Error { kind, .. }) => ctx.env.to_js_value(kind)?,
      _ => ctx.env.get_null()?.into_unknown(),
    };

    match self {
      CompileError::ParseError(Error { loc, .. })
      | CompileError::PrinterError(Error { loc, .. })
      | CompileError::MinifyError(Error { loc, .. })
      | CompileError::BundleError(Error { loc, .. }) => {
        // Generate an error with location information.
        let syntax_error = ctx.env.get_global()?.get_named_property::<napi::JsFunction>("SyntaxError")?;
        let reason = ctx.env.create_string_from_std(reason)?;
        let mut obj = syntax_error.new_instance(&[reason])?;
        if let Some(loc) = loc {
          let line = ctx.env.create_int32((loc.line + 1) as i32)?;
          let col = ctx.env.create_int32(loc.column as i32)?;
          let filename = ctx.env.create_string_from_std(loc.filename)?;
          obj.set_named_property("fileName", filename)?;
          if let Some(code) = code {
            let source = ctx.env.create_string(code)?;
            obj.set_named_property("source", source)?;
          }
          let mut loc = ctx.env.create_object()?;
          loc.set_named_property("line", line)?;
          loc.set_named_property("column", col)?;
          obj.set_named_property("loc", loc)?;
        }
        obj.set_named_property("data", data)?;
        ctx.env.throw(obj)?;
        Ok(ctx.env.get_undefined()?.into_unknown())
      }
      _ => Err(self.into()),
    }
  }
}

impl<'i> From<Error<ParserError<'i>>> for CompileError<'i> {
  fn from(e: Error<ParserError<'i>>) -> CompileError<'i> {
    CompileError::ParseError(e)
  }
}

impl<'i> From<Error<MinifyErrorKind>> for CompileError<'i> {
  fn from(err: Error<MinifyErrorKind>) -> CompileError<'i> {
    CompileError::MinifyError(err)
  }
}

impl<'i> From<Error<PrinterErrorKind>> for CompileError<'i> {
  fn from(err: Error<PrinterErrorKind>) -> CompileError<'i> {
    CompileError::PrinterError(err)
  }
}

impl<'i> From<parcel_sourcemap::SourceMapError> for CompileError<'i> {
  fn from(e: parcel_sourcemap::SourceMapError) -> CompileError<'i> {
    CompileError::SourceMapError(e)
  }
}

impl<'i> From<Error<BundleErrorKind<'i>>> for CompileError<'i> {
  fn from(e: Error<BundleErrorKind<'i>>) -> CompileError<'i> {
    CompileError::BundleError(e)
  }
}

#[cfg(not(target_arch = "wasm32"))]
impl<'i> From<CompileError<'i>> for napi::Error {
  fn from(e: CompileError) -> napi::Error {
    match e {
      CompileError::SourceMapError(e) => napi::Error::from_reason(e.to_string()),
      CompileError::PatternError(e) => napi::Error::from_reason(e.to_string()),
      _ => napi::Error::new(napi::Status::GenericFailure, e.to_string()),
    }
  }
}

#[cfg(target_arch = "wasm32")]
impl<'i> From<CompileError<'i>> for wasm_bindgen::JsValue {
  fn from(e: CompileError) -> wasm_bindgen::JsValue {
    match e {
      CompileError::SourceMapError(e) => js_sys::Error::new(&e.to_string()).into(),
      CompileError::PatternError(e) => js_sys::Error::new(&e.to_string()).into(),
      _ => js_sys::Error::new(&e.to_string()).into(),
    }
  }
}
