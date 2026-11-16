//! Tests of `files.rs`.

use super::*;

#[test]
fn picked_files_and_folders_become_descriptors_in_list_order() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::create_dir_all(root.join("docs/deep")).unwrap();
    std::fs::write(root.join("docs/a.txt"), "aaa").unwrap();
    std::fs::write(root.join("docs/deep/b.txt"), "bb").unwrap();
    std::fs::write(root.join("single.txt"), "s").unwrap();

    let (descriptors, offered) = collect(&[root.join("docs"), root.join("single.txt")]).unwrap();

    let names: Vec<String> = descriptors
        .iter()
        .map(|d| match &d.relative_path {
            Some(parents) => format!("{parents}\\{}", d.name),
            None => d.name.clone(),
        })
        .collect();
    assert_eq!(
        names,
        [
            "docs",
            "docs\\a.txt",
            "docs\\deep",
            "docs\\deep\\b.txt",
            "single.txt"
        ]
    );
    assert!(
        descriptors[0]
            .attributes
            .unwrap()
            .contains(ClipboardFileAttributes::DIRECTORY)
    );
    assert_eq!(descriptors[1].file_size, Some(3));
    // The local file behind each entry is in the same order, with none for folders.
    assert!(offered[0].path.is_none());
    assert_eq!(
        offered[1].path.as_deref(),
        Some(root.join("docs/a.txt").as_path())
    );
    assert_eq!(offered.iter().map(|o| o.size).sum::<u64>(), 6);
}

#[test]
fn nothing_picked_is_refused() {
    assert!(collect(&[]).is_err());
}

#[test]
fn a_missing_path_is_refused_with_its_name() {
    let refused = collect(&[PathBuf::from("/definitely/not/here.txt")]);

    assert!(refused.unwrap_err().contains("here.txt"));
}

#[test]
fn a_name_windows_cannot_hold_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let odd = directory.path().join("what?.txt");
    std::fs::write(&odd, "x").unwrap();

    assert!(collect(&[odd]).is_err());
}

#[cfg(unix)]
#[test]
fn links_are_left_out() {
    let directory = tempfile::tempdir().unwrap();
    let real = directory.path().join("real.txt");
    std::fs::write(&real, "x").unwrap();
    let link = directory.path().join("link.txt");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let (descriptors, _) = collect(&[real, link]).unwrap();

    assert_eq!(descriptors.len(), 1);
}

fn offered(content: &[u8]) -> (tempfile::TempDir, Files) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("f.bin");
    std::fs::write(&path, content).unwrap();
    let files = Files {
        offered: vec![
            Offered {
                path: None,
                size: 0,
            },
            Offered {
                path: Some(path),
                size: content.len() as u64,
            },
        ],
        offered_bytes: content.len() as u64,
        ..Files::default()
    };
    (directory, files)
}

fn range(index: i32, position: u64, size: u32) -> FileContentsRequest {
    FileContentsRequest {
        stream_id: 5,
        index,
        flags: FileContentsFlags::RANGE,
        position,
        requested_size: size,
        data_id: None,
    }
}

#[test]
fn the_server_gets_the_range_it_asks_for() {
    let (_directory, mut files) = offered(b"0123456789");

    let response = files.answer(&range(1, 4, 3));

    assert!(!response.is_error());
    assert_eq!(response.data(), b"456");
    assert_eq!(files.sent, 3);
}

#[test]
fn a_range_past_the_end_is_short_and_a_wrong_index_is_an_error() {
    let (_directory, mut files) = offered(b"0123456789");

    assert_eq!(files.answer(&range(1, 8, 100)).data(), b"89");
    assert!(files.answer(&range(7, 0, 1)).is_error());
    assert!(files.answer(&range(-1, 0, 1)).is_error());
    // A folder has no bytes.
    assert!(files.answer(&range(0, 0, 1)).is_error());
}

#[test]
fn a_huge_range_is_refused_instead_of_allocated() {
    let (_directory, mut files) = offered(b"x");

    assert!(files.answer(&range(1, 0, MAX_RANGE + 1)).is_error());
}

#[test]
fn the_size_of_an_offered_file_is_answered() {
    let (_directory, mut files) = offered(b"0123456789");
    let mut request = range(1, 0, 8);
    request.flags = FileContentsFlags::SIZE;

    let response = files.answer(&request);

    assert_eq!(response.data_as_size().unwrap(), 10);
}

#[test]
fn what_the_server_copied_is_summarised_with_slashes() {
    let descriptors = [
        FileDescriptor::new("docs").with_attributes(ClipboardFileAttributes::DIRECTORY),
        FileDescriptor::new("a.txt")
            .with_file_size(3)
            .with_relative_path("docs\\deep"),
    ];

    let summary = summary(&descriptors);

    assert_eq!(
        summary[0],
        RemoteFile {
            path: "docs".into(),
            size: 0,
            is_folder: true
        }
    );
    assert_eq!(
        summary[1],
        RemoteFile {
            path: "docs/deep/a.txt".into(),
            size: 3,
            is_folder: false
        }
    );
}

#[test]
fn progress_reports_are_spaced_out_but_the_last_one_always_goes() {
    let mut files = Files::default();

    assert!(files.report_due(false));
    assert!(!files.report_due(false));
    assert!(files.report_due(true));
}

#[test]
fn sending_is_reported_finished_once_when_everything_was_taken() {
    let (_directory, mut files) = offered(b"0123456789");
    let mut events = Vec::new();
    files.sent = 4;
    files.finish_sent(&mut events);
    assert!(events.is_empty());

    files.sent = 10;
    files.finish_sent(&mut events);
    files.finish_sent(&mut events);

    assert_eq!(
        events,
        [FilesEvent::Finished {
            direction: Direction::Sent,
            files: 1
        }]
    );
}
