//! The context menu of the connection tree, drawn over the window.

use iced::widget::{button, column, container, mouse_area, opaque, pin, space, stack, text};
use iced::{Element, Length, Point, Size};
use remotething_core::Store;

use crate::i18n::fl;
use crate::icons;
use crate::theme;
use crate::tree::Selection;

const WIDTH: f32 = 200.0;
const PADDING: f32 = 6.0;
const ITEM_HEIGHT: f32 = 30.0;
/// The icon grid: 23 cells of 30 px, six to a row.
const ICONS_HEIGHT: f32 = 4.0 * 32.0;
/// Kept free between the menu and the edge of the window.
const MARGIN: f32 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// The row that was right-clicked, or `None` for the empty space.
    target: Option<Selection>,
    /// Where the pointer was, in window coordinates.
    position: Point,
    /// Whether the icon grid replaced the list of actions.
    picking_icon: bool,
}

impl State {
    pub fn new(target: Option<Selection>, position: Point) -> Self {
        Self {
            target,
            position,
            picking_icon: false,
        }
    }

    pub fn target(&self) -> Option<&Selection> {
        self.target.as_ref()
    }

    pub fn show_icons(&mut self) {
        self.picking_icon = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Connect,
    NewFolder,
    NewConnection,
    /// Open the properties panel.
    Edit,
    Rename,
    ChangeIcon,
    IconPicked(Option<&'static str>),
    MoveUp,
    MoveDown,
    Remove,
    /// A click outside the menu.
    Dismissed,
}

/// The actions offered for what was right-clicked, in the order shown.
pub fn actions(target: Option<&Selection>) -> &'static [Message] {
    match target {
        None => &[Message::NewFolder, Message::NewConnection],
        Some(Selection::Folder(_)) => &[
            Message::NewFolder,
            Message::NewConnection,
            Message::Edit,
            Message::Rename,
            Message::ChangeIcon,
            Message::MoveUp,
            Message::MoveDown,
            Message::Remove,
        ],
        Some(Selection::Connection(_)) => &[
            Message::Connect,
            Message::Edit,
            Message::Rename,
            Message::ChangeIcon,
            Message::MoveUp,
            Message::MoveDown,
            Message::Remove,
        ],
    }
}

fn label(action: Message) -> String {
    match action {
        Message::Connect => fl!("menu-connect"),
        Message::NewFolder => fl!("tree-new-folder"),
        Message::NewConnection => fl!("tree-new-connection"),
        Message::Edit => fl!("menu-edit"),
        Message::Rename => fl!("menu-rename"),
        Message::ChangeIcon => fl!("menu-change-icon"),
        Message::MoveUp => fl!("menu-move-up"),
        Message::MoveDown => fl!("menu-move-down"),
        Message::Remove => fl!("menu-remove"),
        Message::IconPicked(_) | Message::Dismissed => String::new(),
    }
}

/// A layer the size of the window: the menu, and around it a catcher for clicks outside.
pub fn view<'a>(state: &'a State, store: &'a Store, window: Size) -> Element<'a, Message> {
    let (content, height): (Element<'a, Message>, f32) = if state.picking_icon {
        (icon_page(state, store), ICONS_HEIGHT)
    } else {
        let actions = actions(state.target());
        let items = actions.iter().map(|action| item(*action));
        (column(items).into(), ITEM_HEIGHT * actions.len() as f32)
    };
    let size = Size::new(WIDTH, height + 2.0 * PADDING);
    let panel = container(content)
        .width(Length::Fixed(WIDTH))
        .padding(PADDING)
        .style(theme::floating);

    let outside = mouse_area(space().width(Length::Fill).height(Length::Fill))
        .on_press(Message::Dismissed)
        .on_right_press(Message::Dismissed);
    stack![
        opaque(outside),
        pin(opaque(panel)).position(place(state.position, size, window))
    ]
    .into()
}

fn item<'a>(action: Message) -> Element<'a, Message> {
    let style = if action == Message::Remove {
        text::danger
    } else {
        text::default
    };
    button(text(label(action)).style(style))
        .width(Length::Fill)
        .height(Length::Fixed(ITEM_HEIGHT))
        .padding([6, 10])
        .style(theme::entry(false))
        .on_press(action)
        .into()
}

fn icon_page<'a>(state: &State, store: &'a Store) -> Element<'a, Message> {
    let (chosen, default_icon) = match state.target() {
        Some(Selection::Connection(id)) => {
            let connection = store.connections.iter().find(|c| &c.id == id);
            let default_icon = match connection {
                Some(connection) => icons::for_connection(None, connection.protocol),
                None => icons::for_folder(None),
            };
            (connection.and_then(|c| c.icon.as_deref()), default_icon)
        }
        Some(Selection::Folder(id)) => {
            let folder = store.folders.iter().find(|folder| &folder.id == id);
            (
                folder.and_then(|folder| folder.icon.as_deref()),
                icons::for_folder(None),
            )
        }
        None => (None, icons::for_folder(None)),
    };
    icons::grid(chosen, default_icon, Message::IconPicked)
}

/// Where the menu's top-left corner goes: at the pointer, pulled back inside the window.
pub fn place(pointer: Point, menu: Size, window: Size) -> Point {
    let furthest_x = (window.width - menu.width - MARGIN).max(MARGIN);
    let furthest_y = (window.height - menu.height - MARGIN).max(MARGIN);
    Point::new(pointer.x.min(furthest_x), pointer.y.min(furthest_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Size = Size::new(1000.0, 700.0);
    const MENU: Size = Size::new(200.0, 150.0);

    #[test]
    fn the_empty_space_only_offers_new_items() {
        assert_eq!(actions(None), [Message::NewFolder, Message::NewConnection]);
    }

    #[test]
    fn a_connection_can_be_connected_but_holds_no_new_items() {
        let target = Selection::Connection("web-01".to_string());

        let actions = actions(Some(&target));

        assert_eq!(actions[0], Message::Connect);
        assert!(!actions.contains(&Message::NewFolder));
        assert!(actions.contains(&Message::Remove));
    }

    #[test]
    fn a_folder_offers_new_items_inside_and_its_own_edits() {
        let target = Selection::Folder("prod".to_string());

        let actions = actions(Some(&target));

        assert!(actions.contains(&Message::NewConnection));
        assert!(actions.contains(&Message::Rename));
        assert!(!actions.contains(&Message::Connect));
    }

    #[test]
    fn every_action_has_a_label() {
        let folder = Selection::Folder("prod".to_string());
        let connection = Selection::Connection("web-01".to_string());

        for target in [None, Some(&folder), Some(&connection)] {
            for action in actions(target) {
                assert!(!label(*action).is_empty());
            }
        }
    }

    #[test]
    fn the_menu_opens_at_the_pointer_when_there_is_room() {
        let pointer = Point::new(120.0, 300.0);

        assert_eq!(place(pointer, MENU, WINDOW), pointer);
    }

    #[test]
    fn the_menu_is_pulled_back_inside_the_window() {
        let placed = place(Point::new(990.0, 690.0), MENU, WINDOW);

        assert_eq!(placed, Point::new(792.0, 542.0));
    }
}
