//! Tests of `app.rs`.

use super::*;
use crate::credential_actions::apply_secret;
use crate::credentials::SecretAction;
use crate::palette;
use crate::tabs;
use remotething_secrets::{MemoryStore, Secret};

const ONE_CONNECTION: &str = r#"
    [[folder]]
    id = "prod"
    name = "Production"

    [[connection]]
    id = "web-01"
    name = "web-01"
    parent = "prod"
    protocol = "ssh"
    host = "web-01.example.net"
"#;

/// A window state backed by a real file in a temporary directory.
fn state(directory: &tempfile::TempDir, content: &str) -> State {
    let path = directory.path().join("nested").join("connections.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    let store = Store::load(&path).unwrap();
    State::new(path, Ok(store), Arc::new(MemoryStore::new()), None)
}

// The returned tasks need a running app; these tests check the synchronous part.
fn tree(state: &mut State, message: tree::Message) {
    let _ = update(state, Message::Tree(message));
}

fn properties(state: &mut State, message: properties::Message) {
    let _ = update(state, Message::Properties(message));
}

fn credentials(state: &mut State, message: credentials::Message) {
    let _ = update(state, Message::Credentials(message));
}

fn on_disk(state: &State) -> Store {
    Store::load(&state.path).unwrap()
}

#[test]
fn opening_a_connection_in_the_tree_opens_a_tab() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    tree(
        &mut state,
        tree::Message::ConnectionOpened("web-01".to_string()),
    );

    assert_eq!(state.tabs.open_ids(), ["web-01"]);
}

#[test]
fn selecting_a_connection_loads_its_properties_without_opening_a_tab() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    tree(
        &mut state,
        tree::Message::ConnectionSelected("web-01".to_string()),
    );

    assert!(state.tabs.open_ids().is_empty());
    assert_eq!(state.draft.as_ref().unwrap().host, "web-01.example.net");
}

#[test]
fn a_new_connection_lands_in_the_selected_folder_and_on_disk() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    tree(
        &mut state,
        tree::Message::FolderSelected("prod".to_string()),
    );

    tree(&mut state, tree::Message::NewConnectionPressed);

    let saved = on_disk(&state);
    let added = saved.connections.last().unwrap();
    assert_eq!(added.id, "new-connection");
    assert_eq!(added.parent.as_deref(), Some("prod"));
    // The new item is selected, ready to be renamed.
    assert_eq!(
        state.tree.selected(),
        Some(&Selection::Connection(added.id.clone()))
    );
    assert!(state.draft.is_some());
    assert!(state.properties_open);
}

#[test]
fn a_new_folder_with_nothing_selected_goes_to_the_top_level() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, "");

    tree(&mut state, tree::Message::NewFolderPressed);
    tree(
        &mut state,
        tree::Message::FolderSelected("new-folder".to_string()),
    );
    tree(&mut state, tree::Message::NewFolderPressed);

    let saved = on_disk(&state);
    assert_eq!(saved.folders[0].parent, None);
    assert_eq!(saved.folders[1].id, "new-folder-2");
    assert_eq!(saved.folders[1].parent.as_deref(), Some("new-folder"));
}

#[test]
fn saving_an_edit_writes_it_to_disk() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    tree(
        &mut state,
        tree::Message::ConnectionSelected("web-01".to_string()),
    );

    properties(
        &mut state,
        properties::Message::HostChanged("10.0.0.5".to_string()),
    );
    properties(&mut state, properties::Message::SavePressed);

    assert_eq!(on_disk(&state).connections[0].host, "10.0.0.5");
    assert_eq!(state.error, None);
}

#[test]
fn a_refused_edit_changes_nothing_and_says_why() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    tree(
        &mut state,
        tree::Message::ConnectionSelected("web-01".to_string()),
    );

    properties(
        &mut state,
        properties::Message::PortChanged("not a port".to_string()),
    );
    properties(&mut state, properties::Message::SavePressed);

    assert!(state.error.is_some());
    assert_eq!(on_disk(&state).connections[0].settings.port, None);
}

