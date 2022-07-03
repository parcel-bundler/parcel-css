//! CSS properties related to fonts.

use super::{Property, PropertyId};
use crate::compat::Feature;
use crate::context::PropertyHandlerContext;
use crate::declaration::{DeclarationBlock, DeclarationList};
use crate::error::{ParserError, PrinterError};
use crate::macros::*;
use crate::printer::Printer;
use crate::traits::{Parse, PropertyHandler, Shorthand, ToCss};
use crate::values::number::CSSNumber;
use crate::values::string::CowArcStr;
use crate::values::{angle::Angle, length::LengthPercentage, percentage::Percentage};
use cssparser::*;

/// A value for the [font-weight](https://www.w3.org/TR/css-fonts-4/#font-weight-prop) property.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  feature = "serde",
  derive(serde::Serialize, serde::Deserialize),
  serde(tag = "type", content = "value", rename_all = "kebab-case")
)]
pub enum FontWeight {
  /// An absolute font weight.
  Absolute(AbsoluteFontWeight),
  /// The `bolder` keyword.
  Bolder,
  /// The `lighter` keyword.
  Lighter,
}

impl Default for FontWeight {
  fn default() -> FontWeight {
    FontWeight::Absolute(AbsoluteFontWeight::default())
  }
}

impl<'i> Parse<'i> for FontWeight {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    if let Ok(val) = input.try_parse(AbsoluteFontWeight::parse) {
      return Ok(FontWeight::Absolute(val));
    }

    let location = input.current_source_location();
    let ident = input.expect_ident()?;
    match_ignore_ascii_case! { &*ident,
      "bolder" => Ok(FontWeight::Bolder),
      "lighter" => Ok(FontWeight::Lighter),
      _ => Err(location.new_unexpected_token_error(
        cssparser::Token::Ident(ident.clone())
      ))
    }
  }
}

impl ToCss for FontWeight {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
  where
    W: std::fmt::Write,
  {
    use FontWeight::*;
    match self {
      Absolute(val) => val.to_css(dest),
      Bolder => dest.write_str("bolder"),
      Lighter => dest.write_str("lighter"),
    }
  }
}

/// An [absolute font weight](https://www.w3.org/TR/css-fonts-4/#font-weight-absolute-values),
/// as used in the `font-weight` property.
///
/// See [FontWeight](FontWeight).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  feature = "serde",
  derive(serde::Serialize, serde::Deserialize),
  serde(tag = "type", content = "value", rename_all = "kebab-case")
)]
pub enum AbsoluteFontWeight {
  /// An explicit weight.
  Weight(CSSNumber),
  /// Same as `400`.
  Normal,
  /// Same as `700`.
  Bold,
}

impl Default for AbsoluteFontWeight {
  fn default() -> AbsoluteFontWeight {
    AbsoluteFontWeight::Normal
  }
}

impl<'i> Parse<'i> for AbsoluteFontWeight {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    if let Ok(val) = input.try_parse(CSSNumber::parse) {
      return Ok(AbsoluteFontWeight::Weight(val));
    }

    let location = input.current_source_location();
    let ident = input.expect_ident()?;
    match_ignore_ascii_case! { &*ident,
      "normal" => Ok(AbsoluteFontWeight::Normal),
      "bold" => Ok(AbsoluteFontWeight::Bold),
      _ => Err(location.new_unexpected_token_error(
        cssparser::Token::Ident(ident.clone())
      ))
    }
  }
}

impl ToCss for AbsoluteFontWeight {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
  where
    W: std::fmt::Write,
  {
    use AbsoluteFontWeight::*;
    match self {
      Weight(val) => val.to_css(dest),
      Normal => dest.write_str(if dest.minify { "400" } else { "normal" }),
      Bold => dest.write_str(if dest.minify { "700" } else { "bold" }),
    }
  }
}

