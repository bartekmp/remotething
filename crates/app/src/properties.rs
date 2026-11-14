//! The panel on the right: edit the selected folder or connection.

use std::fmt;

use iced::widget::{button, column, row, text};
use iced::{Element, Length};
use remotething_core::{Protocol, Settings, Store};

use crate::i18n::fl;
use crate::icons;
use crate::theme;
use crate::tree::{Selection, protocol_label};

const PROTOCOLS: [ProtocolChoice; 3] = [
    ProtocolChoice(Protocol::Ssh),
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
    /// `None` means the default icon for the kind of item.
    pub icon: Option<String>,
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
            icon: icon.clone(),
        }
    }

    /// Returns a copy of the store with this draft written into the selected item.
    ///
    /// Nothing is changed when a field is invalid or the result would not validate.
    pub fn apply(&self, store: &Store, selection: &Selection) -> Result<Store, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(fl!("properties-error-name"));
        }
        let port = parse_port(&self.port)?;

        let mut updated = store.clone();
        match selection {
            Selection::Folder(id) => {
                let folder = updated.folders.iter_mut().find(|folder| &folder.id == id);
                if let Some(folder) = folder {
                    folder.name = name.to_string();
                    folder.parent = self.parent.clone();
                    folder.icon = self.icon.clone();
                    self.write_settings(&mut folder.settings, port);
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
                    self.write_settings(&mut connection.settings, port);
                }
            }
        }

        updated.validate().map_err(|error| error.to_string())?;
        Ok(updated)
    }

    fn write_settings(&self, settings: &mut Settings, port: Option<u16>) {
        settings.port = port;
        settings.credential = self.credential.clone();
        settings.jump = self.jump.clone();
    }
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

    let default_icon = match selection {
        Selection::Folder(_) => icons::for_folder(None),
        Selection::Connection(_) => icons::for_connection(None, draft.protocol),
    };
    fields = fields.push(labelled(
        fl!("properties-icon"),
        icons::grid(draft.icon.as_deref(), default_icon, Message::IconPicked),
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
    fields = fields.push(choice_field(
        fl!("properties-jump"),
        jump_choices(store, selection),
        &draft.jump,
        Message::JumpPicked,
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

    fields.push(buttons).width(Length::Fill).into()
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
mod tests {
    use super::*;

    fn store() -> Store {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        let text = r#"
            [[folder]]
            id = "prod"
            name = "Production"
            port = 2222

            [[connection]]
            id = "bastion"
            name = "bastion"
            protocol = "ssh"
            host = "bastion.example.net"

            [[connection]]
            id = "web-01"
            name = "web-01"
            parent = "prod"
            protocol = "ssh"
            host = "web-01.example.net"
        "#;
        std::fs::write(&path, text).unwrap();
        Store::load(&path).unwrap()
    }

    fn web() -> Selection {
        Selection::Connection("web-01".to_string())
    }

    #[test]
    fn a_draft_shows_the_items_own_values_not_inherited_ones() {
        let draft = Draft::load(&store(), &web()).unwrap();

        assert_eq!(draft.name, "web-01");
        assert_eq!(draft.host, "web-01.example.net");
        // The folder sets 2222, but the connection itself sets nothing.
        assert_eq!(draft.port, "");
        assert_eq!(draft.parent.as_deref(), Some("prod"));
    }

    #[test]
    fn a_missing_item_has_no_draft() {
        let gone = Selection::Connection("gone".to_string());

        assert_eq!(Draft::load(&store(), &gone), None);
    }

    #[test]
    fn applying_writes_the_fields_into_a_copy() {
        let original = store();
        let mut draft = Draft::load(&original, &web()).unwrap();
        draft.name = "web".to_string();
        draft.port = "8022".to_string();
        draft.jump = Some("bastion".to_string());
        draft.protocol = Protocol::Rdp;

        let updated = draft.apply(&original, &web()).unwrap();

        let connection = &updated.connections[1];
        assert_eq!(connection.name, "web");
        assert_eq!(connection.settings.port, Some(8022));
        assert_eq!(connection.settings.jump.as_deref(), Some("bastion"));
        assert_eq!(connection.protocol, Protocol::Rdp);
        assert_eq!(original.connections[1].name, "web-01");
    }

    #[test]
    fn picking_an_icon_saves_its_name_and_the_default_clears_it() {
        let original = store();
        let mut draft = Draft::load(&original, &web()).unwrap();
        assert_eq!(draft.icon, None);

        update(&mut draft, Message::IconPicked(Some("database")));
        let updated = draft.apply(&original, &web()).unwrap();
        assert_eq!(updated.connections[1].icon.as_deref(), Some("database"));

        let mut draft = Draft::load(&updated, &web()).unwrap();
        assert_eq!(draft.icon.as_deref(), Some("database"));
        update(&mut draft, Message::IconPicked(None));
        let cleared = draft.apply(&updated, &web()).unwrap();
        assert_eq!(cleared.connections[1].icon, None);
    }

    #[test]
    fn clearing_the_port_goes_back_to_inheriting() {
        let folder = Selection::Folder("prod".to_string());
        let original = store();
        let mut draft = Draft::load(&original, &folder).unwrap();
        assert_eq!(draft.port, "2222");
        draft.port = " ".to_string();

        let updated = draft.apply(&original, &folder).unwrap();

        assert_eq!(updated.folders[0].settings.port, None);
    }

    #[test]
    fn a_bad_port_is_refused() {
        let original = store();
        let mut draft = Draft::load(&original, &web()).unwrap();

        for bad in ["ssh", "0", "70000", "-1"] {
            draft.port = bad.to_string();
            assert!(draft.apply(&original, &web()).is_err(), "port {bad}");
        }
    }

    #[test]
    fn an_empty_name_or_host_is_refused() {
        let original = store();
        let mut draft = Draft::load(&original, &web()).unwrap();

        draft.name = "  ".to_string();
        assert!(draft.apply(&original, &web()).is_err());

        draft.name = "web".to_string();
        draft.host = String::new();
        assert!(draft.apply(&original, &web()).is_err());
    }

    #[test]
    fn a_change_that_breaks_the_tree_is_refused() {
        let folder = Selection::Folder("prod".to_string());
        let original = store();
        let mut draft = Draft::load(&original, &folder).unwrap();
        draft.parent = Some("prod".to_string());

        assert!(draft.apply(&original, &folder).is_err());
    }

    #[test]
    fn a_folder_is_not_offered_as_its_own_parent() {
        let folder = Selection::Folder("prod".to_string());

        let choices = parent_choices(&store(), &folder);

        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].id, None);
    }

    #[test]
    fn a_connection_is_not_offered_as_its_own_jump_host() {
        let ids: Vec<Option<String>> = jump_choices(&store(), &web())
            .into_iter()
            .map(|choice| choice.id)
            .collect();

        assert_eq!(ids, [None, Some("bastion".to_string())]);
    }

    #[test]
    fn save_and_delete_are_reported_to_the_window() {
        let mut draft = Draft::load(&store(), &web()).unwrap();

        assert_eq!(update(&mut draft, Message::SavePressed), Some(Event::Save));
        assert_eq!(
            update(&mut draft, Message::DeletePressed),
            Some(Event::Delete)
        );
        assert_eq!(
            update(&mut draft, Message::NameChanged("x".to_string())),
            None
        );
        assert_eq!(draft.name, "x");
    }
}