#[test]
fn deleting_a_connection_removes_it_and_clears_the_panel() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    tree(
        &mut state,
        tree::Message::ConnectionSelected("web-01".to_string()),
    );

    properties(&mut state, properties::Message::DeletePressed);
    // Nothing goes before the user says yes.
    assert!(state.removal.is_some());
    assert_eq!(on_disk(&state).connections.len(), 1);

    let _ = update(&mut state, Message::Removal(removal::Message::Confirmed));

    assert!(on_disk(&state).connections.is_empty());
    assert_eq!(state.tree.selected(), None);
    assert_eq!(state.draft, None);
}

#[test]
fn saying_no_keeps_the_connection() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    tree(
        &mut state,
        tree::Message::ConnectionSelected("web-01".to_string()),
    );

    properties(&mut state, properties::Message::DeletePressed);
    let _ = update(&mut state, Message::Removal(removal::Message::Cancelled));

    assert_eq!(on_disk(&state).connections.len(), 1);
    assert!(state.removal.is_none());
}

#[test]
fn a_credential_is_removed_only_after_a_yes() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    credentials(&mut state, credentials::Message::NewPressed);
    let id = state.credentials.selected().unwrap().to_string();

    credentials(&mut state, credentials::Message::DeletePressed);
    assert!(state.removal.is_some());
    assert_eq!(on_disk(&state).credentials.len(), 1);

    let _ = update(&mut state, Message::Removal(removal::Message::Confirmed));
    assert!(on_disk(&state).credentials.iter().all(|c| c.id != id));
}

#[test]
fn removing_a_folder_with_content_asks_first() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    tree(
        &mut state,
        tree::Message::FolderSelected("prod".to_string()),
    );

    properties(&mut state, properties::Message::DeletePressed);

    assert!(state.removal.is_some());
    assert_eq!(on_disk(&state).folders.len(), 1);
}

#[test]
fn confirming_removes_the_folder_and_what_is_inside() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    tree(
        &mut state,
        tree::Message::FolderSelected("prod".to_string()),
    );
    properties(&mut state, properties::Message::DeletePressed);

    let _ = update(&mut state, Message::Removal(removal::Message::Confirmed));

    assert!(on_disk(&state).folders.is_empty());
    assert!(on_disk(&state).connections.is_empty());
    assert!(state.removal.is_none());
}

#[test]
fn declining_keeps_the_folder() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    tree(
        &mut state,
        tree::Message::FolderSelected("prod".to_string()),
    );
    properties(&mut state, properties::Message::DeletePressed);

    let _ = update(&mut state, Message::Removal(removal::Message::Cancelled));

    assert_eq!(on_disk(&state).folders.len(), 1);
    assert!(state.removal.is_none());
}

#[test]
fn the_first_save_creates_the_directory() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fresh").join("connections.toml");
    let secrets = Arc::new(MemoryStore::new());
    let mut state = State::new(path.clone(), Ok(Store::default()), secrets, None);

    tree(&mut state, tree::Message::NewFolderPressed);

    assert!(path.exists());
}

#[test]
fn a_load_failure_starts_with_an_empty_tree_and_shows_the_reason() {
    let secrets = Arc::new(MemoryStore::new());
    let broken = Err("file is broken".to_string());
    let state = State::new(PathBuf::from("unused"), broken, secrets, None);

    assert_eq!(state.store, Store::default());
    assert!(state.error.as_deref().unwrap().contains("file is broken"));
}

const WITH_CREDENTIAL: &str = r#"
    [[credential]]
    id = "cred-ops"
    name = "Ops team"
    kind = "password"

    [[connection]]
    id = "web-01"
    name = "web-01"
    protocol = "ssh"
    host = "web-01.example.net"
    credential = "cred-ops"
"#;

#[test]
fn the_credentials_button_opens_a_tab_and_closing_it_returns() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    tree(&mut state, tree::Message::CredentialsPressed);
    assert_eq!(state.centre(), Centre::Credentials);

    state.tabs.close_page(crate::tabs::Page::Credentials);
    assert_eq!(state.centre(), Centre::Sessions);
}

#[test]
fn a_new_credential_is_saved_and_selected() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, "");

    credentials(&mut state, credentials::Message::NewPressed);

    let saved = on_disk(&state);
    assert_eq!(saved.credentials[0].id, "new-credential");
    assert_eq!(state.credentials.selected(), Some("new-credential"));
}

