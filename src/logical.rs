use crate::rules::Location;
use crate::rules::{CssRule, CssRuleList, style::StyleRule};
use parcel_selectors::SelectorList;
use crate::selector::{SelectorIdent, SelectorString};
use crate::declaration::{DeclarationBlock, DeclarationList};
use crate::vendor_prefix::VendorPrefix;
use crate::compat::Feature;
use crate::targets::Browsers;
use crate::traits::ToCss;
use crate::printer::Printer;
use crate::error::PrinterError;
use parcel_selectors::{
  parser::{Selector, Component},
  attr::{AttrSelectorOperator, ParsedCaseSensitivity}
};
use crate::properties::{
  Property,
  PropertyId,
  custom::{CustomProperty, TokenList, Token},
};

#[derive(Debug)]
pub(crate) struct LogicalProperties {
  targets: Option<Browsers>,
  pub used: bool
}

impl LogicalProperties {
  pub fn new(targets: Option<Browsers>) -> LogicalProperties {
    LogicalProperties {
      used: false,
      targets,
    }
  }

  pub fn is_supported(&self, feature: Feature) -> bool {
    if let Some(targets) = self.targets {
      feature.is_compatible(targets)
    } else {
      true
    }
  }

  pub fn add<'i>(&mut self, dest: &mut DeclarationList<'i>, property_id: PropertyId<'i>, ltr: Property<'i>, rtl: Property<'i>) {
    self.used = true;
    dest.push(Property::Logical(LogicalProperty {
      property_id,
      ltr: Some(Box::new(ltr)),
      rtl: Some(Box::new(rtl))
    }));
  }

  pub fn add_inline<'i>(&mut self, dest: &mut DeclarationList<'i>, left: PropertyId<'i>, right: PropertyId<'i>, start: Option<Property<'i>>, end: Option<Property<'i>>) {
    self.used = true;
    dest.push(Property::Logical(LogicalProperty {
      property_id: left,
      ltr: start.clone().map(|v| Box::new(v)),
      rtl: end.clone().map(|v| Box::new(v)),
    }));

    dest.push(Property::Logical(LogicalProperty {
      property_id: right,
      ltr: end.map(|v| Box::new(v)),
      rtl: start.map(|v| Box::new(v)),
    }));
  }

  pub fn to_rules<T>(&mut self, dest: &mut CssRuleList<T>) {
    // Generate rules for [dir="ltr"] and [dir="rtl"] to define --ltr and --rtl vars.
    macro_rules! style_rule {
      ($dir: ident, $ltr: expr, $rtl: expr) => {
        dest.0.push(CssRule::Style(StyleRule {
          selectors: SelectorList(smallvec::smallvec![
            Selector::from_vec2(vec![
              Component::AttributeInNoNamespace {
                local_name: SelectorIdent("dir".into()),
                operator: AttrSelectorOperator::Equal,
                value: SelectorString(stringify!($dir).into()),
                case_sensitivity: ParsedCaseSensitivity::CaseSensitive,
                never_matches: false
              }
            ])
          ]),
          rules: CssRuleList(vec![]),
          vendor_prefix: VendorPrefix::empty(),
          declarations: DeclarationBlock {
            important_declarations: vec![],
            declarations: vec![
              Property::Custom(CustomProperty {
                name: "--ltr".into(),
                value: TokenList(vec![$ltr.into()])
              }),
              Property::Custom(CustomProperty {
                name: "--rtl".into(),
                value: TokenList(vec![$rtl.into()])
              })
            ]
          },
          loc: Location {
            source_index: 0,
            line: 0,
            column: 0
          }
        }));
      };
    }

    if self.used {
      style_rule!(ltr, Token::Ident("initial".into()), Token::WhiteSpace(" "));
      style_rule!(rtl, Token::WhiteSpace(" "), Token::Ident("initial".into()));
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogicalProperty<'i> {
  pub property_id: PropertyId<'i>,
  pub ltr: Option<Box<Property<'i>>>,
  pub rtl: Option<Box<Property<'i>>>
}

impl<'i> ToCss for LogicalProperty<'i> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    if let Some(ltr) = &self.ltr {
      dest.write_str("var(--ltr,")?;
      dest.whitespace()?;
      ltr.value_to_css(dest)?;
      dest.write_char(')')?;
    }

    if self.ltr.is_some() && self.rtl.is_some() {
      dest.whitespace()?;
    }

    if let Some(rtl) = &self.rtl {
      dest.write_str("var(--rtl,")?;
      dest.whitespace()?;
      rtl.value_to_css(dest)?;
      dest.write_char(')')?;
    }

    Ok(())
  }
}

#[derive(Debug, PartialEq)]
pub(crate) enum PropertyCategory {
  Logical,
  Physical
}

impl Default for PropertyCategory {
  fn default() -> PropertyCategory {
    PropertyCategory::Physical
  }
}
