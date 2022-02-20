use crate::rules::layer::{LayerBlockRule, LayerStatementRule};
use crate::values::string::CowArcStr;
use cssparser::*;
use parcel_selectors::{SelectorList, parser::NestingRequirement};
use crate::media_query::*;
use crate::rules::viewport::ViewportRule;
use crate::traits::Parse;
use crate::selector::{Selectors, SelectorParser};
use crate::rules::{
  Location,
  CssRule,
  CssRuleList,
  keyframes::{KeyframeListParser, KeyframesRule},
  font_face::{FontFaceRule, FontFaceDeclarationParser},
  page::{PageSelector, PageRule},
  supports::{SupportsCondition, SupportsRule},
  counter_style::CounterStyleRule,
  namespace::NamespaceRule,
  import::ImportRule,
  media::MediaRule,
  style::StyleRule,
  document::MozDocumentRule,
  nesting::NestingRule,
  custom_media::CustomMediaRule,
  layer::{LayerName}
};
use crate::values::ident::CustomIdent;
use crate::declaration::{DeclarationBlock, DeclarationList, parse_declaration};
use crate::vendor_prefix::VendorPrefix;
use std::collections::HashMap;
use crate::error::ParserError;

#[derive(Clone, Debug)]
pub struct ParserOptions<T> {
  pub nesting: bool,
  pub custom_media: bool,
  pub css_modules: bool,
  pub source_index: u32,
  pub at_rule_parser: Option<T>
}

impl ParserOptions<DefaultAtRuleParser> {
  pub fn default() -> Self {
    ParserOptions {
      nesting: false,
      custom_media: false,
      css_modules: false,
      source_index: 0,
      at_rule_parser: None
    }
  }
}

#[derive(Clone)]
pub struct DefaultAtRuleParser;
impl<'i> AtRuleParser<'i> for DefaultAtRuleParser {
  type AtRule = DefaultAtRule;
  type Error = ();
  type Prelude = ();
}

pub struct DefaultAtRule;
impl cssparser::ToCss for DefaultAtRule {
  fn to_css<W>(&self, _: &mut W) -> std::fmt::Result where W: std::fmt::Write {
    Err(std::fmt::Error)
  }
}

#[derive(PartialEq, PartialOrd)]
enum State {
  Start = 1,
  Layers = 2,
  Imports = 3,
  Namespaces = 4,
  Body = 5
}

/// The parser for the top-level rules in a stylesheet.
pub struct TopLevelRuleParser<'a, 'i, T> {
  default_namespace: Option<CowArcStr<'i>>,
  namespace_prefixes: HashMap<CowArcStr<'i>, CowArcStr<'i>>,
  options: &'a mut ParserOptions<T>,
  state: State
}

impl<'a, 'b, 'i, T: AtRuleParser<'i>> TopLevelRuleParser<'a, 'i, T> {
  pub fn new(options: &'a mut ParserOptions<T>) -> TopLevelRuleParser<'a, 'i, T> {
    TopLevelRuleParser {
      default_namespace: None,
      namespace_prefixes: HashMap::new(),
      options,
      state: State::Start
    }
  }

  fn nested<'x: 'b>(&'x mut self) -> NestedRuleParser<'_, 'i, T> {
      NestedRuleParser {
        default_namespace: &mut self.default_namespace,
        namespace_prefixes: &mut self.namespace_prefixes,
        options: &mut self.options
      }
  }
}