#[test]
fn saving_a_credential_writes_the_description_and_keeps_the_secret_out_of_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, WITH_CREDENTIAL);
    credentials(
        &mut state,
        credentials::Message::Selected("cred-ops".to_string()),
    );
    credentials(
        &mut state,
        credentials::Message::UsernameChanged("ops".to_string()),
    );
    let typed = credentials::Hidden("hunter2".to_string());
    credentials(&mut state, credentials::Message::SecretChanged(typed));

    credentials(&mut state, credentials::Message::SavePressed);

    assert_eq!(
        on_disk(&state).credentials[0].username.as_deref(),
        Some("ops")
    );
    let file_text = std::fs::read_to_string(&state.path).unwrap();
    assert!(!file_text.contains("hunter2"));
    // The typed secret has left the form on its way to the store.
    assert_eq!(state.credentials.draft().unwrap().secret, "");
}

#[test]
fn a_credential_in_use_is_not_deleted() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, WITH_CREDENTIAL);
    credentials(
        &mut state,
        credentials::Message::Selected("cred-ops".to_string()),
    );

    credentials(&mut state, credentials::Message::DeletePressed);

    assert_eq!(on_disk(&state).credentials.len(), 1);
    assert!(state.error.as_deref().unwrap().contains("web-01"));
}

#[test]
fn secret_actions_reach_the_store() {
    let secrets = MemoryStore::new();
    let set = SecretAction::Set(Secret::new("hunter2".to_string()));

    apply_secret(&secrets, "cred-ops", set).unwrap();
    let stored = secrets.get("cred-ops").unwrap().unwrap();
    assert_eq!(stored.expose(), "hunter2");

    apply_secret(&secrets, "cred-ops", SecretAction::Keep).unwrap();
    assert!(secrets.get("cred-ops").unwrap().is_some());

    apply_secret(&secrets, "cred-ops", SecretAction::Delete).unwrap();
    assert!(secrets.get("cred-ops").unwrap().is_none());
}

#[test]
fn a_failed_secret_save_is_shown() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    let _ = update(
        &mut state,
        Message::SecretStored(Err("keyring locked".to_string())),
    );

    assert!(state.error.as_deref().unwrap().contains("keyring locked"));
}

#[test]
fn a_start_up_notice_is_shown_when_loading_worked() {
    let secrets = Arc::new(MemoryStore::new());
    let notice = Some("no keyring".to_string());

    let state = State::new(
        PathBuf::from("unused"),
        Ok(Store::default()),
        secrets,
        notice,
    );

    assert_eq!(state.error.as_deref(), Some("no keyring"));
}

#[test]
fn the_settings_button_opens_a_tab_and_closing_it_returns() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    let _ = update(&mut state, Message::SettingsPressed);
    assert_eq!(state.centre(), Centre::Settings);

    state.tabs.close_page(crate::tabs::Page::Settings);
    assert_eq!(state.centre(), Centre::Sessions);
}

#[test]
fn a_theme_change_applies_at_once_and_is_saved_beside_the_connections_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    assert!(!theme(&state).extended_palette().is_dark);

    let _ = update(&mut state, Message::SystemModeChanged(Mode::Dark));
    assert!(theme(&state).extended_palette().is_dark);
    let _ = update(&mut state, Message::SystemModeChanged(Mode::Light));

    let dark = Preferences {
        theme: ThemeChoice::Dark,
        ..Preferences::default()
    };
    let state_with_dark = State::new(
        state.path.clone(),
        Ok(Store::default()),
        Arc::new(MemoryStore::new()),
        None,
    )
    .with_preferences(dark.clone());
    assert!(theme(&state_with_dark).extended_palette().is_dark);

    state.preferences = dark.clone();
    state.save_preferences();

    let saved = preferences::load(&preferences::path_beside(&state.path)).unwrap();
    assert_eq!(saved, dark);
}

fn locked_state(directory: &tempfile::TempDir) -> State {
    let vault_file = directory.path().join("secrets.toml");
    state(directory, ONE_CONNECTION).with_vault_prompt(vault_file, "no keyring".to_string())
}

