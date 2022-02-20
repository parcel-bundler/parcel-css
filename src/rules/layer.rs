use cssparser::*;
use smallvec::SmallVec;
use crate::traits::{Parse, ToCss};
use super::{Location, CssRuleList};
use crate::values::string::CowArcStr;
use crate::error::{ParserError, PrinterError};
use crate::printer::Printer;

/// https://drafts.csswg.org/css-cascade-5/#typedef-layer-name
#[derive(Debug, Clone, PartialEq)]
pub struct LayerName<'i>(pub SmallVec<[CowArcStr<'i>; 1]>);

macro_rules! expect_non_whitespace {
  ($parser: ident, $($branches: tt)+) => {{
    let start_location = $parser.current_source_location();
    match *$parser.next_including_whitespace()? {
      $($branches)+
      ref token => {
        return Err(start_location.new_basic_unexpected_token_error(token.clone()))
      }
    }
  }}
}

impl<'i> Parse<'i> for LayerName<'i> {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    let mut parts = SmallVec::new();
    let ident = input.expect_ident()?;
    parts.push(ident.into());

    loop {
      let name = input.try_parse(|input| {
        expect_non_whitespace!{input,
          Token::Delim('.') => Ok(()),
        }?;

        expect_non_whitespace!{input,
          Token::Ident(ref id) => Ok(id.into()),
        }
      });

      match name {
        Ok(name) => parts.push(name),
        Err(_) => break
      }
    }

    Ok(LayerName(parts))
  }
}

impl<'i> ToCss for LayerName<'i> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    let mut first = true;
    for name in &self.0 {
      if first {
        first = false;
      } else {
        dest.write_char('.')?;
      }

      dest.write_str(name)?;
    }

    Ok(())
  }
}

/// https://drafts.csswg.org/css-cascade-5/#layer-empty
#[derive(Debug, Clone, PartialEq)]
pub struct LayerStatementRule<'i> {
  pub names: Vec<LayerName<'i>>,
  pub loc: Location
}

impl<'i> ToCss for LayerStatementRule<'i> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    dest.add_mapping(self.loc);
    dest.write_str("@layer ")?;
    self.names.to_css(dest)?;
    dest.write_char(';')
  }
}

/// https://drafts.csswg.org/css-cascade-5/#layer-block
#[derive(Debug, Clone, PartialEq)]
pub struct LayerBlockRule<'i, T> {
  pub name: Option<LayerName<'i>>,
  pub rules: CssRuleList<'i, T>,
  pub loc: Location
}

impl<'i, T: cssparser::ToCss> ToCss for LayerBlockRule<'i, T> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    dest.add_mapping(self.loc);
    dest.write_str("@layer")?;
    if let Some(name) = &self.name {
      dest.write_char(' ')?;
      name.to_css(dest)?;
    }

    dest.whitespace()?;
    dest.write_char('{')?;
    dest.indent();
    dest.newline()?;
    self.rules.to_css(dest)?;
    dest.dedent();
    dest.newline()?;
    dest.write_char('}')
  }
}