/// A rule prelude for at-rule with block.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AtRulePrelude<'i, T> {
  /// A @font-face rule prelude.
  FontFace,
  /// A @font-feature-values rule prelude, with its FamilyName list.
  FontFeatureValues,//(Vec<FamilyName>),
  /// A @counter-style rule prelude, with its counter style name.
  CounterStyle(CustomIdent<'i>),
  /// A @media rule prelude, with its media queries.
  Media(MediaList<'i>),
  /// A @custom-media rule prelude.
  CustomMedia(CowRcStr<'i>, MediaList<'i>),
  /// An @supports rule, with its conditional
  Supports(SupportsCondition<'i>),
  /// A @viewport rule prelude.
  Viewport(VendorPrefix),
  /// A @keyframes rule, with its animation name and vendor prefix if exists.
  Keyframes(CustomIdent<'i>, VendorPrefix),
  /// A @page rule prelude.
  Page(Vec<PageSelector<'i>>),
  /// A @-moz-document rule.
  MozDocument,
  /// A @import rule prelude.
  Import(CowRcStr<'i>, MediaList<'i>, Option<SupportsCondition<'i>>, Option<Option<LayerName<'i>>>),
  /// A @namespace rule prelude.
  Namespace(Option<CowRcStr<'i>>, CowRcStr<'i>),
  /// A @charset rule prelude.
  Charset,
  /// A @nest prelude.
  Nest(SelectorList<'i, Selectors>),
  Layer(Vec<LayerName<'i>>),
  Custom(T)
}

impl<'a, 'i, T: AtRuleParser<'i>> AtRuleParser<'i> for TopLevelRuleParser<'a, 'i, T> {
  type Prelude = AtRulePrelude<'i, T::Prelude>;
  type AtRule = (SourcePosition, CssRule<'i, T::AtRule>);
  type Error = ParserError<'i>;

  fn parse_prelude<'t>(
      &mut self,
      name: CowRcStr<'i>,
      input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
      match_ignore_ascii_case! { &*name,
        "import" => {
          if self.state > State::Imports {
            return Err(input.new_custom_error(ParserError::UnexpectedImportRule))
          }

          let url_string = input.expect_url_or_string()?.clone();

          let layer = if input.try_parse(|input| input.expect_ident_matching("layer")).is_ok() {
            Some(None)
          } else if input.try_parse(|input| input.expect_function_matching("layer")).is_ok() {
            let name = input.parse_nested_block(LayerName::parse).map(|name| Some(name))?;
            Some(name)
          } else {
            None
          };

          let supports = if input.try_parse(|input| input.expect_function_matching("supports")).is_ok() {
            Some(input.parse_nested_block(|input| {
              input.try_parse(SupportsCondition::parse).or_else(|_| SupportsCondition::parse_declaration(input))
            })?)
          } else {
            None
          };
          let media = MediaList::parse(input)?;
          return Ok(AtRulePrelude::Import(url_string, media, supports, layer));
        },
        "namespace" => {
          if self.state > State::Namespaces {
            return Err(input.new_custom_error(ParserError::UnexpectedNamespaceRule))
          }

          let prefix = input.try_parse(|input| input.expect_ident_cloned()).ok();
          let namespace = input.expect_url_or_string()?;
          let prelude = AtRulePrelude::Namespace(prefix, namespace);
          return Ok(prelude);
        },
        "charset" => {
          // @charset is removed by rust-cssparser if it’s the first rule in the stylesheet.
          // Anything left is technically invalid, however, users often concatenate CSS files
          // together, so we are more lenient and simply ignore @charset rules in the middle of a file.
          input.expect_string()?;
          return Ok(AtRulePrelude::Charset)
        },
        "custom-media" if self.options.custom_media => {
          let name = input.expect_ident_cloned()?;
          if !name.starts_with("--") {
            return Err(input.new_unexpected_token_error(Token::Ident(name.into())));
          }
          let media = MediaList::parse(input)?;
          return Ok(AtRulePrelude::CustomMedia(name, media))
        },
        _ => {}
      }

      AtRuleParser::parse_prelude(&mut self.nested(), name, input)
  }

  #[inline]
  fn parse_block<'t>(
      &mut self,
      prelude: Self::Prelude,
      start: &ParserState,
      input: &mut Parser<'i, 't>,
  ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
    self.state = State::Body;
    let rule = AtRuleParser::parse_block(&mut self.nested(), prelude, start, input)?;
    Ok((start.position(), rule))
  }

  #[inline]
  fn rule_without_block(
      &mut self,
      prelude: AtRulePrelude<'i, T::Prelude>,
      start: &ParserState,
  ) -> Result<Self::AtRule, ()> {
      let loc = start.source_location();
      let loc = Location {
        source_index: self.options.source_index,
        line: loc.line,
        column: loc.column
      };

      let rule = match prelude {
        AtRulePrelude::Import(url, media, supports, layer) => {
          self.state = State::Imports;
          CssRule::Import(ImportRule {
            url: url.into(),
            layer,
            supports,
            media,
            loc
          })
        },
        AtRulePrelude::Namespace(prefix, url) => {
          self.state = State::Namespaces;

          if let Some(prefix) = &prefix {
            self.namespace_prefixes.insert(prefix.into(), url.clone().into());
          } else {
            self.default_namespace = Some(url.clone().into());
          }

          CssRule::Namespace(NamespaceRule {
            prefix: prefix.map(|x| x.into()),
            url: url.into(),
            loc
          })
        },
        AtRulePrelude::CustomMedia(name, query) => {
          self.state = State::Body;
          CssRule::CustomMedia(CustomMediaRule {
            name: name.into(),
            query,
            loc
          })
        },
        AtRulePrelude::Layer(_) => {
          // @layer statements are allowed before @import rules, but cannot be interleaved.
          if self.state <= State::Layers {
            self.state = State::Layers;
          } else {
            self.state = State::Body;
          }
          AtRuleParser::rule_without_block(&mut self.nested(), prelude, start)?
        },
        AtRulePrelude::Charset => CssRule::Ignored,
        AtRulePrelude::Custom(_) => {
          self.state = State::Body;
          AtRuleParser::rule_without_block(&mut self.nested(), prelude, start)?
        },
        _ => return Err(())
      };

      Ok((start.position(), rule))
  }
}

impl<'a, 'i, T: AtRuleParser<'i>> QualifiedRuleParser<'i> for TopLevelRuleParser<'a, 'i, T> {
  type Prelude = SelectorList<'i, Selectors>;
  type QualifiedRule = (SourcePosition, CssRule<'i, T::AtRule>);
  type Error = ParserError<'i>;

  #[inline]
  fn parse_prelude<'t>(
      &mut self,
      input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    self.state = State::Body;
    QualifiedRuleParser::parse_prelude(&mut self.nested(), input)
  }

  #[inline]
  fn parse_block<'t>(
      &mut self,
      prelude: Self::Prelude,
      start: &ParserState,
      input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    let rule = QualifiedRuleParser::parse_block(&mut self.nested(), prelude, start, input)?;
    Ok((start.position(), rule))
  }
}

struct NestedRuleParser<'a, 'i, T> {
  default_namespace: &'a Option<CowArcStr<'i>>,
  namespace_prefixes: &'a HashMap<CowArcStr<'i>, CowArcStr<'i>>,
  options: &'a mut ParserOptions<T>
}

impl<'a, 'b, 'i, T: AtRuleParser<'i>> NestedRuleParser<'a, 'i, T> {
  fn parse_nested_rules<'t>(&mut self, input: &mut Parser<'i, 't>) -> CssRuleList<'i, T::AtRule> {
    let nested_parser = NestedRuleParser {
      default_namespace: self.default_namespace,
      namespace_prefixes: self.namespace_prefixes,
      options: self.options
    };

    let mut iter = RuleListParser::new_for_nested_rule(input, nested_parser);
    let mut rules = Vec::new();
    while let Some(result) = iter.next() {
      match result {
        Ok(CssRule::Ignored) => {},
        Ok(rule) => rules.push(rule),
        Err(_) => {
          // TODO
        },
      }
    }

    CssRuleList(rules)
  }

  fn loc(&self, start: &ParserState) -> Location {
    let loc = start.source_location();
    Location {
      source_index: self.options.source_index,
      line: loc.line,
      column: loc.column
    }
  }
}

impl<'a, 'b, 'i, T: AtRuleParser<'i>> AtRuleParser<'i> for NestedRuleParser<'a, 'i, T> {
  type Prelude = AtRulePrelude<'i, T::Prelude>;
  type AtRule = CssRule<'i, T::AtRule>;
  type Error = ParserError<'i>;

  fn parse_prelude<'t>(
      &mut self,
      name: CowRcStr<'i>,
      input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    match_ignore_ascii_case! { &*name,
      "media" => {
        let media = MediaList::parse(input)?;
        Ok(AtRulePrelude::Media(media))
      },
      "supports" => {
        let cond = SupportsCondition::parse(input)?;
        Ok(AtRulePrelude::Supports(cond))
      },
      "font-face" => {
        Ok(AtRulePrelude::FontFace)
      },
      // "font-feature-values" => {
      //     if !cfg!(feature = "gecko") {
      //         // Support for this rule is not fully implemented in Servo yet.
      //         return Err(input.new_custom_error(StyleParseErrorKind::UnsupportedAtRule(name.clone())))
      //     }
      //     let family_names = parse_family_name_list(self.context, input)?;
      //     Ok(AtRuleType::WithBlock(AtRuleBlockPrelude::FontFeatureValues(family_names)))
      // },
      "counter-style" => {
        let name = CustomIdent::parse(input)?;
        Ok(AtRulePrelude::CounterStyle(name))
      },
      "viewport" | "-ms-viewport" => {
        let prefix = if starts_with_ignore_ascii_case(&*name, "-ms") {
          VendorPrefix::Ms
        } else {
          VendorPrefix::None
        };
        Ok(AtRulePrelude::Viewport(prefix))
      },
      "keyframes" | "-webkit-keyframes" | "-moz-keyframes" | "-o-keyframes" | "-ms-keyframes" => {
        let prefix = if starts_with_ignore_ascii_case(&*name, "-webkit-") {
          VendorPrefix::WebKit
        } else if starts_with_ignore_ascii_case(&*name, "-moz-") {
          VendorPrefix::Moz
        } else if starts_with_ignore_ascii_case(&*name, "-o-") {
          VendorPrefix::O
        } else if starts_with_ignore_ascii_case(&*name, "-ms-") {
          VendorPrefix::Ms
        } else {
          VendorPrefix::None
        };

        let location = input.current_source_location();
        let name = match *input.next()? {
          Token::Ident(ref s) => s.into(),
          Token::QuotedString(ref s) => s.into(),
          ref t => return Err(location.new_unexpected_token_error(t.clone())),
        };

        Ok(AtRulePrelude::Keyframes(CustomIdent(name), prefix))
      },
      "page" => {
        let selectors = input.try_parse(|input| input.parse_comma_separated(PageSelector::parse)).unwrap_or_default();
        Ok(AtRulePrelude::Page(selectors))
      },
      "-moz-document" => {
        // Firefox only supports the url-prefix() function with no arguments as a legacy CSS hack.
        // See https://css-tricks.com/snippets/css/css-hacks-targeting-firefox/
        input.expect_function_matching("url-prefix")?;
        input.parse_nested_block(|input| input.expect_exhausted().map_err(|e| e.into()))?;

        Ok(AtRulePrelude::MozDocument)
      },
      "layer" => {
        let names = match Vec::<LayerName>::parse(input) {
          Ok(names) => names,
          Err(ParseError { kind: ParseErrorKind::Basic(BasicParseErrorKind::EndOfInput), .. }) => Vec::new(),
          Err(e) => return Err(e)
        };
        Ok(AtRulePrelude::Layer(names))
      },
      _ => {
        if let Some(at_rule_parser) = &mut self.options.at_rule_parser {
          at_rule_parser.parse_prelude(name.clone(), input)
            .map(|prelude| AtRulePrelude::Custom(prelude))
            .map_err(|_| input.new_error(BasicParseErrorKind::AtRuleInvalid(name)))
        } else {
          Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)))
        }
      }
    }
  }

  fn parse_block<'t>(
    &mut self,
    prelude: Self::Prelude,
    start: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<CssRule<'i, T::AtRule>, ParseError<'i, Self::Error>> {
    let loc = self.loc(start);
    match prelude {
      AtRulePrelude::FontFace => {
        let mut parser = DeclarationListParser::new(input, FontFaceDeclarationParser);
        let mut properties = vec![];
        while let Some(decl) = parser.next() {
          if let Ok(decl) = decl {
            properties.push(decl);
          }
        }
        Ok(CssRule::FontFace(FontFaceRule {
          properties,
          loc
        }))
      },
      // AtRuleBlockPrelude::FontFeatureValues(family_names) => {
      //     let context = ParserContext::new_with_rule_type(
      //         self.context,
      //         CssRuleType::FontFeatureValues,
      //         self.namespaces,
      //     );

      //     Ok(CssRule::FontFeatureValues(Arc::new(self.shared_lock.wrap(
      //         FontFeatureValuesRule::parse(
      //             &context,
      //             input,
      //             family_names,
      //             start.source_location(),
      //         ),
      //     ))))
      // },
      AtRulePrelude::CounterStyle(name) => {
        Ok(CssRule::CounterStyle(CounterStyleRule {
          name,
          declarations: DeclarationBlock::parse(input, self.options)?,
          loc
        }))
      },
      AtRulePrelude::Media(query) => {
        Ok(CssRule::Media(MediaRule {
          query,
          rules: self.parse_nested_rules(input),
          loc
        }))
      },
      AtRulePrelude::Supports(condition) => {
        Ok(CssRule::Supports(SupportsRule {
          condition,
          rules: self.parse_nested_rules(input),
          loc
        }))
      },
      AtRulePrelude::Viewport(vendor_prefix) => {
        Ok(CssRule::Viewport(ViewportRule {
          vendor_prefix,
          // TODO: parse viewport descriptors rather than properties
          // https://drafts.csswg.org/css-device-adapt/#viewport-desc
          declarations: DeclarationBlock::parse(input, self.options)?,
          loc
        }))
      },
      AtRulePrelude::Keyframes(name, vendor_prefix) => {
        let iter = RuleListParser::new_for_nested_rule(input, KeyframeListParser);
        Ok(CssRule::Keyframes(KeyframesRule {
          name,
          keyframes: iter.filter_map(Result::ok).collect(),
          vendor_prefix,
          loc
        }))
      },
      AtRulePrelude::Page(selectors) => {
        Ok(CssRule::Page(PageRule {
          selectors,
          declarations: DeclarationBlock::parse(input, self.options)?,
          loc
        }))
      },
      AtRulePrelude::MozDocument => {
        Ok(CssRule::MozDocument(MozDocumentRule {
          rules: self.parse_nested_rules(input),
          loc
        }))
      },
      AtRulePrelude::Layer(names) => {
        let name = if names.is_empty() {
          None
        } else if names.len() == 1 {
          names.into_iter().next()
        } else {
          return Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid))
        };

        Ok(CssRule::LayerBlock(LayerBlockRule {
          name,
          rules: self.parse_nested_rules(input),
          loc
        }))
      },
      AtRulePrelude::Import(..) | 
      AtRulePrelude::Namespace(..) | 
      AtRulePrelude::CustomMedia(..) | 
      AtRulePrelude::Charset => {
        // These rules don't have blocks.
        Err(input.new_unexpected_token_error(Token::CurlyBracketBlock))
      },
      AtRulePrelude::FontFeatureValues | AtRulePrelude::Nest(..) => unreachable!(),
      AtRulePrelude::Custom(prelude) => {
        if let Some(at_rule_parser) = &mut self.options.at_rule_parser {
          at_rule_parser.parse_block(prelude, start, input)
            .map(|prelude| CssRule::Custom(prelude))
            .map_err(|_| input.new_error(BasicParseErrorKind::AtRuleBodyInvalid))
        } else {
          Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid))
        }
      }
    }
  }

  #[inline]
  fn rule_without_block(
    &mut self,
    prelude: AtRulePrelude<'i, T::Prelude>,
    start: &ParserState,
  ) -> Result<Self::AtRule, ()> {
    let loc = self.loc(start);
    match prelude {
      AtRulePrelude::Layer(names) => {
        if names.is_empty() {
          return Err(())
        }

        Ok(CssRule::LayerStatement(LayerStatementRule {
          names,
          loc
        }))
      }
      AtRulePrelude::Custom(prelude) => {
        if let Some(at_rule_parser) = &mut self.options.at_rule_parser {
          at_rule_parser.rule_without_block(prelude, start)
            .map(|prelude| CssRule::Custom(prelude))
        } else {
          Err(())
        }
      }
      _ => Err(())
    }
  }
}

