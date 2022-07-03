use clap::Parser;
use parcel_css::bundler::{Bundler, FileProvider};
use parcel_css::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use parcel_css::targets::Browsers;
use parcel_sourcemap::SourceMap;
use serde::Serialize;
use std::{ffi, fs, io, path, path::Path};

#[cfg(target_os = "macos")]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Parser, Debug)]
#[clap(author, about, long_about = None)]
struct CliArgs {
  /// Target CSS file
  input_file: String,
  /// Destination file for the output
  #[clap(short, long, group = "output_file")]
  output_file: Option<String>,
  /// Minify the output
  #[clap(short, long)]
  minify: bool,
  /// Enable parsing CSS nesting
  #[clap(long)]
  nesting: bool,
  /// Enable parsing custom media queries
  #[clap(long)]
  custom_media: bool,
  /// Enable CSS modules in output.
  /// If no filename is provided, <output_file>.json will be used.
  /// If no --output-file is specified, code and exports will be printed to stdout as JSON.
  #[clap(long, group = "css_modules")]
  css_modules: Option<Option<String>>,
  #[clap(long, requires = "css_modules")]
  css_modules_pattern: Option<String>,
  #[clap(long, requires = "css_modules")]
  css_modules_dashed_idents: bool,
  /// Enable sourcemap, at <output_file>.map
  #[clap(long, requires = "output_file")]
  sourcemap: bool,
  #[clap(long)]
  bundle: bool,
  #[clap(short, long)]
  targets: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceMapJson<'a> {
  version: u8,
  mappings: String,
  sources: &'a Vec<String>,
  sources_content: &'a Vec<String>,
  names: &'a Vec<String>,
}

#[tokio::main]
pub async fn main() -> Result<(), std::io::Error> {
  let cli_args = CliArgs::parse();
  let source = fs::read_to_string(&cli_args.input_file)?;

  let absolute_path = fs::canonicalize(&cli_args.input_file)?;
  let filename = pathdiff::diff_paths(absolute_path, std::env::current_dir()?).unwrap();
  let filename = filename.to_str().unwrap();

  let css_modules = if let Some(_) = cli_args.css_modules {
    let pattern = if let Some(pattern) = cli_args.css_modules_pattern.as_ref() {
      match parcel_css::css_modules::Pattern::parse(pattern) {
        Ok(p) => p,
        Err(e) => {
          eprintln!("{}", e);
          std::process::exit(1);
        }
      }
    } else {
      Default::default()
    };

    Some(parcel_css::css_modules::Config {
      pattern,
      dashed_idents: cli_args.css_modules_dashed_idents,
      ..Default::default()
    })
  } else {
    cli_args.css_modules.as_ref().map(|_| Default::default())
  };

  let options = ParserOptions {
    nesting: cli_args.nesting,
    css_modules,
    custom_media: cli_args.custom_media,
    ..ParserOptions::default()
  };

  let fs = FileProvider::new();
  let mut source_map = if cli_args.sourcemap {
    Some(SourceMap::new("/"))
  } else {
    None
  };

  let mut stylesheet = if cli_args.bundle {
    let mut bundler = Bundler::new(&fs, source_map.as_mut(), options);
    bundler.bundle(Path::new(&cli_args.input_file)).await.unwrap()
  } else {
    if let Some(sm) = &mut source_map {
      sm.add_source(&filename);
      let _ = sm.set_source_content(0, &source);
    }
    StyleSheet::parse(filename, &source, options).unwrap()
  };

  let targets = browserslist_to_targets(cli_args.targets).unwrap();
  stylesheet
    .minify(MinifyOptions {
      targets,
      ..MinifyOptions::default()
    })
    .unwrap();

  let res = stylesheet
    .to_css(PrinterOptions {
      minify: cli_args.minify,
      source_map: source_map.as_mut(),
      targets,
      ..PrinterOptions::default()
    })
    .unwrap();

  let map = if let Some(ref mut source_map) = source_map {
    let mut vlq_output: Vec<u8> = Vec::new();
    source_map
      .write_vlq(&mut vlq_output)
      .map_err(|_| io::Error::new(io::ErrorKind::Other, "Error writing sourcemap vlq"))?;

    let sm = SourceMapJson {
      version: 3,
      mappings: unsafe { String::from_utf8_unchecked(vlq_output) },
      sources: source_map.get_sources(),
      sources_content: source_map.get_sources_content(),
      names: source_map.get_names(),
    };

    serde_json::to_vec(&sm).ok()
  } else {
    None
  };

  if let Some(output_file) = &cli_args.output_file {
    let mut code = res.code;
    if cli_args.sourcemap {
      if let Some(map_buf) = map {
        let map_filename: String = output_file.to_owned() + ".map";
        code += &format!("\n/*# sourceMappingURL={} */\n", map_filename);
        fs::write(map_filename, map_buf)?;
      }
    }

    fs::write(output_file, code.as_bytes())?;

    if let Some(css_modules) = cli_args.css_modules {
      let css_modules_filename = if let Some(name) = css_modules {
        name
      } else {
        infer_css_modules_filename(&output_file)?
      };
      if let Some(exports) = res.exports {
        let css_modules_json = serde_json::to_string(&exports)?;
        fs::write(css_modules_filename, css_modules_json)?;
      }
    }
  } else {
    if let Some(exports) = res.exports {
      println!(
        "{}",
        serde_json::json!({
          "code": res.code,
          "exports": exports
        })
      );
    } else {
      println!("{}", res.code);
    }
  }

  Ok(())
}

fn infer_css_modules_filename(output_file: &str) -> Result<String, std::io::Error> {
  let path = path::Path::new(output_file);
  if path.extension() == Some(ffi::OsStr::new("json")) {
    Err(io::Error::new(
      io::ErrorKind::Other,
      "Cannot infer a css modules json filename, since the output file extension is '.json'",
    ))
  } else {
    // unwrap: the filename option is a String from clap, so is valid utf-8
    Ok(path.with_extension("json").to_str().unwrap().into())
  }
}

fn browserslist_to_targets(query: Vec<String>) -> Result<Option<Browsers>, browserslist::Error> {
  use browserslist::{resolve, Opts};

  if query.is_empty() {
    return Ok(None);
  }

  let res = resolve(query, &Opts::new())?;

  let mut browsers = Browsers::default();
  let mut has_any = false;
  for distrib in res {
    macro_rules! browser {
      ($browser: ident) => {{
        if let Some(v) = parse_version(distrib.version()) {
          if browsers.$browser.is_none() || v < browsers.$browser.unwrap() {
            browsers.$browser = Some(v);
            has_any = true;
          }
        }
      }};
    }

    match distrib.name() {
      "android" => browser!(android),
      "chrome" | "and_chr" => browser!(chrome),
      "edge" => browser!(edge),
      "firefox" | "and_ff" => browser!(firefox),
      "ie" => browser!(ie),
      "ios_saf" => browser!(ios_saf),
      "opera" | "op_mob" => browser!(opera),
      "safari" => browser!(safari),
      "samsung" => browser!(samsung),
      _ => {}
    }
  }

  if !has_any {
    return Ok(None);
  }

  Ok(Some(browsers))
}

fn parse_version(version: &str) -> Option<u32> {
  let version = version.split('-').next();
  if version.is_none() {
    return None;
  }

  let mut version = version.unwrap().split('.');
  let major = version.next().and_then(|v| v.parse::<u32>().ok());
  if let Some(major) = major {
    let minor = version.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let patch = version.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let v: u32 = (major & 0xff) << 16 | (minor & 0xff) << 8 | (patch & 0xff);
    return Some(v);
  }

  None
}
