//! The tabbed sessions in the centre: what is open and what happens to it.
//! A tab holds an SSH session, the page of a new tab, or (for RDP and VNC,
//! which cannot connect yet) a summary. Drawing is in `tabs_view.rs`.

use iced::Task;
use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use remotething_core::{Protocol, Quality, QuickTarget, ResolvedConnection, parse_quick};

use crate::desktop;
use crate::desktop_files;
use crate::files_pane;
use crate::ftp_tab;
use crate::i18n::fl;
use crate::launcher;
use crate::recent::Recent;
use crate::session::{self, Context};
use crate::vnc_tab;

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
    /// A tab that was just dropped in a new place, and how its glow fades.
    pub(crate) glowing: Option<(u64, crate::motion::Tween)>,
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
    Desktop(Box<desktop::State>),
    Vnc(Box<vnc_tab::State>),
    Ftp(Box<ftp_tab::State>),
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
    Desktop(u64, desktop::Message),
    Vnc(u64, vnc_tab::Message),
    Ftp(u64, ftp_tab::Message),
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
        let (title, content, task) = start(id, connection, context, recent);
        self.tabs.push(Tab {
            id,
            title,
            content,
            unsaved: None,
        });
        self.active = self.tabs.len() - 1;
        task
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
            Content::Desktop(desktop) => &mut desktop.connection,
            Content::Vnc(vnc) => &mut vnc.connection,
            Content::Ftp(ftp) => &mut ftp.connection,
            Content::Launcher(_) => return,
        };
        connection.id = connection_id.to_string();
        connection.name = name.to_string();
    }

    /// Whether the tab in front is a live terminal, which gets Ctrl+W for itself.
    pub fn front_is_terminal(&self) -> bool {
        self.tabs.get(self.active).is_some_and(Tab::is_live)
    }

    /// The message for a dropped file, when the tab in front takes files: a session with
    /// its file pane open, a file-only tab, or a remote desktop.
    pub fn file_dropped(&self, path: std::path::PathBuf) -> Option<Message> {
        let tab = self.tabs.get(self.active)?;
        let upload = || files_pane::Message::UploadPicked(Some(path.clone()));
        match &tab.content {
            Content::Session(session) if session.has_files_open() => {
                Some(Message::Session(tab.id, session::Message::Files(upload())))
            }
            Content::Ftp(ftp) if ftp.is_connected() => {
                Some(Message::Ftp(tab.id, ftp_tab::Message::Files(upload())))
            }
            // A dropped file goes to the remote desktop, which can paste it.
            Content::Desktop(desktop) if desktop.is_connected() => Some(Message::Desktop(
                tab.id,
                desktop::Message::Files(desktop_files::Message::Dropped(path)),
            )),
            _ => None,
        }
    }

    /// Whether a question or a menu of the tab bar is open, which then takes the keys.
    pub fn has_question(&self) -> bool {
        self.confirming_close.is_some() || self.menu.is_some()
    }

    /// Whether a connected remote desktop (RDP or VNC) is in front.
    pub fn front_is_desktop(&self) -> bool {
        match self.tabs.get(self.active).map(|tab| &tab.content) {
            Some(Content::Desktop(desktop)) => desktop.is_connected(),
            Some(Content::Vnc(vnc)) => vnc.is_connected(),
            _ => false,
        }
    }

    /// Passes a key to the remote desktop in front, if there is one. RDP takes the
    /// place of the key, VNC the character it makes.
    pub fn desktop_key(
        &mut self,
        key: &iced::keyboard::Key,
        physical: iced::keyboard::key::Physical,
        text: Option<&str>,
        pressed: bool,
    ) {
        match self.tabs.get_mut(self.active).map(|tab| &mut tab.content) {
            Some(Content::Desktop(desktop)) => {
                if let Some(scancode) = crate::scancodes::of(physical) {
                    desktop.key(scancode, pressed);
                }
            }
            Some(Content::Vnc(vnc)) => vnc.key(key, physical, text, pressed),
            _ => {}
        }
    }

    /// The terminal colours or text size changed: open terminals are drawn again.
    pub fn terminal_look_changed(&mut self) {
        for tab in &mut self.tabs {
            if let Content::Session(session) = &mut tab.content {
                session.redraw();
            }
        }
    }

    /// The screen scaling changed; remote desktops follow.
    pub fn rescale_desktops(&mut self) {
        for tab in &mut self.tabs {
            if let Content::Desktop(desktop) = &mut tab.content {
                desktop.rescaled();
            }
        }
    }

    /// Offers the local clipboard text to the remote desktop in front, if there is one.
    pub fn desktop_clipboard(&mut self, text: String) {
        match self.tabs.get_mut(self.active).map(|tab| &mut tab.content) {
            Some(Content::Desktop(desktop)) => desktop.local_clipboard(text),
            Some(Content::Vnc(vnc)) => vnc.local_clipboard(text),
            _ => {}
        }
    }

    /// Offers a picture from the local clipboard to the remote desktop in front.
    /// Only RDP shares pictures.
    pub fn desktop_clipboard_picture(&mut self, picture: remotething_rdp::Picture) {
        if let Some(Content::Desktop(desktop)) =
            self.tabs.get_mut(self.active).map(|tab| &mut tab.content)
        {
            desktop.local_picture(picture);
        }
    }

    /// Offers files from the local clipboard to the remote desktop in front. Only RDP shares
    /// files; false when the tab in front is not one, so the clipboard is looked at for text.
    pub fn desktop_clipboard_files(&mut self, paths: Vec<std::path::PathBuf>) -> bool {
        let Some(Content::Desktop(desktop)) =
            self.tabs.get_mut(self.active).map(|tab| &mut tab.content)
        else {
            return false;
        };
        desktop.local_files(paths);
        true
    }

    /// Whether any remote desktop is being recorded, which needs a tick ten times a second.
    pub fn any_recording(&self) -> bool {
        self.tabs.iter().any(|tab| match &tab.content {
            Content::Desktop(desktop) => desktop.is_recording(),
            Content::Vnc(vnc) => vnc.is_recording(),
            _ => false,
        })
    }

    /// Takes the next slot of every desktop recording.
    pub fn record_tick(&mut self) {
        for tab in &mut self.tabs {
            match &mut tab.content {
                Content::Desktop(desktop) => desktop.tick_recording(),
                Content::Vnc(vnc) => vnc.tick_recording(),
                _ => {}
            }
        }
    }

    /// Brings the next tab, or the previous one, to the front, wrapping round the ends.
    pub fn cycle(&mut self, forward: bool) {
        let count = self.tabs.len();
        if count < 2 {
            return;
        }
        self.active = if forward {
            (self.active + 1) % count
        } else {
            (self.active + count - 1) % count
        };
        self.dragged = None;
    }

    /// Brings the tab with this number (from 1) to the front. 9 is always the last tab, as in browsers.
    pub fn activate_number(&mut self, number: usize) {
        let last = self.tabs.len().saturating_sub(1);
        let index = if number >= 9 {
            last
        } else {
            number.saturating_sub(1)
        };
        if index < self.tabs.len() {
            self.active = index;
            self.dragged = None;
        }
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
    ///
    /// With `animate`, the tab that moved glows for a moment so the eye finds it.
    pub fn release(&mut self, animate: bool) {
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
        if animate {
            let glow = crate::motion::Tween::between(
                1.0,
                0.0,
                std::time::Instant::now(),
                crate::motion::GLOW,
            );
            self.glowing = Some((id, glow));
        }
    }

    /// How brightly a tab glows at `now`, from 0 (not at all) to 1.
    pub fn glow_of(&self, id: u64, now: std::time::Instant) -> f32 {
        match &self.glowing {
            Some((glowing, tween)) if *glowing == id => tween.value(now),
            _ => 0.0,
        }
    }

    /// Whether a tab is still glowing at `now`.
    pub fn is_glowing(&self, now: std::time::Instant) -> bool {
        self.glowing
            .as_ref()
            .is_some_and(|(_, tween)| tween.is_running(now))
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
        let id = tab.id;
        match &mut tab.content {
            Content::Session(session) => session
                .key(key, modifiers, text)
                .map(move |message| Message::Session(id, message)),
            Content::Ftp(ftp) => ftp
                .key(key, modifiers)
                .map(move |message| Message::Ftp(id, message)),
            _ => Task::none(),
        }
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
                Content::Desktop(desktop) => Some(desktop.connection.id.as_str()),
                Content::Vnc(vnc) => Some(vnc.connection.id.as_str()),
                Content::Ftp(ftp) => Some(ftp.connection.id.as_str()),
                Content::Launcher(_) => None,
            })
            .collect()
    }
}