enum_property! {
  /// An [absolute font size](https://www.w3.org/TR/css-fonts-3/#absolute-size-value),
  /// as used in the `font-size` property.
  ///
  /// See [FontSize](FontSize).
  #[allow(missing_docs)]
  pub enum AbsoluteFontSize {
    "xx-small": XXSmall,
    "x-small": XSmall,
    "small": Small,
    "medium": Medium,
    "large": Large,
    "x-large": XLarge,
    "xx-large": XXLarge,
  }
}

enum_property! {
  /// A [relative font size](https://www.w3.org/TR/css-fonts-3/#relative-size-value),
  /// as used in the `font-size` property.
  ///
  /// See [FontSize](FontSize).
  #[allow(missing_docs)]
  pub enum RelativeFontSize {
    Smaller,
    Larger,
  }
}

/// A value for the [font-size](https://www.w3.org/TR/css-fonts-4/#font-size-prop) property.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  feature = "serde",
  derive(serde::Serialize, serde::Deserialize),
  serde(tag = "type", content = "value", rename_all = "kebab-case")
)]
pub enum FontSize {
  /// An explicit size.
  Length(LengthPercentage),
  /// An absolute font size keyword.
  Absolute(AbsoluteFontSize),
  /// A relative font size keyword.
  Relative(RelativeFontSize),
}

impl<'i> Parse<'i> for FontSize {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    if let Ok(val) = input.try_parse(LengthPercentage::parse) {
      return Ok(FontSize::Length(val));
    }

    if let Ok(val) = input.try_parse(AbsoluteFontSize::parse) {
      return Ok(FontSize::Absolute(val));
    }

    let val = RelativeFontSize::parse(input)?;
    Ok(FontSize::Relative(val))
  }
}

impl ToCss for FontSize {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
  where
    W: std::fmt::Write,
  {
    use FontSize::*;
    match self {
      Absolute(val) => val.to_css(dest),
      Length(val) => val.to_css(dest),
      Relative(val) => val.to_css(dest),
    }
  }
}

enum_property! {
  /// A [font stretch keyword](https://www.w3.org/TR/css-fonts-4/#font-stretch-prop),
  /// as used in the `font-stretch` property.
  ///
  /// See [FontStretch](FontStretch).
  pub enum FontStretchKeyword {
    /// 100%
    "normal": Normal,
    /// 50%
    "ultra-condensed": UltraCondensed,
    /// 62.5%
    "extra-condensed": ExtraCondensed,
    /// 75%
    "condensed": Condensed,
    /// 87.5%
    "semi-condensed": SemiCondensed,
    /// 112.5%
    "semi-expanded": SemiExpanded,
    /// 125%
    "expanded": Expanded,
    /// 150%
    "extra-expanded": ExtraExpanded,
    /// 200%
    "ultra-expanded": UltraExpanded,
  }
}

impl Default for FontStretchKeyword {
  fn default() -> FontStretchKeyword {
    FontStretchKeyword::Normal
  }
}

impl Into<Percentage> for &FontStretchKeyword {
  fn into(self) -> Percentage {
    use FontStretchKeyword::*;
    let val = match self {
      UltraCondensed => 0.5,
      ExtraCondensed => 0.625,
      Condensed => 0.75,
      SemiCondensed => 0.875,
      Normal => 1.0,
      SemiExpanded => 1.125,
      Expanded => 1.25,
      ExtraExpanded => 1.5,
      UltraExpanded => 2.0,
    };
    Percentage(val)
  }
}

/// A value for the [font-stretch](https://www.w3.org/TR/css-fonts-4/#font-stretch-prop) property.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  feature = "serde",
  derive(serde::Serialize, serde::Deserialize),
  serde(tag = "type", content = "value", rename_all = "kebab-case")
)]
pub enum FontStretch {
  /// A font stretch keyword.
  Keyword(FontStretchKeyword),
  /// A percentage.
  Percentage(Percentage),
}

impl Default for FontStretch {
  fn default() -> FontStretch {
    FontStretch::Keyword(FontStretchKeyword::default())
  }
}

impl<'i> Parse<'i> for FontStretch {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    if let Ok(val) = input.try_parse(Percentage::parse) {
      return Ok(FontStretch::Percentage(val));
    }

