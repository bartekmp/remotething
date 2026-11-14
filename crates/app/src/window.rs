//! How the window is drawn: three floating panels, and over them the
//! context menu and the row being dragged.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, opaque, pin, row, space, stack, text};
use iced::{Element, Length, Point};

use crate::app::{Centre, Message, State};
use crate::credentials;
use crate::i18n::fl;
use crate::icons::{self, Icon};
use crate::menu;
use crate::organise;
use crate::properties;
use crate::putty_import;
use crate::settings;
use crate::tabs_view;
use crate::theme as look;
use crate::tree;
use crate::vault;

const TREE_WIDTH: f32 = 280.0;
const PROPERTIES_WIDTH: f32 = 320.0;
const COLLAPSE_ICON_SIZE: f32 = 18.0;
/// How far the dragged row's label trails the pointer, so it is never under it.
const CARRIED_OFFSET: f32 = 14.0;

pub fn view(state: &State) -> Element<'_, Message> {
    let tree = if state.preferences.tree_collapsed {
        // Folded: the button that brings the tree back, and settings at the bottom.
        let settings = button(icons::view(icons::SETTINGS, COLLAPSE_ICON_SIZE))
            .padding(8)
            .style(look::pill)
            .on_press(Message::SettingsPressed);
        let expand = icon_button(icons::PANEL_LEFT_OPEN, Message::TreeCollapseToggled);
        container(column![expand, space::vertical(), settings].align_x(Horizontal::Center))
            .height(Length::Fill)
            .padding(look::GAP)
            .style(look::panel)
            .into()
    } else {
        let header = row![
            text(fl!("tree-title"))
                .size(look::TEXT_TITLE)
                .width(Length::Fill),
            icon_button(icons::PANEL_LEFT_CLOSE, Message::TreeCollapseToggled),
        ]
        .align_y(Vertical::Center);
        let content = tree::view(&state.tree, &state.store).map(Message::Tree);
        let settings_label = row![
            icons::view(icons::SETTINGS, look::TEXT_BODY),
            text(fl!("tree-settings")).size(look::TEXT_SMALL),
        ]
        .spacing(6)
        .align_y(Vertical::Center);
        let settings = button(settings_label)
            .style(look::pill)
            .on_press(Message::SettingsPressed);
        side_panel(
            column![header, content, settings].spacing(look::GAP),
            TREE_WIDTH,
        )
    };
    let centre = match (&state.vault, state.centre) {
        // The prompt covers the sessions; credentials and settings stay reachable.
        (Some(prompt), Centre::Sessions) => vault::view(prompt).map(Message::Vault),
        (_, Centre::Sessions) => {
            let layout = state.preferences.tab_layout;
            tabs_view::view(&state.tabs, &state.store, &state.recent, layout).map(Message::Tabs)
        }
        (_, Centre::Credentials) => {
            credentials::view(&state.credentials, &state.store).map(Message::Credentials)
        }
        (_, Centre::Settings) => settings::view(&state.preferences).map(Message::Settings),
        (_, Centre::Putty) => putty_import::view(&state.putty).map(Message::Putty),
    };
    let tabs = container(centre)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(look::PANEL_PADDING)
        .style(look::panel);
    // Two floating panels with the window background showing between them.
    let body = row![tree, tabs].spacing(look::GAP);

    // An error wins over good news; both sit above the panels.
    let message = match (&state.error, &state.notice) {
        (Some(reason), _) => Some(text(reason.as_str()).style(text::danger)),
        (None, Some(notice)) => Some(text(notice.as_str())),
        (None, None) => None,
    };
    let content = match message {
        Some(message) => {
            let banner = container(message)
                .width(Length::Fill)
                .padding(look::GAP)
                .style(look::panel);
            column![banner, body].spacing(look::GAP)
        }
        None => column![body],
    };
    let mut layers = stack![container(content).padding(look::GAP)];
    if state.properties_open {
        layers = layers.push(properties_panel(state));
    }
    if let Some(label) = carried_label(state) {
        layers = layers.push(label);
    }
    if let Some(menu) = &state.menu {
        layers = layers.push(menu::view(menu, &state.store, state.window).map(Message::Menu));
    }
    layers.into()
}

/// The properties of the selected item, floating over the right of the centre panel.
fn properties_panel(state: &State) -> Element<'_, Message> {
    let header = row![
        text(fl!("properties-title"))
            .size(look::TEXT_TITLE)
            .width(Length::Fill),
        icon_button(icons::CLOSE, Message::PropertiesClosed),
    ]
    .align_y(Vertical::Center);
    let content = properties::view(state.draft.as_ref(), &state.store, state.tree.selected())
        .map(Message::Properties);
    let panel = container(column![header, content].spacing(look::GAP))
        .width(Length::Fixed(PROPERTIES_WIDTH))
        .height(Length::Fill)
        .padding(look::PANEL_PADDING)
        .style(look::floating);
    // Clicks on the panel must not reach the session underneath.
    container(opaque(panel))
        .align_right(Length::Fill)
        .height(Length::Fill)
        .padding(look::GAP * 2.0)
        .into()
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

fn icon_button<'a>(icon: Icon, on_press: Message) -> Element<'a, Message> {
    button(icons::view(icon, COLLAPSE_ICON_SIZE))
        .padding(6)
        .style(look::entry(false))
        .on_press(on_press)
        .into()
}
