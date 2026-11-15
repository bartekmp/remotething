//! The tabbed sessions in the centre: what is open and what happens to it.
//! A tab holds an SSH session, the page of a new tab, or (for RDP and VNC,
//! which cannot connect yet) a summary. Drawing is in `tabs_view.rs`.

use iced::Task;
use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use remotething_core::{Protocol, Quality, QuickTarget, ResolvedConnection, parse_quick};

use crate::i18n::fl;
use crate::launcher;
use crate::recent::Recent;
use crate::session::{self, Context};

#[derive(Default)]
pub struct State {
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active: usize,
    /// Tabs move when others close, so results of background work find theirs by id.
    pub(crate) next_id: u64,
    /// The tab whose close is waiting for a yes or no.
    pub(crate) confirming_close: Option<u64>,
    /// The tab picked up while the mouse button is down.
    pub(crate) dragged: Option<u64>,
    /// The position in the bar under the pointer.
    pub(crate) hovered: Option<usize>,
    /// Whether the list of tabs that do not fit is showing.
    pub(crate) overflow_open: bool,
    /// The tab whose right-click menu is showing.
    pub(crate) menu: Option<u64>,
    /// A typed connection the user asked to save, until the window collects it.
    pub(crate) save_request: Option<(u64, QuickTarget)>,
}

pub(crate) struct Tab {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) content: Content,
    /// What was typed to open this tab, while the connection is not in the tree.
    pub(crate) unsaved: Option<QuickTarget>,
}

pub(crate) enum Content {
    /// A new tab that has not picked its connection yet, with the search typed so far.
    Launcher(String),
    Session(Box<session::State>),
    /// A protocol that cannot connect yet.
    Summary(ResolvedConnection),
}

#[derive(Debug, Clone)]
pub enum Message {
    /// A press on a tab: bring it to the front and pick it up.
    TabPressed(usize),
    TabEntered(usize),
    TabExited(usize),
    /// The close button of a tab.
    CloseRequested(usize),
    CloseConfirmed,
    CloseCancelled,
    NewTabPressed,
    OverflowToggled,
    /// A tab chosen from the list of those that do not fit.
    OverflowPicked(usize),
    /// A right-click on a tab.
    MenuRequested(usize),
    MenuDismissed,
    /// "Save this connection" in a tab's menu.
    MenuSavePressed,
    /// "Close" in a tab's menu.
    MenuClosePressed,
    Launcher(u64, launcher::Message),
    Session(u64, session::Message),
}

impl State {
    /// Opens a new tab for the connection, brings it to the front and starts connecting.
    pub fn open(
        &mut self,
        connection: ResolvedConnection,
        context: &Context<'_>,
        recent: &mut Recent,
    ) -> Task<Message> {
        let id = self.new_id();
        let (title, content, task) = start(connection, context, recent);
        self.tabs.push(Tab {
            id,
            title,
            content,
            unsaved: None,
        });
        self.active = self.tabs.len() - 1;
        task.map(move |message| Message::Session(id, message))
    }

    /// Opens an empty tab that lists recent connections, with the keyboard in its search field.
    pub fn open_launcher(&mut self) -> Task<Message> {
        let id = self.new_id();
        self.tabs.push(Tab {
            id,
            title: fl!("tabs-new"),
            content: Content::Launcher(String::new()),
            unsaved: None,
        });
        self.active = self.tabs.len() - 1;
        iced::widget::operation::focus(launcher::SEARCH_INPUT)
    }

    fn new_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// A typed connection the user asked to save, with the id of its tab. It is handed out once.
    pub fn take_save_request(&mut self) -> Option<(u64, QuickTarget)> {
        self.save_request.take()
    }

