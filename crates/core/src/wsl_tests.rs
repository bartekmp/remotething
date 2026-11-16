//! Tests of `wsl.rs`.

use std::collections::HashMap;

use super::*;

fn utf16(text: &str, with_bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if with_bom {
        bytes.extend([0xFF, 0xFE]);
    }
    for unit in text.encode_utf16() {
        bytes.extend(unit.to_le_bytes());
    }
    bytes
}

#[test]
fn what_wsl_prints_is_decoded_from_utf16_with_or_without_a_byte_order_mark() {
    let text = "Ubuntu-24.04\r\nDebian\r\n";

    assert_eq!(decode_wsl_output(&utf16(text, true)), text);
    assert_eq!(decode_wsl_output(&utf16(text, false)), text);
}

#[test]
fn plain_utf8_from_inside_a_distribution_is_left_alone() {
    assert_eq!(decode_wsl_output("zażółć\n".as_bytes()), "zażółć\n");
    assert_eq!(decode_wsl_output(b""), "");
    assert_eq!(decode_wsl_output(b"ab"), "ab");
}

#[test]
fn running_distributions_are_the_non_empty_lines() {
    let names = parse_running("Ubuntu-24.04\r\n\r\n  Debian \r\n");

    assert_eq!(names, ["Ubuntu-24.04", "Debian"]);
    assert!(parse_running("").is_empty());
}

#[test]
fn the_address_is_the_first_ipv4_one() {
    assert_eq!(
        first_ipv4("172.28.144.5 fd00::1 10.0.0.2 \n").as_deref(),
        Some("172.28.144.5")
    );
    assert_eq!(first_ipv4("fd00::1\n"), None);
    assert_eq!(first_ipv4(""), None);
}

#[test]
fn the_user_is_the_first_line_if_it_looks_like_a_name() {
    assert_eq!(parse_user("bartek\n").as_deref(), Some("bartek"));
    assert_eq!(parse_user("\n ops.admin \n").as_deref(), Some("ops.admin"));
    assert_eq!(parse_user("wsl: something went wrong\n"), None);
    assert_eq!(parse_user(""), None);
}

/// A fake `wsl.exe`: answers by the arguments it is given.
struct Fake {
    answers: HashMap<String, Vec<u8>>,
    open: Vec<&'static str>,
}

impl WslRunner for Fake {
    fn run(&mut self, arguments: &[&str]) -> std::result::Result<Vec<u8>, String> {
        self.answers
            .get(&arguments.join(" "))
            .cloned()
            .ok_or_else(|| format!("no answer for {arguments:?}"))
    }

    fn ssh_answers(&mut self, address: &str) -> bool {
        self.open.contains(&address)
    }
}

fn fake() -> Fake {
    let mut answers = HashMap::new();
    answers.insert(
        "--list --running --quiet".to_string(),
        utf16("Ubuntu\r\nDebian\r\ndocker-desktop\r\n", true),
    );
    answers.insert(
        "-d Ubuntu -- hostname -I".to_string(),
        b"172.28.0.5 \n".to_vec(),
    );
    answers.insert("-d Ubuntu -- whoami".to_string(), b"bartek\n".to_vec());
    answers.insert(
        "-d Debian -- hostname -I".to_string(),
        b"172.28.0.9\n".to_vec(),
    );
    answers.insert("-d Debian -- whoami".to_string(), b"root\n".to_vec());
    // docker-desktop answers nothing.
    Fake {
        answers,
        open: vec!["172.28.0.5"],
    }
}

#[test]
fn every_running_distribution_gets_its_address_user_and_ssh_state() {
    let found = discover(&mut fake()).unwrap();

    assert_eq!(found.len(), 3);
    assert_eq!(
        found[0],
        WslDistro {
            name: "Ubuntu".to_string(),
            address: Some("172.28.0.5".to_string()),
            user: Some("bartek".to_string()),
            ssh_answers: Some(true),
        }
    );
    assert_eq!(found[1].ssh_answers, Some(false));
    // A distribution that does not answer is still listed, without an address.
    assert_eq!(found[2].address, None);
    assert_eq!(found[2].ssh_answers, None);
}

