//! The panel of "Send keys": the menu of combinations, and the creator of others.

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Length};

use crate::i18n::fl;
use crate::send_keys::{Draft, MODIFIERS, Message, State, name};
use crate::theme;

const WIDTH: f32 = 260.0;

/// The label of the status bar button: it points up, where the panel opens.
pub(crate) fn button_label() -> String {
    format!("{} ▴", fl!("send-keys-button"))
}

/// The panel, or `None` while it is closed.
pub(crate) fn view(state: &State) -> Option<Element<'_, Message>> {
    if !state.open {
        return None;
    }
    let content = match &state.draft {
        Some(draft) => creator(draft),
        None => menu(state),
    };
    Some(
        container(content)
            .width(Length::Fixed(WIDTH))
            .padding(theme::PANEL_PADDING)
            .style(theme::floating)
            .into(),
    )
}

fn menu(state: &State) -> Element<'_, Message> {
    let items = state
        .presets()
        .into_iter()
        .enumerate()
        .map(|(index, combo)| {
            button(text(name(&combo)).size(theme::TEXT_SMALL))
                .width(Length::Fill)
                .padding([4, 8])
                .style(theme::entry(false))
                .on_press(Message::PresetPressed(index))
                .into()
        });
    column(items)
        .push(theme::horizontal_divider())
        .push(
            button(text(fl!("send-keys-custom")).size(theme::TEXT_SMALL))
                .width(Length::Fill)
                .padding([4, 8])
                .style(theme::entry(false))
                .on_press(Message::CustomPressed),
        )
        .spacing(2)
        .into()
}

fn creator(draft: &Draft) -> Element<'_, Message> {
    let chips = row(MODIFIERS.into_iter().map(|modifier| {
        button(text(modifier.label()).size(theme::TEXT_SMALL))
            .padding([4, 10])
            .style(theme::entry(draft.has(modifier)))
            .on_press(Message::ModifierToggled(modifier))
            .into()
    }))
    .spacing(4);
    let key = text_input(&fl!("send-keys-key"), &draft.text)
        .on_input(Message::KeyEdited)
        .size(theme::TEXT_SMALL)
        .width(Length::Fill);
    let capture_label = if draft.capturing {
        fl!("send-keys-capturing")
    } else {
        fl!("send-keys-capture")
    };
    let capture = button(text(capture_label).size(theme::TEXT_SMALL))
        .padding([4, 10])
        .style(theme::entry(draft.capturing))
        .on_press(Message::CaptureToggled);
    let combo = draft.combo();
    let unknown = !draft.text.trim().is_empty() && draft.key().is_none();
    let preview = match &combo {
        Some(combo) => text(name(combo)),
        None => text(fl!("send-keys-nothing")).style(theme::secondary_text),
    };
    let actions = row![
        button(text(fl!("send-keys-back")).size(theme::TEXT_SMALL))
            .style(theme::pill)
            .on_press(Message::BackPressed),
        button(text(fl!("send-keys-send")).size(theme::TEXT_SMALL))
            .style(theme::primary)
            .on_press_maybe(combo.map(|_| Message::SendPressed)),
    ]
    .spacing(8);
    let mut panel = column![chips, key].spacing(theme::GAP);
    if unknown {
        panel = panel.push(
            text(fl!("send-keys-unknown"))
                .size(theme::TEXT_SMALL)
                .style(text::danger),
        );
    }
    panel
        .push(capture)
        .push(preview.size(theme::TEXT_BODY))
        .push(actions)
        .into()
}
