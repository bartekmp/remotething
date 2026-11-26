//! How the window is drawn: three floating panels, and over them the
//! context menu and the row being dragged.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{
    button, column, container, mouse_area, opaque, pin, row, space, stack, text, tooltip,
};
use iced::{Element, Length, Point};

use crate::app::{Centre, Message, State};
use crate::credentials;
use crate::i18n::fl;
use crate::icons::{self, Icon};
use crate::import_preview;
use crate::menu;
use crate::organise;
use crate::properties;
use crate::putty_import;
use crate::removal;
use crate::settings;
use crate::tabs::Page;
use crate::tabs_view;
use crate::theme as look;
use crate::tree;
use crate::vault;
use crate::wsl_import;

/// The room the message bar takes above the panels: one line of text and its padding.
const BANNER_HEIGHT: f32 = 44.0;
const PROPERTIES_WIDTH: f32 = 360.0;
const COLLAPSE_ICON_SIZE: f32 = 18.0;
/// How far the dragged row's label trails the pointer, so it is never under it.
const CARRIED_OFFSET: f32 = 14.0;

pub fn view(state: &State) -> Element<'_, Message> {
    let tree = tree_panel(state);
    let centre = match (&state.vault, state.centre()) {
        // The prompt covers the sessions; credentials and settings stay reachable.
        (Some(prompt), Centre::Sessions) => vault::view(prompt).map(Message::Vault),
        // The credentials and the settings are tabs, drawn in the place of a session.
        _ => {
            let layout = state.preferences.tab_layout;
            tabs_view::view(
                &state.tabs,
                &state.store,
                &state.recent,
                layout,
                state.now,
                state,
            )
            .map(Message::Tabs)
        }
    };
    let tabs = container(centre)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(look::PANEL_PADDING)
        .style(look::panel);
    // Two floating panels with the window background showing between them.
    let body = row![tree, tree_edge(state), tabs];

    // An error wins over good news; both sit above the panels.
    let message = match (&state.error, &state.notice) {
        (Some(reason), _) => Some(text(reason.as_str()).style(text::danger)),
        (None, Some(notice)) => Some(text(notice.as_str())),
        (None, None) => None,
    };
    let has_message = message.is_some();
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
    if state.properties_open || state.properties_slide.is_running(state.now) {
        layers = layers.push(properties_panel(state, has_message));
    }
    if let Some(label) = carried_label(state) {
        layers = layers.push(label);
    }
    if let Some(ghost) = carried_tab(state) {
        layers = layers.push(ghost);
    }
    if let Some(menu) = &state.menu {
        layers = layers.push(menu::view(menu, &state.store, state.window).map(Message::Menu));
    }
    if let Some(palette) = &state.palette {
        let entries = crate::palette::entries(&state.store, state.recent.ids(), palette.query());
        layers = layers.push(crate::palette::view(palette, entries).map(Message::Palette));
    }
    if state.shortcuts_open {
        layers = layers.push(crate::shortcuts::view().map(Message::Shortcuts));
    }
    if let Some(question) = &state.removal {
        layers = layers.push(removal::view(question).map(Message::Removal));
    }
    layers.into()
}

/// A page that lives in a tab, as messages of the tabs: the window handles them.
pub(crate) fn page_view(state: &State, page: Page) -> Element<'_, crate::tabs::Message> {
    let screen: Element<'_, Message> = match page {
        Page::Credentials => {
            credentials::view(&state.credentials, &state.store).map(Message::Credentials)
        }
        Page::Settings => match state.centre() {
            Centre::Putty => putty_import::view(&state.putty).map(Message::Putty),
            Centre::Wsl => wsl_import::view(&state.wsl).map(Message::Wsl),
            Centre::Import => match &state.import {
                Some(screen) => import_preview::view(screen).map(Message::Preview),
                None => settings_view(state),
            },
            _ => settings_view(state),
        },
    };
    screen.map(|message| crate::tabs::Message::Page(Box::new(message)))
}

/// The width of the folded tree: a rail with one button above and one below.
const RAIL_WIDTH: f32 = 56.0;
/// Narrower than this, a sliding tree shows the rail instead of a squeezed panel.
const RAIL_UNTIL: f32 = 150.0;