#[test]
fn skipping_the_master_password_keeps_secrets_in_memory_and_says_so() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = locked_state(&directory);

    let _ = update(&mut state, Message::Vault(vault::Message::SkipPressed));

    assert!(state.vault.is_none());
    assert_eq!(state.error, Some(fl!("vault-skipped")));
}

#[test]
fn an_unlocked_store_replaces_the_temporary_one_and_closes_the_prompt() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = locked_state(&directory);
    let unlocked = MemoryStore::new();
    unlocked
        .set("cred-ops", &Secret::new("hunter2".to_string()))
        .unwrap();

    let opened = Message::VaultOpened(Ok(Unlocked(Arc::new(unlocked))));
    let _ = update(&mut state, opened);

    assert!(state.vault.is_none());
    assert!(state.secrets.get("cred-ops").unwrap().is_some());
}

#[test]
fn a_failed_unlock_keeps_the_prompt_open() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = locked_state(&directory);

    let failed = Message::VaultOpened(Err("wrong password".to_string()));
    let _ = update(&mut state, failed);

    assert!(state.vault.is_some());
}

#[test]
fn collapsing_a_sidebar_is_remembered_in_the_settings_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    let saved = |state: &State| preferences::load(&preferences::path_beside(&state.path)).unwrap();

    let _ = update(&mut state, Message::TreeCollapseToggled);
    assert!(state.preferences.tree_collapsed);
    assert!(saved(&state).tree_collapsed);

    let _ = update(&mut state, Message::TreeCollapseToggled);
    assert!(!saved(&state).tree_collapsed);
}

#[test]
fn applying_a_putty_import_adds_a_folder_and_returns_to_the_sessions() {
    use remotething_core::{PuttySession, plan_putty_import};

    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    state.open_settings(Centre::Putty);
    let session = PuttySession {
        name: "Build".to_string(),
        host: Some("build.example.net".to_string()),
        username: Some("ci".to_string()),
        ..PuttySession::default()
    };
    let plan = plan_putty_import(&[session]);

    state.apply_putty(&plan, &["Build".to_string()]);

    let saved = on_disk(&state);
    assert_eq!(saved.folders.last().unwrap().name, "PuTTY");
    assert_eq!(saved.connections.last().unwrap().host, "build.example.net");
    assert_eq!(
        saved.credentials.last().unwrap().username.as_deref(),
        Some("ci")
    );
    assert_eq!(state.centre(), Centre::Sessions);
    assert!(state.notice.is_some());
}

#[test]
fn the_putty_screen_is_reached_from_settings_and_goes_back_to_it() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    let _ = update(
        &mut state,
        Message::Settings(settings::Message::ImportPuttyPressed),
    );
    assert_eq!(state.centre(), Centre::Putty);

    let _ = update(
        &mut state,
        Message::Putty(putty_import::Message::BackPressed),
    );
    assert_eq!(state.centre(), Centre::Settings);
}

#[test]
fn a_picked_folder_is_saved_and_used_by_its_feature() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    let chosen = directory.path().join("casts");

    let _ = update(
        &mut state,
        Message::FolderPicked(
            settings::FolderKind::ConsoleRecordings,
            Some(chosen.clone()),
        ),
    );

    assert_eq!(features::console_recordings_folder(), chosen);
    let saved = preferences::load(&preferences::path_beside(&state.path)).unwrap();
    assert_eq!(saved.folders.console_recordings, Some(chosen));
    // A cancelled dialog changes nothing.
    let _ = update(
        &mut state,
        Message::FolderPicked(settings::FolderKind::Screenshots, None),
    );
    assert!(state.preferences.folders.screenshots.is_none());
}

#[test]
fn the_chosen_settings_page_is_kept_while_the_screen_is_open() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    assert_eq!(state.settings_page, settings::Tab::General);

    let _ = update(
        &mut state,
        Message::Settings(settings::Message::PagePicked(settings::Tab::Features)),
    );

    assert_eq!(state.settings_page, settings::Tab::Features);
}

