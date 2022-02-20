use crate::{values::string::CowArcStr, parser::DefaultAtRule};
use cssparser::*;
use parcel_selectors::{SelectorList, parser::{SelectorImpl, Selector, Combinator, Component}, attr::{AttrSelectorOperator, ParsedAttrSelectorOperation, ParsedCaseSensitivity}};
use std::fmt;
use std::fmt::Write;
use crate::printer::Printer;
use crate::traits::ToCss;
use crate::compat::Feature;
use crate::vendor_prefix::VendorPrefix;
use crate::targets::Browsers;
use crate::rules::{ToCssWithContext, StyleContext};
use std::collections::HashMap;
use crate::error::{ParserError, PrinterError};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selectors;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectorString<'a>(pub CowArcStr<'a>);

impl<'a> std::convert::From<CowRcStr<'a>> for SelectorString<'a> {
  fn from(s: CowRcStr<'a>) -> SelectorString<'a> {
    SelectorString(s.into())
  }
}

impl<'a> cssparser::ToCss for SelectorString<'a> {
  fn to_css<W>(&self, dest: &mut W) -> std::fmt::Result where W: std::fmt::Write {
    write!(CssStringWriter::new(dest), "{}", &self.0)
  }
}

impl<'a> SelectorString<'a> {
  pub fn write_identifier<W>(&self, dest: &mut W)-> Result<(), PrinterError> where W: fmt::Write {
    serialize_identifier(&self.0, dest)?;
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct SelectorIdent<'i>(pub CowArcStr<'i>);

impl<'a> std::convert::From<CowRcStr<'a>> for SelectorIdent<'a> {
  fn from(s: CowRcStr<'a>) -> SelectorIdent {
    SelectorIdent(s.into())
  }
}

impl<'i> cssparser::ToCss for SelectorIdent<'i> {
  fn to_css<W>(&self, dest: &mut W) -> std::fmt::Result where W: std::fmt::Write {
    serialize_identifier(&self.0, dest)
  }
}

impl<'i> SelectorImpl<'i> for Selectors {
  type AttrValue = SelectorString<'i>;
  type Identifier = SelectorIdent<'i>;
  type LocalName = SelectorIdent<'i>;
  type NamespacePrefix = SelectorIdent<'i>;
  type NamespaceUrl = SelectorIdent<'i>;
  type BorrowedNamespaceUrl = SelectorIdent<'i>;
  type BorrowedLocalName = SelectorIdent<'i>;

  type NonTSPseudoClass = PseudoClass<'i>;
  type PseudoElement = PseudoElement<'i>;

  type ExtraMatchingData = ();

  fn to_css<W: fmt::Write>(selectors: &SelectorList<'i, Self>, dest: &mut W) -> std::fmt::Result {
    let mut printer = Printer::new(dest, None, false, None);
    serialize_selector_list::<_, _, DefaultAtRule>(selectors.0.iter(), &mut printer, None).map_err(|_| std::fmt::Error)
  }
}

pub struct SelectorParser<'a, 'i> {
  pub default_namespace: &'a Option<CowArcStr<'i>>,
  pub namespace_prefixes: &'a HashMap<CowArcStr<'i>, CowArcStr<'i>>,
  pub is_nesting_allowed: bool,
  pub css_modules: bool
}

