pub mod keyframes;
pub mod font_face;
pub mod page;
pub mod supports;
pub mod counter_style;
pub mod namespace;
pub mod import;
pub mod media;
pub mod style;
pub mod document;
pub mod nesting;
pub mod viewport;
pub mod custom_media;
pub mod layer;

use crate::values::string::CowArcStr;
use media::MediaRule;
use import::ImportRule;
use style::StyleRule;
use keyframes::KeyframesRule;
use font_face::FontFaceRule;
use page::PageRule;
use supports::SupportsRule;
use counter_style::CounterStyleRule;
use namespace::NamespaceRule;
use document::MozDocumentRule;
use nesting::NestingRule;
use viewport::ViewportRule;
use custom_media::CustomMediaRule;
use crate::traits::ToCss;
use crate::printer::Printer;
use crate::declaration::DeclarationHandler;
use crate::vendor_prefix::VendorPrefix;
use crate::prefixes::Feature;
use crate::targets::Browsers;
use std::collections::{HashMap, HashSet};
use crate::selector::{is_equivalent, get_prefix, get_necessary_prefixes};
use crate::error::{Error, MinifyError, PrinterError, PrinterErrorKind};
use crate::logical::LogicalProperties;
use crate::dependencies::{Dependency, ImportDependency};
use self::layer::{LayerBlockRule, LayerStatementRule};

pub(crate) trait ToCssWithContext<'a, 'i, T> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>) -> Result<(), PrinterError> where W: std::fmt::Write;
}