#[test]
fn importing_wsl_distributions_adds_them_to_a_wsl_folder_and_saves() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    let plan = remotething_core::plan_wsl(&[remotething_core::WslDistro {
        name: "Ubuntu".to_string(),
        running: true,
    }]);

    state.apply_wsl(&plan, &["Ubuntu".to_string()]);

    let saved = on_disk(&state);
    let added = saved
        .connections
        .iter()
        .find(|c| c.name == "Ubuntu")
        .unwrap();
    assert_eq!(added.host, "Ubuntu");
    assert_eq!(added.protocol, remotething_core::Protocol::Wsl);
    assert!(saved.folders.iter().any(|folder| folder.name == "WSL"));
    assert_eq!(state.centre(), Centre::Sessions);
    assert!(state.notice.is_some());
}

#[test]
fn the_wsl_screen_closes_back_to_the_settings() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    state.open_settings(Centre::Wsl);

    let _ = update(&mut state, Message::Wsl(wsl_import::Message::BackPressed));

    assert_eq!(state.centre(), Centre::Settings);
}

#[test]
fn saving_a_connection_with_a_typed_password_adds_its_credential_to_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    tree(
        &mut state,
        tree::Message::ConnectionSelected("web-01".to_string()),
    );
    properties(
        &mut state,
        properties::Message::NewUsernameChanged("ops".to_string()),
    );
    properties(
        &mut state,
        properties::Message::NewPasswordChanged(credentials::Hidden("s3cret".to_string())),
    );

    let _task = update(
        &mut state,
        Message::Properties(properties::Message::SavePressed),
    );

    let saved = on_disk(&state);
    let credential = saved
        .credentials
        .iter()
        .find(|c| c.name == "web-01")
        .unwrap();
    assert_eq!(credential.username.as_deref(), Some("ops"));
    let connection = saved.connections.iter().find(|c| c.id == "web-01").unwrap();
    assert_eq!(
        connection.settings.credential.as_deref(),
        Some(credential.id.as_str())
    );
    assert!(
        !std::fs::read_to_string(&state.path)
            .unwrap()
            .contains("s3cret")
    );
}

fn key(state: &mut State, key: keyboard::Key, modifiers: keyboard::Modifiers) {
    let _ = update(
        state,
        Message::KeyPressed {
            key,
            physical: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            modifiers,
            text: None,
            captured: false,
        },
    );
}

#[test]
fn the_palette_opens_with_its_shortcut_and_closes_with_escape_or_the_shortcut() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    let ctrl_shift = keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT;
    let escape = keyboard::Key::Named(keyboard::key::Named::Escape);

    key(&mut state, keyboard::Key::Character("p".into()), ctrl_shift);
    assert!(state.palette.is_some());
    key(&mut state, escape, keyboard::Modifiers::empty());
    assert!(state.palette.is_none());

    key(&mut state, keyboard::Key::Character("p".into()), ctrl_shift);
    key(&mut state, keyboard::Key::Character("p".into()), ctrl_shift);
    assert!(state.palette.is_none());
}

#[test]
fn choosing_a_connection_in_the_palette_opens_it_and_closes_the_palette() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    state.open_settings(Centre::Settings);
    state.palette = Some(palette::State::default());

    let _ = update(
        &mut state,
        Message::Palette(palette::Message::Typed("web".into())),
    );
    let _ = update(&mut state, Message::Palette(palette::Message::Submitted));

    assert!(state.palette.is_none());
    assert_eq!(state.tabs.open_ids(), ["web-01"]);
    assert_eq!(state.centre(), Centre::Sessions);
}

#[test]
fn choosing_settings_in_the_palette_shows_the_settings() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    state.palette = Some(palette::State::default());

    let _ = update(
        &mut state,
        Message::Palette(palette::Message::Typed("settings".into())),
    );
    let _ = update(&mut state, Message::Palette(palette::Message::Submitted));

    assert_eq!(state.centre(), Centre::Settings);
}

#[test]
fn tab_keys_switch_tabs_and_bring_the_sessions_back() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    let _ = state.tabs.open_launcher();
    let _ = state.tabs.open_launcher();
    state.open_settings(Centre::Settings);

    key(
        &mut state,
        keyboard::Key::Character("1".into()),
        keyboard::Modifiers::ALT,
    );

    assert_eq!(state.centre(), Centre::Sessions);
    assert_eq!(state.tabs.active, 0);
}