    /// The typed connection of a tab is now in the tree under `connection_id`.
    pub fn mark_saved(&mut self, tab_id: u64, connection_id: &str, name: &str) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        tab.unsaved = None;
        tab.title = name.to_string();
        let connection = match &mut tab.content {
            Content::Session(session) => &mut session.connection,
            Content::Summary(connection) => connection,
            Content::Launcher(_) => return,
        };
        connection.id = connection_id.to_string();
        connection.name = name.to_string();
    }

    /// Whether the tab in front is a live terminal, which gets Ctrl+W for itself.
    pub fn front_is_terminal(&self) -> bool {
        self.tabs.get(self.active).is_some_and(Tab::is_live)
    }

    /// Closes the tab in front, after a question when it holds a live session.
    pub fn close_front(&mut self) {
        self.request_close(self.active);
    }

    fn request_close(&mut self, index: usize) {
        self.dragged = None;
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        if tab.is_live() {
            self.confirming_close = Some(tab.id);
        } else {
            self.close(index);
        }
    }

    fn close(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        // Dropping the tab drops its session, which closes the connection.
        self.tabs.remove(index);
        // Keep the same tab in front; when it was the one closed, its left neighbour.
        if self.active > index || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        self.hovered = None;
    }

    pub(crate) fn index_of(&self, id: u64) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == id)
    }

    /// The mouse button went up somewhere in the window: drop a carried tab where it is.
    pub fn release(&mut self) {
        let (Some(id), Some(target)) = (self.dragged.take(), self.hovered) else {
            return;
        };
        let Some(source) = self.index_of(id) else {
            return;
        };
        if source == target || target >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(source);
        self.tabs.insert(target, tab);
        // The carried tab was brought to the front when it was pressed.
        self.active = target;
    }

    /// A key press that no text field took. A pending close question gets it
    /// first, then the session in front.
    pub fn key(&mut self, key: &Key, modifiers: Modifiers, text: Option<&str>) -> Task<Message> {
        if self.menu.is_some() {
            // Escape closes the menu; no key reaches the session behind it.
            if *key == Key::Named(Named::Escape) {
                self.menu = None;
            }
            return Task::none();
        }
        if self.confirming_close.is_some() {
            match key {
                Key::Named(Named::Enter) => self.confirm_close(),
                Key::Named(Named::Escape) => self.confirming_close = None,
                // Nothing reaches the session while the question is open.
                _ => {}
            }
            return Task::none();
        }

        let Some(tab) = self.tabs.get_mut(self.active) else {
            return Task::none();
        };
        let Content::Session(session) = &mut tab.content else {
            return Task::none();
        };
        let id = tab.id;
        session
            .key(key, modifiers, text)
            .map(move |message| Message::Session(id, message))
    }

    fn confirm_close(&mut self) {
        if let Some(index) = self
            .confirming_close
            .take()
            .and_then(|id| self.index_of(id))
        {
            self.close(index);
        }
    }

    #[cfg(test)]
    pub fn open_ids(&self) -> Vec<&str> {
        self.tabs
            .iter()
            .filter_map(|tab| match &tab.content {
                Content::Session(session) => Some(session.connection.id.as_str()),
                Content::Summary(connection) => Some(connection.id.as_str()),
                Content::Launcher(_) => None,
            })
            .collect()
    }
}

impl Tab {
    /// Whether closing this tab would cut a connection.
    fn is_live(&self) -> bool {
        matches!(&self.content, Content::Session(session) if session.is_connected())
    }
}

/// The title, content and first task of a tab for a connection.
fn start(
    connection: ResolvedConnection,
    context: &Context<'_>,
    recent: &mut Recent,
) -> (String, Content, Task<session::Message>) {
    recent.touch(&connection.id);
    let title = connection.name.clone();
    match connection.protocol {
        Protocol::Ssh => {
            let (session, task) = session::State::start(connection, context);
            (title, Content::Session(Box::new(session)), task)
        }
        Protocol::Rdp | Protocol::Vnc => (title, Content::Summary(connection), Task::none()),
    }
}

/// The title, content and first task of a tab for a typed connection string.
/// Nothing is written to the tree or to the recent list.
fn start_quick(
    target: &QuickTarget,
    context: &Context<'_>,
) -> (String, Content, Task<session::Message>) {
    let connection = ResolvedConnection {
        // Not in the tree, so it has no id yet.
        id: String::new(),
        name: target.label(),
        protocol: target.protocol,
        host: target.host.clone(),
        port: target.port_or_default(),
        credential: None,
        jump: None,
        forwards: Vec::new(),
        rdp_quality: Quality::default(),
    };
    let title = connection.name.clone();
    match connection.protocol {
        Protocol::Ssh => {
            let username = target.username.clone();
            let (session, task) = session::State::start_as(connection, username, context);
            (title, Content::Session(Box::new(session)), task)
        }
        Protocol::Rdp | Protocol::Vnc => (title, Content::Summary(connection), Task::none()),
    }
}