/// The connection tree: a panel, a rail, or while it moves a panel of in-between width.
fn tree_panel(state: &State) -> Element<'_, Message> {
    let unfolded = state.tree_slide.value(state.now);
    if !state.tree_slide.is_running(state.now) {
        return if state.preferences.tree_collapsed {
            rail(None, state.centre())
        } else {
            expanded_tree(state)
        };
    }
    let width = RAIL_WIDTH + (tree_width(state) - RAIL_WIDTH) * unfolded;
    if width < RAIL_UNTIL {
        return rail(Some(width), state.centre());
    }
    // The panel is cut off by the edge of its box instead of being squeezed into it.
    container(expanded_tree(state))
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .clip(true)
        .into()
}

/// Narrower than this, the buttons above the tree show only their icons.
const LABELS_FROM: f32 = 300.0;

fn tree_width(state: &State) -> f32 {
    let range = crate::preferences::TREE_WIDTH_RANGE;
    f32::from(
        state
            .preferences
            .tree_width
            .clamp(*range.start(), *range.end()),
    )
}

/// The gap between the tree and the centre. While the tree is open, dragging it sets the
/// tree's width.
fn tree_edge(state: &State) -> Element<'_, Message> {
    let gap = container(space::horizontal())
        .width(Length::Fixed(look::GAP))
        .height(Length::Fill);
    if state.preferences.tree_collapsed {
        return gap.into();
    }
    mouse_area(gap)
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(Message::TreeResizeStarted)
        .into()
}

/// What the sidebar buttons do, in the order shown: new folder, new connection, credentials.
fn sidebar_actions() -> [(Icon, String, Message); 3] {
    [
        (
            icons::FOLDER_PLUS,
            fl!("tree-new-folder"),
            Message::Tree(tree::Message::NewFolderPressed),
        ),
        (
            icons::CIRCLE_PLUS,
            fl!("tree-new-connection"),
            Message::Tree(tree::Message::NewConnectionPressed),
        ),
        (
            icons::KEY,
            fl!("tree-credentials"),
            Message::Tree(tree::Message::CredentialsPressed),
        ),
    ]
}

/// Whether the credentials screen is the one showing, for the button that opens it.
fn is_credentials(message: &Message, centre: Centre) -> bool {
    matches!(message, Message::Tree(tree::Message::CredentialsPressed))
        && centre == Centre::Credentials
}

/// The settings screen and the import previews it leads to.
fn settings_showing(centre: Centre) -> bool {
    matches!(
        centre,
        Centre::Settings | Centre::Putty | Centre::Wsl | Centre::Import
    )
}

/// A button of the sidebar: the icon and its name.
/// `active` marks the screen that is showing.
fn labelled_button<'a>(
    icon: Icon,
    label: String,
    message: Message,
    active: bool,
) -> Element<'a, Message> {
    let content = row![
        icons::view(icon, look::TEXT_BODY),
        text(label).size(look::TEXT_SMALL)
    ]
    .spacing(6)
    .align_y(Vertical::Center);
    button(content)
        .style(pill_style(active))
        .on_press(message)
        .into()
}

fn pill_style(active: bool) -> fn(&iced::Theme, button::Status) -> button::Style {
    if active {
        look::pill_active
    } else {
        look::pill
    }
}

/// A button of the folded sidebar: the icon, with the name when the pointer rests on it.
fn rail_button<'a>(
    icon: Icon,
    label: String,
    message: Message,
    active: bool,
) -> Element<'a, Message> {
    let icon_button = button(icons::view(icon, COLLAPSE_ICON_SIZE))
        .padding(8)
        .style(pill_style(active))
        .on_press(message);
    tooltip(
        icon_button,
        text(label).size(look::TEXT_SMALL),
        tooltip::Position::Right,
    )
    .gap(4)
    .padding(6)
    .style(look::floating)
    .into()
}

/// The folded tree: the button that brings it back, the same actions as icons, and settings
/// at the bottom. Pressing an action also unfolds the tree.
fn rail<'a>(width: Option<f32>, centre: Centre) -> Element<'a, Message> {
    let expand = icon_button(icons::PANEL_LEFT_OPEN, Message::TreeCollapseToggled);
    let mut actions = column![].spacing(look::GAP).align_x(Horizontal::Center);
    for (icon, label, message) in sidebar_actions() {
        let active = is_credentials(&message, centre);
        actions = actions.push(rail_button(icon, label, message, active));
    }
    let settings = rail_button(
        icons::SETTINGS,
        fl!("tree-settings"),
        Message::SettingsPressed,
        settings_showing(centre),
    );
    let rail = container(
        column![expand, actions, space::vertical(), settings]
            .spacing(look::GAP * 2.0)
            .align_x(Horizontal::Center),
    )
    .height(Length::Fill)
    .padding(look::GAP)
    .style(look::panel);
    match width {
        Some(width) => rail.width(Length::Fixed(width)).into(),
        None => rail.into(),
    }
}

