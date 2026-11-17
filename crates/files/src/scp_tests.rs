//! Tests of `scp.rs`.

use super::*;

#[test]
fn a_listing_line_gives_name_size_kind_and_permissions() {
    let file = parse_ls("-rw-r--r--  1 1000 1000    252 Nov 11 12:24 my notes.txt").unwrap();
    assert_eq!(file.name, "my notes.txt");
    assert_eq!(file.kind, EntryKind::File);
    assert_eq!(file.size, Some(252));
    assert_eq!(file.permissions, Some(0o644));

    let folder = parse_ls("drwxr-xr-x 4 0 0 4096 Jun  8 2024 etc").unwrap();
    assert_eq!(folder.kind, EntryKind::Folder);
    assert_eq!(folder.permissions, Some(0o755));
}

#[test]
fn a_link_is_listed_by_its_own_name() {
    let link = parse_ls("lrwxrwxrwx 1 0 0 7 Jun  8 2024 lib -> usr/lib").unwrap();
    assert_eq!(link.name, "lib");
    assert_eq!(link.kind, EntryKind::Link);
}

#[test]
fn the_total_line_and_devices_are_left_out() {
    assert!(parse_ls("total 500").is_none());
    assert!(parse_ls("crw-rw-rw- 1 0 0 1, 3 Jun  8 2024 null").is_none());
    assert!(parse_ls("").is_none());
}

#[test]
fn names_are_quoted_for_the_shell() {
    assert_eq!(quote("a b"), "'a b'");
    assert_eq!(quote("it's"), "'it'\\''s'");
}

#[test]
fn a_file_header_gives_its_size() {
    assert_eq!(
        parse_header("C0644 12345 a b.txt"),
        Ok(Header { size: 12345 })
    );
}

#[test]
fn errors_folders_and_nonsense_are_refused_with_a_reason() {
    assert_eq!(
        parse_header("\u{1}no such file"),
        Err("no such file".into())
    );
    assert_eq!(parse_header("\u{2}denied"), Err("denied".into()));
    assert!(parse_header("D0755 0 dir").is_err());
    assert!(parse_header("Cx y z").is_err());
    assert!(parse_header("").is_err());
}

#[tokio::test]
async fn an_answer_is_a_zero_byte_or_an_error_line() {
    let mut ok: &[u8] = &[0];
    assert_eq!(read_ack(&mut ok).await, Ok(()));

    let mut refused: &[u8] = b"\x01scp: denied\nrest";
    assert_eq!(read_ack(&mut refused).await, Err("scp: denied".to_string()));

    let mut empty: &[u8] = &[];
    assert!(read_ack(&mut empty).await.is_err());
}
