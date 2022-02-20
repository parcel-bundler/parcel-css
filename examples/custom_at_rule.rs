use cssparser::*;
use parcel_css::{
  stylesheet::{StyleSheet, ParserOptions, PrinterOptions},
  values::color::CssColor,
  properties::Property
};

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let source = std::fs::read_to_string(&args[1]).unwrap();
  let opts = ParserOptions {
    at_rule_parser: Some(TailwindAtRuleParser),
    nesting: true,
    custom_media: false,
    css_modules: false,
    source_index: 0,
  };

  let stylesheet = StyleSheet::parse(
    args[1].clone(), 
    &source, 
    opts
  ).unwrap();

  println!("{:?}", stylesheet);
  
  let result = stylesheet.to_css(PrinterOptions::default()).unwrap();
  println!("{}", result.code);
}

/// An @tailwind directive.
#[derive(Debug)]
enum TailwindDirective {
  Base,
  Components,
  Utilities,
  Variants
}

/// A custom at rule prelude.
enum Prelude {
  Tailwind(TailwindDirective),
  Apply(Vec<String>, bool)
}

/// A @tailwind rule.
#[derive(Debug)]
struct TailwindRule {
  directive: TailwindDirective,
  loc: SourceLocation
}

/// An @apply rule.
#[derive(Debug)]
struct ApplyRule {
  names: Vec<String>,
  important: bool,
  loc: SourceLocation
}

/// A custom at rule.
#[derive(Debug)]
enum AtRule {
  Tailwind(TailwindRule),
  Apply(ApplyRule)
}

#[derive(Debug)]
struct TailwindAtRuleParser;
impl<'i> AtRuleParser<'i> for TailwindAtRuleParser {
  type Prelude = Prelude;
  type Error = ();
  type AtRule = AtRule;

  fn parse_prelude<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    match_ignore_ascii_case! {&*name,
      "tailwind" => {
        let location = input.current_source_location();
        let ident = input.expect_ident()?;
        let directive = match_ignore_ascii_case! { &*ident,
          "base" => TailwindDirective::Base,
          "components" => TailwindDirective::Components,
          "utilities" => TailwindDirective::Utilities,
          "variants" => TailwindDirective::Variants,
          _ => return Err(location.new_unexpected_token_error(
            Token::Ident(ident.clone())
          ))
        };
        Ok(Prelude::Tailwind(directive))
      },
      "apply" => {
        let mut names = Vec::new();
        loop {
          if let Ok(name) = input.try_parse(|input| input.expect_ident_cloned()) {
            names.push(name.as_ref().into());
          } else {
            break
          }
        }

        let important = input.try_parse(|input| {
          input.expect_delim('!')?;
          input.expect_ident_matching("important")
        }).is_ok();

        Ok(Prelude::Apply(names, important))
      },
      _ => Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)))
    }
  }

  fn rule_without_block(
    &mut self,
    prelude: Self::Prelude,
    start: &ParserState,
  ) -> Result<Self::AtRule, ()> {
    let loc = start.source_location();
    match prelude {
      Prelude::Tailwind(directive) => {
        Ok(AtRule::Tailwind(TailwindRule {
          directive,
          loc
        }))
      }
      Prelude::Apply(names, important) => {
        Ok(AtRule::Apply(ApplyRule {
          names,
          important,
          loc
        }))
      }
    }
  }
}

// TODO: expose printer??
impl cssparser::ToCss for AtRule {
  fn to_css<W>(&self, dest: &mut W) -> std::fmt::Result where W: std::fmt::Write {
    match self {
      AtRule::Tailwind(rule) => {
        let _ = rule.loc; // TODO: source maps
        let directive = match rule.directive {
          TailwindDirective::Base => "TAILWIND BASE HERE",
          TailwindDirective::Components => "TAILWIND COMPONENTS HERE",
          TailwindDirective::Utilities => "TAILWIND UTILITIES HERE",
          TailwindDirective::Variants => "TAILWIND VARIANTS HERE"
        };
        dest.write_str(directive)
      }
      AtRule::Apply(rule) => {
        // TODO: possibly better to expose a way to transform these rules to inline the 
        // declarations rather than doing it during printing.
        let _ = rule.loc; // TODO: source maps
        for name in &rule.names {
          match name.as_ref() {
            "bg-blue-400" => {
              let color = Color::RGBA(RGBA { red: 0, green: 0, blue: 255, alpha: 255 });
              let property = Property::BackgroundColor(CssColor(color));
              property.temp_to_css(dest, rule.important)?;
            }
            "text-red-400" => {
              let color = Color::RGBA(RGBA { red: 255, green: 0, blue: 0, alpha: 255 });
              let property = Property::Color(CssColor(color));
              property.temp_to_css(dest, rule.important)?;
            }
            _ => {}
          }
          dest.write_str("; ")?;
        }
        Ok(())
      }
    }
  }
}