fn expanded_tree(state: &State) -> Element<'_, Message> {
    let header = row![
        text(fl!("tree-title"))
            .size(look::TEXT_TITLE)
            .width(Length::Fill),
        icon_button(icons::PANEL_LEFT_CLOSE, Message::TreeCollapseToggled),
    ]
    .align_y(Vertical::Center);
    let [folder, connection, credentials] = sidebar_actions();
    let credentials_showing = is_credentials(&credentials.2, state.centre());
    let toolbar: Element<'_, Message> = if tree_width(state) < LABELS_FROM {
        // No room for the names: icons only, with the name when the pointer rests on one.
        row![
            rail_button(folder.0, folder.1, folder.2, false),
            rail_button(connection.0, connection.1, connection.2, false),
            rail_button(
                credentials.0,
                credentials.1,
                credentials.2,
                credentials_showing,
            ),
        ]
        .spacing(6)
        .into()
    } else {
        column![
            row![
                labelled_button(folder.0, folder.1, folder.2, false),
                labelled_button(connection.0, connection.1, connection.2, false),
            ]
            .spacing(6),
            labelled_button(
                credentials.0,
                credentials.1,
                credentials.2,
                credentials_showing,
            ),
        ]
        .spacing(6)
        .into()
    };
    let content = tree::view(&state.tree, &state.store).map(Message::Tree);
    let settings = if tree_width(state) < LABELS_FROM {
        rail_button(
            icons::SETTINGS,
            fl!("tree-settings"),
            Message::SettingsPressed,
            settings_showing(state.centre()),
        )
    } else {
        labelled_button(
            icons::SETTINGS,
            fl!("tree-settings"),
            Message::SettingsPressed,
            settings_showing(state.centre()),
        )
    };
    side_panel(
        column![header, toolbar, content, settings].spacing(look::GAP),
        tree_width(state),
    )
}

/// The properties of the selected item, floating over the right of the centre panel.
fn properties_panel(state: &State, below_banner: bool) -> Element<'_, Message> {
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
    // The message bar sits at the top; the panel starts below it.
    let top = if below_banner {
        look::GAP * 2.0 + BANNER_HEIGHT
    } else {
        look::GAP * 2.0
    };
    let slid_in = state.properties_slide.value(state.now);
    if !state.properties_slide.is_running(state.now) {
        return container(opaque(panel))
            .align_right(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top,
                ..iced::Padding::new(look::GAP * 2.0)
            })
            .into();
    }
    // Sliding: the panel is placed by hand, from past the right edge to its place.
    let edge = look::GAP * 2.0;
    let resting = state.window.width - PROPERTIES_WIDTH - edge;
    let x = state.window.width + (resting - state.window.width) * slid_in;
    let height = (state.window.height - top - edge).max(0.0);
    pin(opaque(panel.height(Length::Fixed(height))))
        .position(Point::new(x, top))
        .into()
}

/// The silhouette of the tab being dragged, following the pointer.
fn carried_tab(state: &State) -> Option<Element<'_, Message>> {
    let title = state.tabs.carried_title()?;
    let ghost = container(text(title).wrapping(text::Wrapping::None))
        .width(Length::Fixed(crate::tab_bar::TAB_WIDTH))
        .height(Length::Fixed(crate::tab_bar::TAB_HEIGHT))
        .padding([0, 10])
        .align_y(Vertical::Center)
        .clip(true)
        .style(look::ghost_tab);
    let position = Point::new(
        state.pointer.x + CARRIED_OFFSET,
        state.pointer.y + CARRIED_OFFSET,
    );
    Some(pin(ghost).position(position).into())
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

fn settings_view(state: &State) -> Element<'_, Message> {
    let dark = crate::app::theme(state).extended_palette().is_dark;
    settings::view(
        &state.preferences,
        &state.terminal_themes,
        dark,
        state.settings_page,
    )
    .map(Message::Settings)
}