#[test]
fn folding_the_tree_glides_and_unfolding_glides_back() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    assert!(!state.is_animating());

    let _ = update(&mut state, Message::TreeCollapseToggled);

    assert_eq!(state.tree_slide.target(), 0.0);
    // The first frame has not come yet, so it is still running and the panel is nearly full.
    assert!(state.is_animating());
    assert!(state.tree_slide.value(state.now) > 0.5);

    state.now = std::time::Instant::now() + crate::motion::DURATION;
    assert!(!state.is_animating());
    assert_eq!(state.tree_slide.value(state.now), 0.0);

    let _ = update(&mut state, Message::TreeCollapseToggled);
    assert_eq!(state.tree_slide.target(), 1.0);
}

#[test]
fn with_animations_off_the_panels_jump() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    state.preferences.animations = false;

    let _ = update(&mut state, Message::TreeCollapseToggled);
    state.properties_open = true;
    let later = std::time::Instant::now() + crate::motion::DURATION;
    let _ = update(&mut state, Message::Frame(later));

    assert_eq!(state.tree_slide.value(state.now), 0.0);
    assert_eq!(state.properties_slide.value(state.now), 1.0);
    assert!(!state.is_animating());
}

#[test]
fn the_properties_panel_slides_in_and_out_with_its_flag() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    tree(&mut state, tree::Message::NewConnectionPressed);
    assert!(state.properties_open);
    assert_eq!(state.properties_slide.target(), 1.0);

    let _ = update(&mut state, Message::PropertiesClosed);
    assert_eq!(state.properties_slide.target(), 0.0);
}

#[test]
fn a_tab_dropped_in_a_new_place_glows_and_fades() {
    let mut tabs = tabs::State::default();
    let _ = tabs.open_launcher();
    let _ = tabs.open_launcher();
    let moved = tabs.tabs[0].id;
    tabs.dragged = Some(moved);
    tabs.hovered = Some(1);

    tabs.release(true);

    let now = std::time::Instant::now();
    assert!(tabs.glow_of(moved, now) > 0.5);
    assert_eq!(tabs.glow_of(tabs.tabs[0].id.wrapping_add(99), now), 0.0);
    assert!(tabs.is_glowing(now));
    assert!(!tabs.is_glowing(now + crate::motion::GLOW));
}

#[test]
fn a_drop_without_animations_does_not_glow() {
    let mut tabs = tabs::State::default();
    let _ = tabs.open_launcher();
    let _ = tabs.open_launcher();
    tabs.dragged = Some(tabs.tabs[0].id);
    tabs.hovered = Some(1);

    tabs.release(false);

    assert!(!tabs.is_glowing(std::time::Instant::now()));
}

#[test]
fn f1_shows_the_shortcuts_and_escape_or_f1_hides_them() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    let f1 = keyboard::Key::Named(keyboard::key::Named::F1);
    let escape = keyboard::Key::Named(keyboard::key::Named::Escape);

    key(&mut state, f1.clone(), keyboard::Modifiers::empty());
    assert!(state.shortcuts_open);
    key(&mut state, escape, keyboard::Modifiers::empty());
    assert!(!state.shortcuts_open);

    key(&mut state, f1.clone(), keyboard::Modifiers::empty());
    key(&mut state, f1, keyboard::Modifiers::empty());
    assert!(!state.shortcuts_open);
}

#[test]
fn zooming_the_terminal_changes_the_saved_size_within_the_limits() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, "");
    let start = state.preferences.terminal.font_size;

    state.zoom_terminal(1);
    assert_eq!(state.preferences.terminal.font_size, start + 1);

    state.zoom_terminal(100);
    assert_eq!(
        state.preferences.terminal.font_size,
        *crate::terminal_view::FONT_SIZES.end()
    );
    state.zoom_terminal(-100);
    assert_eq!(
        state.preferences.terminal.font_size,
        *crate::terminal_view::FONT_SIZES.start()
    );
}

#[test]
fn the_interface_size_is_saved_and_scales_the_window() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, "");
    assert_eq!(scale_factor(&state), 1.0);

    // Picking an option is a plain preference change; it is not sent through `update`,
    // which would change the screen factor that other tests share.
    let mut preferences = state.preferences.clone();
    let event = settings::update(
        &mut preferences,
        settings::Message::InterfaceScalePicked(settings::ScaleOption::of(150)),
    );
    assert!(matches!(event, settings::Event::Changed));
    state.preferences = preferences;
    assert_eq!(scale_factor(&state), 1.5);

    state.preferences.interface_scale = 0;
    assert_eq!(scale_factor(&state), 0.5);
}

