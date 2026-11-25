//! The connection tree on the left: folders that fold, connections that open.
//! Rows can be renamed in place, dragged onto each other and right-clicked.

use std::collections::HashSet;

use iced::alignment::Vertical;
use iced::widget::{column, container, mouse_area, row, scrollable, space, text};
use iced::{Element, Length};
use remotething_core::{Protocol, Store};

use crate::i18n::fl;
use crate::icons;
use crate::rows::{self, Row, RowKind};
use crate::theme;

const INDENT: f32 = 16.0;
/// Room taken by a folder's arrow.
const ARROW_WIDTH: f32 = 14.0;
const ICON_SIZE: f32 = 16.0;
/// The widget id of the text field of the row being renamed.
pub const RENAME_INPUT: &str = "tree-rename";
/// The id of the search field above the list.
pub const SEARCH_INPUT: &str = "tree-search";

/// What is highlighted in the tree and shown in the properties panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Folder(String),
    Connection(String),
}

/// Where a dragged row is let go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropTarget {
    Row(Selection),
    /// The empty space of the list.
    TopLevel,
}

#[derive(Debug, Default)]
pub struct State {
    /// Folders start expanded, so only the collapsed ones are remembered.
    collapsed: HashSet<String>,
    selected: Option<Selection>,
    /// The row being renamed in place, with the text typed so far.
    rename: Option<(Selection, String)>,
    /// The row picked up while the mouse button is down.
    dragged: Option<Selection>,
    /// The row under the pointer.
    hovered: Option<Selection>,
    /// Whether the pointer is over the list at all.
    over_list: bool,
    /// The search field above the list, with what was typed; `None` while it is closed.
    search: Option<String>,
}

impl State {
    pub fn selected(&self) -> Option<&Selection> {
        self.selected.as_ref()
    }

    pub fn select(&mut self, selection: Option<Selection>) {
        self.selected = selection;
    }

    /// Turns the row's name into a text field holding `name`.
    pub fn start_rename(&mut self, selection: Selection, name: String) {
        self.rename = Some((selection, name));
    }

    /// Whether a rename, a drag or a search is in progress, so Escape belongs to the tree.
    pub fn is_busy(&self) -> bool {
        self.rename.is_some() || self.dragged.is_some() || self.search.is_some()
    }

    /// Opens the search field, empty, or keeps what is typed when it is already open.
    pub fn open_search(&mut self) {
        self.search.get_or_insert_with(String::new);
        self.cancel();
    }

    /// Escape: drops a rename or a drag, and closes the search.
    pub fn escape(&mut self) {
        self.cancel();
        self.search = None;
    }

    /// Drops a rename or a drag in progress.
    pub fn cancel(&mut self) {
        self.rename = None;
        self.dragged = None;
    }

    /// The rows moved under the pointer, so what it was over is no longer known.
    pub fn rows_changed(&mut self) {
        self.hovered = None;
    }

    pub fn expand(&mut self, folder_id: &str) {
        self.collapsed.remove(folder_id);
    }

    /// The row being carried, once the pointer has left it.
    pub fn carried(&self) -> Option<&Selection> {
        let dragged = self.dragged.as_ref()?;
        (self.hovered.as_ref() != Some(dragged)).then_some(dragged)
    }

    /// The mouse button went up somewhere in the window: finish a drag, if any.
    pub fn release(&mut self) -> Option<Event> {
        let source = self.dragged.take()?;
        let target = match &self.hovered {
            Some(row) if *row == source => return None,
            Some(row) => DropTarget::Row(row.clone()),
            None if self.over_list => DropTarget::TopLevel,
            None => return None,
        };
        Some(Event::Moved { source, target })
    }