impl<'a, 'i> parcel_selectors::parser::Parser<'i> for SelectorParser<'a, 'i> {
  type Impl = Selectors;
  type Error = ParserError<'i>;

  fn parse_non_ts_pseudo_class(
    &self,
    _: SourceLocation,
    name: CowRcStr<'i>,
  ) -> Result<PseudoClass<'i>, ParseError<'i, Self::Error>> {
      use PseudoClass::*;
      let pseudo_class = match_ignore_ascii_case! { &name,
        // https://drafts.csswg.org/selectors-4/#useraction-pseudos
        "hover" => Hover,
        "active" => Active,
        "focus" => Focus,
        "focus-visible" => FocusVisible,
        "focus-within" => FocusWithin,

        // https://drafts.csswg.org/selectors-4/#time-pseudos
        "current" => Current,
        "past" => Past,
        "future" => Future,

        // https://drafts.csswg.org/selectors-4/#resource-pseudos
        "playing" => Playing,
        "paused" => Paused,
        "seeking" => Seeking,
        "buffering" => Buffering,
        "stalled" => Stalled,
        "muted" => Muted,
        "volume-locked" => VolumeLocked,

        // https://fullscreen.spec.whatwg.org/#:fullscreen-pseudo-class
        "fullscreen" => Fullscreen(VendorPrefix::None),
        "-webkit-full-screen" => Fullscreen(VendorPrefix::WebKit),
        "-moz-full-screen" => Fullscreen(VendorPrefix::Moz),
        "-ms-fullscreen" => Fullscreen(VendorPrefix::Ms),

        // https://drafts.csswg.org/selectors-4/#the-defined-pseudo
        "defined" => Defined,

        // https://drafts.csswg.org/selectors-4/#location
        "any-link" => AnyLink(VendorPrefix::None),
        "-webkit-any-link" => AnyLink(VendorPrefix::WebKit),
        "-moz-any-link" => AnyLink(VendorPrefix::Moz),
        "link" => Link,
        "local-link" => LocalLink,
        "target" => Target,
        "target-within" => TargetWithin,
        "visited" => Visited,

        // https://drafts.csswg.org/selectors-4/#input-pseudos
        "enabled" => Enabled,
        "disabled" => Disabled,
        "read-only" => ReadOnly(VendorPrefix::None),
        "-moz-read-only" => ReadOnly(VendorPrefix::Moz),
        "read-write" => ReadWrite(VendorPrefix::None),
        "-moz-read-write" => ReadWrite(VendorPrefix::Moz),
        "placeholder-shown" => PlaceholderShown(VendorPrefix::None),
        "-moz-placeholder-shown" => PlaceholderShown(VendorPrefix::Moz),
        "-ms-placeholder-shown" => PlaceholderShown(VendorPrefix::Ms),
        "default" => Default,
        "checked" => Checked,
        "indeterminate" => Indeterminate,
        "blank" => Blank,
        "valid" => Valid,
        "invalid" => Invalid,
        "in-range" => InRange,
        "out-of-range" => OutOfRange,
        "required" => Required,
        "optional" => Optional,
        "user-valid" => UserValid,
        "user-invalid" => UserInvalid,

        // https://html.spec.whatwg.org/multipage/semantics-other.html#selector-autofill
        "autofill" => Autofill(VendorPrefix::None),
        "-webkit-autofill" => Autofill(VendorPrefix::WebKit),
        "-o-autofill" => Autofill(VendorPrefix::O),

        _ => Custom(name.into())
      };

      Ok(pseudo_class)
  }

  fn parse_non_ts_functional_pseudo_class<'t>(
      &self,
      name: CowRcStr<'i>,
      parser: &mut cssparser::Parser<'i, 't>,
  ) -> Result<PseudoClass<'i>, ParseError<'i, Self::Error>> {
      use PseudoClass::*;
      let pseudo_class = match_ignore_ascii_case! { &name,
        "lang" => Lang(parser.expect_ident_or_string()?.as_ref().into()),
        "dir" => Dir(parser.expect_ident_or_string()?.as_ref().into()),
        "local" if self.css_modules => Local(Box::new(parcel_selectors::parser::Selector::parse(self, parser)?)),
        "global" if self.css_modules => Global(Box::new(parcel_selectors::parser::Selector::parse(self, parser)?)),
        _ => return Err(parser.new_custom_error(parcel_selectors::parser::SelectorParseErrorKind::UnexpectedIdent(name.clone()))),
      };

      Ok(pseudo_class)
  }

  fn parse_pseudo_element(
    &self,
    _: SourceLocation,
    name: CowRcStr<'i>,
  ) -> Result<PseudoElement<'i>, ParseError<'i, Self::Error>> {
    use PseudoElement::*;
    let pseudo_element = match_ignore_ascii_case! { &name,
      "before" => Before,
      "after" => After,
      "first-line" => FirstLine,
      "first-letter" => FirstLetter,
      "selection" => Selection(VendorPrefix::None),
      "-moz-selection" => Selection(VendorPrefix::Moz),
      "placeholder" => Placeholder(VendorPrefix::None),
      "-webkit-input-placeholder" => Placeholder(VendorPrefix::WebKit),
      "-moz-placeholder" => Placeholder(VendorPrefix::Moz),
      "-ms-input-placeholder" => Placeholder(VendorPrefix::Moz),
      "marker" => Marker,
      "backdrop" => Backdrop(VendorPrefix::None),
      "-webkit-backdrop" => Backdrop(VendorPrefix::WebKit),
      "file-selector-button" => FileSelectorButton(VendorPrefix::None),
      "-webkit-file-upload-button" => FileSelectorButton(VendorPrefix::WebKit),
      "-ms-browse" => FileSelectorButton(VendorPrefix::Ms),
      _ => Custom(name.into())
    };

    Ok(pseudo_element)
  }

  #[inline]
  fn parse_slotted(&self) -> bool {
    true
  }

  #[inline]
  fn parse_host(&self) -> bool {
    true
  }

  #[inline]
  fn parse_is_and_where(&self) -> bool {
    true
  }

  #[inline]
  fn parse_part(&self) -> bool {
    true
  }

  fn default_namespace(&self) -> Option<SelectorIdent<'i>> {
    self.default_namespace.clone().map(SelectorIdent)
  }

  fn namespace_for_prefix(&self, prefix: &SelectorIdent<'i>) -> Option<SelectorIdent<'i>> {
    self.namespace_prefixes.get(&prefix.0).cloned().map(SelectorIdent)
  }

  #[inline]
  fn is_nesting_allowed(&self) -> bool {
    self.is_nesting_allowed
  }
}