    let keyword = FontStretchKeyword::parse(input)?;
    Ok(FontStretch::Keyword(keyword))
  }
}

impl Into<Percentage> for &FontStretch {
  fn into(self) -> Percentage {
    match self {
      FontStretch::Percentage(val) => val.clone(),
      FontStretch::Keyword(keyword) => keyword.into(),
    }
  }
}

impl ToCss for FontStretch {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
  where
    W: std::fmt::Write,
  {
    if dest.minify {
      let percentage: Percentage = self.into();
      return percentage.to_css(dest);
    }

    match self {
      FontStretch::Percentage(val) => val.to_css(dest),
      FontStretch::Keyword(val) => val.to_css(dest),
    }
  }
}

enum_property! {
  /// A [generic font family](https://www.w3.org/TR/css-fonts-4/#generic-font-families) name,
  /// as used in the `font-family` property.
  ///
  /// See [FontFamily](FontFamily).
  #[allow(missing_docs)]
  pub enum GenericFontFamily {
    "serif": Serif,
    "sans-serif": SansSerif,
    "cursive": Cursive,
    "fantasy": Fantasy,
    "monospace": Monospace,
    "system-ui": SystemUI,
    "emoji": Emoji,
    "math": Math,
    "fangsong": FangSong,
    "ui-serif": UISerif,
    "ui-sans-serif": UISansSerif,
    "ui-monospace": UIMonospace,
    "ui-rounded": UIRounded,

    // CSS wide keywords. These must be parsed as identifiers so they
    // don't get serialized as strings.
    // https://www.w3.org/TR/css-values-4/#common-keywords
    "initial": Initial,
    "inherit": Inherit,
    "unset": Unset,
    // Default is also reserved by the <custom-ident> type.
    // https://www.w3.org/TR/css-values-4/#custom-idents
    "default": Default,

    // CSS defaulting keywords
    // https://drafts.csswg.org/css-cascade-5/#defaulting-keywords
    "revert": Revert,
    "revert-layer": RevertLayer,
  }
}

/// A value for the [font-family](https://www.w3.org/TR/css-fonts-4/#font-family-prop) property.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  feature = "serde",
  derive(serde::Serialize, serde::Deserialize),
  serde(tag = "type", content = "value", rename_all = "kebab-case")
)]
pub enum FontFamily<'i> {
  /// A custom family name.
  #[cfg_attr(feature = "serde", serde(borrow))]
  FamilyName(CowArcStr<'i>),
  /// A generic family name.
  Generic(GenericFontFamily),
}

impl<'i> Parse<'i> for FontFamily<'i> {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    if let Ok(value) = input.try_parse(|i| i.expect_string_cloned()) {
      return Ok(FontFamily::FamilyName(value.into()));
    }

    if let Ok(value) = input.try_parse(GenericFontFamily::parse) {
      return Ok(FontFamily::Generic(value));
    }

    let value: CowArcStr<'i> = input.expect_ident()?.into();
    let mut string = None;
    while let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
      if string.is_none() {
        string = Some(value.to_string());
      }

      if let Some(string) = &mut string {
        string.push(' ');
        string.push_str(&ident);
      }
    }

    let value = if let Some(string) = string {
      string.into()
    } else {
      value
    };

    Ok(FontFamily::FamilyName(value))
  }
}

impl<'i> ToCss for FontFamily<'i> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
  where
    W: std::fmt::Write,
  {
    match self {
      FontFamily::Generic(val) => val.to_css(dest),
      FontFamily::FamilyName(val) => {
        // Generic family names such as sans-serif must be quoted if parsed as a string.
        // CSS wide keywords, as well as "default", must also be quoted.
        // https://www.w3.org/TR/css-fonts-4/#family-name-syntax
        if !val.is_empty() && !GenericFontFamily::parse_string(val).is_ok() {
          let mut id = String::new();
          let mut first = true;
          for slice in val.split(' ') {
            if first {
              first = false;
            } else {
              id.push(' ');
            }
            serialize_identifier(slice, &mut id)?;
          }
          if id.len() < val.len() + 2 {
            return dest.write_str(&id);
          }
        }
        serialize_string(&val, dest)?;
        Ok(())
      }
    }
  }
}