impl<'a, 'b, 'i, T: AtRuleParser<'i>> QualifiedRuleParser<'i> for NestedRuleParser<'a, 'i, T> {
  type Prelude = SelectorList<'i, Selectors>;
  type QualifiedRule = CssRule<'i, T::AtRule>;
  type Error = ParserError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    let selector_parser = SelectorParser {
      default_namespace: self.default_namespace,
      namespace_prefixes: self.namespace_prefixes,
      is_nesting_allowed: false,
      css_modules: self.options.css_modules
    };
    SelectorList::parse(&selector_parser, input, NestingRequirement::None)
  }

  fn parse_block<'t>(
    &mut self,
    selectors: Self::Prelude,
    start: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<CssRule<'i, T::AtRule>, ParseError<'i, Self::Error>> {
    let loc = self.loc(start);
    let (declarations, rules) = if self.options.nesting {
      parse_declarations_and_nested_rules(input, self.default_namespace, self.namespace_prefixes, self.options)?
    } else {
      (DeclarationBlock::parse(input, self.options)?, CssRuleList(vec![]))
    };
    Ok(CssRule::Style(StyleRule {
      selectors,
      vendor_prefix: VendorPrefix::empty(),
      declarations,
      rules,
      loc
    }))
  }
}

fn parse_declarations_and_nested_rules<'a, 'i, 't, T: AtRuleParser<'i>>(
  input: &mut Parser<'i, 't>,
  default_namespace: &'a Option<CowArcStr<'i>>,
  namespace_prefixes: &'a HashMap<CowArcStr<'i>, CowArcStr<'i>>,
  options: &'a mut ParserOptions<T>
) -> Result<(DeclarationBlock<'i>, CssRuleList<'i, T::AtRule>), ParseError<'i, ParserError<'i>>> {
  let mut important_declarations = DeclarationList::new();
  let mut declarations = DeclarationList::new();
  let mut rules = CssRuleList(vec![]);
  let parser = StyleRuleParser {
    default_namespace,
    namespace_prefixes,
    options,
    declarations: &mut declarations,
    important_declarations: &mut important_declarations,
    rules: &mut rules
  };

  let mut declaration_parser = DeclarationListParser::new(input, parser);
  let mut last = declaration_parser.input.state();
  while let Some(decl) = declaration_parser.next() {
    match decl {
      Ok(_) => {}
      _ => {
        declaration_parser.input.reset(&last);
        break
      }
    }

    last = declaration_parser.input.state();
  }

  let mut iter = RuleListParser::new_for_nested_rule(declaration_parser.input, declaration_parser.parser);
  while let Some(result) = iter.next() {
    if let Err((err, _)) = result {
      return Err(err)
    }
  }

  Ok((DeclarationBlock { declarations, important_declarations }, rules))
}