/// https://drafts.csswg.org/selectors-4/#structural-pseudos
#[derive(Clone, Eq, PartialEq)]
pub enum PseudoClass<'i> {
  // https://drafts.csswg.org/selectors-4/#linguistic-pseudos
  Lang(Box<str>),
  Dir(Box<str>),

  // https://drafts.csswg.org/selectors-4/#useraction-pseudos
  Hover,
  Active,
  Focus,
  FocusVisible,
  FocusWithin,

  // https://drafts.csswg.org/selectors-4/#time-pseudos
  Current,
  Past,
  Future,

  // https://drafts.csswg.org/selectors-4/#resource-pseudos
  Playing,
  Paused,
  Seeking,
  Buffering,
  Stalled,
  Muted,
  VolumeLocked,

  // https://fullscreen.spec.whatwg.org/#:fullscreen-pseudo-class
  Fullscreen(VendorPrefix),

  // https://drafts.csswg.org/selectors-4/#the-defined-pseudo
  Defined,

  // https://drafts.csswg.org/selectors-4/#location
  AnyLink(VendorPrefix),
  Link,
  LocalLink,
  Target,
  TargetWithin,
  Visited,

  // https://drafts.csswg.org/selectors-4/#input-pseudos
  Enabled,
  Disabled,
  ReadOnly(VendorPrefix),
  ReadWrite(VendorPrefix),
  PlaceholderShown(VendorPrefix),
  Default,
  Checked,
  Indeterminate,
  Blank,
  Valid,
  Invalid,
  InRange,
  OutOfRange,
  Required,
  Optional,
  UserValid,
  UserInvalid,

  // https://html.spec.whatwg.org/multipage/semantics-other.html#selector-autofill
  Autofill(VendorPrefix),

  // CSS modules
  Local(Box<parcel_selectors::parser::Selector<'i, Selectors>>),
  Global(Box<parcel_selectors::parser::Selector<'i, Selectors>>),

  Custom(CowArcStr<'i>)
}

impl<'i> parcel_selectors::parser::NonTSPseudoClass<'i> for PseudoClass<'i> {
  type Impl = Selectors;

  fn is_active_or_hover(&self) -> bool {
      matches!(*self, PseudoClass::Active | PseudoClass::Hover)
  }

  fn is_user_action_state(&self) -> bool {
      matches!(*self, PseudoClass::Active | PseudoClass::Hover | PseudoClass::Focus)
  }
}

impl<'i> cssparser::ToCss for PseudoClass<'i> {
  fn to_css<W>(&self, _: &mut W)-> std::fmt::Result where W: fmt::Write {
    unreachable!()
  }
}

impl<'a, 'i, T> ToCssWithContext<'a, 'i, T> for PseudoClass<'i> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>) -> Result<(), PrinterError> where W: fmt::Write {
      use PseudoClass::*;
      match &self {
        Lang(lang) => {
          dest.write_str(":lang(")?;
          serialize_identifier(lang, dest)?;
          return dest.write_str(")");
        }
        Dir(dir) => {
          dest.write_str(":dir(")?;
          serialize_identifier(dir, dest)?;
          return dest.write_str(")");
        }
        _ => {}
      }

      macro_rules! write_prefixed {
        ($prefix: ident, $val: expr) => {{
          dest.write_char(':')?;
          // If the printer has a vendor prefix override, use that.
          let vp = if !dest.vendor_prefix.is_empty() {
            dest.vendor_prefix
          } else {
            *$prefix
          };
          vp.to_css(dest)?;
          dest.write_str($val)
        }};
      }

      macro_rules! pseudo {
        ($key: ident, $s: literal) => {{
          let class = if let Some(pseudo_classes) = &dest.pseudo_classes {
            pseudo_classes.$key
          } else {
            None
          };

          if let Some(class) = class {
            dest.write_char('.')?;
            dest.write_ident(class)
          } else {
            dest.write_str($s)
          }
        }};
      }

      match &self {
        // https://drafts.csswg.org/selectors-4/#useraction-pseudos
        Hover => pseudo!(hover, ":hover"),
        Active => pseudo!(active, ":active"),
        Focus => pseudo!(focus, ":focus"),
        FocusVisible => pseudo!(focus_visible, ":focus-visible"),
        FocusWithin => pseudo!(focus_within, ":focus-within"),

        // https://drafts.csswg.org/selectors-4/#time-pseudos
        Current => dest.write_str(":current"),
        Past => dest.write_str(":past"),
        Future => dest.write_str(":future"),

        // https://drafts.csswg.org/selectors-4/#resource-pseudos
        Playing => dest.write_str(":playing"),
        Paused => dest.write_str(":paused"),
        Seeking => dest.write_str(":seeking"),
        Buffering => dest.write_str(":buffering"),
        Stalled => dest.write_str(":stalled"),
        Muted => dest.write_str(":muted"),
        VolumeLocked => dest.write_str(":volume-locked"),

        // https://fullscreen.spec.whatwg.org/#:fullscreen-pseudo-class
        Fullscreen(prefix) => {
          dest.write_char(':')?;
          let vp = if !dest.vendor_prefix.is_empty() {
            dest.vendor_prefix
          } else {
            *prefix
          };
          vp.to_css(dest)?;
          if vp == VendorPrefix::WebKit || vp == VendorPrefix::Moz {
            dest.write_str("full-screen")
          } else {
            dest.write_str("fullscreen")
          }
        },

        // https://drafts.csswg.org/selectors-4/#the-defined-pseudo
        Defined => dest.write_str(":defined"),

        // https://drafts.csswg.org/selectors-4/#location
        AnyLink(prefix) => write_prefixed!(prefix, "any-link"),
        Link => dest.write_str(":link"),
        LocalLink => dest.write_str(":local-link"),
        Target => dest.write_str(":target"),
        TargetWithin => dest.write_str(":target-within"),
        Visited => dest.write_str(":visited"),

        // https://drafts.csswg.org/selectors-4/#input-pseudos
        Enabled => dest.write_str(":enabled"),
        Disabled => dest.write_str(":disabled"),
        ReadOnly(prefix) => write_prefixed!(prefix, "read-only"),
        ReadWrite(prefix) => write_prefixed!(prefix, "read-write"),
        PlaceholderShown(prefix) => write_prefixed!(prefix, "placeholder-shown"),
        Default => dest.write_str(":default"),
        Checked => dest.write_str(":checked"),
        Indeterminate => dest.write_str(":indeterminate"),
        Blank => dest.write_str(":blank"),
        Valid => dest.write_str(":valid"),
        Invalid => dest.write_str(":invalid"),
        InRange => dest.write_str(":in-range"),
        OutOfRange => dest.write_str(":out-of-range"),
        Required => dest.write_str(":required"),
        Optional => dest.write_str(":optional"),
        UserValid => dest.write_str(":user-valid"),
        UserInvalid => dest.write_str(":user-invalid"),

        // https://html.spec.whatwg.org/multipage/semantics-other.html#selector-autofill
        Autofill(prefix) => write_prefixed!(prefix, "autofill"),

        Local(selector) => selector.to_css_with_context(dest, context),
        Global(selector) => {
          let css_module = std::mem::take(&mut dest.css_module);
          selector.to_css_with_context(dest, context)?;
          dest.css_module = css_module;
          Ok(())
        },

        Lang(_) | Dir(_) => unreachable!(),
        Custom(val) => {
          dest.write_char(':')?;
          return dest.write_str(&val)
        }
      }
  }
}