    fn press(&mut self, selection: Selection) -> Option<Event> {
        self.rename = None;
        self.dragged = Some(selection.clone());
        // A pressed row is under the pointer, whatever was reported before.
        self.hovered = Some(selection.clone());
        self.selected = Some(selection);
        Some(Event::SelectionChanged)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    FolderToggled(String),
    FolderSelected(String),
    ConnectionSelected(String),
    ConnectionOpened(String),
    NewFolderPressed,
    NewConnectionPressed,
    CredentialsPressed,
    RowEntered(Selection),
    RowExited(Selection),
    ListEntered,
    ListExited,
    /// A press on the empty space of the list.
    ListPressed,
    /// A right-click on a row, or on the empty space for `None`.
    MenuRequested(Option<Selection>),
    RenameChanged(String),
    RenameSubmitted,
    SearchChanged(String),
    /// Enter in the search field: opens the first match.
    SearchSubmitted,
}

/// What the tree asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    SelectionChanged,
    Open(String),
    /// Enter was pressed in the search for this text: open its first match.
    Search(String),
    NewFolder,
    NewConnection,
    ShowCredentials,
    /// Show the context menu for a row, or for the empty space.
    Menu(Option<Selection>),
    Renamed(Selection, String),
    Moved {
        source: Selection,
        target: DropTarget,
    },
}

pub fn update(state: &mut State, message: Message) -> Option<Event> {
    match message {
        Message::FolderToggled(id) => {
            state.dragged = None;
            state.rows_changed();
            if !state.collapsed.remove(&id) {
                state.collapsed.insert(id);
            }
            None
        }
        Message::FolderSelected(id) => state.press(Selection::Folder(id)),
        Message::ConnectionSelected(id) => state.press(Selection::Connection(id)),
        Message::ConnectionOpened(id) => Some(Event::Open(id)),
        Message::NewFolderPressed => Some(Event::NewFolder),
        Message::NewConnectionPressed => Some(Event::NewConnection),
        Message::CredentialsPressed => Some(Event::ShowCredentials),
        Message::RowEntered(row) => {
            state.hovered = Some(row);
            None
        }
        Message::RowExited(row) => {
            // Entering the next row can be reported before leaving this one.
            if state.hovered.as_ref() == Some(&row) {
                state.hovered = None;
            }
            None
        }
        Message::ListEntered => {
            state.over_list = true;
            None
        }
        Message::ListExited => {
            state.over_list = false;
            None
        }
        Message::ListPressed => {
            state.cancel();
            None
        }
        Message::MenuRequested(target) => {
            state.cancel();
            Some(Event::Menu(target))
        }
        Message::RenameChanged(name) => {
            if let Some((_, typed)) = &mut state.rename {
                *typed = name;
            }
            None
        }
        Message::RenameSubmitted => {
            let (selection, name) = state.rename.take()?;
            Some(Event::Renamed(selection, name))
        }
        Message::SearchChanged(text) => {
            state.search = Some(text);
            state.rows_changed();
            None
        }
        Message::SearchSubmitted => {
            let query = state.search.clone()?;
            Some(Event::Search(query))
        }
    }
}

pub fn view<'a>(state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let query = state
        .search
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty());
    let rows = match query {
        Some(query) => rows::matching_rows(store, query),
        None => rows::visible_rows(store, &state.collapsed),
    };
    let content: Element<'a, Message> = if rows.is_empty() {
        let empty = if query.is_some() {
            fl!("tree-search-empty")
        } else {
            fl!("tree-empty")
        };
        text(empty).style(theme::secondary_text).into()
    } else {
        // No gap between rows: a drop between two of them must land on one.
        let lines = rows.into_iter().map(|row| view_row(state, row));
        scrollable(column(lines).width(Length::Fill)).into()
    };
    // The list takes the rest of the panel, so its empty space can be clicked too.
    let list = mouse_area(container(content).width(Length::Fill).height(Length::Fill))
        .on_enter(Message::ListEntered)
        .on_exit(Message::ListExited)
        .on_press(Message::ListPressed)
        .on_right_press(Message::MenuRequested(None));
    let mut panel = column![].spacing(theme::GAP);
    if let Some(typed) = &state.search {
        panel = panel.push(
            theme::input(&fl!("tree-search-hint"), typed)
                .id(SEARCH_INPUT)
                .padding(6)
                .on_input(Message::SearchChanged)
                .on_submit(Message::SearchSubmitted),
        );
    }
    panel.push(list).into()
}