/// A value for the [font-style](https://www.w3.org/TR/css-fonts-4/#font-style-prop) property.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  feature = "serde",
  derive(serde::Serialize, serde::Deserialize),
  serde(tag = "type", content = "value", rename_all = "kebab-case")
)]
pub enum FontStyle {
  /// Normal font style.
  Normal,
  /// Italic font style.
  Italic,
  /// Oblique font style, with a custom angle.
  Oblique(Angle),
}

impl Default for FontStyle {
  fn default() -> FontStyle {
    FontStyle::Normal
  }
}

impl<'i> Parse<'i> for FontStyle {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;
    match_ignore_ascii_case! { &*ident,
      "normal" => Ok(FontStyle::Normal),
      "italic" => Ok(FontStyle::Italic),
      "oblique" => {
        let angle = input.try_parse(Angle::parse).unwrap_or(Angle::Deg(14.0));
        Ok(FontStyle::Oblique(angle))
      },
      _ => Err(location.new_unexpected_token_error(
        cssparser::Token::Ident(ident.clone())
      ))
    }
  }
}

impl ToCss for FontStyle {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
  where
    W: std::fmt::Write,
  {
    match self {
      FontStyle::Normal => dest.write_str("normal"),
      FontStyle::Italic => dest.write_str("italic"),
      FontStyle::Oblique(angle) => {
        dest.write_str("oblique")?;
        if *angle != Angle::Deg(14.0) {
          dest.write_char(' ')?;
          angle.to_css(dest)?;
        }
        Ok(())
      }
    }
  }
}

enum_property! {
  /// A value for the [font-variant-caps](https://www.w3.org/TR/css-fonts-4/#font-variant-caps-prop) property.
  pub enum FontVariantCaps {
    /// No special capitalization features are applied.
    "normal": Normal,
    /// The small capitals feature is used for lower case letters.
    "small-caps": SmallCaps,
    /// Small capitals are used for both upper and lower case letters.
    "all-small-caps": AllSmallCaps,
    /// Petite capitals are used.
    "petite-caps": PetiteCaps,
    /// Petite capitals are used for both upper and lower case letters.
    "all-petite-caps": AllPetiteCaps,
    /// Enables display of mixture of small capitals for uppercase letters with normal lowercase letters.
    "unicase": Unicase,
    /// Uses titling capitals.
    "titling-caps": TitlingCaps,
  }
}

impl Default for FontVariantCaps {
  fn default() -> FontVariantCaps {
    FontVariantCaps::Normal
  }
}

impl FontVariantCaps {
  fn is_css2(&self) -> bool {
    matches!(self, FontVariantCaps::Normal | FontVariantCaps::SmallCaps)
  }

  fn parse_css2<'i, 't>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    let value = Self::parse(input)?;
    if !value.is_css2() {
      return Err(input.new_custom_error(ParserError::InvalidValue));
    }
    Ok(value)
  }
}

/// A value for the [line-height](https://www.w3.org/TR/2020/WD-css-inline-3-20200827/#propdef-line-height) property.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  feature = "serde",
  derive(serde::Serialize, serde::Deserialize),
  serde(tag = "type", content = "value", rename_all = "kebab-case")
)]
pub enum LineHeight {
  /// The UA sets the line height based on the font.
  Normal,
  /// A multiple of the element's font size.
  Number(CSSNumber),
  /// An explicit height.
  Length(LengthPercentage),
}

impl Default for LineHeight {
  fn default() -> LineHeight {
    LineHeight::Normal
  }
}

impl<'i> Parse<'i> for LineHeight {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    if input.try_parse(|input| input.expect_ident_matching("normal")).is_ok() {
      return Ok(LineHeight::Normal);
    }

    if let Ok(val) = input.try_parse(CSSNumber::parse) {
      return Ok(LineHeight::Number(val));
    }

    Ok(LineHeight::Length(LengthPercentage::parse(input)?))
  }
}

