//! Tests of `wsl.rs`.

use std::collections::HashMap;

use super::*;
use crate::quick::parse_quick;

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
fn plain_utf8_is_left_alone() {
    assert_eq!(decode_wsl_output("zażółć\n".as_bytes()), "zażółć\n");
    assert_eq!(decode_wsl_output(b""), "");
    assert_eq!(decode_wsl_output(b"ab"), "ab");
}

#[test]
fn names_are_the_non_empty_lines() {
    assert_eq!(
        parse_names("Ubuntu-24.04\r\n\r\n  Debian \r\n"),
        ["Ubuntu-24.04", "Debian"]
    );
    assert!(parse_names("").is_empty());
}

/// A fake `wsl.exe`: answers by the arguments it is given.
struct Fake {
    answers: HashMap<String, std::result::Result<Vec<u8>, String>>,
}

impl WslRunner for Fake {
    fn run(&mut self, arguments: &[&str]) -> std::result::Result<Vec<u8>, String> {
        self.answers
            .get(&arguments.join(" "))
            .cloned()
            .unwrap_or_else(|| Err(format!("no answer for {arguments:?}")))
    }
}

fn fake(installed: &str, running: std::result::Result<&str, &str>) -> Fake {
    let mut answers = HashMap::new();
    answers.insert("--list --quiet".to_string(), Ok(utf16(installed, true)));
    answers.insert(
        "--list --running --quiet".to_string(),
        running
            .map(|text| utf16(text, true))
            .map_err(str::to_string),
    );
    Fake { answers }
}

#[test]
fn installed_distributions_are_marked_running_or_not() {
    let found = discover(&mut fake(
        "Ubuntu\r\nDebian\r\ndocker-desktop\r\n",
        Ok("Ubuntu\r\n"),
    ))
    .unwrap();

    assert_eq!(
        found,
        [
            WslDistro {
                name: "Ubuntu".to_string(),
                running: true
            },
            WslDistro {
                name: "Debian".to_string(),
                running: false
            },
            WslDistro {
                name: "docker-desktop".to_string(),
                running: false
            },
        ]
    );
}

#[test]
fn nothing_running_is_not_an_error() {
    let found = discover(&mut fake(
        "Ubuntu\r\n",
        Err("There are no running distributions."),
    ))
    .unwrap();

    assert_eq!(found.len(), 1);
    assert!(!found[0].running);
}

#[test]
fn a_failing_list_of_installed_distributions_is_an_error() {
    let mut nothing = Fake {
        answers: HashMap::new(),
    };

    assert!(discover(&mut nothing).is_err());
}

fn plan() -> WslPlan {
    plan_wsl(
        &discover(&mut fake(
            "Ubuntu\r\nDebian\r\ndocker-desktop\r\n",
            Ok("Ubuntu\r\n"),
        ))
        .unwrap(),
    )
}

#[test]
fn the_plan_skips_docker_desktops_own_distributions() {
    let plan = plan();

    let names: Vec<&str> = plan.connections.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Ubuntu", "Debian"]);
    assert!(plan.connections[0].running && !plan.connections[1].running);
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(plan.skipped[0].name, "docker-desktop");
}

#[test]
fn importing_adds_the_chosen_distributions_as_wsl_connections_named_by_the_distribution() {
    let mut store = Store::default();

    let done = store.import_wsl(&plan(), &["Ubuntu".to_string()]).unwrap();

    assert_eq!(
        done,
        WslImported {
            added: 1,
            already_there: 0
        }
    );
    assert_eq!(store.folders.len(), 1);
    assert_eq!(store.folders[0].name, "WSL");
    let connection = &store.connections[0];
    assert_eq!(connection.name, "Ubuntu");
    assert_eq!(connection.host, "Ubuntu");
    assert_eq!(connection.protocol, Protocol::Wsl);
    assert_eq!(
        connection.parent.as_deref(),
        Some(store.folders[0].id.as_str())
    );
    // No address to keep up to date and no user to ask for.
    assert!(store.credentials.is_empty());
}

#[test]
fn what_is_not_chosen_is_not_added_and_nothing_chosen_changes_nothing() {
    let mut store = Store::default();

    let done = store.import_wsl(&plan(), &[]).unwrap();

    assert_eq!(done, WslImported::default());
    assert!(store.folders.is_empty() && store.connections.is_empty());
}

#[test]
fn importing_again_adds_only_what_is_new_and_reuses_the_folder() {
    let mut store = Store::default();
    store.import_wsl(&plan(), &["Ubuntu".to_string()]).unwrap();

    let done = store
        .import_wsl(&plan(), &["Ubuntu".to_string(), "Debian".to_string()])
        .unwrap();

    assert_eq!(
        done,
        WslImported {
            added: 1,
            already_there: 1
        }
    );
    assert_eq!(store.connections.len(), 2);
    assert_eq!(store.folders.len(), 1);
}

#[test]
fn a_wsl_connection_loads_and_saves_under_its_own_protocol_name() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connections.toml");
    std::fs::write(
        &path,
        "[[connection]]\nid = \"u\"\nname = \"Ubuntu\"\nprotocol = \"wsl\"\nhost = \"Ubuntu\"\n",
    )
    .unwrap();

    let store = Store::load(&path).unwrap();

    assert_eq!(store.connections[0].protocol, Protocol::Wsl);
    assert!(!Protocol::Wsl.uses_ssh() && !Protocol::Wsl.is_files_only());
}

#[test]
fn the_wsl_scheme_is_understood_in_a_typed_connection_string() {
    let target = parse_quick("wsl://Ubuntu-24.04").unwrap();

    assert_eq!(target.protocol, Protocol::Wsl);
    assert_eq!(target.host, "Ubuntu-24.04");
}