pub(crate) struct StyleContext<'a, 'i, T> {
  pub rule: &'a StyleRule<'i, T>,
  pub parent: Option<&'a StyleContext<'a, 'i, T>>
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct Location {
  /// The index of the source file within the source map.
  pub source_index: u32,
  /// The line number, starting at 0.
  pub line: u32,
  /// The column number within a line, starting at 1 for first the character of the line.
  /// Column numbers are counted in UTF-16 code units.
  pub column: u32
}

#[derive(Debug, PartialEq, Clone)]
pub enum CssRule<'i, T> {
  Media(MediaRule<'i, T>),
  Import(ImportRule<'i>),
  Style(StyleRule<'i, T>),
  Keyframes(KeyframesRule<'i>),
  FontFace(FontFaceRule<'i>),
  Page(PageRule<'i>),
  Supports(SupportsRule<'i, T>),
  CounterStyle(CounterStyleRule<'i>),
  Namespace(NamespaceRule<'i>),
  MozDocument(MozDocumentRule<'i, T>),
  Nesting(NestingRule<'i, T>),
  Viewport(ViewportRule<'i>),
  CustomMedia(CustomMediaRule<'i>),
  LayerStatement(LayerStatementRule<'i>),
  LayerBlock(LayerBlockRule<'i, T>),
  Ignored,
  Custom(T)
}

impl<'a, 'i, T: cssparser::ToCss> ToCssWithContext<'a, 'i, T> for CssRule<'i, T> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>) -> Result<(), PrinterError> where W: std::fmt::Write {
    match self {
      CssRule::Media(media) => media.to_css_with_context(dest, context),
      CssRule::Import(import) => import.to_css(dest),
      CssRule::Style(style) => style.to_css_with_context(dest, context),
      CssRule::Keyframes(keyframes) => keyframes.to_css(dest),
      CssRule::FontFace(font_face) => font_face.to_css(dest),
      CssRule::Page(font_face) => font_face.to_css(dest),
      CssRule::Supports(supports) => supports.to_css_with_context(dest, context),
      CssRule::CounterStyle(counter_style) => counter_style.to_css(dest),
      CssRule::Namespace(namespace) => namespace.to_css(dest),
      CssRule::MozDocument(document) => document.to_css(dest),
      CssRule::Nesting(nesting) => nesting.to_css_with_context(dest, context),
      CssRule::Viewport(viewport) => viewport.to_css(dest),
      CssRule::CustomMedia(custom_media) => custom_media.to_css(dest),
      CssRule::LayerStatement(layer) => layer.to_css(dest),
      CssRule::LayerBlock(layer) => layer.to_css(dest),
      CssRule::Ignored => Ok(()),
      CssRule::Custom(rule) => rule.to_css(dest).map_err(|_| Error { kind: PrinterErrorKind::FmtError, loc: None }),
    }
  }
}

impl<'i, T: cssparser::ToCss> ToCss for CssRule<'i, T> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    self.to_css_with_context(dest, None)
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct CssRuleList<'i, T>(pub Vec<CssRule<'i, T>>);

pub(crate) struct MinifyContext<'a, 'i> {
  pub targets: &'a Option<Browsers>,
  pub handler: &'a mut DeclarationHandler<'i>,
  pub important_handler: &'a mut DeclarationHandler<'i>,
  pub logical_properties: &'a mut LogicalProperties,
  pub unused_symbols: &'a HashSet<String>,
  pub custom_media: Option<HashMap<CowArcStr<'i>, CustomMediaRule<'i>>>
}

impl<'i, T> CssRuleList<'i, T> {
  pub(crate) fn minify(&mut self, context: &mut MinifyContext<'_, 'i>, parent_is_unused: bool) -> Result<(), MinifyError> {
    let mut keyframe_rules = HashMap::new();
    let mut rules = Vec::new();
    for mut rule in self.0.drain(..) {
      match &mut rule {
        CssRule::Keyframes(keyframes) => {
          if context.unused_symbols.contains(keyframes.name.0.as_ref()) {
            continue
          }
          keyframes.minify(context);

          macro_rules! set_prefix {
            ($keyframes: ident) => {
              if $keyframes.vendor_prefix.contains(VendorPrefix::None) {
                if let Some(targets) = context.targets {
                  $keyframes.vendor_prefix = Feature::AtKeyframes.prefixes_for(*targets)
                }
              }
            };
          }
  
          // If there is an existing rule with the same name and identical keyframes,
          // merge the vendor prefixes from this rule into it.
          if let Some(existing_idx) = keyframe_rules.get(&keyframes.name) {
            if let Some(CssRule::Keyframes(existing)) = &mut rules.get_mut(*existing_idx) {
              if existing.keyframes == keyframes.keyframes {
                existing.vendor_prefix |= keyframes.vendor_prefix;
                set_prefix!(existing);
                continue;
              }
            }
          }
  
          set_prefix!(keyframes);
          keyframe_rules.insert(keyframes.name.clone(), rules.len());
        },
        CssRule::CustomMedia(_) => {
          if context.custom_media.is_some() {
            continue;
          }
        },
        CssRule::Media(media) => {
          if media.minify(context, parent_is_unused)? {
            continue
          }
        },
        CssRule::Supports(supports) => {
          supports.minify(context, parent_is_unused)?;
          if supports.rules.0.is_empty() {
            continue
          }
        },
        CssRule::MozDocument(document) => document.minify(context)?,
        CssRule::Style(style) => {
          if parent_is_unused || style.minify(context, parent_is_unused)? {
            continue
          }

          if let Some(targets) = context.targets {
            style.vendor_prefix = get_prefix(&style.selectors);
            if style.vendor_prefix.contains(VendorPrefix::None) {
              style.vendor_prefix = get_necessary_prefixes(&style.selectors, *targets);
            }
          }

          if let Some(CssRule::Style(last_style_rule)) = rules.last_mut() {
            // Merge declarations if the selectors are equivalent, and both are compatible with all targets.
            if style.selectors == last_style_rule.selectors && style.is_compatible(*context.targets) && last_style_rule.is_compatible(*context.targets) && style.rules.0.is_empty() && last_style_rule.rules.0.is_empty() {
              last_style_rule.declarations.declarations.extend(style.declarations.declarations.drain(..));
              last_style_rule.declarations.important_declarations.extend(style.declarations.important_declarations.drain(..));
              last_style_rule.declarations.minify(context.handler, context.important_handler, context.logical_properties);
              continue
            } else if style.declarations == last_style_rule.declarations && style.rules.0.is_empty() && last_style_rule.rules.0.is_empty() {
              // Append the selectors to the last rule if the declarations are the same, and all selectors are compatible.
              if style.is_compatible(*context.targets) && last_style_rule.is_compatible(*context.targets) {
                last_style_rule.selectors.0.extend(style.selectors.0.drain(..));
                continue
              }

              // If both selectors are potentially vendor prefixable, and they are 
              // equivalent minus prefixes, add the prefix to the last rule.
              if !style.vendor_prefix.is_empty() && 
                !last_style_rule.vendor_prefix.is_empty() &&
                is_equivalent(&style.selectors, &last_style_rule.selectors)
              {
                // If the new rule is unprefixed, replace the prefixes of the last rule.
                // Otherwise, add the new prefix.
                if style.vendor_prefix.contains(VendorPrefix::None) {
                  last_style_rule.vendor_prefix = style.vendor_prefix;
                } else {
                  last_style_rule.vendor_prefix |= style.vendor_prefix;
                }
                continue
              }
            }
          }
        },
        CssRule::CounterStyle(counter_style) => {
          if context.unused_symbols.contains(counter_style.name.0.as_ref()) {
            continue
          }
        }
        CssRule::Nesting(nesting) => {
          if nesting.minify(context, parent_is_unused)? {
            continue
          }
        }
        _ => {}
      }

      rules.push(rule)
    }

    self.0 = rules;
    Ok(())
  }
}

impl<'i, T: cssparser::ToCss> ToCss for CssRuleList<'i, T> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    self.to_css_with_context(dest, None)
  }
}

impl<'a, 'i, T: cssparser::ToCss> ToCssWithContext<'a, 'i, T> for CssRuleList<'i, T> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>) -> Result<(), PrinterError> where W: std::fmt::Write {
    let mut first = true;
    let mut last_without_block = false;

    for rule in &self.0 {
      if let CssRule::Ignored = &rule {
        continue
      }

      // Skip @import rules if collecting dependencies.
      if let CssRule::Import(rule) = &rule {
        let dep = if dest.dependencies.is_some() {
          Some(Dependency::Import(ImportDependency::new(&rule, dest.filename())))
        } else {
          None
        };

        if let Some(dependencies) = &mut dest.dependencies {
          dependencies.push(dep.unwrap());
          continue;
        }
      }

      if first {
        first = false;
      } else {
        if !dest.minify && !(last_without_block && matches!(rule, CssRule::Import(..) | CssRule::Namespace(..) | CssRule::LayerStatement(..))) {
          dest.write_char('\n')?;
        }
        dest.newline()?;
      }
      rule.to_css_with_context(dest, context)?;
      last_without_block = matches!(rule, CssRule::Import(..) | CssRule::Namespace(..) | CssRule::LayerStatement(..));
    }

    Ok(())
  }
}
