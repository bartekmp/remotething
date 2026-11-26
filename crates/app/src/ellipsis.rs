//! A line of text that fills the room it is given and ends in "…" when the text is longer.

use std::cell::Cell;
use std::rc::Rc;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as advanced_text, Paragraph};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Widget, mouse, widget};
use iced::widget::text::{LineHeight, Shaping, Wrapping};
use iced::widget::{container, text, tooltip};
use iced::{Element, Length, Pixels, Rectangle, Size, Theme};

use crate::theme as look;

const DOTS: char = '…';

pub struct Ellipsized {
    content: String,
    size: Option<Pixels>,
    /// Set at layout: whether the text had to be cut short.
    cut: Rc<Cell<bool>>,
}

/// A line of text cut short with "…" when it does not fit the width it is given.
pub fn ellipsized(content: impl Into<String>) -> Ellipsized {
    Ellipsized {
        content: content.into(),
        size: None,
        cut: Rc::new(Cell::new(false)),
    }
}

impl Ellipsized {
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }
}

/// The text as laid out last time, which is what gets drawn.
struct State<P: Paragraph>(widget::text::State<P>);

impl<P: Paragraph> Default for State<P> {
    fn default() -> Self {
        State(widget::text::State::default())
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Ellipsized
where
    Renderer: advanced_text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Renderer::Paragraph>::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let paragraph = &mut state.0;
        let size = self.size.unwrap_or_else(|| renderer.default_size());
        let font = renderer.default_font();
        let room = limits.max();

        layout::sized(limits, Length::Fill, Length::Shrink, |limits| {
            let bounds = limits.max();
            let mut fits = |text: &str| {
                let _ = paragraph.update(advanced_text::Text {
                    content: text,
                    bounds,
                    size,
                    line_height: LineHeight::default(),
                    font,
                    align_x: advanced_text::Alignment::Default,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: Shaping::Basic,
                    wrapping: Wrapping::None,
                });
                paragraph.min_bounds().width <= room.width
            };

            let cut = !fits(&self.content);
            self.cut.set(cut);
            if cut {
                // The most characters, with the dots after them, that still fit.
                let ends: Vec<usize> = self
                    .content
                    .char_indices()
                    .map(|(index, _)| index)
                    .collect();
                let (mut low, mut high) = (0, ends.len());
                while low < high {
                    let middle = (low + high).div_ceil(2);
                    let candidate = format!("{}{DOTS}", &self.content[..ends[middle]]);
                    if fits(&candidate) {
                        low = middle;
                    } else {
                        high = middle - 1;
                    }
                }
                let _ = fits(&format!("{}{DOTS}", &self.content[..ends[low]]));
            }
            Size::new(room.width, paragraph.min_bounds().height)
        })
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        widget::text::draw(
            renderer,
            defaults,
            layout.bounds(),
            state.0.raw(),
            widget::text::Style::default(),
            viewport,
        );
    }
}

impl<'a, Message: 'a, Theme: 'a, Renderer> From<Ellipsized>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: advanced_text::Renderer + 'a,
{
    fn from(text: Ellipsized) -> Self {
        Element::new(text)
    }
}

/// A name that ends in "…" when it does not fit, with the whole name in a hint under the
/// pointer, shown only while the name is cut.
pub fn with_hint<'a, Message: 'a>(
    content: String,
    size: impl Into<Pixels>,
) -> Element<'a, Message> {
    let line = ellipsized(content.clone()).size(size);
    let cut = Rc::clone(&line.cut);
    let hint = Hint {
        cut,
        content: container(text(content).size(look::TEXT_SMALL))
            .padding([4, 8])
            .style(look::chip)
            .into(),
    };
    tooltip(line, Element::new(hint), tooltip::Position::Bottom)
        .gap(4)
        .style(|_| container::Style::default())
        .into()
}

/// What a tooltip shows: the content while `cut` is set, nothing otherwise.
struct Hint<'a, Message> {
    cut: Rc<Cell<bool>>,
    content: Element<'a, Message>,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Hint<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content]);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        if !self.cut.get() {
            return layout::Node::new(Size::ZERO);
        }
        let inner = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        layout::Node::with_children(inner.size(), vec![inner])
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if !self.cut.get() {
            return;
        }
        if let Some(inner) = layout.children().next() {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                inner,
                cursor,
                viewport,
            );
        }
    }
}
