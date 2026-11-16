//! What a transfer reports, and the copy loop every backend shares.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::fs::OnProgress;

/// How many bytes are moved before progress is reported again.
pub(crate) const CHUNK: usize = 64 * 1024;

/// How far a transfer has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Bytes of the file that are in place, including those from an earlier try.
    pub done: u64,
    /// The size of the whole file; 0 when the server did not say.
    pub total: u64,
}

/// What a finished transfer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transferred {
    /// The size of the file now.
    pub bytes: u64,
    /// Where the transfer took up an earlier try; 0 when it started from the beginning.
    pub resumed_from: u64,
}

/// Which end of a copy failed.
pub(crate) enum Failure {
    Reading(std::io::Error),
    Writing(std::io::Error),
}

/// Copies everything from `source` to `target`, calling `progress` after each chunk.
/// Returns the size of the file when done: `start` plus what was copied.
pub(crate) async fn copy(
    source: &mut (impl AsyncReadExt + Unpin),
    target: &mut (impl AsyncWriteExt + Unpin),
    start: u64,
    total: u64,
    progress: OnProgress<'_>,
) -> std::result::Result<u64, Failure> {
    let mut buffer = vec![0u8; CHUNK];
    let mut done = start;
    progress(Progress { done, total });
    loop {
        let read = source.read(&mut buffer).await.map_err(Failure::Reading)?;
        if read == 0 {
            return Ok(done);
        }
        target
            .write_all(&buffer[..read])
            .await
            .map_err(Failure::Writing)?;
        done += read as u64;
        progress(Progress {
            done,
            total: total.max(done),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn copy_reports_progress_up_to_the_whole_size() {
        let data = vec![7u8; CHUNK * 2 + 100];
        let mut source = Cursor::new(data.clone());
        let mut target = Vec::new();
        let mut seen = Vec::new();

        let done = copy(
            &mut source,
            &mut target,
            0,
            data.len() as u64,
            &mut |progress| seen.push(progress.done),
        )
        .await
        .ok()
        .unwrap();

        assert_eq!(done, data.len() as u64);
        assert_eq!(target, data);
        // Once at the start, then after each chunk, always growing.
        assert_eq!(seen.first(), Some(&0));
        assert_eq!(seen.last(), Some(&(data.len() as u64)));
        assert!(seen.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[tokio::test]
    async fn copy_counts_from_where_a_resumed_transfer_starts() {
        let mut source = Cursor::new(vec![1u8; 100]);
        let mut target = Vec::new();
        let mut first = None;

        let done = copy(&mut source, &mut target, 900, 1000, &mut |progress| {
            first.get_or_insert(progress);
        })
        .await
        .ok()
        .unwrap();

        assert_eq!(done, 1000);
        assert_eq!(
            first,
            Some(Progress {
                done: 900,
                total: 1000
            })
        );
    }

    #[tokio::test]
    async fn the_total_never_falls_behind_the_done_count() {
        // A server that did not say the size (0) still gives a total that grows with the file.
        let mut source = Cursor::new(vec![1u8; 10]);
        let mut target = Vec::new();
        let mut last = None;

        copy(&mut source, &mut target, 0, 0, &mut |progress| {
            last = Some(progress)
        })
        .await
        .ok()
        .unwrap();

        assert_eq!(
            last,
            Some(Progress {
                done: 10,
                total: 10
            })
        );
    }
}
