use cssparser::*;
use crate::properties::Property;
use crate::traits::{PropertyHandler, ToCss};
use crate::printer::Printer;
use crate::properties::{
  align::AlignHandler,
  background::BackgroundHandler,
  flex::FlexHandler,
  font::FontHandler,
  margin_padding::*,
  outline::OutlineHandler,
  border::BorderHandler,
  transition::TransitionHandler,
  animation::AnimationHandler,
  prefix_handler::PrefixHandler,
  display::DisplayHandler,
  transform::TransformHandler,
  text::TextDecorationHandler,
  position::PositionHandler,
  overflow::OverflowHandler,
  list::ListStyleHandler,
  grid::GridHandler,
  size::SizeHandler,
};
use crate::targets::Browsers;
use crate::parser::ParserOptions;
use crate::error::{ParserError, PrinterError};
use crate::logical::LogicalProperties;

#[derive(Debug, PartialEq, Clone)]
pub struct DeclarationBlock<'i> {
  pub important_declarations: Vec<Property<'i>>,
  pub declarations: Vec<Property<'i>>
}

impl<'i> DeclarationBlock<'i> {
  pub fn parse<'t, T>(input: &mut Parser<'i, 't>, options: &ParserOptions<T>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
    let mut important_declarations = DeclarationList::new();
    let mut declarations = DeclarationList::new();
    let mut parser = DeclarationListParser::new(input, PropertyDeclarationParser {
      important_declarations: &mut important_declarations,
      declarations: &mut declarations,
      options
    });
    while let Some(res) = parser.next() {
      if let Err((err, _)) = res {
        return Err(err)
      }
    }

    Ok(DeclarationBlock {
      important_declarations,
      declarations
    })
  }
}

impl<'i> ToCss for DeclarationBlock<'i> {
  fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError> where W: std::fmt::Write {
    dest.whitespace()?;
    dest.write_char('{')?;
    dest.indent();

    let mut i = 0;
    let len = self.declarations.len() + self.important_declarations.len();

    macro_rules! write {
      ($decls: expr, $important: literal) => {
        for decl in &$decls {
          dest.newline()?;
          decl.to_css(dest, $important)?;
          if i != len - 1 || !dest.minify {
            dest.write_char(';')?;
          }
          i += 1;
        }
      };
    }

    write!(self.declarations, false);
    write!(self.important_declarations, true);
    
    dest.dedent();
    dest.newline()?;
    dest.write_char('}')
  }
}

impl<'i> DeclarationBlock<'i> {
  pub(crate) fn minify(
    &mut self,
    handler: &mut DeclarationHandler<'i>,
    important_handler: &mut DeclarationHandler<'i>,
    logical_properties: &mut LogicalProperties
  ) {
    macro_rules! handle {
      ($decls: expr, $handler: expr) => {
        for decl in $decls.iter() {
          let handled = $handler.handle_property(decl, logical_properties);
    
          if !handled {
            $handler.decls.push(decl.clone());
          }
        }
      };
    }

    handle!(self.important_declarations, important_handler);
    handle!(self.declarations, handler);

    handler.finalize(logical_properties);
    important_handler.finalize(logical_properties);
    self.important_declarations = std::mem::take(&mut important_handler.decls);
    self.declarations = std::mem::take(&mut handler.decls);
  }
}

struct PropertyDeclarationParser<'a, 'i, T> {
  important_declarations: &'a mut Vec<Property<'i>>,
  declarations: &'a mut Vec<Property<'i>>,
  options: &'a ParserOptions<T>
}

/// Parse a declaration within {} block: `color: blue`
impl<'a, 'i, T> cssparser::DeclarationParser<'i> for PropertyDeclarationParser<'a, 'i, T> {
  type Declaration = ();
  type Error = ParserError<'i>;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut cssparser::Parser<'i, 't>,
  ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
    parse_declaration(name, input, &mut self.declarations, &mut self.important_declarations, &self.options)
  }
}

/// Default methods reject all at rules.
impl<'a, 'i, T> AtRuleParser<'i> for PropertyDeclarationParser<'a, 'i, T> {
  type Prelude = ();
  type AtRule = ();
  type Error = ParserError<'i>;
}

pub(crate) fn parse_declaration<'i, 't, T>(
  name: CowRcStr<'i>,
  input: &mut cssparser::Parser<'i, 't>,
  declarations: &mut DeclarationList<'i>,
  important_declarations: &mut DeclarationList<'i>,
  options: &ParserOptions<T>
) -> Result<(), cssparser::ParseError<'i, ParserError<'i>>> {
  let property = input.parse_until_before(Delimiter::Bang, |input| Property::parse(name, input, options))?;
  let important = input.try_parse(|input| {
    input.expect_delim('!')?;
    input.expect_ident_matching("important")
  }).is_ok();
  if important {
    important_declarations.push(property);
  } else {
    declarations.push(property);
  }
  Ok(())
}

pub(crate) type DeclarationList<'i> = Vec<Property<'i>>;