pub struct StyleRuleParser<'a, 'i, T, R> {
  default_namespace: &'a Option<CowArcStr<'i>>,
  namespace_prefixes: &'a HashMap<CowArcStr<'i>, CowArcStr<'i>>,
  options: &'a mut ParserOptions<T>,
  declarations: &'a mut DeclarationList<'i>,
  important_declarations: &'a mut DeclarationList<'i>,
  rules: &'a mut CssRuleList<'i, R>
}

/// Parse a declaration within {} block: `color: blue`
impl<'a, 'i, T: AtRuleParser<'i>> cssparser::DeclarationParser<'i> for StyleRuleParser<'a, 'i, T, T::AtRule> {
  type Declaration = ();
  type Error = ParserError<'i>;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut cssparser::Parser<'i, 't>,
  ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
    if !self.rules.0.is_empty() {
      // Declarations cannot come after nested rules.
      return Err(input.new_custom_error(ParserError::InvalidNesting))
    }
    parse_declaration(name, input, &mut self.declarations, &mut self.important_declarations, &self.options)
  }
}

impl<'a, 'i, T: AtRuleParser<'i>> AtRuleParser<'i> for StyleRuleParser<'a, 'i, T, T::AtRule> {
  type Prelude = AtRulePrelude<'i, T::Prelude>;
  type AtRule = ();
  type Error = ParserError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    match_ignore_ascii_case! { &*name,
      "media" => {
        let media = MediaList::parse(input)?;
        Ok(AtRulePrelude::Media(media))
      },
      "supports" => {
        let cond = SupportsCondition::parse(input)?;
        Ok(AtRulePrelude::Supports(cond))
      },
      "nest" => {
        let selector_parser = SelectorParser {
          default_namespace: self.default_namespace,
          namespace_prefixes: self.namespace_prefixes,
          is_nesting_allowed: true,
          css_modules: self.options.css_modules
        };
        let selectors = SelectorList::parse(&selector_parser, input, NestingRequirement::Contained)?;
        Ok(AtRulePrelude::Nest(selectors))
      },
      _ => {
        if let Some(at_rule_parser) = &mut self.options.at_rule_parser {
          at_rule_parser.parse_prelude(name.clone(), input)
            .map(|prelude| AtRulePrelude::Custom(prelude))
            .map_err(|_| input.new_error(BasicParseErrorKind::AtRuleInvalid(name)))
        } else {
          Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)))
        }
      }
    }
  }

  fn parse_block<'t>(
    &mut self,
    prelude: AtRulePrelude<'i, T::Prelude>,
    start: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<(), ParseError<'i, Self::Error>> {
    let loc = start.source_location();
    let loc = Location {
      source_index: self.options.source_index,
      line: loc.line,
      column: loc.column
    };
    match prelude {
      AtRulePrelude::Media(query) => {
        self.rules.0.push(CssRule::Media(MediaRule {
          query,
          rules: parse_nested_at_rule(input, self.options.source_index, self.default_namespace, self.namespace_prefixes, self.options)?,
          loc
        }));
        Ok(())
      },
      AtRulePrelude::Supports(condition) => {
        self.rules.0.push(CssRule::Supports(SupportsRule {
          condition,
          rules: parse_nested_at_rule(input, self.options.source_index, self.default_namespace, self.namespace_prefixes, self.options)?,
          loc
        }));
        Ok(())
      },
      AtRulePrelude::Nest(selectors) => {
        let (declarations, rules) = parse_declarations_and_nested_rules(input, self.default_namespace, self.namespace_prefixes, self.options)?;
        self.rules.0.push(CssRule::Nesting(NestingRule {
          style: StyleRule {
            selectors,
            declarations,
            vendor_prefix: VendorPrefix::empty(),
            rules,
            loc
          },
          loc
        }));
        Ok(())
      },
      AtRulePrelude::Custom(prelude) => {
        if let Some(at_rule_parser) = &mut self.options.at_rule_parser {
          let rule = at_rule_parser.parse_block(prelude, start, input)
            .map_err(|_| input.new_error(BasicParseErrorKind::AtRuleBodyInvalid))?;
          self.rules.0.push(CssRule::Custom(rule));
          Ok(())
        } else {
          Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid))
        }
      },
      _ => Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid))
    }
  }

  #[inline]
  fn rule_without_block(
    &mut self,
    prelude: AtRulePrelude<'i, T::Prelude>,
    start: &ParserState,
  ) -> Result<Self::AtRule, ()> {
    match prelude {
      AtRulePrelude::Custom(prelude) => {
        if let Some(at_rule_parser) = &mut self.options.at_rule_parser {
          let rule = at_rule_parser.rule_without_block(prelude, start)?;
          self.rules.0.push(CssRule::Custom(rule));
          Ok(())
        } else {
          Err(())
        }
      }
      _ => Err(())
    }
  }
}

