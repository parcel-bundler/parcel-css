use super::Location;
use parcel_selectors::SelectorList;
use crate::selector::{Selectors, is_compatible, is_unused};
use crate::traits::ToCss;
use crate::printer::Printer;
use crate::declaration::DeclarationBlock;
use crate::vendor_prefix::VendorPrefix;
use crate::targets::Browsers;
use crate::rules::{CssRuleList, ToCssWithContext, StyleContext};
use crate::compat::Feature;
use crate::error::{PrinterError, PrinterErrorKind, MinifyError};
use super::MinifyContext;

#[derive(Debug, PartialEq, Clone)]
pub struct StyleRule<'i, T> {
  pub selectors: SelectorList<'i, Selectors>,
  pub vendor_prefix: VendorPrefix,
  pub declarations: DeclarationBlock<'i>,
  pub rules: CssRuleList<'i, T>,
  pub loc: Location
}

impl<'i, T> StyleRule<'i, T> {
  pub(crate) fn minify(&mut self, context: &mut MinifyContext<'_, 'i>, parent_is_unused: bool) -> Result<bool, MinifyError> {
    let mut unused = false;
    if !context.unused_symbols.is_empty() {
      if is_unused(&mut self.selectors.0.iter(), &context.unused_symbols, parent_is_unused) {
        if self.rules.0.is_empty() {
          return Ok(true)
        }

        self.declarations.declarations.clear();
        self.declarations.important_declarations.clear();
        unused = true;
      }
    }

    self.declarations.minify(context.handler, context.important_handler, context.logical_properties);

    if !self.rules.0.is_empty() {
      self.rules.minify(context, unused)?;
      if unused && self.rules.0.is_empty() {
        return Ok(true)
      }
    }

    Ok(false)
  }

  pub fn is_compatible(&self, targets: Option<Browsers>) -> bool {
    is_compatible(&self.selectors, targets)
  }
}

impl<'a, 'i, T: cssparser::ToCss> ToCssWithContext<'a, 'i, T> for StyleRule<'i, T> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>) -> Result<(), PrinterError> where W: std::fmt::Write {
    if self.vendor_prefix.is_empty() {
      self.to_css_base(dest, context)
    } else {
      let mut first_rule = true;
      macro_rules! prefix {
        ($prefix: ident) => {
          if self.vendor_prefix.contains(VendorPrefix::$prefix) {
            #[allow(unused_assignments)]
            if first_rule {
              first_rule = false;
            } else {
              if !dest.minify {
                dest.write_char('\n')?; // no indent
              }
              dest.newline()?;
            }
            dest.vendor_prefix = VendorPrefix::$prefix;
            self.to_css_base(dest, context)?;
          }
        };
      }

      prefix!(WebKit);
      prefix!(Moz);
      prefix!(Ms);
      prefix!(O);
      prefix!(None);

      dest.vendor_prefix = VendorPrefix::empty();
      Ok(())
    }
  }
}

impl<'a, 'i, T: cssparser::ToCss> StyleRule<'i, T> {
  fn to_css_base<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>) -> Result<(), PrinterError> where W: std::fmt::Write {
    // If supported, or there are no targets, preserve nesting. Otherwise, write nested rules after parent.
    let supports_nesting = self.rules.0.is_empty() || dest.targets.is_none() || Feature::CssNesting.is_compatible(dest.targets.unwrap());
    let len = self.declarations.declarations.len() + self.declarations.important_declarations.len();
    let has_declarations = supports_nesting || len > 0 || self.rules.0.is_empty();

    if has_declarations {
      dest.add_mapping(self.loc);
      self.selectors.to_css_with_context(dest, context)?;
      dest.whitespace()?;
      dest.write_char('{')?;
      dest.indent();

      let mut i = 0;
      macro_rules! write {
        ($decls: ident, $important: literal) => {
          for decl in &self.declarations.$decls {
            // The CSS modules `composes` property is handled specially, and omitted during printing.
            // We need to add the classes it references to the list for the selectors in this rule.
            if let crate::properties::Property::Composes(composes) = &decl {
              if dest.is_nested() && dest.css_module.is_some() {
                return Err(dest.error(PrinterErrorKind::InvalidComposesNesting, composes.loc))
              }
    
              if let Some(css_module) = &mut dest.css_module {
                css_module.handle_composes(&self.selectors, &composes)
                  .map_err(|e| dest.error(e, composes.loc))?;
                continue;
              }
            }
    
            dest.newline()?;
            decl.to_css(dest, $important)?;
            if i != len - 1 || !dest.minify {
              dest.write_char(';')?;
            }
    
            i += 1;
          }
        };
      }
      
      write!(declarations, false);
      write!(important_declarations, true);
    }

    macro_rules! newline {
      () => {
        if !dest.minify && (supports_nesting || len > 0) && !self.rules.0.is_empty() {
          if len > 0 {
            dest.write_char('\n')?;
          }
          dest.newline()?;
        }
      };
    }

    macro_rules! end {
      () => {
        if has_declarations {
          dest.dedent();
          dest.newline()?;
          dest.write_char('}')?;
        }
      };
    }

    // Write nested rules after the parent.
    if supports_nesting {
      newline!();
      self.rules.to_css(dest)?;
      end!();
    } else {
      end!();
      newline!();
      self.rules.to_css_with_context(dest, Some(&StyleContext {
        rule: self,
        parent: context
      }))?;
    }

    Ok(())
  }
}
