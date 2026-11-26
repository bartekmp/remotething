//! A line of text that fills the room it is given and ends in "…" when the text is longer.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as advanced_text, Paragraph};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Widget, mouse, widget};
use iced::widget::text::{LineHeight, Shaping, Wrapping};
use iced::{Element, Length, Pixels, Rectangle, Size};

const DOTS: char = '…';

pub struct Ellipsized {
    content: String,
    size: Option<Pixels>,
}

/// A line of text cut short with "…" when it does not fit the width it is given.
pub fn ellipsized(content: impl Into<String>) -> Ellipsized {
    Ellipsized {
        content: content.into(),
        size: None,
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

            if !fits(&self.content) {
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