#[inline]
fn parse_nested_at_rule<'a, 'i, 't, T: AtRuleParser<'i>>(
  input: &mut Parser<'i, 't>,
  source_index: u32,
  default_namespace: &'a Option<CowArcStr<'i>>,
  namespace_prefixes: &'a HashMap<CowArcStr<'i>, CowArcStr<'i>>,
  options: &'a mut ParserOptions<T>
) -> Result<CssRuleList<'i, T::AtRule>, ParseError<'i, ParserError<'i>>> {
  let loc = input.current_source_location();
  let loc = Location { source_index, line: loc.line, column: loc.column };

  // Declarations can be immediately within @media and @supports blocks that are nested within a parent style rule.
  // These act the same way as if they were nested within a `& { ... }` block.
  let (declarations, mut rules) = parse_declarations_and_nested_rules(input, default_namespace, namespace_prefixes, options)?;

  if declarations.declarations.len() > 0 {
    rules.0.insert(0, CssRule::Style(StyleRule {
      selectors: SelectorList(smallvec::smallvec![parcel_selectors::parser::Selector::from_vec2(vec![parcel_selectors::parser::Component::Nesting])]),
      declarations,
      vendor_prefix: VendorPrefix::empty(),
      rules: CssRuleList(vec![]),
      loc
    }))
  }

  Ok(rules)
}