impl<'i> PseudoClass<'i> {
  pub fn is_equivalent(&self, other: &PseudoClass<'i>) -> bool {
    use PseudoClass::*;
    match (self, other) {
      (Fullscreen(_), Fullscreen(_)) |
      (AnyLink(_), AnyLink(_)) |
      (ReadOnly(_), ReadOnly(_)) |
      (ReadWrite(_), ReadWrite(_)) |
      (PlaceholderShown(_), PlaceholderShown(_)) |
      (Autofill(_), Autofill(_)) => true,
      (a, b) => a == b
    }
  }

  pub fn get_prefix(&self) -> VendorPrefix {
    use PseudoClass::*;
    match self {
      Fullscreen(p) |
      AnyLink(p) |
      ReadOnly(p) |
      ReadWrite(p) |
      PlaceholderShown(p) |
      Autofill(p) => *p,
      _ => VendorPrefix::empty()
    }
  }

  pub fn get_necessary_prefixes(&self, targets: Browsers) -> VendorPrefix {
    use PseudoClass::*;
    use crate::prefixes::Feature;
    let feature = match self {
      Fullscreen(p) if *p == VendorPrefix::None => Feature::PseudoClassFullscreen,
      AnyLink(p) if *p == VendorPrefix::None => Feature::PseudoClassAnyLink,
      ReadOnly(p) if *p == VendorPrefix::None => Feature::PseudoClassReadOnly,
      ReadWrite(p) if *p == VendorPrefix::None => Feature::PseudoClassReadWrite,
      PlaceholderShown(p) if *p == VendorPrefix::None => Feature::PseudoClassPlaceholderShown,
      Autofill(p) if *p == VendorPrefix::None => Feature::PseudoClassAutofill,
      _ => return VendorPrefix::empty()
    };

    feature.prefixes_for(targets)
  }
}


#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub enum PseudoElement<'i> {
  After,
  Before,
  FirstLine,
  FirstLetter,
  Selection(VendorPrefix),
  Placeholder(VendorPrefix),
  Marker,
  Backdrop(VendorPrefix),
  FileSelectorButton(VendorPrefix),
  Custom(CowArcStr<'i>)
}

impl<'i> cssparser::ToCss for PseudoElement<'i> {
  fn to_css<W>(&self, _: &mut W)-> std::fmt::Result where W: fmt::Write {
    unreachable!();
  }
}

impl<'i> ToCss for PseudoElement<'i> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: fmt::Write {
    use PseudoElement::*;

    macro_rules! write_prefix {
      ($prefix: ident) => {{
        dest.write_str("::")?;
        // If the printer has a vendor prefix override, use that.
        let vp = if !dest.vendor_prefix.is_empty() {
          dest.vendor_prefix
        } else {
          *$prefix
        };
        vp.to_css(dest)?;
        vp
      }};
    }

    macro_rules! write_prefixed {
      ($prefix: ident, $val: expr) => {{
        write_prefix!($prefix);
        dest.write_str($val)
      }};
    }

    match &self {
      // CSS2 pseudo elements support a single colon syntax in addition
      // to the more correct double colon for other pseudo elements.
      // We use that here because it's supported everywhere and is shorter.
      After => dest.write_str(":after"),
      Before => dest.write_str(":before"),
      FirstLine => dest.write_str(":first-line"),
      FirstLetter => dest.write_str(":first-letter"),
      Marker => dest.write_str("::marker"),
      Selection(prefix) => write_prefixed!(prefix, "selection"),
      Placeholder(prefix) => {
        let vp = write_prefix!(prefix);
        if vp == VendorPrefix::WebKit || vp == VendorPrefix::Ms {
          dest.write_str("input-placeholder")
        } else {
          dest.write_str("placeholder")
        }
      },
      Backdrop(prefix) => write_prefixed!(prefix, "backdrop"),
      FileSelectorButton(prefix) => {
        let vp = write_prefix!(prefix);
        if vp == VendorPrefix::WebKit {
          dest.write_str("file-upload-button")
        } else if vp == VendorPrefix::Ms {
          dest.write_str("browse")
        } else {
          dest.write_str("file-selector-button")
        }
      }
      Custom(val) => {
        dest.write_str("::")?;
        return dest.write_str(val)
      }
    }
  }
}

impl<'i> parcel_selectors::parser::PseudoElement<'i> for PseudoElement<'i> {
  type Impl = Selectors;

  fn accepts_state_pseudo_classes(&self) -> bool {
    // Be lenient.
    true
  }

  fn valid_after_slotted(&self) -> bool {
    // ::slotted() should support all tree-abiding pseudo-elements, see
    // https://drafts.csswg.org/css-scoping/#slotted-pseudo
    // https://drafts.csswg.org/css-pseudo-4/#treelike
    matches!(
      *self,
      PseudoElement::Before |
      PseudoElement::After |
      PseudoElement::Marker |
      PseudoElement::Placeholder(_) |
      PseudoElement::FileSelectorButton(_)
    )
  }
}

impl<'i> PseudoElement<'i> {
  pub fn is_equivalent(&self, other: &PseudoElement) -> bool {
    use PseudoElement::*;
    match (self, other) {
      (Selection(_), Selection(_)) |
      (Placeholder(_), Placeholder(_)) |
      (Backdrop(_), Backdrop(_)) |
      (FileSelectorButton(_), FileSelectorButton(_)) => true,
      (a, b) => a == b
    }
  }