impl Tab {
    /// Whether closing this tab would cut a connection.
    fn is_live(&self) -> bool {
        match &self.content {
            Content::Session(session) => session.is_connected(),
            Content::Desktop(desktop) => desktop.is_connected(),
            Content::Vnc(vnc) => vnc.is_connected(),
            Content::Ftp(ftp) => ftp.is_connected(),
            _ => false,
        }
    }
}

/// The title, content and first task of a tab for a connection.
fn start(
    id: u64,
    connection: ResolvedConnection,
    context: &Context<'_>,
    recent: &mut Recent,
) -> (String, Content, Task<Message>) {
    recent.touch(&connection.id);
    start_connection(id, connection, None, context)
}

/// The title, content and first task of a tab for a connection, signing in as
/// `typed_username` when the connection has no credential.
fn start_connection(
    id: u64,
    connection: ResolvedConnection,
    typed_username: Option<String>,
    context: &Context<'_>,
) -> (String, Content, Task<Message>) {
    let title = connection.name.clone();
    match connection.protocol {
        Protocol::Ssh | Protocol::Sftp | Protocol::Wsl => {
            let (session, task) = session::State::start_as(connection, typed_username, context);
            let task = task.map(move |message| Message::Session(id, message));
            (title, Content::Session(Box::new(session)), task)
        }
        Protocol::Rdp => {
            let (desktop, task) = desktop::State::start(connection, typed_username, context);
            let task = task.map(move |message| Message::Desktop(id, message));
            (title, Content::Desktop(Box::new(desktop)), task)
        }
        Protocol::Ftp | Protocol::Ftps | Protocol::Smb => {
            let (ftp, task) = ftp_tab::State::start(connection, typed_username, context);
            let task = task.map(move |message| Message::Ftp(id, message));
            (title, Content::Ftp(Box::new(ftp)), task)
        }
        Protocol::Vnc => {
            let (vnc, task) = vnc_tab::State::start(connection, context);
            let task = task.map(move |message| Message::Vnc(id, message));
            (title, Content::Vnc(Box::new(vnc)), task)
        }
    }
}

