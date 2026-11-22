//! The panel on the right: edit the selected folder or connection.

use std::fmt;

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};
use remotething_core::{
    Credential, CredentialKind, Forward, Protocol, Quality, Settings, Store, format_forwards,
    parse_forwards,
};
use remotething_secrets::Secret;

use crate::credentials::Hidden;
use crate::i18n::fl;
use crate::icons;
use crate::quality_choice::{self, QualityChoice};
use crate::theme;
use crate::tree::{Selection, protocol_label};

/// Space kept free beside the fields for the scrollbar of the panel.
const SCROLLBAR_ROOM: f32 = 14.0;

const PROTOCOLS: [ProtocolChoice; 9] = [
    ProtocolChoice(Protocol::Ssh),
    ProtocolChoice(Protocol::Sftp),
    ProtocolChoice(Protocol::Ftp),
    ProtocolChoice(Protocol::Ftps),
    ProtocolChoice(Protocol::Smb),
    ProtocolChoice(Protocol::Nfs),
    ProtocolChoice(Protocol::Wsl),
    ProtocolChoice(Protocol::Rdp),
    ProtocolChoice(Protocol::Vnc),
];

/// The fields as typed, before they are checked and written to the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub name: String,
    /// Connections only.
    pub host: String,
    /// Connections only.
    pub protocol: Protocol,
    /// Empty means "inherit".
    pub port: String,
    pub parent: Option<String>,
    pub credential: Option<String>,
    pub jump: Option<String>,
    /// Local port forwards as typed, `local:host:port` separated by commas. Empty means "inherit".
    pub forwards: String,
    /// RDP only. `None` means "inherit".
    pub desktop_quality: Option<Quality>,
    /// `None` means the default icon for the kind of item.
    pub icon: Option<String>,
    /// The user name of a password typed here to be saved as a credential. Connections only.
    pub new_username: String,
    /// A password typed here; saving makes it a credential of its own, with no detour through
    /// the credentials screen. Empty means none.
    pub new_password: Hidden,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// An icon from the grid, or `None` for the default.
    IconPicked(Option<&'static str>),
    NameChanged(String),
    HostChanged(String),
    PortChanged(String),
    ProtocolPicked(ProtocolChoice),
    ParentPicked(Choice),
    CredentialPicked(Choice),
    JumpPicked(Choice),
    ForwardsChanged(String),
    NewUsernameChanged(String),
    NewPasswordChanged(Hidden),
    QualityPicked(QualityChoice),
    SavePressed,
    DeletePressed,
}

/// What the panel asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Save,
    Delete,
}

/// An entry in a drop-down: an id, or `None` for "top level" or "inherit".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    id: Option<String>,
    label: String,
}

impl fmt::Display for Choice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolChoice(Protocol);

impl fmt::Display for ProtocolChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&protocol_label(self.0))
    }
}

impl Draft {
    /// The current values of the selected item, or `None` if it no longer exists.
    pub fn load(store: &Store, selection: &Selection) -> Option<Draft> {
        match selection {
            Selection::Folder(id) => {
                let folder = store.folders.iter().find(|folder| &folder.id == id)?;
                Some(Draft::new(
                    &folder.name,
                    "",
                    Protocol::Ssh,
                    &folder.parent,
                    &folder.settings,
                    &folder.icon,
                ))
            }
            Selection::Connection(id) => {
                let connection = store.connections.iter().find(|c| &c.id == id)?;
                Some(Draft::new(
                    &connection.name,
                    &connection.host,
                    connection.protocol,
                    &connection.parent,
                    &connection.settings,
                    &connection.icon,
                ))
            }
        }
    }

    fn new(
        name: &str,
        host: &str,
        protocol: Protocol,
        parent: &Option<String>,
        settings: &Settings,
        icon: &Option<String>,
    ) -> Draft {
        Draft {
            name: name.to_string(),
            host: host.to_string(),
            protocol,
            port: settings
                .port
                .map(|port| port.to_string())
                .unwrap_or_default(),
            parent: parent.clone(),
            credential: settings.credential.clone(),
            jump: settings.jump.clone(),
            forwards: format_forwards(&settings.forwards),
            desktop_quality: settings.desktop.quality,
            icon: icon.clone(),
            new_username: String::new(),
            new_password: Hidden(String::new()),
        }
    }

    /// Returns a copy of the store with this draft written into the selected item.
    ///
    /// Nothing is changed when a field is invalid or the result would not validate.
    #[cfg(test)]
    pub fn apply(&self, store: &Store, selection: &Selection) -> Result<Store, String> {
        self.apply_with_password(store, selection)
            .map(|saved| saved.store)
    }