#[test]
fn a_failing_list_is_an_error() {
    let mut nothing = Fake {
        answers: HashMap::new(),
        open: Vec::new(),
    };

    assert!(discover(&mut nothing).is_err());
}

#[test]
fn the_plan_skips_docker_and_what_has_no_address_and_warns_about_ssh() {
    let plan = plan_wsl(&discover(&mut fake()).unwrap());

    let names: Vec<&str> = plan.connections.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Ubuntu", "Debian"]);
    assert!(plan.connections[0].notes.is_empty());
    assert!(plan.connections[1].notes[0].contains("port 22"));
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(plan.skipped[0].name, "docker-desktop");
    // Root is not offered as the user to sign in as.
    assert_eq!(plan.connections[1].user, None);
    assert_eq!(plan.connections[0].user.as_deref(), Some("bartek"));
}

fn plan() -> WslPlan {
    plan_wsl(&discover(&mut fake()).unwrap())
}

#[test]
fn importing_adds_the_chosen_distributions_in_a_new_wsl_folder() {
    let mut store = Store::default();

    let done = store.import_wsl(&plan(), &["Ubuntu".to_string()]).unwrap();

    assert_eq!(
        done,
        WslImported {
            added: 1,
            updated: 0
        }
    );
    assert_eq!(store.folders.len(), 1);
    assert_eq!(store.folders[0].name, "WSL");
    let connection = &store.connections[0];
    assert_eq!(connection.name, "Ubuntu");
    assert_eq!(connection.host, "172.28.0.5");
    assert_eq!(connection.protocol, Protocol::Ssh);
    assert_eq!(
        connection.parent.as_deref(),
        Some(store.folders[0].id.as_str())
    );
    // The user got a credential that asks for the password.
    let credential = &store.credentials[0];
    assert_eq!(credential.username.as_deref(), Some("bartek"));
    assert_eq!(credential.kind, CredentialKind::Ask);
    assert_eq!(
        connection.settings.credential.as_deref(),
        Some(credential.id.as_str())
    );
}

#[test]
fn what_is_not_chosen_is_not_added_and_nothing_chosen_changes_nothing() {
    let mut store = Store::default();

    let done = store.import_wsl(&plan(), &[]).unwrap();

    assert_eq!(done, WslImported::default());
    assert!(store.folders.is_empty() && store.connections.is_empty());
}

#[test]
fn importing_again_after_a_restart_only_updates_the_address() {
    let mut store = Store::default();
    store.import_wsl(&plan(), &["Ubuntu".to_string()]).unwrap();
    // The user renamed the credential's connection settings nowhere; the address changes.
    let mut restarted = plan();
    restarted.connections[0].address = "172.28.7.7".to_string();

    let done = store
        .import_wsl(&restarted, &["Ubuntu".to_string()])
        .unwrap();

    assert_eq!(
        done,
        WslImported {
            added: 0,
            updated: 1
        }
    );
    assert_eq!(store.connections.len(), 1);
    assert_eq!(store.connections[0].host, "172.28.7.7");
    assert_eq!(store.folders.len(), 1, "the folder is reused");
}

#[test]
fn an_unchanged_address_counts_as_nothing_done() {
    let mut store = Store::default();
    store.import_wsl(&plan(), &["Ubuntu".to_string()]).unwrap();

    let done = store.import_wsl(&plan(), &["Ubuntu".to_string()]).unwrap();

    assert_eq!(done, WslImported::default());
}

#[test]
fn two_distributions_of_one_user_share_a_credential_and_root_has_none() {
    let mut store = Store::default();
    let mut both = plan();
    both.connections[1].user = Some("bartek".to_string());

    store
        .import_wsl(&both, &["Ubuntu".to_string(), "Debian".to_string()])
        .unwrap();

    assert_eq!(store.credentials.len(), 1);
    assert_eq!(store.connections.len(), 2);

    let mut other = Store::default();
    other.import_wsl(&plan(), &["Debian".to_string()]).unwrap();
    assert!(other.credentials.is_empty());
    assert_eq!(other.connections[0].settings.credential, None);
}
