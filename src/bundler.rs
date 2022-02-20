use cssparser::AtRuleParser;
use parcel_sourcemap::SourceMap;
use crate::{rules::{Location, layer::{LayerBlockRule, LayerName}}, error::ErrorLocation};
use std::{fs, path::{Path, PathBuf}, sync::Mutex, collections::HashSet};
use rayon::prelude::*;
use dashmap::DashMap;
use crate::{
  stylesheet::{StyleSheet, ParserOptions},
  rules::{CssRule, CssRuleList,
    media::MediaRule,
    supports::{SupportsRule, SupportsCondition},
    import::ImportRule
  },
  media_query::MediaList,
  error::{Error, ParserError}
};

pub struct Bundler<'a, 's, P, T, R> {
  source_map: Option<Mutex<&'s mut SourceMap>>,
  fs: &'a P,
  source_indexes: DashMap<PathBuf, u32>,
  stylesheets: Mutex<Vec<BundleStyleSheet<'a, T, R>>>,
  options: ParserOptions<T>
}

#[derive(Debug)]
struct BundleStyleSheet<'i, T, R> {
  stylesheet: Option<StyleSheet<'i, T, R>>,
  dependencies: Vec<u32>,
  parent_source_index: u32,
  parent_dep_index: u32,
  layer: Option<Option<LayerName<'i>>>,
  supports: Option<SupportsCondition<'i>>,
  media: MediaList<'i>,
  loc: Location
}

pub trait SourceProvider: Send + Sync {
  fn read<'a>(&'a self, file: &Path) -> std::io::Result<&'a str>;
}

pub struct FileProvider {
  inputs: Mutex<Vec<*mut String>>
}

impl FileProvider {
  pub fn new() -> FileProvider {
    FileProvider {
      inputs: Mutex::new(Vec::new()),
    }
  }
}

unsafe impl Sync for FileProvider {}
unsafe impl Send for FileProvider {}

impl SourceProvider for FileProvider {
  fn read<'a>(&'a self, file: &Path) -> std::io::Result<&'a str> {
    let source = fs::read_to_string(file)?;
    let ptr = Box::into_raw(Box::new(source));
    self.inputs.lock().unwrap().push(ptr);
    // SAFETY: this is safe because the pointer is not dropped
    // until the FileProvider is, and we never remove from the
    // list of pointers stored in the vector.
    Ok(unsafe { &*ptr })
  }
}

impl Drop for FileProvider {
  fn drop(&mut self) {
    for ptr in self.inputs.lock().unwrap().iter() {
      std::mem::drop(unsafe { Box::from_raw(*ptr) })
    }
  }
}

#[derive(Debug)]
pub enum BundleErrorKind<'i> {
  IOError(std::io::Error),
  ParserError(ParserError<'i>),
  UnsupportedImportCondition,
  UnsupportedMediaBooleanLogic,
  UnsupportedLayerCombination
}

impl<'i> From<Error<ParserError<'i>>> for Error<BundleErrorKind<'i>> {
  fn from(err: Error<ParserError<'i>>) -> Self {
    Error {
      kind: BundleErrorKind::ParserError(err.kind),
      loc: err.loc
    }
  }
}

impl<'i> BundleErrorKind<'i> {
  pub fn reason(&self) -> String {
    match self {
      BundleErrorKind::IOError(e) => e.to_string(),
      BundleErrorKind::ParserError(e) => e.reason(),
      BundleErrorKind::UnsupportedImportCondition => "Unsupported import condition".into(),
      BundleErrorKind::UnsupportedMediaBooleanLogic => "Unsupported boolean logic in @import media query".into(),
      BundleErrorKind::UnsupportedLayerCombination => "Unsupported layer combination in @import".into()
    }
  }
}