#[test]
fn the_window_size_shows_while_resizing_and_goes_when_the_resizing_stops() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, "");
    assert!(!state.size_shown);

    // The same size again is not a resize.
    let same = state.window;
    let _ = update(&mut state, Message::WindowResized(same));
    assert!(!state.size_shown);

    let _ = update(&mut state, Message::WindowResized(Size::new(900.0, 700.0)));
    let _ = update(&mut state, Message::WindowResized(Size::new(910.0, 700.0)));
    assert!(state.size_shown);

    // The timer of the first resize is out of date: the second one is still going.
    let _ = update(&mut state, Message::ResizeSettled(1));
    assert!(state.size_shown);
    let _ = update(&mut state, Message::ResizeSettled(2));
    assert!(!state.size_shown);
}

#[test]
fn the_window_size_and_maximized_state_are_kept_for_the_next_start() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, "");
    state.window = iced::Size::new(900.0, 700.0);

    state.remember_window(false);
    state.window = iced::Size::new(1920.0, 1080.0);
    state.remember_window(true);

    let saved = preferences::load(&preferences::path_beside(&state.path)).unwrap();
    let placement = saved.window.unwrap();
    // Maximized keeps the size from before, so un-maximizing goes back to it.
    assert_eq!((placement.width, placement.height), (900, 700));
    assert!(placement.maximized);
}

#[test]
fn a_nonsense_window_size_is_not_used() {
    let tiny = preferences::WindowPlacement {
        width: 3,
        height: 3,
        maximized: true,
    };

    assert!(!tiny.is_usable());
}

#[test]
fn credentials_and_settings_are_tabs_that_are_reused_and_closed_like_any_tab() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    let _ = state.tabs.open_launcher();

    tree(&mut state, tree::Message::CredentialsPressed);
    let _ = update(&mut state, Message::SettingsPressed);
    assert_eq!(state.tabs.len(), 3);
    assert_eq!(state.centre(), Centre::Settings);

    // Pressing the button of a page that is open brings its tab forward, not a second one.
    tree(&mut state, tree::Message::CredentialsPressed);
    assert_eq!(state.tabs.len(), 3);
    assert_eq!(state.centre(), Centre::Credentials);

    // Closing the tab in front shows what was behind it: a page can be left like any tab.
    state.tabs.close_front();
    assert_eq!(state.tabs.len(), 2);
    assert_ne!(state.centre(), Centre::Credentials);
}

#[test]
fn closing_a_new_connection_that_was_never_filled_in_removes_it() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    tree(&mut state, tree::Message::NewConnectionPressed);
    assert_eq!(on_disk(&state).connections.len(), 2);
    let _ = update(&mut state, Message::PropertiesClosed);

    assert_eq!(on_disk(&state).connections.len(), 1);
    assert_eq!(state.tree.selected(), None);
}

#[test]
fn a_new_connection_that_was_saved_stays_when_the_panel_closes() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    tree(&mut state, tree::Message::NewConnectionPressed);
    properties(
        &mut state,
        properties::Message::NameChanged("db-01".to_string()),
    );
    properties(&mut state, properties::Message::SavePressed);
    let _ = update(&mut state, Message::PropertiesClosed);

    assert_eq!(on_disk(&state).connections.len(), 2);
}

#[test]
fn an_old_connection_is_never_removed_by_closing_its_panel() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);

    tree(
        &mut state,
        tree::Message::ConnectionSelected("web-01".to_string()),
    );
    let _ = update(&mut state, Message::PropertiesClosed);

    assert_eq!(on_disk(&state).connections.len(), 1);
}

#[test]
fn a_sign_in_form_in_the_front_tab_is_a_form_where_tab_moves_between_fields() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory, ONE_CONNECTION);
    assert!(!state.tabs.front_is_asking());

    // No credential: the tab asks for a user name and a password.
    let _ = state.open("web-01");

    assert!(state.tabs.front_is_asking());
}