fn view_row<'a>(state: &'a State, tree_row: Row) -> Element<'a, Message> {
    let selection = tree_row.selection();
    let is_selected = state.selected.as_ref() == Some(&selection);
    let is_drop_target = state.carried().is_some() && state.hovered.as_ref() == Some(&selection);
    let style = if is_drop_target {
        RowStyle::DropTarget
    } else if is_selected {
        RowStyle::Selected
    } else {
        RowStyle::Plain
    };
    let renamed = match &state.rename {
        Some((renaming, typed)) if *renaming == selection => Some(typed.as_str()),
        _ => None,
    };
    let indent = space::horizontal().width(Length::Fixed(INDENT * tree_row.depth as f32));
    let icon = icons::view(tree_row.icon, ICON_SIZE);

    let line: Element<'a, Message> = match (tree_row.kind, renamed) {
        (RowKind::Folder { expanded }, renamed) => {
            let arrow = if expanded {
                icons::CHEVRON_DOWN
            } else {
                icons::CHEVRON_RIGHT
            };
            // The arrow folds; the icon and name select.
            let toggle = mouse_area(icons::view(arrow, ARROW_WIDTH).style(theme::secondary_text))
                .on_press(Message::FolderToggled(tree_row.id.clone()));
            let name: Element<'a, Message> = match renamed {
                Some(typed) => row![icon, rename_input(typed)]
                    .spacing(8)
                    .align_y(Vertical::Center)
                    .into(),
                None => {
                    let label = row![icon, name_text(tree_row.name)]
                        .spacing(8)
                        .align_y(Vertical::Center);
                    mouse_area(label)
                        .on_press(Message::FolderSelected(tree_row.id))
                        .into()
                }
            };
            let line = row![indent, toggle, name]
                .spacing(6)
                .align_y(Vertical::Center);
            row_container(line, style)
        }
        (RowKind::Connection(_), Some(typed)) => {
            let arrow_space = space::horizontal().width(Length::Fixed(ARROW_WIDTH));
            let line = row![indent, arrow_space, icon, rename_input(typed)]
                .spacing(6)
                .align_y(Vertical::Center);
            row_container(line, style)
        }
        (RowKind::Connection(protocol), None) => {
            let label = name_text(tree_row.name);
            let badge = container(text(protocol_label(protocol)).size(11))
                .padding([2, 6])
                .style(theme::chip);
            // Lines up connection names with folder names, which follow an arrow.
            let arrow_space = space::horizontal().width(Length::Fixed(ARROW_WIDTH));
            let line = row![indent, arrow_space, icon, label, badge]
                .spacing(6)
                .align_y(Vertical::Center);
            mouse_area(row_container(line, style))
                .on_press(Message::ConnectionSelected(tree_row.id.clone()))
                .on_double_click(Message::ConnectionOpened(tree_row.id))
                .into()
        }
    };

    mouse_area(line)
        .on_enter(Message::RowEntered(selection.clone()))
        .on_exit(Message::RowExited(selection.clone()))
        .on_right_press(Message::MenuRequested(Some(selection)))
        .into()
}

/// A name that wraps inside the row, breaking inside a long word when it must.
fn name_text<'a>(name: String) -> iced::widget::Text<'a> {
    text(name)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
}

/// The name of a row as a text field. Enter keeps the new name.
fn rename_input<'a>(typed: &str) -> Element<'a, Message> {
    theme::input("", typed)
        .id(RENAME_INPUT)
        .padding(2)
        .on_input(Message::RenameChanged)
        .on_submit(Message::RenameSubmitted)
        .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowStyle {
    Plain,
    Selected,
    DropTarget,
}

/// One full-width line of the tree.
fn row_container<'a>(
    line: iced::widget::Row<'a, Message>,
    style: RowStyle,
) -> Element<'a, Message> {
    let line = container(line).padding([6, 8]).width(Length::Fill);
    match style {
        RowStyle::Plain => line.style(theme::list_row(false)),
        RowStyle::Selected => line.style(theme::list_row(true)),
        RowStyle::DropTarget => line.style(theme::drop_target),
    }
    .into()
}

pub fn protocol_label(protocol: Protocol) -> String {
    match protocol {
        Protocol::Ssh => fl!("protocol-ssh"),
        Protocol::Sftp => fl!("protocol-sftp"),
        Protocol::Ftp => fl!("protocol-ftp"),
        Protocol::Ftps => fl!("protocol-ftps"),
        Protocol::Smb => fl!("protocol-smb"),
        Protocol::Nfs => fl!("protocol-nfs"),
        Protocol::Wsl => fl!("protocol-wsl"),
        Protocol::Local => fl!("protocol-local"),
        Protocol::Rdp => fl!("protocol-rdp"),
        Protocol::Vnc => fl!("protocol-vnc"),
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
