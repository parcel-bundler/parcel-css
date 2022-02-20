use super::Location;
use crate::media_query::MediaList;
use crate::traits::ToCss;
use crate::printer::Printer;
use super::{CssRuleList, MinifyContext};
use crate::rules::{ToCssWithContext, StyleContext};
use crate::error::{MinifyError, PrinterError};

#[derive(Debug, PartialEq, Clone)]
pub struct MediaRule<'i, T> {
  pub query: MediaList<'i>,
  pub rules: CssRuleList<'i, T>,
  pub loc: Location
}

impl<'i, T> MediaRule<'i, T> {
  pub(crate) fn minify(&mut self, context: &mut MinifyContext<'_, 'i>, parent_is_unused: bool) -> Result<bool, MinifyError> {
    self.rules.minify(context, parent_is_unused)?;

    if let Some(custom_media) = &context.custom_media {
      self.query.transform_custom_media(self.loc, custom_media)?;
    }

    Ok(self.rules.0.is_empty() || self.query.never_matches())
  }
}

impl<'a, 'i, T: cssparser::ToCss> ToCssWithContext<'a, 'i, T> for MediaRule<'i, T> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>) -> Result<(), PrinterError> where W: std::fmt::Write {
    // If the media query always matches, we can just output the nested rules.
    if dest.minify && self.query.always_matches() {
      self.rules.to_css_with_context(dest, context)?;
      return Ok(())
    }
    
    dest.add_mapping(self.loc);
    dest.write_str("@media ")?;
    self.query.to_css(dest)?;
    dest.whitespace()?;
    dest.write_char('{')?;
    dest.indent();
    dest.newline()?;
    self.rules.to_css_with_context(dest, context)?;
    dest.dedent();
    dest.newline()?;
    dest.write_char('}')
  }
}