  pub fn get_prefix(&self) -> VendorPrefix {
    use PseudoElement::*;
    match self {
      Selection(p) |
      Placeholder(p) |
      Backdrop(p) |
      FileSelectorButton(p) => *p,
      _ => VendorPrefix::empty()
    }
  }

  pub fn get_necessary_prefixes(&self, targets: Browsers) -> VendorPrefix {
    use PseudoElement::*;
    use crate::prefixes::Feature;
    let feature = match self {
      Selection(p) if *p == VendorPrefix::None => Feature::PseudoElementSelection,
      Placeholder(p) if *p == VendorPrefix::None => Feature::PseudoElementPlaceholder,
      Backdrop(p) if *p == VendorPrefix::None => Feature::PseudoElementBackdrop,
      FileSelectorButton(p) if *p == VendorPrefix::None => Feature::PseudoElementFileSelectorButton,
      _ => return VendorPrefix::empty()
    };

    feature.prefixes_for(targets)
  }
}

impl<'a, 'i, T> ToCssWithContext<'a, 'i, T> for SelectorList<'i, Selectors> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>)-> Result<(), PrinterError> where W: fmt::Write {
    serialize_selector_list(self.0.iter(), dest, context)
  }
}

impl ToCss for Combinator {
  fn to_css<W>(&self, dest: &mut Printer<W>)-> Result<(), PrinterError>
  where
      W: fmt::Write,
  {
      match *self {
          Combinator::Child => dest.delim('>', true),
          Combinator::Descendant => dest.write_str(" "),
          Combinator::NextSibling => dest.delim('+', true),
          Combinator::LaterSibling => dest.delim('~', true),
          Combinator::PseudoElement | Combinator::Part | Combinator::SlotAssignment => Ok(()),
      }
  }
}

// Copied from the selectors crate and modified to override to_css implementation.
impl<'a, 'i, T> ToCssWithContext<'a, 'i, T> for parcel_selectors::parser::Selector<'i, Selectors> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>)-> Result<(), PrinterError>
  where
      W: fmt::Write,
  {
    use parcel_selectors::parser::*;
      // Compound selectors invert the order of their contents, so we need to
      // undo that during serialization.
      //
      // This two-iterator strategy involves walking over the selector twice.
      // We could do something more clever, but selector serialization probably
      // isn't hot enough to justify it, and the stringification likely
      // dominates anyway.
      //
      // NB: A parse-order iterator is a Rev<>, which doesn't expose as_slice(),
      // which we need for |split|. So we split by combinators on a match-order
      // sequence and then reverse.

      let mut combinators = self
          .iter_raw_match_order()
          .rev()
          .filter_map(|x| x.as_combinator());
      let compound_selectors = self
          .iter_raw_match_order()
          .as_slice()
          .split(|x| x.is_combinator())
          .rev();

      let mut combinators_exhausted = false;
      for compound in compound_selectors {
          debug_assert!(!combinators_exhausted);

          // https://drafts.csswg.org/cssom/#serializing-selectors
          if compound.is_empty() {
              continue;
          }

          let has_leading_nesting = matches!(compound[0], Component::Nesting);
          let first_index = if has_leading_nesting { 1 } else { 0 };

          // 1. If there is only one simple selector in the compound selectors
          //    which is a universal selector, append the result of
          //    serializing the universal selector to s.
          //
          // Check if `!compound.empty()` first--this can happen if we have
          // something like `... > ::before`, because we store `>` and `::`
          // both as combinators internally.
          //
          // If we are in this case, after we have serialized the universal
          // selector, we skip Step 2 and continue with the algorithm.
          let (can_elide_namespace, first_non_namespace) = match compound.get(first_index) {
              Some(Component::ExplicitAnyNamespace) |
              Some(Component::ExplicitNoNamespace) |
              Some(Component::Namespace(..)) => (false, first_index + 1),
              Some(Component::DefaultNamespace(..)) => (true, first_index + 1),
              _ => (true, first_index),
          };
          let mut perform_step_2 = true;
          let next_combinator = combinators.next();
          if first_non_namespace == compound.len() - 1 {
              match (next_combinator, &compound[first_non_namespace]) {
                  // We have to be careful here, because if there is a
                  // pseudo element "combinator" there isn't really just
                  // the one simple selector. Technically this compound
                  // selector contains the pseudo element selector as well
                  // -- Combinator::PseudoElement, just like
                  // Combinator::SlotAssignment, don't exist in the
                  // spec.
                  (Some(Combinator::PseudoElement), _) |
                  (Some(Combinator::SlotAssignment), _) => (),
                  (_, &Component::ExplicitUniversalType) => {
                      // Iterate over everything so we serialize the namespace
                      // too.
                      let mut iter = compound.iter();
                      let swap_nesting = has_leading_nesting && context.is_some();
                      if swap_nesting {
                        // Swap nesting and type selector (e.g. &div -> div&).
                        iter.next();
                      }
                      
                      for simple in iter {
                          simple.to_css_with_context(dest, context)?;
                      }

                      if swap_nesting {
                        serialize_nesting(dest, context, false)?;
                      }
                      
                      // Skip step 2, which is an "otherwise".
                      perform_step_2 = false;
                  },
                  _ => (),
              }
          }

          // 2. Otherwise, for each simple selector in the compound selectors
          //    that is not a universal selector of which the namespace prefix
          //    maps to a namespace that is not the default namespace
          //    serialize the simple selector and append the result to s.
          //
          // See https://github.com/w3c/csswg-drafts/issues/1606, which is
          // proposing to change this to match up with the behavior asserted
          // in cssom/serialize-namespaced-type-selectors.html, which the
          // following code tries to match.
          if perform_step_2 {
              let mut iter = compound.iter();
              if has_leading_nesting && context.is_some() && is_type_selector(compound.get(first_non_namespace)) {
                // Swap nesting and type selector (e.g. &div -> div&).
                // This ensures that the compiled selector is valid. e.g. (div.foo is valid, .foodiv is not).
                let nesting = iter.next().unwrap();
                let local = iter.next().unwrap();
                local.to_css_with_context(dest, context)?;

                // Also check the next item in case of namespaces.
                if first_non_namespace > first_index {
                  let local = iter.next().unwrap();
                  local.to_css_with_context(dest, context)?;
                }

                nesting.to_css_with_context(dest, context)?;
              } else if has_leading_nesting && context.is_some() {
                // Nesting selector may serialize differently if it is leading, due to type selectors.
                iter.next();
                serialize_nesting(dest, context, true)?;
              }

              for simple in iter {
                  if let Component::ExplicitUniversalType = *simple {
                      // Can't have a namespace followed by a pseudo-element
                      // selector followed by a universal selector in the same
                      // compound selector, so we don't have to worry about the
                      // real namespace being in a different `compound`.
                      if can_elide_namespace {
                          continue;
                      }
                  }
                  simple.to_css_with_context(dest, context)?;
              }
          }

          // 3. If this is not the last part of the chain of the selector
          //    append a single SPACE (U+0020), followed by the combinator
          //    ">", "+", "~", ">>", "||", as appropriate, followed by another
          //    single SPACE (U+0020) if the combinator was not whitespace, to
          //    s.
          match next_combinator {
              Some(c) => c.to_css(dest)?,
              None => combinators_exhausted = true,
          };

          // 4. If this is the last part of the chain of the selector and
          //    there is a pseudo-element, append "::" followed by the name of
          //    the pseudo-element, to s.
          //
          // (we handle this above)
      }

      Ok(())
  }
}