pub(crate) struct DeclarationHandler<'i> {
  background: BackgroundHandler<'i>,
  border: BorderHandler<'i>,
  outline: OutlineHandler,
  flex: FlexHandler,
  grid: GridHandler<'i>,
  align: AlignHandler,
  size: SizeHandler,
  margin: MarginHandler,
  padding: PaddingHandler,
  scroll_margin: ScrollMarginHandler,
  scroll_padding: ScrollPaddingHandler,
  font: FontHandler<'i>,
  text: TextDecorationHandler<'i>,
  list: ListStyleHandler<'i>,
  transition: TransitionHandler<'i>,
  animation: AnimationHandler<'i>,
  display: DisplayHandler<'i>,
  position: PositionHandler,
  inset: InsetHandler,
  overflow: OverflowHandler,
  transform: TransformHandler,
  prefix: PrefixHandler,
  decls: DeclarationList<'i>
}

impl<'i> DeclarationHandler<'i> {
  pub fn new(targets: Option<Browsers>) -> Self {
    DeclarationHandler {
      background: BackgroundHandler::new(targets),
      border: BorderHandler::new(targets),
      outline: OutlineHandler::default(),
      flex: FlexHandler::new(targets),
      grid: GridHandler::default(),
      align: AlignHandler::new(targets),
      size: SizeHandler::default(),
      margin: MarginHandler::default(),
      padding: PaddingHandler::default(),
      scroll_margin: ScrollMarginHandler::default(),
      scroll_padding: ScrollPaddingHandler::default(),
      font: FontHandler::default(),
      text: TextDecorationHandler::new(targets),
      list: ListStyleHandler::default(),
      transition: TransitionHandler::new(targets),
      animation: AnimationHandler::new(targets),
      display: DisplayHandler::new(targets),
      position: PositionHandler::new(targets),
      inset: InsetHandler::default(),
      overflow: OverflowHandler::new(targets),
      transform: TransformHandler::new(targets),
      prefix: PrefixHandler::new(targets),
      decls: DeclarationList::new()
    }
  }

  pub fn handle_property(&mut self, property: &Property<'i>, logical_properties: &mut LogicalProperties) -> bool {
    self.background.handle_property(property, &mut self.decls, logical_properties) ||
    self.border.handle_property(property, &mut self.decls, logical_properties) ||
    self.outline.handle_property(property, &mut self.decls, logical_properties) ||
    self.flex.handle_property(property, &mut self.decls, logical_properties) ||
    self.grid.handle_property(property, &mut self.decls, logical_properties) ||
    self.align.handle_property(property, &mut self.decls, logical_properties) ||
    self.size.handle_property(property, &mut self.decls, logical_properties) ||
    self.margin.handle_property(property, &mut self.decls, logical_properties) ||
    self.padding.handle_property(property, &mut self.decls, logical_properties) ||
    self.scroll_margin.handle_property(property, &mut self.decls, logical_properties) ||
    self.scroll_padding.handle_property(property, &mut self.decls, logical_properties) ||
    self.font.handle_property(property, &mut self.decls, logical_properties) ||
    self.text.handle_property(property, &mut self.decls, logical_properties) ||
    self.list.handle_property(property, &mut self.decls, logical_properties) ||
    self.transition.handle_property(property, &mut self.decls, logical_properties) ||
    self.animation.handle_property(property, &mut self.decls, logical_properties) ||
    self.display.handle_property(property, &mut self.decls, logical_properties) ||
    self.position.handle_property(property, &mut self.decls, logical_properties) ||
    self.inset.handle_property(property, &mut self.decls, logical_properties) ||
    self.overflow.handle_property(property, &mut self.decls, logical_properties) ||
    self.transform.handle_property(property, &mut self.decls, logical_properties) ||
    self.prefix.handle_property(property, &mut self.decls, logical_properties)
  }

  pub fn finalize(&mut self, logical_properties: &mut LogicalProperties) {
    self.background.finalize(&mut self.decls, logical_properties);
    self.border.finalize(&mut self.decls, logical_properties);
    self.outline.finalize(&mut self.decls, logical_properties);
    self.flex.finalize(&mut self.decls, logical_properties);
    self.grid.finalize(&mut self.decls, logical_properties);
    self.align.finalize(&mut self.decls, logical_properties);
    self.size.finalize(&mut self.decls, logical_properties);
    self.margin.finalize(&mut self.decls, logical_properties);
    self.padding.finalize(&mut self.decls, logical_properties);
    self.scroll_margin.finalize(&mut self.decls, logical_properties);
    self.scroll_padding.finalize(&mut self.decls, logical_properties);
    self.font.finalize(&mut self.decls, logical_properties);
    self.text.finalize(&mut self.decls, logical_properties);
    self.list.finalize(&mut self.decls, logical_properties);
    self.transition.finalize(&mut self.decls, logical_properties);
    self.animation.finalize(&mut self.decls, logical_properties);
    self.display.finalize(&mut self.decls, logical_properties);
    self.position.finalize(&mut self.decls, logical_properties);
    self.inset.finalize(&mut self.decls, logical_properties);
    self.overflow.finalize(&mut self.decls, logical_properties);
    self.transform.finalize(&mut self.decls, logical_properties);
    self.prefix.finalize(&mut self.decls, logical_properties);
  }
}
