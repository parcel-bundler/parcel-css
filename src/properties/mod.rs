pub mod custom;
pub mod margin_padding;
pub mod background;
pub mod outline;
pub mod flex;
pub mod align;
pub mod font;
pub mod box_shadow;
pub mod border;
pub mod border_image;
pub mod border_radius;
pub mod transition;
pub mod animation;
pub mod transform;
pub mod prefix_handler;
pub mod display;
pub mod text;
pub mod position;
pub mod overflow;
pub mod ui;
pub mod list;
#[cfg(feature = "grid")]
pub mod grid;
pub mod css_modules;
pub mod size;
pub mod svg;
pub mod masking;
pub mod effects;

use crate::values::string::CowArcStr;
use cssparser::*;
use custom::*;
use background::*;
use outline::*;
use flex::*;
use align::*;
use font::*;
use box_shadow::*;
use border::*;
use border_image::*;
use border_radius::*;
use transition::*;
use animation::*;
use transform::*;
use display::*;
use text::*;
use overflow::*;
use ui::*;
use list::*;
#[cfg(feature = "grid")]
use grid::*;
use css_modules::*;
use size::*;
use svg::*;
use masking::*;
use effects::*;
use crate::values::{image::*, length::*, position::*, alpha::*, size::Size2D, rect::*, color::*, time::Time, easing::EasingFunction, shape::FillRule};
use crate::traits::{Parse, ToCss};
use crate::printer::Printer;
use smallvec::{SmallVec, smallvec};
use crate::vendor_prefix::VendorPrefix;
use crate::parser::ParserOptions;
use crate::error::{ParserError, PrinterError};
use crate::logical::LogicalProperty;
use crate::targets::Browsers;
use crate::prefixes::Feature;
use crate::parser::starts_with_ignore_ascii_case;