impl<'a, 'i, T> ToCssWithContext<'a, 'i, T> for Component<'i, Selectors> {
  fn to_css_with_context<W>(&self, dest: &mut Printer<W>, context: Option<&StyleContext<'a, 'i, T>>)-> Result<(), PrinterError> where W: fmt::Write {
    use Component::*;
    match &self {
      Combinator(ref c) => c.to_css(dest),
      AttributeInNoNamespace {
        ref local_name,
        operator,
        ref value,
        case_sensitivity,
        ..
    } => {
        use cssparser::ToCss;
        dest.write_char('[')?;
        local_name.to_css(dest)?;
        cssparser::ToCss::to_css(operator, dest)?;

        if dest.minify {
          // Serialize as both an identifier and a string and choose the shorter one.
          let mut id = String::new();
          value.write_identifier(&mut id)?;

          let mut s = String::new();
          value.to_css(&mut s)?;

          if id.len() > 0 && id.len() < s.len() + 2 {
            dest.write_str(&id)?;
          } else {
            dest.write_char('"')?;
            dest.write_str(&s)?;
            dest.write_char('"')?;
          }
        } else {
          dest.write_char('"')?;
          value.to_css(dest)?;
          dest.write_char('"')?;
        }

        match case_sensitivity {
          parcel_selectors::attr::ParsedCaseSensitivity::CaseSensitive |
          parcel_selectors::attr::ParsedCaseSensitivity::AsciiCaseInsensitiveIfInHtmlElementInHtmlDocument => {},
          parcel_selectors::attr::ParsedCaseSensitivity::AsciiCaseInsensitive => dest.write_str(" i")?,
          parcel_selectors::attr::ParsedCaseSensitivity::ExplicitCaseSensitive => dest.write_str(" s")?,
        }
        dest.write_char(']')
      },
      Is(ref list) | Where(ref list) | Negation(ref list) => {
        match *self {
          Where(..) => dest.write_str(":where(")?,
          Is(..) => dest.write_str(":is(")?,
          Negation(..) => dest.write_str(":not(")?,
          _ => unreachable!(),
        }
        serialize_selector_list(list.iter(), dest, context)?;
        dest.write_str(")")
      },
      NonTSPseudoClass(pseudo) => {
        pseudo.to_css_with_context(dest, context)
      },
      PseudoElement(pseudo) => {
        pseudo.to_css(dest)
      },
      Nesting => {
        serialize_nesting(dest, context, false)
      },
      Class(ref class) => {
        dest.write_char('.')?;
        dest.write_ident(&class.0)
      }
      ID(ref id) => {
        dest.write_char('#')?;
        dest.write_ident(&id.0)
      }
      _ => {
        cssparser::ToCss::to_css(self, dest)?;
        Ok(())
      }
    }
  }
}

fn serialize_nesting<W, T>(dest: &mut Printer<W>, context: Option<&StyleContext<T>>, first: bool)-> Result<(), PrinterError> where W: fmt::Write {
  if let Some(ctx) = context {
    // If there's only one simple selector, just serialize it directly.
    // Otherwise, use an :is() pseudo class.
    // Type selectors are only allowed at the start of a compound selector,
    // so use :is() if that is not the case.
    if ctx.rule.selectors.0.len() == 1 && (first || (!has_type_selector(&ctx.rule.selectors.0[0]) && is_simple(&ctx.rule.selectors.0[0]))) {
      ctx.rule.selectors.0.first().unwrap().to_css_with_context(dest, ctx.parent)
    } else {
      dest.write_str(":is(")?;
      serialize_selector_list(ctx.rule.selectors.0.iter(), dest, ctx.parent)?;
      dest.write_char(')')
    }
  } else {
    dest.write_char('&')
  }
}