impl ToCss for LineHeight {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
  where
    W: std::fmt::Write,
  {
    match self {
      LineHeight::Normal => dest.write_str("normal"),
      LineHeight::Number(val) => val.to_css(dest),
      LineHeight::Length(val) => val.to_css(dest),
    }
  }
}

enum_property! {
  /// A keyword for the [vertical align](https://drafts.csswg.org/css2/#propdef-vertical-align) property.
  pub enum VerticalAlignKeyword {
    /// Align the baseline of the box with the baseline of the parent box.
    "baseline": Baseline,
    /// Lower the baseline of the box to the proper position for subscripts of the parent’s box.
    "sub": Sub,
    /// Raise the baseline of the box to the proper position for superscripts of the parent’s box.
    "super": Super,
    /// Align the top of the aligned subtree with the top of the line box.
    "top": Top,
    /// Align the top of the box with the top of the parent’s content area.
    "text-top": TextTop,
    /// Align the vertical midpoint of the box with the baseline of the parent box plus half the x-height of the parent.
    "middle": Middle,
    /// Align the bottom of the aligned subtree with the bottom of the line box.
    "bottom": Bottom,
    /// Align the bottom of the box with the bottom of the parent’s content area.
    "text-bottom": TextBottom,
  }
}

/// A value for the [vertical align](https://drafts.csswg.org/css2/#propdef-vertical-align) property.
// TODO: there is a more extensive spec in CSS3 but it doesn't seem any browser implements it? https://www.w3.org/TR/css-inline-3/#transverse-alignment
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  feature = "serde",
  derive(serde::Serialize, serde::Deserialize),
  serde(tag = "type", content = "value", rename_all = "kebab-case")
)]
pub enum VerticalAlign {
  /// A vertical align keyword.
  Keyword(VerticalAlignKeyword),
  /// An explicit length.
  Length(LengthPercentage),
}

impl<'i> Parse<'i> for VerticalAlign {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    if let Ok(len) = input.try_parse(LengthPercentage::parse) {
      return Ok(VerticalAlign::Length(len));
    }

    let kw = VerticalAlignKeyword::parse(input)?;
    Ok(VerticalAlign::Keyword(kw))
  }
}

impl ToCss for VerticalAlign {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
  where
    W: std::fmt::Write,
  {
    match self {
      VerticalAlign::Keyword(kw) => kw.to_css(dest),
      VerticalAlign::Length(len) => len.to_css(dest),
    }
  }
}

define_shorthand! {
  /// A value for the [font](https://www.w3.org/TR/css-fonts-4/#font-prop) shorthand property.
  pub struct Font<'i> {
    /// The font family.
    #[cfg_attr(feature = "serde", serde(borrow))]
    family: FontFamily(Vec<FontFamily<'i>>),
    /// The font size.
    size: FontSize(FontSize),
    /// The font style.
    style: FontStyle(FontStyle),
    /// The font weight.
    weight: FontWeight(FontWeight),
    /// The font stretch.
    stretch: FontStretch(FontStretch),
    /// The line height.
    line_height: LineHeight(LineHeight),
    /// How the text should be capitalized. Only CSS 2.1 values are supported.
    variant_caps: FontVariantCaps(FontVariantCaps),
  }
}

