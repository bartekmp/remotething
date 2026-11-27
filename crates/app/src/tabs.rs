//! The tabbed sessions in the centre: what is open and what happens to it.
//! A tab holds an SSH session, the page of a new tab, or (for RDP and VNC,
//! which cannot connect yet) a summary. Drawing is in `tabs_view.rs`.

use iced::Task;
use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use remotething_core::{
    LocalSettings, Protocol, Quality, QuickTarget, ResolvedConnection, parse_quick,
};

use crate::desktop;
use crate::desktop_files;
use crate::files_pane;
use crate::ftp_tab;
use crate::i18n::fl;
use crate::launcher;
use crate::recent::Recent;
use crate::session::{self, Context};
use crate::vnc_tab;

/// A size to show while the window is being resized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewSize {
    /// Columns and lines of a terminal.
    Characters(usize, usize),
    /// Pixels of a desktop or of the window.
    Pixels(u32, u32),
}

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
    /// The tab shown beside the one in front, when the view is split. It is never the one in front.
    pub(crate) split: Option<u64>,
    /// Whether the panes of a split are stacked, one above the other, instead of side by side.
    pub(crate) split_stacked: bool,
    /// Whether the size of the view shows over the tab, while the window is being resized.
    pub(crate) size_badge: bool,
    /// How much of the split the first pane (left, or top) takes, from 0 to 1.
    pub(crate) split_share: Option<f32>,
    /// Whether the divider is being dragged.
    pub(crate) resizing: bool,
}

/// The smallest and largest share of a split that a pane can have.
const SHARE_LIMITS: (f32, f32) = (0.2, 0.8);

pub(crate) struct Tab {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) content: Content,
    /// What was typed to open this tab, while the connection is not in the tree.
    pub(crate) unsaved: Option<QuickTarget>,
}

/// A screen of the app that lives in a tab: it is switched to and closed like a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Credentials,
    Settings,
}

pub(crate) enum Content {
    /// The credentials or the settings. Their state belongs to the window, not to the tab.
    Page(Page),
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
    /// "Show beside the front tab" in a tab's menu.
    MenuSplitPressed,
    /// "Back to one tab" in a tab's menu.
    MenuUnsplitPressed,
    /// "Stack the panes" or "Put the panes side by side" in a tab's menu.
    MenuRotatePressed,
    /// A double-click on the divider: both panes take half again.
    DividerReset,
    /// The mouse went down on the divider between the panes.
    DividerPressed,
    /// While the divider is dragged: the pointer is at this share of the split.
    DividerMoved(f32),
    /// A click on the pane that does not have the keyboard: it gets it.
    PaneFocused(u64),
    Launcher(u64, launcher::Message),
    Session(u64, session::Message),
    Desktop(u64, desktop::Message),
    Vnc(u64, vnc_tab::Message),
    Ftp(u64, ftp_tab::Message),
    /// A message of a page shown in a tab, for the window to handle.
    Page(Box<crate::app::Message>),
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

    /// The title of the tab being carried, once the pointer has left it: for the ghost that
    /// follows the pointer.
    pub fn carried_title(&self) -> Option<&str> {
        let index = self.index_of(self.dragged?)?;
        (self.hovered != Some(index)).then(|| self.tabs[index].title.as_str())
    }

    /// How many tabs are open.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// The page in front, when the tab in front is one.
    pub fn front_page(&self) -> Option<Page> {
        match &self.tabs.get(self.active)?.content {
            Content::Page(page) => Some(*page),
            _ => None,
        }
    }

    /// Brings the tab of a page to the front, opening it first when there is none.
    pub fn open_page(&mut self, page: Page) {
        let existing = self
            .tabs
            .iter()
            .position(|tab| matches!(tab.content, Content::Page(shown) if shown == page));
        if let Some(index) = existing {
            self.set_active(index);
            return;
        }
        let id = self.new_id();
        let title = match page {
            Page::Credentials => fl!("tree-credentials"),
            Page::Settings => fl!("tree-settings"),
        };
        self.tabs.push(Tab {
            id,
            title,
            content: Content::Page(page),
            unsaved: None,
        });
        self.active = self.tabs.len() - 1;
    }

    /// Closes the tab of a page, when it is open.
    pub fn close_page(&mut self, page: Page) {
        let existing = self
            .tabs
            .iter()
            .position(|tab| matches!(tab.content, Content::Page(shown) if shown == page));
        if let Some(index) = existing {
            self.close(index);
        }
    }

