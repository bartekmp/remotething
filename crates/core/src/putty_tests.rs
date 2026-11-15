//! Tests of `putty.rs`.

use super::*;

fn session(name: &str, host: &str) -> PuttySession {
    PuttySession {
        name: name.to_string(),
        host: Some(host.to_string()),
        ..PuttySession::default()
    }
}

#[test]
fn session_names_are_percent_decoded() {
    assert_eq!(decode_name("My%20Server"), "My Server");
    assert_eq!(decode_name("web%2F01"), "web/01");
    assert_eq!(decode_name("100%"), "100%");
    assert_eq!(decode_name("caf%C3%A9"), "café");
    assert_eq!(decode_name("bad%zzhex"), "bad%zzhex");
}

#[test]
fn a_session_file_gives_its_fields() {
    let text = "HostName=web-01.example.net\nPortNumber=2222\nUserName=ops\nProtocol=ssh\nPublicKeyFile=/home/me/key.ppk\nFontHeight=10\n";

    let session = parse_session_file("Web%2001", text);

    assert_eq!(session.name, "Web 01");
    assert_eq!(session.host.as_deref(), Some("web-01.example.net"));
    assert_eq!(session.port, Some(2222));
    assert_eq!(session.username.as_deref(), Some("ops"));
    assert_eq!(session.key_file.as_deref(), Some("/home/me/key.ppk"));
}

#[test]
fn a_reg_export_gives_one_session_per_key() {
    let text = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\Default%20Settings]
"PortNumber"=dword:00000016
"UserName"="admin"

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\Build%20Server]
"HostName"="build.example.net"
"PortNumber"=dword:000008ae
"Protocol"="ssh"
"PublicKeyFile"="C:\\Users\\me\\key.ppk"

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\SshHostKeys]
"rsa2@22:host"="0x1"
"#;

    let sessions = parse_reg(text);

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].name, "Default Settings");
    assert_eq!(sessions[0].port, Some(22));
    assert_eq!(sessions[1].name, "Build Server");
    assert_eq!(sessions[1].port, Some(2222));
    assert_eq!(
        sessions[1].key_file.as_deref(),
        Some(r"C:\Users\me\key.ppk")
    );
}

#[test]
fn regedit_exports_are_utf16_with_a_byte_order_mark() {
    let text = "[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\a]\r\n\"HostName\"=\"ü.example.net\"\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }

    let decoded = decode_reg_bytes(&bytes);

    let sessions = parse_reg(&decoded);
    assert_eq!(sessions[0].host.as_deref(), Some("ü.example.net"));
    assert_eq!(decode_reg_bytes(b"plain"), "plain");
}

#[test]
fn only_ssh_sessions_with_a_host_are_imported_and_the_rest_say_why() {
    let mut telnet = session("Old router", "10.0.0.1");
    telnet.protocol = Some("telnet".to_string());
    let hostless = PuttySession {
        name: "Empty".to_string(),
        ..PuttySession::default()
    };

    let plan = plan(&[session("Web", "web.example.net"), telnet, hostless]);

    assert_eq!(plan.connections.len(), 1);
    assert_eq!(plan.skipped.len(), 2);
    assert!(plan.skipped[0].reason.contains("telnet"));
    assert!(plan.skipped[1].reason.contains("host"));
}

#[test]
fn default_settings_become_folder_defaults_not_a_connection() {
    let mut defaults = session("Default Settings", "");
    defaults.host = None;
    defaults.port = Some(2200);
    defaults.username = Some("admin".to_string());

    let plan = plan(&[defaults, session("Web", "web.example.net")]);

    assert_eq!(plan.connections.len(), 1);
    assert_eq!(plan.default_port, Some(2200));
    assert_eq!(plan.default_username.as_deref(), Some("admin"));
}

#[test]
fn what_cannot_be_carried_over_is_noted_not_lost_silently() {
    let mut web = session("Web", "web.example.net");
    web.proxy_method = Some(3);
    web.forwardings = Some("L8080=localhost:80".to_string());

    let plan = plan(&[web]);

    assert_eq!(plan.connections[0].notes.len(), 2);
}

#[test]
fn importing_builds_a_folder_with_connections_and_shared_credentials() {
    let mut a = session("A", "a.example.net");
    a.username = Some("ops".to_string());
    a.port = Some(2222);
    let mut b = session("B", "b.example.net");
    b.username = Some("ops".to_string());
    let mut c = session("C", "c.example.net");
    c.username = Some("deploy".to_string());
    c.key_file = Some("/keys/c.ppk".to_string());
    let plan = plan(&[a, b, c, session("D", "d.example.net")]);
    let mut store = Store::default();
    let chosen: Vec<String> = ["A", "B", "C", "D"].map(String::from).to_vec();

    let added = store.import_putty(&plan, &chosen).unwrap();

    assert_eq!(added, 4);
    assert_eq!(store.folders.len(), 1);
    assert_eq!(store.folders[0].name, "PuTTY");
    // "ops" is one credential for A and B; "deploy" with its key is another; D has none.
    assert_eq!(store.credentials.len(), 2);
    assert_eq!(
        store.connections[0].settings.credential,
        store.connections[1].settings.credential
    );
    assert_eq!(store.connections[0].settings.port, Some(2222));
    assert_eq!(store.credentials[1].kind, CredentialKind::PrivateKey);
    assert_eq!(store.connections[3].settings.credential, None);
    assert!(
        store
            .connections
            .iter()
            .all(|c| c.parent.as_deref() == Some("putty"))
    );
    assert!(store.validate().is_ok());
}

#[test]
fn only_the_chosen_sessions_are_imported_and_nothing_existing_changes() {
    let plan = plan(&[session("A", "a.example.net"), session("B", "b.example.net")]);
    let mut store = Store::default();
    store
        .import_putty(&plan, &["A".to_string(), "B".to_string()])
        .unwrap();
    let before = store.clone();

    let added = store.import_putty(&plan, &["B".to_string()]).unwrap();

    assert_eq!(added, 1);
    assert_eq!(&store.folders[..1], &before.folders[..]);
    assert_eq!(store.folders[1].id, "putty-2");
    assert_eq!(store.connections[..2], before.connections[..]);
    assert_eq!(store.connections[2].id, "b-2");
}

#[test]
fn choosing_nothing_adds_nothing_not_even_a_folder() {
    let plan = plan(&[session("A", "a.example.net")]);
    let mut store = Store::default();

    assert_eq!(store.import_putty(&plan, &[]).unwrap(), 0);

    assert!(store.folders.is_empty());
}