impl<'i> Parse<'i> for Font<'i> {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    let mut style = None;
    let mut weight = None;
    let mut stretch = None;
    let size;
    let mut variant_caps = None;
    let mut count = 0;

    loop {
      // Skip "normal" since it is valid for several properties, but we don't know which ones it will be used for yet.
      if input.try_parse(|input| input.expect_ident_matching("normal")).is_ok() {
        count += 1;
        continue;
      }
      if style.is_none() {
        if let Ok(value) = input.try_parse(FontStyle::parse) {
          style = Some(value);
          count += 1;
          continue;
        }
      }
      if weight.is_none() {
        if let Ok(value) = input.try_parse(FontWeight::parse) {
          weight = Some(value);
          count += 1;
          continue;
        }
      }
      if variant_caps.is_none() {
        if let Ok(value) = input.try_parse(FontVariantCaps::parse_css2) {
          variant_caps = Some(value);
          count += 1;
          continue;
        }
      }

      if stretch.is_none() {
        if let Ok(value) = input.try_parse(FontStretchKeyword::parse) {
          stretch = Some(FontStretch::Keyword(value));
          count += 1;
          continue;
        }
      }
      size = Some(FontSize::parse(input)?);
      break;
    }

    if count > 4 {
      return Err(input.new_custom_error(ParserError::InvalidDeclaration));
    }

    let size = match size {
      Some(s) => s,
      None => return Err(input.new_custom_error(ParserError::InvalidDeclaration)),
    };

    let line_height = if input.try_parse(|input| input.expect_delim('/')).is_ok() {
      Some(LineHeight::parse(input)?)
    } else {
      None
    };

    let family = input.parse_comma_separated(FontFamily::parse)?;
    Ok(Font {
      family,
      size,
      style: style.unwrap_or_default(),
      weight: weight.unwrap_or_default(),
      stretch: stretch.unwrap_or_default(),
      line_height: line_height.unwrap_or_default(),
      variant_caps: variant_caps.unwrap_or_default(),
    })
  }
}

impl<'i> ToCss for Font<'i> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
  where
    W: std::fmt::Write,
  {
    if self.style != FontStyle::default() {
      self.style.to_css(dest)?;
      dest.write_char(' ')?;
    }

    if self.variant_caps != FontVariantCaps::default() {
      self.variant_caps.to_css(dest)?;
      dest.write_char(' ')?;
    }

    if self.weight != FontWeight::default() {
      self.weight.to_css(dest)?;
      dest.write_char(' ')?;
    }

    if self.stretch != FontStretch::default() {
      self.stretch.to_css(dest)?;
      dest.write_char(' ')?;
    }

    self.size.to_css(dest)?;

    if self.line_height != LineHeight::default() {
      dest.delim('/', true)?;
      self.line_height.to_css(dest)?;
    }

    dest.write_char(' ')?;

    let len = self.family.len();
    for (idx, val) in self.family.iter().enumerate() {
      val.to_css(dest)?;
      if idx < len - 1 {
        dest.delim(',', false)?;
      }
    }

    Ok(())
  }
}

#[derive(Default, Debug)]
pub(crate) struct FontHandler<'i> {
  family: Option<Vec<FontFamily<'i>>>,
  size: Option<FontSize>,
  style: Option<FontStyle>,
  weight: Option<FontWeight>,
  stretch: Option<FontStretch>,
  line_height: Option<LineHeight>,
  variant_caps: Option<FontVariantCaps>,
  has_any: bool,
}