    /// Opens a tab with the user's own shell on this machine. It is not in the tree, but can be saved.
    pub fn open_local_terminal(&mut self, context: &Context<'_>) -> Task<Message> {
        let target = QuickTarget {
            protocol: Protocol::Local,
            username: None,
            host: String::new(),
            port: None,
        };
        let id = self.new_id();
        let (title, content, task) = start_quick(id, &target, context);
        self.tabs.push(Tab {
            id,
            title,
            content,
            unsaved: Some(target),
        });
        self.active = self.tabs.len() - 1;
        task
    }

    /// Opens an empty tab that lists recent connections, with the keyboard in its search field.
    pub fn open_launcher(&mut self) -> Task<Message> {
        let id = self.new_id();
        let tab = Tab {
            id,
            title: fl!("tabs-new"),
            content: Content::Launcher(String::new()),
            unsaved: None,
        };
        // In a split view the panes are in tab order, so the new tab goes right after the
        // one in front to take over its pane rather than jump to the other side.
        if self.split.is_some() && self.active < self.tabs.len() {
            self.active += 1;
            self.tabs.insert(self.active, tab);
        } else {
            self.tabs.push(tab);
            self.active = self.tabs.len() - 1;
        }
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
            Content::Launcher(_) | Content::Page(_) => return,
        };
        connection.id = connection_id.to_string();
        connection.name = name.to_string();
    }

    /// The size to show while resizing, for the tabs where it means something: a terminal's
    /// characters, and an RDP desktop's pixels as it will be asked for. Other tabs show nothing;
    /// a VNC picture is only scaled to fit, so it has no size to aim for.
    pub fn front_view_size(&self) -> Option<ViewSize> {
        match &self.tabs.get(self.active)?.content {
            Content::Session(session) => session
                .terminal_size()
                .map(|(columns, lines)| ViewSize::Characters(columns, lines)),
            Content::Desktop(desktop) => desktop
                .target_size()
                .map(|(width, height)| ViewSize::Pixels(width, height)),
            _ => None,
        }
    }

    /// Whether the tab in front is a terminal, which has a row of buttons above its screen.
    pub fn front_has_toolbar(&self) -> bool {
        matches!(
            self.tabs.get(self.active).map(|tab| &tab.content),
            Some(Content::Session(session)) if session.terminal_size().is_some()
        )
    }

    /// Whether the tab in front is a live terminal, which gets Ctrl+W for itself.
    /// Whether the tab in front shows a sign-in form.
    pub fn front_is_asking(&self) -> bool {
        match self.tabs.get(self.active).map(|tab| &tab.content) {
            Some(Content::Session(session)) => session.is_asking(),
            Some(Content::Desktop(desktop)) => desktop.is_asking(),
            Some(Content::Vnc(vnc)) => vnc.is_asking(),
            Some(Content::Ftp(ftp)) => ftp.is_asking(),
            _ => false,
        }
    }

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

    /// Whether the creator of a key combination is open on the desktop in front, and so
    /// takes the keys.
    pub fn front_creates_keys(&self) -> bool {
        let menu = match self.tabs.get(self.active).map(|tab| &tab.content) {
            Some(Content::Desktop(desktop)) => desktop.send_keys(),
            Some(Content::Vnc(vnc)) => vnc.send_keys(),
            _ => None,
        };
        menu.is_some_and(|menu| menu.wants_keys())
    }

    /// A key went down while the creator of a key combination reads keys.
    pub fn front_captures_key(
        &mut self,
        physical: iced::keyboard::key::Physical,
        modifiers: Modifiers,
    ) -> bool {
        let menu = match self.tabs.get_mut(self.active).map(|tab| &mut tab.content) {
            Some(Content::Desktop(desktop)) => desktop.send_keys_mut(),
            Some(Content::Vnc(vnc)) => vnc.send_keys_mut(),
            _ => None,
        };
        menu.is_some_and(|menu| menu.capture(physical, modifiers))
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
        let next = if forward {
            (self.active + 1) % count
        } else {
            (self.active + count - 1) % count
        };
        self.set_active(next);
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
            self.set_active(index);
            self.dragged = None;
        }
    }

    /// Brings the tab at `index` to the front. A tab that was beside the front one trades
    /// places with it, so no tab is shown twice.
    pub(crate) fn set_active(&mut self, index: usize) {
        if let Some(id) = self.tabs.get(index).map(|tab| tab.id) {
            self.activate_id(id);
        }
    }

    fn activate_id(&mut self, id: u64) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let before = self.tabs.get(self.active).map(|tab| tab.id);
        if self.split == Some(id) {
            self.split = before.filter(|before| *before != id);
        }
        self.active = index;
    }

    /// How much of the split the first pane takes.
    pub(crate) fn share(&self) -> f32 {
        self.split_share.unwrap_or(0.5)
    }

    /// The index of the tab shown beside the front one.
    pub(crate) fn split_index(&self) -> Option<usize> {
        self.split.and_then(|id| self.index_of(id))
    }

    /// Whether the tab at `index` is on show: in front, or beside the one in front.
    pub(crate) fn is_shown(&self, index: usize) -> bool {
        index == self.active || self.split_index() == Some(index)
    }

    /// Splits the view, or goes back to one tab when it is split. The next tab goes beside
    /// the front one; with only one tab open, a new tab does.
    pub fn toggle_split(&mut self) -> Task<Message> {
        if self.split.take().is_some() {
            self.resizing = false;
            return Task::none();
        }
        if self.tabs.len() >= 2 {
            let next = (self.active + 1) % self.tabs.len();
            self.split = self.tabs.get(next).map(|tab| tab.id);
            return Task::none();
        }
        let Some(front) = self.tabs.get(self.active).map(|tab| tab.id) else {
            return Task::none();
        };
        // The new tab takes the keyboard, so a connection can be typed at once.
        let task = self.open_launcher();
        self.split = Some(front);
        task
    }

    /// Stacks the panes of a split view, or puts them side by side again.
    pub fn rotate_split(&mut self) {
        if self.split.is_some() {
            self.split_stacked = !self.split_stacked;
        }
    }

    /// Gives the keyboard to the other pane of a split view.
    pub fn focus_other_pane(&mut self) {
        if let Some(id) = self.split {
            self.activate_id(id);
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
        let was_in_front = index == self.active;
        // Dropping the tab drops its session, which closes the connection.
        let closed = self.tabs.remove(index).id;
        if self.split == Some(closed) {
            self.split = None;
        }
        if was_in_front {
            // The tab beside it, if any, moves up; else the one that took its place.
            let beside = self.split.take().and_then(|id| self.index_of(id));
            self.active = beside.unwrap_or_else(|| index.min(self.tabs.len().saturating_sub(1)));
        } else if self.active > index {
            self.active -= 1;
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
        self.resizing = false;
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
        self.activate_id(id);
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
                Content::Launcher(_) | Content::Page(_) => None,
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
        Protocol::Ssh | Protocol::Sftp | Protocol::Wsl | Protocol::Local => {
            let (session, task) = session::State::start_as(connection, typed_username, context);
            let task = task.map(move |message| Message::Session(id, message));
            (title, Content::Session(Box::new(session)), task)
        }
        Protocol::Rdp => {
            let (desktop, task) = desktop::State::start(connection, typed_username, context);
            let task = task.map(move |message| Message::Desktop(id, message));
            (title, Content::Desktop(Box::new(desktop)), task)
        }
        Protocol::Ftp | Protocol::Ftps | Protocol::Smb | Protocol::Nfs => {
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
    let name = if target.protocol == Protocol::Local {
        crate::local_shell::title(&target.host)
    } else {
        target.label()
    };
    let connection = ResolvedConnection {
        terminal_theme: None,
        remote_command: None,
        // Not in the tree, so it has no id yet.
        id: String::new(),
        name,
        protocol: target.protocol,
        host: target.host.clone(),
        port: target.port_or_default(),
        credential: None,
        jump: None,
        forwards: Vec::new(),
        local: LocalSettings::default(),
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
        // The window takes these before they get here.
        Message::Page(_) => {}
        Message::TabPressed(index) => {
            if let Some(tab) = state.tabs.get(index) {
                let id = tab.id;
                state.set_active(index);
                state.dragged = Some(id);
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
            state.set_active(index);
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
        Message::MenuSplitPressed => {
            // The tab of the menu goes beside the front one, unless it is the front one.
            let beside = state
                .menu
                .take()
                .filter(|id| state.index_of(*id) != Some(state.active));
            if beside.is_some() {
                state.split = beside;
            }
        }
        Message::MenuUnsplitPressed => {
            state.menu = None;
            state.split = None;
            state.resizing = false;
        }
        Message::MenuRotatePressed => {
            state.menu = None;
            state.split_stacked = !state.split_stacked;
        }
        Message::DividerReset => {
            state.resizing = false;
            state.split_share = None;
        }
        Message::DividerPressed => state.resizing = state.split.is_some(),
        Message::DividerMoved(share) => {
            if state.resizing && share.is_finite() {
                state.split_share = Some(share.clamp(SHARE_LIMITS.0, SHARE_LIMITS.1));
            }
        }
        Message::PaneFocused(id) => state.activate_id(id),
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