impl<'a, 's, P: SourceProvider, T: AtRuleParser<'a> + Clone + Sync + Send> Bundler<'a, 's, P, T, T::AtRule>
where
  T::AtRule: Sync + Send + cssparser::ToCss
{
  pub fn new(fs: &'a P, source_map: Option<&'s mut SourceMap>, options: ParserOptions<T>) -> Self {
    Bundler {
      source_map: source_map.map(Mutex::new),
      fs,
      source_indexes: DashMap::new(),
      stylesheets: Mutex::new(Vec::new()),
      options
    }
  }

  pub fn bundle<'e>(&mut self, entry: &'e Path) -> Result<StyleSheet<'a, T, T::AtRule>, Error<BundleErrorKind<'a>>> {
    // Phase 1: load and parse all files. This is done in parallel.
    self.load_file(&entry, ImportRule {
      url: "".into(),
      layer: None,
      supports: None,
      media: MediaList::new(),
      loc: Location {
        source_index: 0,
        line: 1,
        column: 0
      }
    })?;

    // Phase 2: determine the order that the files should be concatenated.
    self.order();

    // Phase 3: concatenate.
    let mut rules: Vec<CssRule<'a, T::AtRule>> = Vec::new();
    self.inline(&mut rules);

    let sources = self.stylesheets.get_mut()
      .unwrap()
      .iter()
      .flat_map(|s| s.stylesheet.as_ref().unwrap().sources.iter().cloned())
      .collect();

    Ok(StyleSheet::new(
      sources,
      CssRuleList(rules), 
      self.options.clone()
    ))
  }

  fn find_filename(&self, source_index: u32) -> String {
    // This function is only used for error handling, so it's ok if this is a bit slow.
    let entry = self.source_indexes.iter()
      .find(|x| *x.value() == source_index)
      .unwrap();
    entry.key().to_str().unwrap().into()
  }

  fn load_file(&self, file: &Path, rule: ImportRule<'a>) -> Result<u32, Error<BundleErrorKind<'a>>> {
    // Check if we already loaded this file.
    let mut stylesheets = self.stylesheets.lock().unwrap();
    let source_index = match self.source_indexes.get(file) {
      Some(source_index) => {
        // If we already loaded this file, combine the media queries and supports conditions
        // from this import rule with the existing ones using a logical or operator.
        let entry = &mut stylesheets[*source_index as usize];

        // We cannot combine a media query and a supports query from different @import rules.
        // e.g. @import "a.css" print; @import "a.css" supports(color: red);
        // This would require duplicating the actual rules in the file.
        if (!rule.media.media_queries.is_empty() && !entry.supports.is_none()) || 
          (!entry.media.media_queries.is_empty() && !rule.supports.is_none()) {
          return Err(Error {
            kind: BundleErrorKind::UnsupportedImportCondition,
            loc: Some(ErrorLocation::from(
              rule.loc, 
              self.find_filename(rule.loc.source_index)
            ))
          })
        }

        if rule.media.media_queries.is_empty() {
          entry.media.media_queries.clear();
        } else if !entry.media.media_queries.is_empty() {
          entry.media.or(&rule.media);
        }

        if let Some(supports) = rule.supports {
          if let Some(existing_supports) = &mut entry.supports {
            existing_supports.or(&supports)
          }
        } else {
          entry.supports = None;
        }

        if let Some(layer) = &rule.layer {
          if let Some(existing_layer) = &entry.layer {
            // We can't OR layer names without duplicating all of the nested rules, so error for now.
            if layer != existing_layer || (layer.is_none() && existing_layer.is_none()) {
              return Err(Error {
                kind: BundleErrorKind::UnsupportedLayerCombination,
                loc: Some(ErrorLocation::from(
                  rule.loc,
                  self.find_filename(rule.loc.source_index)
                ))
              })
            }
          } else {
            entry.layer = rule.layer;
          }
        }
        
        return Ok(*source_index);
      }
      None => {
        let source_index = stylesheets.len() as u32;
        self.source_indexes.insert(file.to_owned(), source_index);

        stylesheets.push(BundleStyleSheet {
          stylesheet: None,
          layer: rule.layer.clone(),
          media: rule.media.clone(),
          supports: rule.supports.clone(),
          loc: rule.loc.clone(),
          dependencies: Vec::new(),
          parent_source_index: 0,
          parent_dep_index: 0
        });

        source_index
      }
    };

    drop(stylesheets); // ensure we aren't holding the lock anymore
    
    let code = self.fs.read(file).map_err(|e| Error {
      kind: BundleErrorKind::IOError(e),
      loc: Some(ErrorLocation::from(
        rule.loc,
        self.find_filename(rule.loc.source_index)
      ))
    })?;

    let mut opts = self.options.clone();
    opts.source_index = source_index;

    let filename = file.to_str().unwrap();
    if let Some(source_map) = &self.source_map {
      let mut source_map = source_map.lock().unwrap();
      let source_index = source_map.add_source(filename);
      let _ = source_map.set_source_content(source_index as usize, code);
    }

    let mut stylesheet = StyleSheet::parse(
      filename.into(),
      code,
      opts,
    )?;

    // Collect and load dependencies for this stylesheet in parallel.
    let dependencies: Result<Vec<u32>, _> = stylesheet.rules.0.par_iter_mut()
      .filter_map(|r| {
        // Prepend parent layer name to @layer statements.
        if let CssRule::LayerStatement(layer) = r {
          if let Some(Some(parent_layer)) = &rule.layer {
            for name in &mut layer.names {
              name.0.insert_many(0, parent_layer.0.iter().cloned())
            }
          }
        }

        if let CssRule::Import(import) = r {
          let path = file.with_file_name(&*import.url);

          // Combine media queries and supports conditions from parent 
          // stylesheet with @import rule using a logical and operator.
          let mut media = rule.media.clone();
          let result = media.and(&import.media).map_err(|_| Error {
            kind: BundleErrorKind::UnsupportedMediaBooleanLogic,
            loc: Some(ErrorLocation::from(
              import.loc,
              self.find_filename(import.loc.source_index)
            ))
          });

          if let Err(e) = result {
            return Some(Err(e))
          }

          let layer = if (rule.layer == Some(None) && import.layer.is_some()) || (import.layer == Some(None) && rule.layer.is_some()) {
            // Cannot combine anonymous layers
            return Some(Err(Error {
              kind: BundleErrorKind::UnsupportedLayerCombination,
              loc: Some(ErrorLocation::from(
                import.loc, 
                self.find_filename(import.loc.source_index)
              ))
            }))
          } else if let Some(Some(a)) = &rule.layer {
            if let Some(Some(b)) = &import.layer {
              let mut name = a.clone();
              name.0.extend(b.0.iter().cloned());
              Some(Some(name))
            } else {
              Some(Some(a.clone()))
            }
          } else {
            import.layer.clone()
          };
          
          let result = self.load_file(&path, ImportRule {
            layer,
            media,
            supports: combine_supports(rule.supports.clone(), &import.supports),
            url: "".into(),
            loc: import.loc
          });

          Some(result)
        } else {
          None
        }
      })
      .collect();
      
    let entry = &mut self.stylesheets.lock().unwrap()[source_index as usize];
    entry.stylesheet = Some(stylesheet);
    entry.dependencies = dependencies?;

    Ok(source_index)
  }

  fn order(&mut self) {
    process(
      self.stylesheets.get_mut().unwrap(),
      0, 
      &mut HashSet::new()
    );

    fn process<'a, T: AtRuleParser<'a>>(stylesheets: &mut Vec<BundleStyleSheet<'a, T, T::AtRule>>, source_index: u32, visited: &mut HashSet<u32>) {
      if visited.contains(&source_index) {
        return
      }

      visited.insert(source_index);

      for dep_index in 0..stylesheets[source_index as usize].dependencies.len() {
        let dep_source_index = stylesheets[source_index as usize].dependencies[dep_index];
        let mut resolved = &mut stylesheets[dep_source_index as usize];

        // In browsers, every instance of an @import is evaluated, so we preserve the last.
        resolved.parent_dep_index = dep_index as u32;
        resolved.parent_source_index = source_index;

        process(stylesheets, dep_source_index, visited);
      }
    }
  }

  fn inline(&mut self, dest: &mut Vec<CssRule<'a, T::AtRule>>) {
    process(
      self.stylesheets.get_mut().unwrap(),
      0,
      dest
    );

    fn process<'a, T: AtRuleParser<'a>>(stylesheets: &mut Vec<BundleStyleSheet<'a, T, T::AtRule>>, source_index: u32, dest: &mut Vec<CssRule<'a, T::AtRule>>) {
      let stylesheet = &mut stylesheets[source_index as usize];
      let mut rules = std::mem::take(&mut stylesheet.stylesheet.as_mut().unwrap().rules.0);

      let mut dep_index = 0;
      for rule in &mut rules {
        match rule {
          CssRule::Import(_) => {
            let dep_source_index = stylesheets[source_index as usize].dependencies[dep_index as usize];
            let resolved = &stylesheets[dep_source_index as usize];

            // Include the dependency if this is the last instance as computed earlier.
            if resolved.parent_source_index == source_index && resolved.parent_dep_index == dep_index {
              process(stylesheets, dep_source_index, dest);
            }

            *rule = CssRule::Ignored;
            dep_index += 1;
          }
          CssRule::LayerStatement(_) => {
            // @layer rules are the only rules that may appear before an @import.
            // We must preserve this order to ensure correctness.
            let layer = std::mem::replace(rule, CssRule::Ignored);
            dest.push(layer);
          }
          CssRule::Ignored => {}
          _ => break
        }
      }

      // Wrap rules in the appropriate @media and @supports rules.
      let stylesheet = &mut stylesheets[source_index as usize];
      if !stylesheet.media.media_queries.is_empty() {
        rules = vec![
          CssRule::Media(MediaRule {
            query: std::mem::replace(&mut stylesheet.media, MediaList::new()),
            rules: CssRuleList(rules),
            loc: stylesheet.loc
          })
        ]
      }

      if stylesheet.supports.is_some() {
        rules = vec![
          CssRule::Supports(SupportsRule {
            condition: stylesheet.supports.take().unwrap(),
            rules: CssRuleList(rules),
            loc: stylesheet.loc
          })
        ]
      }

      if stylesheet.layer.is_some() {
        rules = vec![
          CssRule::LayerBlock(LayerBlockRule {
            name: stylesheet.layer.take().unwrap(),
            rules: CssRuleList(rules),
            loc: stylesheet.loc
          })
        ]
      }

      dest.extend(rules);
    }
  }
}

