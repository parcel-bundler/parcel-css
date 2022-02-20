use super::Location;
use crate::printer::Printer;
use super::style::StyleRule;
use crate::rules::{ToCssWithContext, StyleContext};
use crate::error::{PrinterError, MinifyError};
use super::MinifyContext;

#[derive(Debug, PartialEq, Clone)]
pub struct NestingRule<'i, T> {
  pub style: StyleRule<'i, T>,
  pub loc: Location
}

impl<'i, T> NestingRule<'i, T> {
  pub(crate) fn minify(&mut self, context: &mut MinifyContext<'_, 'i>, parent_is_unused: bool) -> Result<bool, MinifyError> {
    self.style.minify(context, parent_is_unused)
  }
}

impl<'a, 'i, T: cssparser::ToCss> ToCssWithContext<'a, 'i, T> for NestingRule<'i, T> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>) -> Result<(), PrinterError> where W: std::fmt::Write {
    dest.add_mapping(self.loc);
    if context.is_none() {
      dest.write_str("@nest ")?;
    }
    self.style.to_css_with_context(dest, context)
  }
}