#[inline]
fn has_type_selector(selector: &parcel_selectors::parser::Selector<Selectors>) -> bool {
  let mut iter = selector.iter_raw_parse_order_from(0);
  let first = iter.next();
  if is_namespace(first) {
    is_type_selector(iter.next())
  } else {
    is_type_selector(first)
  }
}

#[inline]
fn is_simple(selector: &parcel_selectors::parser::Selector<Selectors>) -> bool {
  !selector
    .iter_raw_match_order()
    .any(|component| component.is_combinator())
}

#[inline]
fn is_type_selector(component: Option<&Component<Selectors>>) -> bool {
  matches!(
    component,
    Some(Component::LocalName(_)) |
    Some(Component::ExplicitUniversalType)
  )
}

#[inline]
fn is_namespace(component: Option<&Component<Selectors>>) -> bool {
  matches!(
    component,
    Some(Component::ExplicitAnyNamespace) | 
    Some(Component::ExplicitNoNamespace) |
    Some(Component::Namespace(..)) |
    Some(Component::DefaultNamespace(_))
  )
}

fn serialize_selector_list<'a, 'i: 'a, I, W, T>(iter: I, dest: &mut Printer<W>, context: Option<&StyleContext<'_, 'i, T>>)-> Result<(), PrinterError>
where
    I: Iterator<Item = &'a Selector<'i, Selectors>>,
    W: fmt::Write,
{
  let mut first = true;
  for selector in iter {
    if !first {
      dest.delim(',', false)?;
    }
    first = false;
    selector.to_css_with_context(dest, context)?;
  }
  Ok(())
}

pub fn is_compatible(selectors: &SelectorList<Selectors>, targets: Option<Browsers>) -> bool {
  for selector in &selectors.0 {
    let iter = selector.iter();
    for component in iter {
      let feature = match component {
        Component::ID(_) |
        Component::Class(_) |
        Component::LocalName(_) => continue,

        Component::ExplicitAnyNamespace |
        Component::ExplicitNoNamespace |
        Component::DefaultNamespace(_) |
        Component::Namespace(_, _) => Feature::CssNamespaces,

        Component::ExplicitUniversalType => Feature::CssSel2,

        Component::AttributeInNoNamespaceExists { .. } => Feature::CssSel2,
        Component::AttributeInNoNamespace { operator, case_sensitivity, .. } => {
          if *case_sensitivity != ParsedCaseSensitivity::CaseSensitive {
            Feature::CssCaseInsensitive
          } else {
            match operator {
              AttrSelectorOperator::Equal |
              AttrSelectorOperator::Includes |
              AttrSelectorOperator::DashMatch => Feature::CssSel2,
              AttrSelectorOperator::Prefix |
              AttrSelectorOperator::Substring |
              AttrSelectorOperator::Suffix => Feature::CssSel3,
            }
          }
        }
        Component::AttributeOther(attr) => {
          match attr.operation {
            ParsedAttrSelectorOperation::Exists => Feature::CssSel2,
            ParsedAttrSelectorOperation::WithValue { operator, case_sensitivity, .. } => {
              if case_sensitivity != ParsedCaseSensitivity::CaseSensitive {
                Feature::CssCaseInsensitive
              } else {
                match operator {
                  AttrSelectorOperator::Equal |
                  AttrSelectorOperator::Includes |
                  AttrSelectorOperator::DashMatch => Feature::CssSel2,
                  AttrSelectorOperator::Prefix |
                  AttrSelectorOperator::Substring |
                  AttrSelectorOperator::Suffix => Feature::CssSel3,
                }
              }
            }
          }
        }

        Component::FirstChild => Feature::CssSel2,

        Component::Empty |
        Component::FirstOfType |
        Component::LastChild |
        Component::LastOfType |
        Component::Negation(_) |
        Component::NthChild(_, _) |
        Component::NthLastChild(_, _) |
        Component::NthLastOfType(_, _) |
        Component::NthOfType(_, _) |
        Component::OnlyChild |
        Component::OnlyOfType |
        Component::Root => Feature::CssSel3,

        Component::Is(_) | Component::Nesting => Feature::CssMatchesPseudo,

        Component::Scope |
        Component::Host(_) |
        Component::Slotted(_) => Feature::Shadowdomv1,

        Component::Part(_) |
        Component::Where(_) => return false, // TODO: find this data in caniuse-lite

        Component::NonTSPseudoClass(pseudo) => {
          match pseudo {
            PseudoClass::Link |
            PseudoClass::Visited |
            PseudoClass::Active |
            PseudoClass::Hover |
            PseudoClass::Focus |
            PseudoClass::Lang(_) => Feature::CssSel2,

            PseudoClass::Checked |
            PseudoClass::Disabled |
            PseudoClass::Enabled |
            PseudoClass::Target => Feature::CssSel3,

            PseudoClass::AnyLink(prefix) if *prefix == VendorPrefix::None => Feature::CssAnyLink,
            PseudoClass::Indeterminate => Feature::CssIndeterminatePseudo,
            
            PseudoClass::Fullscreen(prefix) if *prefix == VendorPrefix::None => Feature::Fullscreen,
            
            PseudoClass::FocusVisible => Feature::CssFocusVisible,
            PseudoClass::FocusWithin => Feature::CssFocusWithin,
            PseudoClass::Default => Feature::CssDefaultPseudo,
            PseudoClass::Dir(_) => Feature::CssDirPseudo,
            PseudoClass::Optional => Feature::CssOptionalPseudo,
            PseudoClass::PlaceholderShown(prefix) if *prefix == VendorPrefix::None => Feature::CssPlaceholderShown,

            PseudoClass::ReadOnly(prefix) |
            PseudoClass::ReadWrite(prefix) if *prefix == VendorPrefix::None => Feature::CssReadOnlyWrite,

            PseudoClass::Valid |
            PseudoClass::Invalid |
            PseudoClass::Required => Feature::FormValidation,

            PseudoClass::InRange |
            PseudoClass::OutOfRange => Feature::CssInOutOfRange,

            PseudoClass::Autofill(prefix) if *prefix == VendorPrefix::None => Feature::CssAutofill,

            // Experimental, no browser support.
            PseudoClass::Current |
            PseudoClass::Past |
            PseudoClass::Future |
            PseudoClass::Playing |
            PseudoClass::Paused |
            PseudoClass::Seeking |
            PseudoClass::Stalled |
            PseudoClass::Buffering |
            PseudoClass::Muted |
            PseudoClass::VolumeLocked |
            PseudoClass::TargetWithin |
            PseudoClass::LocalLink |
            PseudoClass::Blank |
            PseudoClass::UserInvalid |
            PseudoClass::UserValid |
            PseudoClass::Defined => return false,

            PseudoClass::Custom(_) | _ => return false
          }
        }

        Component::PseudoElement(pseudo) => {
          match pseudo {
            PseudoElement::After |
            PseudoElement::Before => Feature::CssGencontent,
            PseudoElement::FirstLine => Feature::CssFirstLine,
            PseudoElement::FirstLetter => Feature::CssFirstLetter,
            PseudoElement::Selection(prefix) if *prefix == VendorPrefix::None => Feature::CssSelection,
            PseudoElement::Placeholder(prefix) if *prefix == VendorPrefix::None => Feature::CssPlaceholder,
            PseudoElement::Marker => Feature::CssMarkerPseudo,
            PseudoElement::Backdrop(prefix) if *prefix == VendorPrefix::None => Feature::Dialog,
            PseudoElement::Custom(_) | _ => return false
          }
        }

        Component::Combinator(combinator) => {
          match combinator {
            Combinator::Child | Combinator::NextSibling => Feature::CssSel2,
            Combinator::LaterSibling => Feature::CssSel3,
            _ => continue
          }
        }
      };

      if let Some(targets) = targets {
        if !feature.is_compatible(targets) {
          return false
        }
      } else {
        return false
      }
    }
  }

  true
}