/// The title, content and first task of a tab for a typed connection string.
/// Nothing is written to the tree or to the recent list.
fn start_quick(
    id: u64,
    target: &QuickTarget,
    context: &Context<'_>,
) -> (String, Content, Task<Message>) {
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
        desktop_quality: Quality::default(),
    };
    start_connection(id, connection, target.username.clone(), context)
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
        Message::Desktop(id, message) => {
            let desktop = state.tabs.iter_mut().find(|tab| tab.id == id);
            if let Some(Tab {
                content: Content::Desktop(desktop),
                ..
            }) = desktop
            {
                return desktop::update(desktop, message, context)
                    .map(move |message| Message::Desktop(id, message));
            }
        }
        Message::Ftp(id, message) => {
            let ftp = state.tabs.iter_mut().find(|tab| tab.id == id);
            if let Some(Tab {
                content: Content::Ftp(ftp),
                ..
            }) = ftp
            {
                return ftp_tab::update(ftp, message, context)
                    .map(move |message| Message::Ftp(id, message));
            }
        }
        Message::Vnc(id, message) => {
            let vnc = state.tabs.iter_mut().find(|tab| tab.id == id);
            if let Some(Tab {
                content: Content::Vnc(vnc),
                ..
            }) = vnc
            {
                return vnc_tab::update(vnc, message, context)
                    .map(move |message| Message::Vnc(id, message));
            }
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
        let (title, content, task) = start_quick(id, &target, context);
        tab.title = title;
        tab.content = content;
        tab.unsaved = Some(target);
        return task;
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
    let (title, content, task) = start(id, connection, context, recent);
    tab.title = title;
    tab.content = content;
    task
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