fn combine_supports<'a>(a: Option<SupportsCondition<'a>>, b: &Option<SupportsCondition<'a>>) -> Option<SupportsCondition<'a>> {
  if let Some(mut a) = a {
    if let Some(b) = b {
      a.and(b)
    }
    Some(a)
  } else {
    b.clone()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{stylesheet::{PrinterOptions, MinifyOptions}, targets::Browsers};
  use indoc::indoc;
  use std::collections::HashMap;

  struct TestProvider {
    map: HashMap<PathBuf, String>
  }

  impl SourceProvider for TestProvider {
    fn read<'a>(&'a self, file: &Path) -> std::io::Result<&'a str> {
      Ok(self.map.get(file).unwrap())
    }
  }

  macro_rules! fs(
    { $($key:literal: $value:expr),* } => {
      {
        #[allow(unused_mut)]
        let mut m = HashMap::new();
        $(
          m.insert(PathBuf::from($key), $value.to_owned());
        )*
        TestProvider {
          map: m
        }
      }
    };
  );

  fn bundle(fs: TestProvider, entry: &str) -> String {
    let mut bundler = Bundler::new(&fs, None, ParserOptions::default());
    let stylesheet = bundler.bundle(Path::new(entry)).unwrap();
    stylesheet.to_css(PrinterOptions::default()).unwrap().code
  }

  fn bundle_css_module(fs: TestProvider, entry: &str) -> String {
    let mut bundler = Bundler::new(&fs, None, ParserOptions { css_modules: true, ..ParserOptions::default() });
    let stylesheet = bundler.bundle(Path::new(entry)).unwrap();
    stylesheet.to_css(PrinterOptions::default()).unwrap().code
  }

  fn bundle_custom_media(fs: TestProvider, entry: &str) -> String {
    let mut bundler = Bundler::new(&fs, None, ParserOptions { custom_media: true, ..ParserOptions::default() });
    let mut stylesheet = bundler.bundle(Path::new(entry)).unwrap();
    let targets = Some(Browsers { safari: Some(13 << 16 ), ..Browsers::default() });
    stylesheet.minify(MinifyOptions { targets, ..MinifyOptions::default() }).unwrap();
    stylesheet.to_css(PrinterOptions { targets, ..PrinterOptions::default() }).unwrap().code
  }

  fn error_test(fs: TestProvider, entry: &str) {
    let mut bundler = Bundler::new(&fs, None, ParserOptions::default());
    let res = bundler.bundle(Path::new(entry));
    match res {
      Ok(_) => unreachable!(),
      Err(e) => assert!(matches!(e.kind, BundleErrorKind::UnsupportedLayerCombination))
    }
  }

  #[test]
  fn test_bundle() {
    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css";
        .a { color: red }
      "#,
      "/b.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      .b {
        color: green;
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" print;
        .a { color: red }
      "#,
      "/b.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @media print {
        .b {
          color: green;
        }
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" supports(color: green);
        .a { color: red }
      "#,
      "/b.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @supports (color: green) {
        .b {
          color: green;
        }
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" supports(color: green) print;
        .a { color: red }
      "#,
      "/b.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @supports (color: green) {
        @media print {
          .b {
            color: green;
          }
        }
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" print;
        @import "b.css" screen;
        .a { color: red }
      "#,
      "/b.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @media print, screen {
        .b {
          color: green;
        }
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" supports(color: red);
        @import "b.css" supports(foo: bar);
        .a { color: red }
      "#,
      "/b.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @supports ((color: red) or (foo: bar)) {
        .b {
          color: green;
        }
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" print;
        .a { color: red }
      "#,
      "/b.css": r#"
        @import "c.css" (color);
        .b { color: yellow }
      "#,
      "/c.css": r#"
        .c { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @media print and (color) {
        .c {
          color: green;
        }
      }
      
      @media print {
        .b {
          color: #ff0;
        }
      }

      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css";
        .a { color: red }
      "#,
      "/b.css": r#"
        @import "c.css";
      "#,
      "/c.css": r#"
        @import "a.css";
        .c { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      .c {
        color: green;
      }

      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b/c.css";
        .a { color: red }
      "#,
      "/b/c.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      .b {
        color: green;
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "./b/c.css";
        .a { color: red }
      "#,
      "/b/c.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      .b {
        color: green;
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle_css_module(fs! {
      "/a.css": r#"
        @import "b.css";
        .a { color: red }
      "#,
      "/b.css": r#"
        .a { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      .a_6lixEq_1 {
        color: green;
      }

      .a_6lixEq {
        color: red;
      }
    "#});

    let res = bundle_custom_media(fs! {
      "/a.css": r#"
        @import "media.css";
        @import "b.css";
        .a { color: red }
      "#,
      "/media.css": r#"
        @custom-media --foo print;
      "#,
      "/b.css": r#"
        @media (--foo) {
          .a { color: green }
        }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @media print {
        .a {
          color: green;
        }
      }

      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" layer(foo);
        .a { color: red }
      "#,
      "/b.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @layer foo {
        .b {
          color: green;
        }
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" layer;
        .a { color: red }
      "#,
      "/b.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @layer {
        .b {
          color: green;
        }
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" layer(foo);
        .a { color: red }
      "#,
      "/b.css": r#"
        @import "c.css" layer(bar);
        .b { color: green }
      "#,
      "/c.css": r#"
        .c { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @layer foo.bar {
        .c {
          color: green;
        }
      }

      @layer foo {
        .b {
          color: green;
        }
      }
      
      .a {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @import "b.css" layer(foo);
        @import "b.css" layer(foo);
      "#,
      "/b.css": r#"
        .b { color: green }
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @layer foo {
        .b {
          color: green;
        }
      }
    "#});

    let res = bundle(fs! {
      "/a.css": r#"
        @layer bar, foo;
        @import "b.css" layer(foo);
        
        @layer bar {
          div {
            background: red;
          }
        }
      "#,
      "/b.css": r#"
        @layer qux, baz;
        @import "c.css" layer(baz);
        
        @layer qux {
          div {
            background: green;
          }
        }
      "#,
      "/c.css": r#"
        div {
          background: yellow;
        }      
      "#
    }, "/a.css");
    assert_eq!(res, indoc! { r#"
      @layer bar, foo;
      @layer foo.qux, foo.baz;

      @layer foo.baz {
        div {
          background: #ff0;
        }
      }

      @layer foo {
        @layer qux {
          div {
            background: green;
          }
        }
      }
      
      @layer bar {
        div {
          background: red;
        }
      }
    "#});

    error_test(fs! {
      "/a.css": r#"
        @import "b.css" layer(foo);
        @import "b.css" layer(bar);
      "#,
      "/b.css": r#"
        .b { color: red }
      "#
    }, "/a.css");

    error_test(fs! {
      "/a.css": r#"
        @import "b.css" layer;
        @import "b.css" layer;
      "#,
      "/b.css": r#"
        .b { color: red }
      "#
    }, "/a.css");
    
    error_test(fs! {
      "/a.css": r#"
        @import "b.css" layer;
        .a { color: red }
      "#,
      "/b.css": r#"
        @import "c.css" layer;
        .b { color: green }
      "#,
      "/c.css": r#"
        .c { color: green }
      "#
    }, "/a.css");

    error_test(fs! {
      "/a.css": r#"
        @import "b.css" layer;
        .a { color: red }
      "#,
      "/b.css": r#"
        @import "c.css" layer(foo);
        .b { color: green }
      "#,
      "/c.css": r#"
        .c { color: green }
      "#
    }, "/a.css");

    let res = bundle(fs! {
      "/index.css": r#"
        @import "a.css";
        @import "b.css";
      "#,
      "/a.css": r#"
        @import "./c.css";
        body { background: red; }
      "#,
      "/b.css": r#"
        @import "./c.css";
        body { color: red; }
      "#,
      "/c.css": r#"
        body {
          background: white;
          color: black; 
        }
      "#
    }, "/index.css");
    assert_eq!(res, indoc! { r#"
      body {
        background: red;
      }

      body {
        background: #fff;
        color: #000;
      }

      body {
        color: red;
      }
    "#});

    let res = bundle(fs! {
      "/index.css": r#"
        @import "a.css";
        @import "b.css";
        @import "a.css";
      "#,
      "/a.css": r#"
        body { background: green; }
      "#,
      "/b.css": r#"
        body { background: red; }
      "#
    }, "/index.css");
    assert_eq!(res, indoc! { r#"
      body {
        background: red;
      }

      body {
        background: green;
      }
    "#});

    // let res = bundle(fs! {
    //   "/a.css": r#"
    //     @import "b.css" supports(color: red) (color);
    //     @import "b.css" supports(foo: bar) (orientation: horizontal);
    //     .a { color: red }
    //   "#,
    //   "/b.css": r#"
    //     .b { color: green }
    //   "#
    // }, "/a.css");

    // let res = bundle(fs! {
    //   "/a.css": r#"
    //     @import "b.css" not print;
    //     .a { color: red }
    //   "#,
    //   "/b.css": r#"
    //     @import "c.css" not screen;
    //     .b { color: green }
    //   "#,
    //   "/c.css": r#"
    //     .c { color: yellow }
    //   "#
    // }, "/a.css");
  }
}