/// Returns whether two selector lists are equivalent, i.e. the same minus any vendor prefix differences.
pub fn is_equivalent<'i>(selectors: &SelectorList<'i, Selectors>, other: &SelectorList<'i, Selectors>) -> bool {
  if selectors.0.len() != other.0.len() {
    return false
  }

  for (i, a) in selectors.0.iter().enumerate() {
    let b = &other.0[i];
    if a.len() != b.len() {
      return false
    }

    for (a, b) in a.iter().zip(b.iter()) {
      let is_equivalent = match (a, b) {
        (Component::NonTSPseudoClass(a_ps), Component::NonTSPseudoClass(b_ps)) => a_ps.is_equivalent(b_ps),
        (Component::PseudoElement(a_pe), Component::PseudoElement(b_pe)) => a_pe.is_equivalent(b_pe),
        (a, b) => a == b
      };
  
      if !is_equivalent {
        return false
      }
    }
  }

  true
}

/// Returns the vendor prefix (if any) used in the given selector list.
/// If multiple vendor prefixes are seen, this is invalid, and an empty result is returned.
pub fn get_prefix(selectors: &SelectorList<Selectors>) -> VendorPrefix {
  let mut prefix = VendorPrefix::empty();
  for selector in &selectors.0 {
    for component in selector.iter() {
      let p = match component {
        Component::NonTSPseudoClass(pc) => pc.get_prefix(),
        Component::PseudoElement(pe) => pe.get_prefix(),
        _ => VendorPrefix::empty()
      };

      if !p.is_empty() {
        if prefix.is_empty() || prefix == p {
          prefix = p;
        } else {
          return VendorPrefix::empty();
        }
      }
    }
  }

  prefix
}

/// Returns the necessary vendor prefixes for a given selector list to meet the provided browser targets.
pub fn get_necessary_prefixes(selectors: &SelectorList<Selectors>, targets: Browsers) -> VendorPrefix {
  let mut necessary_prefixes = VendorPrefix::empty();
  for selector in &selectors.0 {
    for component in selector.iter() {
      let prefixes = match component {
        Component::NonTSPseudoClass(pc) => pc.get_necessary_prefixes(targets),
        Component::PseudoElement(pe) => pe.get_necessary_prefixes(targets),
        _ => VendorPrefix::empty()
      };

      necessary_prefixes |= prefixes;
    }
  }

  necessary_prefixes
}

/// Determines whether a selector list contains only unused selectors.
/// A selector is considered unused if it contains a class or id component that exists in the set of unsed symbols.
pub fn is_unused(selectors: &mut std::slice::Iter<Selector<Selectors>>, unused_symbols: &HashSet<String>, parent_is_unused: bool) -> bool {
  if unused_symbols.is_empty() {
    return false
  }

  selectors.all(|selector| {
    for component in selector.iter_raw_match_order() {
      match component {
        Component::Class(name) | Component::ID(name) => {
          if unused_symbols.contains(&name.0.to_string()) {
            return true
          }
        }
        Component::Is(is) | Component::Where(is) => {
          if is_unused(&mut is.iter(), unused_symbols, parent_is_unused) {
            return true
          }
        }
        Component::Nesting => {
          if parent_is_unused {
            return true
          }
        },
        _ => {}
      }
    }

    false
  })
}