impl<'a, 'b, 'i, T: AtRuleParser<'i>> QualifiedRuleParser<'i> for StyleRuleParser<'a, 'i, T, T::AtRule> {
  type Prelude = SelectorList<'i, Selectors>;
  type QualifiedRule = ();
  type Error = ParserError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    let selector_parser = SelectorParser {
      default_namespace: self.default_namespace,
      namespace_prefixes: self.namespace_prefixes,
      is_nesting_allowed: true,
      css_modules: self.options.css_modules
    };
    SelectorList::parse(&selector_parser, input, NestingRequirement::Prefixed)
  }

  fn parse_block<'t>(
    &mut self,
    selectors: Self::Prelude,
    start: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<(), ParseError<'i, Self::Error>> {
    let loc = start.source_location();
    let (declarations, rules) = parse_declarations_and_nested_rules(input, self.default_namespace, self.namespace_prefixes, self.options)?;
    self.rules.0.push(CssRule::Style(StyleRule {
      selectors,
      vendor_prefix: VendorPrefix::empty(),
      declarations,
      rules,
      loc: Location {
        source_index: self.options.source_index,
        line: loc.line,
        column: loc.column
      }
    }));
    Ok(())
  }
}

#[inline]
pub fn starts_with_ignore_ascii_case(string: &str, prefix: &str) -> bool {
  string.len() >= prefix.len() && string.as_bytes()[0..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}
