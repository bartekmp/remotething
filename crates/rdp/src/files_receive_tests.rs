//! Tests of `files_receive.rs`.

use super::*;

fn file(name: &str, size: u64) -> FileDescriptor {
    FileDescriptor::new(name).with_file_size(size)
}

fn in_folder(name: &str, folder: &str) -> FileDescriptor {
    file(name, 3).with_relative_path(folder)
}

fn directory(name: &str) -> FileDescriptor {
    FileDescriptor::new(name).with_attributes(ClipboardFileAttributes::DIRECTORY)
}

#[test]
fn files_and_folders_are_planned_under_the_chosen_folder() {
    let folder = tempfile::tempdir().unwrap();
    let descriptors = [
        directory("docs"),
        in_folder("a.txt", "docs"),
        file("b.txt", 5),
    ];

    let (planned, total) = plan(&descriptors, folder.path()).unwrap();

    assert_eq!(total, 8);
    assert_eq!(planned[0].target, folder.path().join("docs"));
    assert!(planned[0].is_folder);
    assert_eq!(planned[1].target, folder.path().join("docs").join("a.txt"));
    assert_eq!(planned[2].index, 2);
}

#[test]
fn names_that_lead_out_of_the_folder_refuse_the_whole_plan() {
    let folder = tempfile::tempdir().unwrap();
    let bad = [
        file("..", 1),
        file("a/b", 1),
        file("/etc/passwd", 1),
        file("a\\b", 1),
        in_folder("x", "..\\.."),
        in_folder("x", "ok\\..\\.."),
        file("CON", 1),
        file("nul.txt", 1),
    ];
    for one in bad {
        let descriptors = [file("fine.txt", 1), one.clone()];

        assert!(plan(&descriptors, folder.path()).is_err(), "{one:?}");
    }
}

#[test]
fn a_file_that_would_be_replaced_refuses_the_whole_plan() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("a.txt"), "mine").unwrap();

    let refused = plan(&[file("a.txt", 1)], folder.path());

    assert!(refused.unwrap_err().contains("already exists"));
    assert_eq!(
        std::fs::read_to_string(folder.path().join("a.txt")).unwrap(),
        "mine"
    );
}

#[test]
fn an_existing_folder_is_reused_but_an_existing_file_in_its_place_is_not() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::create_dir(folder.path().join("docs")).unwrap();
    std::fs::write(folder.path().join("flat"), "x").unwrap();

    assert!(plan(&[directory("docs")], folder.path()).is_ok());
    assert!(plan(&[directory("flat")], folder.path()).is_err());
}

#[cfg(unix)]
#[test]
fn a_link_on_the_way_is_never_followed() {
    let folder = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), folder.path().join("docs")).unwrap();

    let refused = plan(&[in_folder("a.txt", "docs")], folder.path());

    assert!(refused.unwrap_err().contains("link"));
}

#[test]
fn a_file_without_a_size_is_refused() {
    let folder = tempfile::tempdir().unwrap();

    assert!(plan(&[FileDescriptor::new("a.txt")], folder.path()).is_err());
}

fn run_to_the_end(
    download: &mut Download,
    chunks: &mut dyn FnMut(&FileContentsRequest) -> Vec<u8>,
) {
    let mut stream = 1;
    loop {
        match download.next(stream).unwrap() {
            Next::Done => return,
            Next::Request(request) => {
                assert_eq!(download.awaiting, Some(stream));
                download.received(Some(chunks(&request))).unwrap();
                stream += 1;
            }
        }
    }
}

#[test]
fn a_download_writes_every_file_in_chunks_and_counts_them() {
    let folder = tempfile::tempdir().unwrap();
    let size = u64::from(CHUNK) * 2 + 10;
    let descriptors = [directory("d"), in_folder("big.bin", "d"), file("empty", 0)];
    let descriptors: Vec<_> = descriptors
        .into_iter()
        .map(|mut one| {
            if one.name == "big.bin" {
                one.file_size = Some(size);
            }
            one
        })
        .collect();
    let (planned, total) = plan(&descriptors, folder.path()).unwrap();
    let mut download = Download::new(planned, total, Some(7));
    let mut positions = Vec::new();

    run_to_the_end(&mut download, &mut |request| {
        assert_eq!(request.data_id, Some(7));
        assert_eq!(request.index, 1);
        positions.push((request.position, request.requested_size));
        vec![9u8; request.requested_size as usize]
    });

    assert_eq!(
        positions,
        [
            (0, CHUNK),
            (u64::from(CHUNK), CHUNK),
            (u64::from(CHUNK) * 2, 10)
        ]
    );
    assert_eq!(download.done, size);
    assert_eq!(download.finished, 3);
    let written = std::fs::read(folder.path().join("d").join("big.bin")).unwrap();
    assert_eq!(written.len() as u64, size);
    assert!(folder.path().join("empty").exists());
}

#[test]
fn more_data_than_asked_for_stops_the_download() {
    let folder = tempfile::tempdir().unwrap();
    let (planned, total) = plan(&[file("a", 10)], folder.path()).unwrap();
    let mut download = Download::new(planned, total, None);
    let Ok(Next::Request(_)) = download.next(1) else {
        panic!("a request is due");
    };

    let result = download.received(Some(vec![0; 11]));

    assert!(result.is_err());
}

#[test]
fn an_error_answer_stops_the_download() {
    let folder = tempfile::tempdir().unwrap();
    let (planned, total) = plan(&[file("a", 10)], folder.path()).unwrap();
    let mut download = Download::new(planned, total, None);
    let Ok(Next::Request(_)) = download.next(1) else {
        panic!("a request is due");
    };

    assert!(download.received(None).is_err());
}

#[test]
fn abandoning_removes_the_file_that_was_only_partly_received() {
    let folder = tempfile::tempdir().unwrap();
    let (planned, total) = plan(&[file("a", 10)], folder.path()).unwrap();
    let mut download = Download::new(planned, total, None);
    let Ok(Next::Request(_)) = download.next(1) else {
        panic!("a request is due");
    };
    assert!(folder.path().join("a").exists());

    download.abandon();

    assert!(!folder.path().join("a").exists());
}
