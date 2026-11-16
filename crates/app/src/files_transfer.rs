//! Runs one transfer of a file pane's queue and reports how it goes.

use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::Task;
use iced::futures::SinkExt;
use iced::task::Handle;
use remotething_files::{Progress, RemoteFs};

use crate::files_pane::Message;
use crate::transfer_queue::{Direction, Job};

/// Progress is reported at most this often, so a fast link does not flood the window.
const REPORT_EVERY: Duration = Duration::from_millis(100);

/// Starts `job`. The returned handle stops it.
pub(crate) fn start(files: Arc<dyn RemoteFs>, job: &Job) -> (Task<Message>, Handle) {
    let id = job.id;
    let direction = job.direction;
    let local = job.local.clone();
    let remote = job.remote.clone();
    let resume = job.resume;

    let messages = iced::stream::channel(16, async move |mut output| {
        let mut progress_output = output.clone();
        let mut last_report: Option<Instant> = None;
        let mut report = move |progress: Progress| {
            let due = last_report.is_none_or(|at| at.elapsed() >= REPORT_EVERY);
            // The end of the file is always reported by the result, so skipping a report is fine.
            if due
                && progress_output
                    .try_send(Message::TransferProgress(id, progress))
                    .is_ok()
            {
                last_report = Some(Instant::now());
            }
        };
        let result = match direction {
            Direction::Upload => {
                files
                    .upload_tracked(&local, &remote, resume, &mut report)
                    .await
            }
            Direction::Download => {
                files
                    .download_tracked(&remote, &local, resume, &mut report)
                    .await
            }
        };
        let result = result.map_err(|error| error.to_string());
        let _ = output.send(Message::TransferEnded(id, result)).await;
    });
    Task::run(messages, |message| message).abortable()
}