macro_rules! define_properties {
  (
    $(
      $(#[$meta: meta])*
      $name: literal: $property: ident($type: ty $(, $vp: ty)?) $( / $prefix: ident )* $( unprefixed: $unprefixed: literal )? $( if $condition: ident )?,
    )+
  ) => {
    #[derive(Debug, Clone, PartialEq)]
    pub enum PropertyId<'i> {
      $(
        $(#[$meta])*
        $property$(($vp))?,
      )+
      All,
      Custom(CowArcStr<'i>)
    }

    macro_rules! vp_name {
      ($x: ty, $n: ident) => {
        $n
      };
    }

    impl<'i> Parse<'i> for PropertyId<'i> {
      fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        let name = input.expect_ident()?;
        let name_ref = name.as_ref();
        let (prefix, name_ref) = if starts_with_ignore_ascii_case(name_ref, "-webkit-") {
          (VendorPrefix::WebKit, &name_ref[8..])
        } else if starts_with_ignore_ascii_case(name_ref, "-moz-") {
          (VendorPrefix::Moz, &name_ref[5..])
        } else if starts_with_ignore_ascii_case(name_ref, "-o-") {
          (VendorPrefix::O, &name_ref[3..])
        } else if starts_with_ignore_ascii_case(name_ref, "-ms-") {
          (VendorPrefix::Ms, &name_ref[4..])
        } else {
          (VendorPrefix::None, name_ref)
        };
        
        macro_rules! get_allowed_prefixes {
          ($v: literal) => {
            VendorPrefix::empty()
          };
          () => {
            VendorPrefix::None
          };
        }

        match_ignore_ascii_case! { name_ref,
          $(
            $(#[$meta])*
            $name => {
              macro_rules! get_propertyid {
                ($v: ty) => {
                  PropertyId::$property(prefix)
                };
                () => {
                  PropertyId::$property
                };
              }

              let allowed_prefixes = get_allowed_prefixes!($($unprefixed)?) $(| VendorPrefix::$prefix)*;
              if allowed_prefixes.contains(prefix) {
                return Ok(get_propertyid!($($vp)?))
              }
            },
          )+
          "all" => return Ok(PropertyId::All),
          _ => {}
        }
        Ok(PropertyId::Custom(name.into()))
      }
    }

    impl<'i> ToCss for PropertyId<'i> {
      fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
        use PropertyId::*;

        let (name, prefix) = match self {
          $(
            $(#[$meta])*
            $property$((vp_name!($vp, prefix)))? => {
              macro_rules! get_prefix {
                ($v: ty) => {
                  *prefix
                };
                () => {
                  VendorPrefix::None
                };
              }
      
              ($name, get_prefix!($($vp)?))
            },
          )+
          All => ("all", VendorPrefix::None),
          Custom(name) => (name.as_ref(), VendorPrefix::None),
        };

        let mut first = true;
        macro_rules! delim {
          () => {
            #[allow(unused_assignments)]
            if first {
              first = false;
            } else {
              dest.delim(',', false)?;
            }
          };
        }

        macro_rules! write {
          ($p: expr) => {
            if prefix.contains($p) {
              delim!();
              $p.to_css(dest)?;
              dest.write_str(name)?;
            }
          };
        }

        write!(VendorPrefix::WebKit);
        write!(VendorPrefix::Moz);
        write!(VendorPrefix::Ms);
        write!(VendorPrefix::O);
        write!(VendorPrefix::None);
        Ok(())
      }
    }

    impl<'i> PropertyId<'i> {
      fn prefix(&self) -> VendorPrefix {
        use PropertyId::*;
        match self {
          $(
            $(#[$meta])*
            $property$((vp_name!($vp, prefix)))? => {
              $(
                macro_rules! return_prefix {
                  ($v: ty) => {
                    return *prefix;
                  };
                }
  
                return_prefix!($vp);
              )?
              #[allow(unreachable_code)]
              VendorPrefix::None
            },
          )+
          _ => VendorPrefix::None
        }
      }

      fn with_prefix(&self, prefix: VendorPrefix) -> PropertyId<'i> {
        use PropertyId::*;
        match self {
          $(
            $(#[$meta])*
            $property$((vp_name!($vp, _p)))? => {
              macro_rules! get_prefixed {
                ($v: ty) => {
                  PropertyId::$property(prefix)
                };
                () => {
                  PropertyId::$property
                }
              }

              get_prefixed!($($vp)?)
            },
          )+
          _ => self.clone()
        }
      }

      fn set_prefixes_for_targets(&mut self, targets: Option<Browsers>) {
        match self {
          $(
            $(#[$meta])*
            #[allow(unused_variables)]
            PropertyId::$property$((vp_name!($vp, prefix)))? => {
              macro_rules! get_prefixed {
                ($v: ty, $u: literal) => {};
                ($v: ty) => {{
                  if prefix.contains(VendorPrefix::None) {
                    if let Some(targets) = targets {
                      *prefix = Feature::$property.prefixes_for(targets);
                    }
                  };
                }};
                () => {};
              }

              get_prefixed!($($vp)? $(, $unprefixed)?);
            },
          )+
          _ => {}
        }
      }

      #[allow(dead_code)]
      pub(crate) fn name(&self) -> &str {
        use PropertyId::*;

        match self {
          $(
            $(#[$meta])*
            $property$((vp_name!($vp, _p)))? => $name,
          )+
          All => "all",
          Custom(name) => &name
        }
      }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Property<'i> {
      $(
        $(#[$meta])*
        $property($type, $($vp)?),
      )+
      Unparsed(UnparsedProperty<'i>),
      Custom(CustomProperty<'i>),
      Logical(LogicalProperty<'i>)
    }

    impl<'i> Property<'i> {
      pub fn parse<'t, T>(name: CowRcStr<'i>, input: &mut Parser<'i, 't>, options: &ParserOptions<T>) -> Result<Property<'i>, ParseError<'i, ParserError<'i>>> {
        let state = input.state();
        let name_ref = name.as_ref();
        let (prefix, name_ref) = if starts_with_ignore_ascii_case(name_ref, "-webkit-") {
          (VendorPrefix::WebKit, &name_ref[8..])
        } else if starts_with_ignore_ascii_case(name_ref, "-moz-") {
          (VendorPrefix::Moz, &name_ref[5..])
        } else if starts_with_ignore_ascii_case(name_ref, "-o-") {
          (VendorPrefix::O, &name_ref[3..])
        } else if starts_with_ignore_ascii_case(name_ref, "-ms-") {
          (VendorPrefix::Ms, &name_ref[4..])
        } else {
          (VendorPrefix::None, name_ref)
        };

        macro_rules! get_allowed_prefixes {
          ($v: literal) => {
            VendorPrefix::empty()
          };
          () => {
            VendorPrefix::None
          };
        }

        let property_id = match_ignore_ascii_case! { name_ref,
          $(
            $(#[$meta])*
            $name $(if options.$condition)? => {
              let allowed_prefixes = get_allowed_prefixes!($($unprefixed)?) $(| VendorPrefix::$prefix)*;
              if allowed_prefixes.contains(prefix) {
                if let Ok(c) = <$type>::parse(input) {
                  if input.expect_exhausted().is_ok() {
                    macro_rules! get_property {
                      ($v: ty) => {
                        Property::$property(c, prefix)
                      };
                      () => {
                        Property::$property(c)
                      };
                    }

                    return Ok(get_property!($($vp)?))
                  }
                }

                macro_rules! get_propertyid {
                  ($v: ty) => {
                    PropertyId::$property(prefix)
                  };
                  () => {
                    PropertyId::$property
                  };
                }

                get_propertyid!($($vp)?)
              } else {
                return Ok(Property::Custom(CustomProperty::parse(name, input)?))
              }
            },
          )+
          _ => return Ok(Property::Custom(CustomProperty::parse(name, input)?))
        };

        // If a value was unable to be parsed, treat as an unparsed property.
        // This is different from a custom property, handled below, in that the property name is known
        // and stored as an enum rather than a string. This lets property handlers more easily deal with it.
        // Ideally we'd only do this if var() or env() references were seen, but err on the safe side for now.
        input.reset(&state);
        return Ok(Property::Unparsed(UnparsedProperty::parse(property_id, input)?))
      }

      #[allow(dead_code)]
      pub(crate) fn name(&self) -> &str {
        use Property::*;

        match self {
          $(
            $(#[$meta])*
            $property(_, $(vp_name!($vp, _p))?) => $name,
          )+
          Unparsed(unparsed) => unparsed.property_id.name(),
          Logical(logical) => logical.property_id.name(),
          Custom(custom) => &custom.name,
        }
      }

      pub(crate) fn value_to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
        use Property::*;

        match self {
          $(
            $(#[$meta])*
            $property(val, $(vp_name!($vp, _p))?) => {
              val.to_css(dest)
            }
          )+
          Unparsed(unparsed) => {
            unparsed.value.to_css(dest)
          }
          Custom(custom) => {
            custom.value.to_css(dest)
          }
          Logical(logical) => {
            logical.to_css(dest)
          }
        }
      }

      pub(crate) fn to_css<W>(&self, dest: &mut Printer<W>, important: bool) -> Result<(), PrinterError> where W: std::fmt::Write {
        use Property::*;

        let mut first = true;
        macro_rules! start {
          () => {
            #[allow(unused_assignments)]
            if first {
              first = false;
            } else {
              dest.write_char(';')?;
              dest.newline()?;
            }
          };
        }

        let (name, prefix) = match self {
          $(
            $(#[$meta])*
            $property(_, $(vp_name!($vp, prefix))?) => {
              macro_rules! get_prefix {
                ($v: ty) => {
                  *prefix
                };
                () => {
                  VendorPrefix::None
                };
              }
      
              ($name, get_prefix!($($vp)?))
            },
          )+
          Unparsed(unparsed) => (unparsed.property_id.name(), unparsed.property_id.prefix()),
          Logical(logical) => (logical.property_id.name(), logical.property_id.prefix()),
          Custom(custom) => (custom.name.as_ref(), VendorPrefix::None),
        };

        macro_rules! write {
          ($p: expr) => {
            if prefix.contains($p) {
              start!();
              $p.to_css(dest)?;
              dest.write_str(name)?;
              dest.delim(':', false)?;
              self.value_to_css(dest)?;
              if important {
                dest.whitespace()?;
                dest.write_str("!important")?;
              }
            }
          }
        }

        write!(VendorPrefix::WebKit);
        write!(VendorPrefix::Moz);
        write!(VendorPrefix::Ms);
        write!(VendorPrefix::O);
        write!(VendorPrefix::None);
        Ok(())
      }

      // TODO: temp
      pub fn temp_to_css<W>(&self, dest: &mut W, important: bool) -> std::fmt::Result where W: std::fmt::Write {
        let mut printer = Printer::new(dest, None, false, None);
        self.to_css(&mut printer, important).map_err(|_| std::fmt::Error)
      }
    }
  };
}

define_properties! {
  "background-color": BackgroundColor(CssColor),
  "background-image": BackgroundImage(SmallVec<[Image<'i>; 1]>),
  "background-position-x": BackgroundPositionX(SmallVec<[HorizontalPosition; 1]>),
  "background-position-y": BackgroundPositionY(SmallVec<[VerticalPosition; 1]>),
  "background-position": BackgroundPosition(SmallVec<[Position; 1]>),
  "background-size": BackgroundSize(SmallVec<[BackgroundSize; 1]>),
  "background-repeat": BackgroundRepeat(SmallVec<[BackgroundRepeat; 1]>),
  "background-attachment": BackgroundAttachment(SmallVec<[BackgroundAttachment; 1]>),
  "background-clip": BackgroundClip(SmallVec<[BackgroundClip; 1]>, VendorPrefix) / WebKit / Moz,
  "background-origin": BackgroundOrigin(SmallVec<[BackgroundBox; 1]>),
  "background": Background(SmallVec<[Background<'i>; 1]>),

  "box-shadow": BoxShadow(SmallVec<[BoxShadow; 1]>, VendorPrefix) / WebKit / Moz,
  "opacity": Opacity(AlphaValue),
  "color": Color(CssColor),
  "display": Display(Display),
  "visibility": Visibility(Visibility),

  "width": Width(Size),
  "height": Height(Size),
  "min-width": MinWidth(MinMaxSize),
  "min-height": MinHeight(MinMaxSize),
  "max-width": MaxWidth(MinMaxSize),
  "max-height": MaxHeight(MinMaxSize),
  "block-size": BlockSize(Size),
  "inline-size": InlineSize(Size),
  "min-block-size": MinBlockSize(MinMaxSize),
  "min-inline-size": MinInlineSize(MinMaxSize),
  "max-block-size": MaxBlockSize(MinMaxSize),
  "max-inline-size": MaxInlineSize(MinMaxSize),
  "box-sizing": BoxSizing(BoxSizing, VendorPrefix) / WebKit / Moz,

  "overflow": Overflow(Overflow),
  "overflow-x": OverflowX(OverflowKeyword),
  "overflow-y": OverflowY(OverflowKeyword),
  "text-overflow": TextOverflow(TextOverflow, VendorPrefix) / O,

  // https://www.w3.org/TR/2020/WD-css-position-3-20200519
  "position": Position(position::Position),
  "top": Top(LengthPercentageOrAuto),
  "bottom": Bottom(LengthPercentageOrAuto),
  "left": Left(LengthPercentageOrAuto),
  "right": Right(LengthPercentageOrAuto),
  "inset-block-start": InsetBlockStart(LengthPercentageOrAuto),
  "inset-block-end": InsetBlockEnd(LengthPercentageOrAuto),
  "inset-inline-start": InsetInlineStart(LengthPercentageOrAuto),
  "inset-inline-end": InsetInlineEnd(LengthPercentageOrAuto),
  "inset-block": InsetBlock(Size2D<LengthPercentageOrAuto>),
  "inset-inline": InsetInline(Size2D<LengthPercentageOrAuto>),
  "inset": Inset(Rect<LengthPercentageOrAuto>),

  "border-top-color": BorderTopColor(CssColor),
  "border-bottom-color": BorderBottomColor(CssColor),
  "border-left-color": BorderLeftColor(CssColor),
  "border-right-color": BorderRightColor(CssColor),
  "border-block-start-color": BorderBlockStartColor(CssColor),
  "border-block-end-color": BorderBlockEndColor(CssColor),
  "border-inline-start-color": BorderInlineStartColor(CssColor),
  "border-inline-end-color": BorderInlineEndColor(CssColor),

  "border-top-style": BorderTopStyle(BorderStyle),
  "border-bottom-style": BorderBottomStyle(BorderStyle),
  "border-left-style": BorderLeftStyle(BorderStyle),
  "border-right-style": BorderRightStyle(BorderStyle),
  "border-block-start-style": BorderBlockStartStyle(BorderStyle),
  "border-block-end-style": BorderBlockEndStyle(BorderStyle),
  "border-inline-start-style": BorderInlineStartStyle(BorderStyle),
  "border-inline-end-style": BorderInlineEndStyle(BorderStyle),

  "border-top-width": BorderTopWidth(BorderSideWidth),
  "border-bottom-width": BorderBottomWidth(BorderSideWidth),
  "border-left-width": BorderLeftWidth(BorderSideWidth),
  "border-right-width": BorderRightWidth(BorderSideWidth),
  "border-block-start-width": BorderBlockStartWidth(BorderSideWidth),
  "border-block-end-width": BorderBlockEndWidth(BorderSideWidth),
  "border-inline-start-width": BorderInlineStartWidth(BorderSideWidth),
  "border-inline-end-width": BorderInlineEndWidth(BorderSideWidth),

  "border-top-left-radius": BorderTopLeftRadius(Size2D<LengthPercentage>, VendorPrefix) / WebKit / Moz,
  "border-top-right-radius": BorderTopRightRadius(Size2D<LengthPercentage>, VendorPrefix) / WebKit / Moz,
  "border-bottom-left-radius": BorderBottomLeftRadius(Size2D<LengthPercentage>, VendorPrefix) / WebKit / Moz,
  "border-bottom-right-radius": BorderBottomRightRadius(Size2D<LengthPercentage>, VendorPrefix) / WebKit / Moz,
  "border-start-start-radius": BorderStartStartRadius(Size2D<LengthPercentage>),
  "border-start-end-radius": BorderStartEndRadius(Size2D<LengthPercentage>),
  "border-end-start-radius": BorderEndStartRadius(Size2D<LengthPercentage>),
  "border-end-end-radius": BorderEndEndRadius(Size2D<LengthPercentage>),
  "border-radius": BorderRadius(BorderRadius, VendorPrefix) / WebKit / Moz,

  "border-image-source": BorderImageSource(Image<'i>),
  "border-image-outset": BorderImageOutset(Rect<LengthOrNumber>),
  "border-image-repeat": BorderImageRepeat(BorderImageRepeat),
  "border-image-width": BorderImageWidth(Rect<BorderImageSideWidth>),
  "border-image-slice": BorderImageSlice(BorderImageSlice),
  "border-image": BorderImage(BorderImage<'i>, VendorPrefix) / WebKit / Moz / O,

  "border-color": BorderColor(Rect<CssColor>),
  "border-style": BorderStyle(Rect<BorderStyle>),
  "border-width": BorderWidth(Rect<BorderSideWidth>),

  "border-block-color": BorderBlockColor(CssColor),
  "border-block-style": BorderBlockStyle(BorderStyle),
  "border-block-width": BorderBlockWidth(BorderSideWidth),

  "border-inline-color": BorderInlineColor(CssColor),
  "border-inline-style": BorderInlineStyle(BorderStyle),
  "border-inline-width": BorderInlineWidth(BorderSideWidth),

  "border": Border(Border),
  "border-top": BorderTop(Border),
  "border-bottom": BorderBottom(Border),
  "border-left": BorderLeft(Border),
  "border-right": BorderRight(Border),
  "border-block": BorderBlock(Border),
  "border-block-start": BorderBlockStart(Border),
  "border-block-end": BorderBlockEnd(Border),
  "border-inline": BorderInline(Border),
  "border-inline-start": BorderInlineStart(Border),
  "border-inline-end": BorderInlineEnd(Border),

  "outline": Outline(Outline),
  "outline-color": OutlineColor(CssColor),
  "outline-style": OutlineStyle(OutlineStyle),
  "outline-width": OutlineWidth(BorderSideWidth),

  // Flex properties: https://www.w3.org/TR/2018/CR-css-flexbox-1-20181119
  "flex-direction": FlexDirection(FlexDirection, VendorPrefix) / WebKit / Ms,
  "flex-wrap": FlexWrap(FlexWrap, VendorPrefix) / WebKit / Ms,
  "flex-flow": FlexFlow(FlexFlow, VendorPrefix) / WebKit / Ms,
  "flex-grow": FlexGrow(f32, VendorPrefix) / WebKit,
  "flex-shrink": FlexShrink(f32, VendorPrefix) / WebKit,
  "flex-basis": FlexBasis(LengthPercentageOrAuto, VendorPrefix) / WebKit,
  "flex": Flex(Flex, VendorPrefix) / WebKit / Ms,
  "order": Order(f32, VendorPrefix) / WebKit,

  // Align properties: https://www.w3.org/TR/2020/WD-css-align-3-20200421
  "align-content": AlignContent(AlignContent, VendorPrefix) / WebKit,
  "justify-content": JustifyContent(JustifyContent, VendorPrefix) / WebKit,
  "place-content": PlaceContent(PlaceContent),
  "align-self": AlignSelf(AlignSelf, VendorPrefix) / WebKit,
  "justify-self": JustifySelf(JustifySelf),
  "place-self": PlaceSelf(PlaceSelf),
  "align-items": AlignItems(AlignItems, VendorPrefix) / WebKit,
  "justify-items": JustifyItems(JustifyItems),
  "place-items": PlaceItems(PlaceItems),
  "row-gap": RowGap(GapValue),
  "column-gap": ColumnGap(GapValue),
  "gap": Gap(Gap),

  // Old flex (2009): https://www.w3.org/TR/2009/WD-css3-flexbox-20090723/
  "box-orient": BoxOrient(BoxOrient, VendorPrefix) / WebKit / Moz unprefixed: false,
  "box-direction": BoxDirection(BoxDirection, VendorPrefix) / WebKit / Moz unprefixed: false,
  "box-ordinal-group": BoxOrdinalGroup(f32, VendorPrefix) / WebKit / Moz unprefixed: false,
  "box-align": BoxAlign(BoxAlign, VendorPrefix) / WebKit / Moz unprefixed: false,
  "box-flex": BoxFlex(f32, VendorPrefix) / WebKit / Moz unprefixed: false,
  "box-flex-group": BoxFlexGroup(f32, VendorPrefix) / WebKit unprefixed: false,
  "box-pack": BoxPack(BoxPack, VendorPrefix) / WebKit / Moz unprefixed: false,
  "box-lines": BoxLines(BoxLines, VendorPrefix) / WebKit / Moz unprefixed: false,

  // Old flex (2012): https://www.w3.org/TR/2012/WD-css3-flexbox-20120322/
  "flex-pack": FlexPack(FlexPack, VendorPrefix) / Ms unprefixed: false,
  "flex-order": FlexOrder(f32, VendorPrefix) / Ms unprefixed: false,
  "flex-align": FlexAlign(BoxAlign, VendorPrefix) / Ms unprefixed: false,
  "flex-item-align": FlexItemAlign(FlexItemAlign, VendorPrefix) / Ms unprefixed: false,
  "flex-line-pack": FlexLinePack(FlexLinePack, VendorPrefix) / Ms unprefixed: false,

  // Microsoft extensions
  "flex-positive": FlexPositive(f32, VendorPrefix) / Ms unprefixed: false,
  "flex-negative": FlexNegative(f32, VendorPrefix) / Ms unprefixed: false,
  "flex-preferred-size": FlexPreferredSize(LengthPercentageOrAuto, VendorPrefix) / Ms unprefixed: false,

  #[cfg(feature = "grid")]
  "grid-template-columns": GridTemplateColumns(TrackSizing<'i>),
  #[cfg(feature = "grid")]
  "grid-template-rows": GridTemplateRows(TrackSizing<'i>),
  #[cfg(feature = "grid")]
  "grid-auto-columns": GridAutoColumns(TrackSizeList),
  #[cfg(feature = "grid")]
  "grid-auto-rows": GridAutoRows(TrackSizeList),
  #[cfg(feature = "grid")]
  "grid-auto-flow": GridAutoFlow(GridAutoFlow),
  #[cfg(feature = "grid")]
  "grid-template-areas": GridTemplateAreas(GridTemplateAreas),
  #[cfg(feature = "grid")]
  "grid-template": GridTemplate(GridTemplate<'i>),
  #[cfg(feature = "grid")]
  "grid": Grid(Grid<'i>),
  #[cfg(feature = "grid")]
  "grid-row-start": GridRowStart(GridLine<'i>),
  #[cfg(feature = "grid")]
  "grid-row-end": GridRowEnd(GridLine<'i>),
  #[cfg(feature = "grid")]
  "grid-column-start": GridColumnStart(GridLine<'i>),
  #[cfg(feature = "grid")]
  "grid-column-end": GridColumnEnd(GridLine<'i>),
  #[cfg(feature = "grid")]
  "grid-row": GridRow(GridPlacement<'i>),
  #[cfg(feature = "grid")]
  "grid-column": GridColumn(GridPlacement<'i>),
  #[cfg(feature = "grid")]
  "grid-area": GridArea(GridArea<'i>),

  "margin-top": MarginTop(LengthPercentageOrAuto),
  "margin-bottom": MarginBottom(LengthPercentageOrAuto),
  "margin-left": MarginLeft(LengthPercentageOrAuto),
  "margin-right": MarginRight(LengthPercentageOrAuto),
  "margin-block-start": MarginBlockStart(LengthPercentageOrAuto),
  "margin-block-end": MarginBlockEnd(LengthPercentageOrAuto),
  "margin-inline-start": MarginInlineStart(LengthPercentageOrAuto),
  "margin-inline-end": MarginInlineEnd(LengthPercentageOrAuto),
  "margin-block": MarginBlock(Size2D<LengthPercentageOrAuto>),
  "margin-inline": MarginInline(Size2D<LengthPercentageOrAuto>),
  "margin": Margin(Rect<LengthPercentageOrAuto>),

  "padding-top": PaddingTop(LengthPercentageOrAuto),
  "padding-bottom": PaddingBottom(LengthPercentageOrAuto),
  "padding-left": PaddingLeft(LengthPercentageOrAuto),
  "padding-right": PaddingRight(LengthPercentageOrAuto),
  "padding-block-start": PaddingBlockStart(LengthPercentageOrAuto),
  "padding-block-end": PaddingBlockEnd(LengthPercentageOrAuto),
  "padding-inline-start": PaddingInlineStart(LengthPercentageOrAuto),
  "padding-inline-end": PaddingInlineEnd(LengthPercentageOrAuto),
  "padding-block": PaddingBlock(Size2D<LengthPercentageOrAuto>),
  "padding-inline": PaddingInline(Size2D<LengthPercentageOrAuto>),
  "padding": Padding(Rect<LengthPercentageOrAuto>),

  "scroll-margin-top": ScrollMarginTop(LengthPercentageOrAuto),
  "scroll-margin-bottom": ScrollMarginBottom(LengthPercentageOrAuto),
  "scroll-margin-left": ScrollMarginLeft(LengthPercentageOrAuto),
  "scroll-margin-right": ScrollMarginRight(LengthPercentageOrAuto),
  "scroll-margin-block-start": ScrollMarginBlockStart(LengthPercentageOrAuto),
  "scroll-margin-block-end": ScrollMarginBlockEnd(LengthPercentageOrAuto),
  "scroll-margin-inline-start": ScrollMarginInlineStart(LengthPercentageOrAuto),
  "scroll-margin-inline-end": ScrollMarginInlineEnd(LengthPercentageOrAuto),
  "scroll-margin-block": ScrollMarginBlock(Size2D<LengthPercentageOrAuto>),
  "scroll-margin-inline": ScrollMarginInline(Size2D<LengthPercentageOrAuto>),
  "scroll-margin": ScrollMargin(Rect<LengthPercentageOrAuto>),

  "scroll-padding-top": ScrollPaddingTop(LengthPercentageOrAuto),
  "scroll-padding-bottom": ScrollPaddingBottom(LengthPercentageOrAuto),
  "scroll-padding-left": ScrollPaddingLeft(LengthPercentageOrAuto),
  "scroll-padding-right": ScrollPaddingRight(LengthPercentageOrAuto),
  "scroll-padding-block-start": ScrollPaddingBlockStart(LengthPercentageOrAuto),
  "scroll-padding-block-end": ScrollPaddingBlockEnd(LengthPercentageOrAuto),
  "scroll-padding-inline-start": ScrollPaddingInlineStart(LengthPercentageOrAuto),
  "scroll-padding-inline-end": ScrollPaddingInlineEnd(LengthPercentageOrAuto),
  "scroll-padding-block": ScrollPaddingBlock(Size2D<LengthPercentageOrAuto>),
  "scroll-padding-inline": ScrollPaddingInline(Size2D<LengthPercentageOrAuto>),
  "scroll-padding": ScrollPadding(Rect<LengthPercentageOrAuto>),

  // shorthands: columns, list-style
  // grid, inset

  "font-weight": FontWeight(FontWeight),
  "font-size": FontSize(FontSize),
  "font-stretch": FontStretch(FontStretch),
  "font-family": FontFamily(Vec<FontFamily<'i>>),
  "font-style": FontStyle(FontStyle),
  "font-variant-caps": FontVariantCaps(FontVariantCaps),
  "line-height": LineHeight(LineHeight),
  "font": Font(Font<'i>),
  "vertical-align": VerticalAlign(VerticalAlign),

  "transition-property": TransitionProperty(SmallVec<[PropertyId<'i>; 1]>, VendorPrefix) / WebKit / Moz / Ms,
  "transition-duration": TransitionDuration(SmallVec<[Time; 1]>, VendorPrefix) / WebKit / Moz / Ms,
  "transition-delay": TransitionDelay(SmallVec<[Time; 1]>, VendorPrefix) / WebKit / Moz / Ms,
  "transition-timing-function": TransitionTimingFunction(SmallVec<[EasingFunction; 1]>, VendorPrefix) / WebKit / Moz / Ms,
  "transition": Transition(SmallVec<[Transition<'i>; 1]>, VendorPrefix) / WebKit / Moz / Ms,

  "animation-name": AnimationName(AnimationNameList<'i>, VendorPrefix) / WebKit / Moz / O,
  "animation-duration": AnimationDuration(SmallVec<[Time; 1]>, VendorPrefix) / WebKit / Moz / O,
  "animation-timing-function": AnimationTimingFunction(SmallVec<[EasingFunction; 1]>, VendorPrefix) / WebKit / Moz / O,
  "animation-iteration-count": AnimationIterationCount(SmallVec<[AnimationIterationCount; 1]>, VendorPrefix) / WebKit / Moz / O,
  "animation-direction": AnimationDirection(SmallVec<[AnimationDirection; 1]>, VendorPrefix) / WebKit / Moz / O,
  "animation-play-state": AnimationPlayState(SmallVec<[AnimationPlayState; 1]>, VendorPrefix) / WebKit / Moz / O,
  "animation-delay": AnimationDelay(SmallVec<[Time; 1]>, VendorPrefix) / WebKit / Moz / O,
  "animation-fill-mode": AnimationFillMode(SmallVec<[AnimationFillMode; 1]>, VendorPrefix) / WebKit / Moz / O,
  "animation": Animation(AnimationList<'i>, VendorPrefix) / WebKit / Moz / O,

  // https://drafts.csswg.org/css-transforms-2/
  "transform": Transform(TransformList, VendorPrefix) / WebKit / Moz / Ms / O,
  "transform-origin": TransformOrigin(Position, VendorPrefix) / WebKit / Moz / Ms / O, // TODO: handle z offset syntax
  "transform-style": TransformStyle(TransformStyle, VendorPrefix) / WebKit / Moz,
  "transform-box": TransformBox(TransformBox),
  "backface-visibility": BackfaceVisibility(BackfaceVisibility, VendorPrefix) / WebKit / Moz,
  "perspective": Perspective(Perspective, VendorPrefix) / WebKit / Moz,
  "perspective-origin": PerspectiveOrigin(Position, VendorPrefix) / WebKit / Moz,
  "translate": Translate(Translate),
  "rotate": Rotate(Rotate),
  "scale": Scale(Scale),

  // https://www.w3.org/TR/2021/CRD-css-text-3-20210422
  "text-transform": TextTransform(TextTransform),
  "white-space": WhiteSpace(WhiteSpace),
  "tab-size": TabSize(LengthOrNumber, VendorPrefix) / Moz / O,
  "word-break": WordBreak(WordBreak),
  "line-break": LineBreak(LineBreak),
  "hyphens": Hyphens(Hyphens, VendorPrefix) / WebKit / Moz / Ms,
  "overflow-wrap": OverflowWrap(OverflowWrap),
  "word-wrap": WordWrap(OverflowWrap),
  "text-align": TextAlign(TextAlign),
  "text-align-last": TextAlignLast(TextAlignLast, VendorPrefix) / Moz,
  "text-justify": TextJustify(TextJustify),
  "word-spacing": WordSpacing(Spacing),
  "letter-spacing": LetterSpacing(Spacing),
  "text-indent": TextIndent(TextIndent),

  // https://www.w3.org/TR/2020/WD-css-text-decor-4-20200506
  "text-decoration-line": TextDecorationLine(TextDecorationLine, VendorPrefix) / WebKit / Moz,
  "text-decoration-style": TextDecorationStyle(TextDecorationStyle, VendorPrefix) / WebKit / Moz,
  "text-decoration-color": TextDecorationColor(CssColor, VendorPrefix) / WebKit / Moz,
  "text-decoration-thickness": TextDecorationThickness(TextDecorationThickness),
  "text-decoration": TextDecoration(TextDecoration, VendorPrefix) / WebKit / Moz,
  "text-decoration-skip-ink": TextDecorationSkipInk(TextDecorationSkipInk, VendorPrefix) / WebKit,
  "text-emphasis-style": TextEmphasisStyle(TextEmphasisStyle<'i>, VendorPrefix) / WebKit,
  "text-emphasis-color": TextEmphasisColor(CssColor, VendorPrefix) / WebKit,
  "text-emphasis": TextEmphasis(TextEmphasis<'i>, VendorPrefix) / WebKit,
  "text-emphasis-position": TextEmphasisPosition(TextEmphasisPosition, VendorPrefix) / WebKit,
  "text-shadow": TextShadow(SmallVec<[TextShadow; 1]>),

  // https://www.w3.org/TR/2021/WD-css-ui-4-20210316
  "resize": Resize(Resize),
  "cursor": Cursor(Cursor<'i>),
  "caret-color": CaretColor(ColorOrAuto),
  "caret-shape": CaretShape(CaretShape),
  "caret": Caret(Caret),
  "user-select": UserSelect(UserSelect, VendorPrefix) / WebKit / Moz / Ms,
  "accent-color": AccentColor(ColorOrAuto),
  "appearance": Appearance(Appearance<'i>, VendorPrefix) / WebKit / Moz / Ms,

  // https://www.w3.org/TR/2020/WD-css-lists-3-20201117
  "list-style-type": ListStyleType(ListStyleType<'i>),
  "list-style-image": ListStyleImage(Image<'i>),
  "list-style-position": ListStylePosition(ListStylePosition),
  "list-style": ListStyle(ListStyle<'i>),
  "marker-side": MarkerSide(MarkerSide),

  // CSS modules
  "composes": Composes(Composes<'i>) if css_modules,

  // https://www.w3.org/TR/SVG2/painting.html
  "fill": Fill(SVGPaint<'i>),
  "fill-rule": FillRule(FillRule),
  "fill-opacity": FillOpacity(AlphaValue),
  "stroke": Stroke(SVGPaint<'i>),
  "stroke-opacity": StrokeOpacity(AlphaValue),
  "stroke-width": StrokeWidth(LengthPercentage),
  "stroke-linecap": StrokeLinecap(StrokeLinecap),
  "stroke-linejoin": StrokeLinejoin(StrokeLinejoin),
  "stroke-miterlimit": StrokeMiterlimit(f32),
  "stroke-dasharray": StrokeDasharray(StrokeDasharray),
  "stroke-dashoffset": StrokeDashoffset(LengthPercentage),
  "marker-start": MarkerStart(Marker<'i>),
  "marker-mid": MarkerMid(Marker<'i>),
  "marker-end": MarkerEnd(Marker<'i>),
  "marker": Marker(Marker<'i>),
  "color-interpolation": ColorInterpolation(ColorInterpolation),
  "color-interpolation-filters": ColorInterpolationFilters(ColorInterpolation),
  "color-rendering": ColorRendering(ColorRendering),
  "shape-rendering": ShapeRendering(ShapeRendering),
  "text-rendering": TextRendering(TextRendering),
  "image-rendering": ImageRendering(ImageRendering),

  // https://www.w3.org/TR/css-masking-1/
  "clip-path": ClipPath(ClipPath<'i>),
  "clip-rule": ClipRule(FillRule),
  "mask-image": MaskImage(SmallVec<[Image<'i>; 1]>),
  "mask-mode": MaskMode(SmallVec<[MaskMode; 1]>),
  "mask-repeat": MaskRepeat(SmallVec<[BackgroundRepeat; 1]>),
  "mask-position-x": MaskPositionX(SmallVec<[HorizontalPosition; 1]>),
  "mask-position-y": MaskPositionY(SmallVec<[VerticalPosition; 1]>),
  "mask-position": MaskPosition(SmallVec<[Position; 1]>),
  "mask-clip": MaskClip(SmallVec<[MaskClip; 1]>),
  "mask-origin": MaskOrigin(SmallVec<[GeometryBox; 1]>),
  "mask-size": MaskSize(SmallVec<[BackgroundSize; 1]>),
  "mask-composite": MaskComposite(SmallVec<[MaskComposite; 1]>),
  "mask-type": MaskType(MaskType),
  "mask": Mask(SmallVec<[Mask<'i>; 1]>),
  "mask-border-source": MaskBorderSource(Image<'i>),
  "mask-border-mode": MaskBorderMode(MaskBorderMode),
  "mask-border-slice": MaskBorderSlice(BorderImageSlice),
  "mask-border-width": MaskBorderWidth(Rect<BorderImageSideWidth>),
  "mask-border-outset": MaskBorderOutset(Rect<LengthOrNumber>),
  "mask-border-repeat": MaskBorderRepeat(BorderImageRepeat),
  "mask-border": MaskBorder(MaskBorder<'i>),

  // https://drafts.fxtf.org/filter-effects-1/
  "filter": Filter(FilterList<'i>),
  "backdrop-filter": BackdropFilter(FilterList<'i>),
}

impl<'i, T: smallvec::Array<Item = V>, V: Parse<'i>> Parse<'i> for SmallVec<T> {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    // Copied from cssparser `parse_comma_separated` but using SmallVec instead of Vec.
    let mut values = smallvec![];
    loop {
      input.skip_whitespace(); // Unnecessary for correctness, but may help try() in parse_one rewind less.
      match input.parse_until_before(Delimiter::Comma, &mut V::parse) {
        Ok(v) => values.push(v),
        Err(err) => return Err(err)
      }
      match input.next() {
        Err(_) => return Ok(values),
        Ok(&Token::Comma) => continue,
        Ok(_) => unreachable!(),
      }
    }
  }
}

impl<T: smallvec::Array<Item = V>, V: ToCss> ToCss for SmallVec<T> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    let len = self.len();
    for (idx, val) in self.iter().enumerate() {
      val.to_css(dest)?;
      if idx < len - 1 {
        dest.delim(',', false)?;
      }
    }
    Ok(())
  }
}

impl<'i, T: Parse<'i>> Parse<'i> for Vec<T> {
  fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    input.parse_comma_separated(|input| T::parse(input))
  }
}

impl <T: ToCss> ToCss for Vec<T> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    let len = self.len();
    for (idx, val) in self.iter().enumerate() {
      val.to_css(dest)?;
      if idx < len - 1 {
        dest.delim(',', false)?;
      }
    }
    Ok(())
  }
}