pub fn update(
    state: &mut State,
    message: Message,
    context: &mut Context<'_>,
    recent: &mut Recent,
) -> Task<Message> {
    match message {
        Message::TabPressed(index) => {
            if let Some(tab) = state.tabs.get(index) {
                state.active = index;
                state.dragged = Some(tab.id);
                // A pressed tab is under the pointer, whatever was reported before.
                state.hovered = Some(index);
            }
        }
        Message::TabEntered(index) => state.hovered = Some(index),
        Message::TabExited(index) => {
            // Entering the next tab can be reported before leaving this one.
            if state.hovered == Some(index) {
                state.hovered = None;
            }
        }
        Message::CloseRequested(index) => state.request_close(index),
        Message::CloseConfirmed => state.confirm_close(),
        Message::CloseCancelled => state.confirming_close = None,
        Message::NewTabPressed => return state.open_launcher(),
        Message::OverflowToggled => state.overflow_open = !state.overflow_open,
        Message::OverflowPicked(index) => {
            state.overflow_open = false;
            if index < state.tabs.len() {
                state.active = index;
            }
        }
        Message::MenuRequested(index) => {
            state.dragged = None;
            state.menu = state.tabs.get(index).map(|tab| tab.id);
        }
        Message::MenuDismissed => state.menu = None,
        Message::MenuSavePressed => {
            let tab = state.menu.take().and_then(|id| state.index_of(id));
            if let Some(tab) = tab.map(|index| &state.tabs[index])
                && let Some(target) = &tab.unsaved
            {
                state.save_request = Some((tab.id, target.clone()));
            }
        }
        Message::MenuClosePressed => {
            if let Some(index) = state.menu.take().and_then(|id| state.index_of(id)) {
                state.request_close(index);
            }
        }
        Message::Launcher(id, message) => {
            return update_launcher(state, id, message, context, recent);
        }
        Message::Session(id, message) => {
            // The tab may have been closed while the result was on its way.
            let session = state.tabs.iter_mut().find(|tab| tab.id == id);
            if let Some(Tab {
                content: Content::Session(session),
                ..
            }) = session
            {
                return session::update(session, message, context)
                    .map(move |message| Message::Session(id, message));
            }
        }
    }
    Task::none()
}

fn update_launcher(
    state: &mut State,
    id: u64,
    message: launcher::Message,
    context: &Context<'_>,
    recent: &mut Recent,
) -> Task<Message> {
    let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id) else {
        return Task::none();
    };
    let Content::Launcher(query) = &mut tab.content else {
        return Task::none();
    };

    // A typed connection string wins over the search results.
    let quick = match &message {
        launcher::Message::QuickPicked(target) => Some(target.clone()),
        launcher::Message::Submitted => parse_quick(query),
        _ => None,
    };
    if let Some(target) = quick {
        let (title, content, task) = start_quick(&target, context);
        tab.title = title;
        tab.content = content;
        tab.unsaved = Some(target);
        return task.map(move |message| Message::Session(id, message));
    }

    let picked = match message {
        launcher::Message::QueryChanged(typed) => {
            *query = typed;
            return Task::none();
        }
        launcher::Message::Picked(connection_id) => Some(connection_id),
        launcher::Message::Submitted => launcher::entries(context.store, recent.ids(), query)
            .first()
            .map(|connection| connection.id.clone()),
        launcher::Message::QuickPicked(_) => None,
    };
    let Some(connection) = picked.and_then(|id| context.store.resolve(&id).ok()) else {
        return Task::none();
    };

    // The connection takes over the tab it was picked in.
    let (title, content, task) = start(connection, context, recent);
    tab.title = title;
    tab.content = content;
    task.map(move |message| Message::Session(id, message))
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