impl<'i> PropertyHandler<'i> for FontHandler<'i> {
  fn handle_property(
    &mut self,
    property: &Property<'i>,
    dest: &mut DeclarationList<'i>,
    context: &mut PropertyHandlerContext<'i, '_>,
  ) -> bool {
    use Property::*;

    macro_rules! property {
      ($prop: ident, $val: ident) => {{
        self.$prop = Some($val.clone());
        self.has_any = true;
      }};
    }

    match property {
      FontFamily(val) => property!(family, val),
      FontSize(val) => property!(size, val),
      FontStyle(val) => property!(style, val),
      FontWeight(val) => property!(weight, val),
      FontStretch(val) => property!(stretch, val),
      FontVariantCaps(val) => property!(variant_caps, val),
      LineHeight(val) => property!(line_height, val),
      Font(val) => {
        self.family = Some(val.family.clone());
        self.size = Some(val.size.clone());
        self.style = Some(val.style.clone());
        self.weight = Some(val.weight.clone());
        self.stretch = Some(val.stretch.clone());
        self.line_height = Some(val.line_height.clone());
        self.variant_caps = Some(val.variant_caps.clone());
        self.has_any = true;
        // TODO: reset other properties
      }
      Unparsed(val) if is_font_property(&val.property_id) => {
        self.finalize(dest, context);
        dest.push(property.clone());
      }
      _ => return false,
    }

    true
  }

  fn finalize(&mut self, decls: &mut DeclarationList<'i>, context: &mut PropertyHandlerContext<'i, '_>) {
    if !self.has_any {
      return;
    }

    self.has_any = false;

    let family = compatible_font_family(
      std::mem::take(&mut self.family),
      context.is_supported(Feature::FontFamilySystemUi),
    );
    let size = std::mem::take(&mut self.size);
    let style = std::mem::take(&mut self.style);
    let weight = std::mem::take(&mut self.weight);
    let stretch = std::mem::take(&mut self.stretch);
    let line_height = std::mem::take(&mut self.line_height);
    let variant_caps = std::mem::take(&mut self.variant_caps);

    if family.is_some()
      && size.is_some()
      && style.is_some()
      && weight.is_some()
      && stretch.is_some()
      && line_height.is_some()
      && variant_caps.is_some()
    {
      let caps = variant_caps.unwrap();
      decls.push(Property::Font(Font {
        family: family.unwrap(),
        size: size.unwrap(),
        style: style.unwrap(),
        weight: weight.unwrap(),
        stretch: stretch.unwrap(),
        line_height: line_height.unwrap(),
        variant_caps: if caps.is_css2() {
          caps
        } else {
          FontVariantCaps::default()
        },
      }));

      // The `font` property only accepts CSS 2.1 values for font-variant caps.
      // If we have a CSS 3+ value, we need to add a separate property.
      if !caps.is_css2() {
        decls.push(Property::FontVariantCaps(variant_caps.unwrap()))
      }
    } else {
      if let Some(val) = family {
        decls.push(Property::FontFamily(val))
      }

      if let Some(val) = size {
        decls.push(Property::FontSize(val))
      }

      if let Some(val) = style {
        decls.push(Property::FontStyle(val))
      }

      if let Some(val) = variant_caps {
        decls.push(Property::FontVariantCaps(val))
      }

      if let Some(val) = weight {
        decls.push(Property::FontWeight(val))
      }

      if let Some(val) = stretch {
        decls.push(Property::FontStretch(val))
      }

      if let Some(val) = line_height {
        decls.push(Property::LineHeight(val))
      }
    }
  }
}

const SYSTEM_UI: FontFamily = FontFamily::Generic(GenericFontFamily::SystemUI);

const DEFAULT_SYSTEM_FONTS: &[&str] = &[
  // #1: Supported as the '-apple-system' value (macOS, Safari >= 9.2 < 11, Firefox >= 43)
  "-apple-system",
  // #2: Supported as the 'BlinkMacSystemFont' value (macOS, Chrome < 56)
  "BlinkMacSystemFont",
  "Segoe UI",  // Windows >= Vista
  "Roboto",    // Android >= 4
  "Noto Sans", // Plasma >= 5.5
  "Ubuntu",    // Ubuntu >= 10.10
  "Cantarell", // GNOME >= 3
  "Helvetica Neue",
];

/// [`system-ui`](https://www.w3.org/TR/css-fonts-4/#system-ui-def) is a special generic font family
/// It is platform dependent but if not supported by the target will simply be ignored
/// This list is an attempt at providing that support
#[inline]
fn compatible_font_family(mut family: Option<Vec<FontFamily>>, is_supported: bool) -> Option<Vec<FontFamily>> {
  if is_supported {
    return family;
  }

  if let Some(families) = &mut family {
    if let Some(position) = families.iter().position(|v| *v == SYSTEM_UI) {
      families.splice(
        (position + 1)..(position + 1),
        DEFAULT_SYSTEM_FONTS
          .iter()
          .map(|name| FontFamily::FamilyName(CowArcStr::from(*name))),
      );
    }
  }

  return family;
}

#[inline]
fn is_font_property(property_id: &PropertyId) -> bool {
  match property_id {
    PropertyId::FontFamily
    | PropertyId::FontSize
    | PropertyId::FontStyle
    | PropertyId::FontWeight
    | PropertyId::FontStretch
    | PropertyId::FontVariantCaps
    | PropertyId::LineHeight
    | PropertyId::Font => true,
    _ => false,
  }
}
