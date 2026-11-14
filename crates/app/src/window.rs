//! How the window is drawn: three floating panels, and over them the
//! context menu and the row being dragged.

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pin, row, stack, text};
use iced::{Element, Length, Point};

use crate::app::{Centre, Message, State};
use crate::credentials;
use crate::i18n::fl;
use crate::icons::{self, Icon};
use crate::menu;
use crate::organise;
use crate::properties;
use crate::settings;
use crate::tabs;
use crate::theme as look;
use crate::tree;
use crate::vault;

const TREE_WIDTH: f32 = 280.0;
const PROPERTIES_WIDTH: f32 = 300.0;
const COLLAPSE_ICON_SIZE: f32 = 18.0;
/// How far the dragged row's label trails the pointer, so it is never under it.
const CARRIED_OFFSET: f32 = 14.0;

pub fn view(state: &State) -> Element<'_, Message> {
    let tree = if state.preferences.tree_collapsed {
        rail(icons::PANEL_LEFT_OPEN, Message::TreeCollapseToggled)
    } else {
        let header = row![
            text(fl!("tree-title"))
                .size(look::TEXT_TITLE)
                .width(Length::Fill),
            icon_button(icons::PANEL_LEFT_CLOSE, Message::TreeCollapseToggled),
        ]
        .align_y(Vertical::Center);
        let content = tree::view(&state.tree, &state.store).map(Message::Tree);
        side_panel(column![header, content].spacing(look::GAP), TREE_WIDTH)
    };
    let centre = match (&state.vault, state.centre) {
        // The prompt covers the sessions; credentials and settings stay reachable.
        (Some(prompt), Centre::Sessions) => vault::view(prompt).map(Message::Vault),
        (_, Centre::Sessions) => tabs::view(&state.tabs).map(Message::Tabs),
        (_, Centre::Credentials) => {
            credentials::view(&state.credentials, &state.store).map(Message::Credentials)
        }
        (_, Centre::Settings) => settings::view(&state.preferences).map(Message::Settings),
    };
    let tabs = container(centre)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(look::PANEL_PADDING)
        .style(look::panel);
    let properties = if state.preferences.properties_collapsed {
        rail(icons::PANEL_RIGHT_OPEN, Message::PropertiesCollapseToggled)
    } else {
        let header = row![
            icon_button(icons::PANEL_RIGHT_CLOSE, Message::PropertiesCollapseToggled),
            text(fl!("properties-title")).size(look::TEXT_TITLE),
        ]
        .spacing(8)
        .align_y(Vertical::Center);
        let content = properties::view(state.draft.as_ref(), &state.store, state.tree.selected())
            .map(Message::Properties);
        side_panel(
            column![header, content].spacing(look::GAP),
            PROPERTIES_WIDTH,
        )
    };
    // Three floating panels with the window background showing between them.
    let body = row![tree, tabs, properties].spacing(look::GAP);

    let content = match &state.error {
        Some(reason) => {
            let banner = container(text(reason.as_str()).style(text::danger))
                .width(Length::Fill)
                .padding(look::GAP)
                .style(look::panel);
            column![banner, body].spacing(look::GAP)
        }
        None => column![body],
    };
    let mut layers = stack![container(content).padding(look::GAP)];
    if let Some(label) = carried_label(state) {
        layers = layers.push(label);
    }
    if let Some(menu) = &state.menu {
        layers = layers.push(menu::view(menu, &state.store, state.window).map(Message::Menu));
    }
    layers.into()
}

/// The name of the row being dragged, following the pointer.
fn carried_label(state: &State) -> Option<Element<'_, Message>> {
    let name = organise::name_of(&state.store, state.tree.carried()?)?;
    let label = container(text(name).size(look::TEXT_SMALL))
        .padding([4, 8])
        .style(look::floating);
    let position = Point::new(
        state.pointer.x + CARRIED_OFFSET,
        state.pointer.y + CARRIED_OFFSET,
    );
    Some(pin(label).position(position).into())
}

/// An expanded sidebar: a floating panel of fixed width.
fn side_panel<'a>(content: impl Into<Element<'a, Message>>, width: f32) -> Element<'a, Message> {
    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding(look::PANEL_PADDING)
        .style(look::panel)
        .into()
}

/// A collapsed sidebar: a narrow panel with only the button that brings it back.
fn rail<'a>(expand: Icon, on_press: Message) -> Element<'a, Message> {
    container(icon_button(expand, on_press))
        .height(Length::Fill)
        .padding(look::GAP)
        .style(look::panel)
        .into()
}

fn icon_button<'a>(icon: Icon, on_press: Message) -> Element<'a, Message> {
    button(icons::view(icon, COLLAPSE_ICON_SIZE))
        .padding(6)
        .style(look::entry(false))
        .on_press(on_press)
        .into()
}