    /// Like `apply`, and when a password was typed for a connection that has no credential, adds
    /// a credential for it and links the connection to it. The secret is handed back to be
    /// stored; the file only ever holds the credential's description.
    pub fn apply_with_password(
        &self,
        store: &Store,
        selection: &Selection,
    ) -> Result<Saved, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(fl!("properties-error-name"));
        }
        let port = parse_port(&self.port)?;
        let forwards = parse_forwards(&self.forwards)
            .map_err(|bad| fl!("properties-error-forwards", text = bad.0))?;

        let mut updated = store.clone();
        let mut password_for = None;
        match selection {
            Selection::Folder(id) => {
                let folder = updated.folders.iter_mut().find(|folder| &folder.id == id);
                if let Some(folder) = folder {
                    folder.name = name.to_string();
                    folder.parent = self.parent.clone();
                    folder.icon = self.icon.clone();
                    self.write_settings(&mut folder.settings, port, &forwards);
                }
            }
            Selection::Connection(id) => {
                let host = self.host.trim();
                if host.is_empty() {
                    return Err(fl!("properties-error-host"));
                }
                let connection = updated.connections.iter_mut().find(|c| &c.id == id);
                if let Some(connection) = connection {
                    connection.name = name.to_string();
                    connection.host = host.to_string();
                    connection.protocol = self.protocol;
                    connection.parent = self.parent.clone();
                    connection.icon = self.icon.clone();
                    self.write_settings(&mut connection.settings, port, &forwards);
                    if self.credential.is_none() && !self.new_password.0.is_empty() {
                        password_for = Some(connection.id.clone());
                    }
                }
            }
        }

        let secret = password_for
            .and_then(|connection_id| self.add_credential(&mut updated, &connection_id));
        updated.validate().map_err(|error| error.to_string())?;
        Ok(Saved {
            store: updated,
            secret,
        })
    }

    /// Adds a credential named after the connection for the typed password, and links the
    /// connection to it. Returns its id and the secret.
    fn add_credential(&self, store: &mut Store, connection_id: &str) -> Option<(String, Secret)> {
        let name = self.name.trim().to_string();
        let username = Some(self.new_username.trim().to_string()).filter(|user| !user.is_empty());
        let id = store.new_credential_id(&name);
        store.credentials.push(Credential {
            id: id.clone(),
            name,
            username,
            domain: None,
            kind: CredentialKind::Password,
            key_file: None,
        });
        let connection = store
            .connections
            .iter_mut()
            .find(|connection| connection.id == connection_id)?;
        connection.settings.credential = Some(id.clone());
        Some((id, Secret::new(self.new_password.0.clone())))
    }

    fn write_settings(&self, settings: &mut Settings, port: Option<u16>, forwards: &[Forward]) {
        settings.port = port;
        settings.forwards = forwards.to_vec();
        settings.credential = self.credential.clone();
        settings.jump = self.jump.clone();
        settings.desktop.quality = self.desktop_quality;
    }
}

/// What saving a draft produced.
pub struct Saved {
    /// The tree with the draft written into it.
    pub store: Store,
    /// The id and secret of a credential made for a typed password, to put in the secret store.
    pub secret: Option<(String, Secret)>,
}

/// Empty means "not set"; anything else must be a valid, non-zero port.
fn parse_port(text: &str) -> Result<Option<u16>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<u16>() {
        Ok(port) if port > 0 => Ok(Some(port)),
        _ => Err(fl!("properties-error-port")),
    }
}

pub fn update(draft: &mut Draft, message: Message) -> Option<Event> {
    match message {
        Message::IconPicked(name) => draft.icon = name.map(str::to_string),
        Message::NameChanged(name) => draft.name = name,
        Message::HostChanged(host) => draft.host = host,
        Message::PortChanged(port) => draft.port = port,
        Message::ProtocolPicked(choice) => draft.protocol = choice.0,
        Message::ParentPicked(choice) => draft.parent = choice.id,
        Message::CredentialPicked(choice) => draft.credential = choice.id,
        Message::JumpPicked(choice) => draft.jump = choice.id,
        Message::ForwardsChanged(text) => draft.forwards = text,
        Message::NewUsernameChanged(user) => draft.new_username = user,
        Message::NewPasswordChanged(password) => draft.new_password = password,
        Message::QualityPicked(choice) => draft.desktop_quality = choice.0,
        Message::SavePressed => return Some(Event::Save),
        Message::DeletePressed => return Some(Event::Delete),
    }
    None
}

pub fn view<'a>(
    draft: Option<&'a Draft>,
    store: &'a Store,
    selection: Option<&'a Selection>,
) -> Element<'a, Message> {
    let (Some(draft), Some(selection)) = (draft, selection) else {
        return text(fl!("properties-empty"))
            .style(theme::secondary_text)
            .into();
    };
    let is_connection = matches!(selection, Selection::Connection(_));

    let heading = if is_connection {
        fl!("properties-connection")
    } else {
        fl!("properties-folder")
    };
    let mut fields = column![text(heading).size(theme::TEXT_TITLE)].spacing(10);

    fields = fields.push(labelled(
        fl!("properties-name"),
        theme::input("", &draft.name).on_input(Message::NameChanged),
    ));

    if is_connection {
        fields = fields.push(labelled(
            fl!("properties-host"),
            theme::input("", &draft.host).on_input(Message::HostChanged),
        ));
        fields = fields.push(labelled(
            fl!("properties-protocol"),
            theme::pick(
                PROTOCOLS,
                Some(ProtocolChoice(draft.protocol)),
                Message::ProtocolPicked,
            ),
        ));
    }

    fields = fields.push(labelled(
        fl!("properties-port"),
        theme::input(&fl!("properties-port-inherit"), &draft.port).on_input(Message::PortChanged),
    ));
    fields = fields.push(choice_field(
        fl!("properties-parent"),
        parent_choices(store, selection),
        &draft.parent,
        Message::ParentPicked,
    ));
    fields = fields.push(choice_field(
        fl!("properties-credential"),
        credential_choices(store),
        &draft.credential,
        Message::CredentialPicked,
    ));
    // With no credential chosen, a password can be typed right here.
    if is_connection && draft.credential.is_none() {
        fields = fields.push(save_password(draft));
    }
    fields = fields.push(choice_field(
        fl!("properties-jump"),
        jump_choices(store, selection),
        &draft.jump,
        Message::JumpPicked,
    ));
    fields = fields.push(labelled(
        fl!("properties-forwards"),
        theme::input(&fl!("properties-forwards-hint"), &draft.forwards)
            .on_input(Message::ForwardsChanged),
    ));
    // Folders can set it for the connections inside; a connection only if it is a remote desktop.
    if !is_connection || draft.protocol.is_desktop() {
        fields = fields.push(labelled(
            fl!("properties-quality"),
            theme::pick(
                quality_choice::CHOICES,
                Some(QualityChoice(draft.desktop_quality)),
                Message::QualityPicked,
            )
            .width(Length::Fill),
        ));
    }

    // The picker is big and rarely needed, so it comes after the fields people fill in.
    let default_icon = match selection {
        Selection::Folder(_) => icons::for_folder(None),
        Selection::Connection(_) => icons::for_connection(None, draft.protocol),
    };
    fields = fields.push(labelled(
        fl!("properties-icon"),
        icons::grid(draft.icon.as_deref(), default_icon, Message::IconPicked),
    ));

    let buttons = row![
        button(text(fl!("properties-save")))
            .style(theme::primary)
            .on_press(Message::SavePressed),
        button(text(fl!("properties-delete")))
            .style(theme::danger)
            .on_press(Message::DeletePressed),
    ]
    .spacing(8);

    // The fields scroll; the buttons stay in reach below them.
    column![
        // Room on the right, so the scrollbar does not sit on the fields.
        scrollable(
            container(fields.width(Length::Fill)).padding(iced::Padding {
                right: SCROLLBAR_ROOM,
                ..iced::Padding::ZERO
            })
        )
        .height(Length::Fill),
        buttons
    ]
    .spacing(theme::GAP)
    .into()
}

/// The fields for a password to be saved for this connection, so a credential need not be made first.
fn save_password(draft: &Draft) -> Element<'_, Message> {
    column![
        text(fl!("properties-save-password"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        theme::input(&fl!("properties-save-password-user"), &draft.new_username)
            .on_input(Message::NewUsernameChanged),
        theme::input(
            &fl!("properties-save-password-secret"),
            &draft.new_password.0
        )
        .secure(true)
        .on_input(|typed| Message::NewPasswordChanged(Hidden(typed))),
        text(fl!("properties-save-password-note"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
    ]
    .spacing(4)
    .into()
}

fn labelled<'a>(label: String, field: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![
        text(label)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        field.into()
    ]
    .spacing(2)
    .into()
}

fn choice_field<'a>(
    label: String,
    choices: Vec<Choice>,
    selected_id: &Option<String>,
    on_pick: fn(Choice) -> Message,
) -> Element<'a, Message> {
    let selected = choices
        .iter()
        .find(|choice| &choice.id == selected_id)
        .cloned();
    labelled(
        label,
        theme::pick(choices, selected, on_pick).width(Length::Fill),
    )
}

fn parent_choices(store: &Store, selection: &Selection) -> Vec<Choice> {
    let mut choices = vec![Choice {
        id: None,
        label: fl!("properties-top-level"),
    }];
    for folder in &store.folders {
        // A folder cannot be its own parent.
        if selection == &Selection::Folder(folder.id.clone()) {
            continue;
        }
        choices.push(Choice {
            id: Some(folder.id.clone()),
            label: folder.name.clone(),
        });
    }
    choices
}

fn credential_choices(store: &Store) -> Vec<Choice> {
    let mut choices = vec![inherit_choice()];
    for credential in &store.credentials {
        choices.push(Choice {
            id: Some(credential.id.clone()),
            label: credential.name.clone(),
        });
    }
    choices
}

fn jump_choices(store: &Store, selection: &Selection) -> Vec<Choice> {
    let mut choices = vec![inherit_choice()];
    for connection in &store.connections {
        // A connection cannot tunnel through itself.
        if selection == &Selection::Connection(connection.id.clone()) {
            continue;
        }
        choices.push(Choice {
            id: Some(connection.id.clone()),
            label: connection.name.clone(),
        });
    }
    choices
}

fn inherit_choice() -> Choice {
    Choice {
        id: None,
        label: fl!("properties-inherit"),
    }
}

#[cfg(test)]
#[path = "properties_tests.rs"]
mod tests;
