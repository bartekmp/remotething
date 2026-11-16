//! The list of uploads and downloads of a file pane: what waits, what runs,
//! what ended. Nothing here touches the network; `files_transfer.rs` does that.

use std::path::PathBuf;

use iced::task::Handle;
use remotething_files::{Progress, Transferred};

/// How many transfers run at the same time; the rest wait their turn.
pub(crate) const MAX_RUNNING: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Upload,
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JobState {
    Waiting,
    Running(Progress),
    Done(u64),
    Failed(String),
    Cancelled,
}

/// One transfer.
#[derive(Debug)]
pub(crate) struct Job {
    pub id: u64,
    pub direction: Direction,
    pub local: PathBuf,
    pub remote: String,
    pub state: JobState,
    /// Whether to take up what an earlier try left behind instead of starting over.
    pub resume: bool,
    /// Stops the running transfer.
    handle: Option<Handle>,
}

impl Job {
    /// Waiting or running.
    pub(crate) fn is_active(&self) -> bool {
        matches!(self.state, JobState::Waiting | JobState::Running(_))
    }
}

#[derive(Debug, Default)]
pub(crate) struct Queue {
    pub jobs: Vec<Job>,
    next_id: u64,
}

impl Queue {
    /// Puts a transfer at the end of the line and returns its id.
    pub(crate) fn add(&mut self, direction: Direction, local: PathBuf, remote: String) -> u64 {
        self.next_id += 1;
        self.jobs.push(Job {
            id: self.next_id,
            direction,
            local,
            remote,
            state: JobState::Waiting,
            resume: false,
            handle: None,
        });
        self.next_id
    }

    pub(crate) fn job(&self, id: u64) -> Option<&Job> {
        self.jobs.iter().find(|job| job.id == id)
    }

    fn job_mut(&mut self, id: u64) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|job| job.id == id)
    }

    /// Moves waiting transfers to running while there is room, oldest first,
    /// and returns the ones that must be started.
    pub(crate) fn take_ready(&mut self) -> Vec<u64> {
        let running = self
            .jobs
            .iter()
            .filter(|job| matches!(job.state, JobState::Running(_)))
            .count();
        let room = MAX_RUNNING.saturating_sub(running);
        let mut started = Vec::new();
        for job in self
            .jobs
            .iter_mut()
            .filter(|job| job.state == JobState::Waiting)
            .take(room)
        {
            job.state = JobState::Running(Progress { done: 0, total: 0 });
            started.push(job.id);
        }
        started
    }

    /// Remembers how to stop a transfer that was just started.
    pub(crate) fn set_handle(&mut self, id: u64, handle: Handle) {
        if let Some(job) = self.job_mut(id) {
            job.handle = Some(handle);
        }
    }

    /// The latest progress of a running transfer. Late reports of an ended one are ignored.
    pub(crate) fn progress(&mut self, id: u64, progress: Progress) {
        if let Some(job) = self.job_mut(id)
            && matches!(job.state, JobState::Running(_))
        {
            job.state = JobState::Running(progress);
        }
    }

    /// A transfer ended. A cancelled one stays cancelled, whatever the stopped task reported.
    pub(crate) fn ended(&mut self, id: u64, result: Result<Transferred, String>) {
        let Some(job) = self.job_mut(id) else {
            return;
        };
        job.handle = None;
        if job.state == JobState::Cancelled {
            return;
        }
        job.state = match result {
            Ok(transferred) => JobState::Done(transferred.bytes),
            Err(reason) => JobState::Failed(reason),
        };
    }

    /// Stops a waiting or running transfer.
    pub(crate) fn cancel(&mut self, id: u64) {
        let Some(job) = self.job_mut(id) else {
            return;
        };
        if !job.is_active() {
            return;
        }
        if let Some(handle) = job.handle.take() {
            handle.abort();
        }
        job.state = JobState::Cancelled;
    }

    /// Puts a failed or cancelled transfer back in line. It carries on from what is already there.
    pub(crate) fn retry(&mut self, id: u64) {
        if let Some(job) = self.job_mut(id)
            && matches!(job.state, JobState::Failed(_) | JobState::Cancelled)
        {
            job.state = JobState::Waiting;
            job.resume = true;
        }
    }

    /// Nothing waits or runs.
    pub(crate) fn is_idle(&self) -> bool {
        self.jobs.iter().all(|job| !job.is_active())
    }

    /// Forgets the transfers that are over.
    pub(crate) fn clear_finished(&mut self) {
        self.jobs.retain(Job::is_active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue_of(count: usize) -> Queue {
        let mut queue = Queue::default();
        for number in 0..count {
            queue.add(
                Direction::Upload,
                PathBuf::from(format!("/tmp/{number}")),
                format!("/srv/{number}"),
            );
        }
        queue
    }

    fn done(bytes: u64) -> Result<Transferred, String> {
        Ok(Transferred {
            bytes,
            resumed_from: 0,
        })
    }

    #[test]
    fn only_two_run_at_once_and_the_oldest_go_first() {
        let mut queue = queue_of(3);

        assert_eq!(queue.take_ready(), [1, 2]);
        assert_eq!(queue.take_ready(), Vec::<u64>::new());
        assert_eq!(queue.job(3).unwrap().state, JobState::Waiting);
    }

    #[test]
    fn a_finished_transfer_makes_room_for_the_next() {
        let mut queue = queue_of(3);
        queue.take_ready();

        queue.ended(1, done(10));

        assert_eq!(queue.job(1).unwrap().state, JobState::Done(10));
        assert_eq!(queue.take_ready(), [3]);
    }

    #[test]
    fn progress_is_kept_for_a_running_transfer_only() {
        let mut queue = queue_of(2);
        queue.take_ready();
        queue.ended(2, done(5));

        queue.progress(1, Progress { done: 4, total: 8 });
        queue.progress(2, Progress { done: 1, total: 8 });

        assert_eq!(
            queue.job(1).unwrap().state,
            JobState::Running(Progress { done: 4, total: 8 })
        );
        assert_eq!(queue.job(2).unwrap().state, JobState::Done(5));
    }

    #[test]
    fn a_waiting_transfer_can_be_cancelled_and_never_starts() {
        let mut queue = queue_of(3);
        queue.take_ready();

        queue.cancel(3);
        queue.ended(1, done(1));

        assert_eq!(queue.job(3).unwrap().state, JobState::Cancelled);
        assert_eq!(queue.take_ready(), Vec::<u64>::new());
    }

    #[test]
    fn a_cancelled_transfer_stays_cancelled_when_the_stopped_task_reports() {
        let mut queue = queue_of(1);
        queue.take_ready();

        queue.cancel(1);
        queue.ended(1, Err("aborted".to_string()));

        assert_eq!(queue.job(1).unwrap().state, JobState::Cancelled);
    }

    #[test]
    fn a_failed_transfer_goes_back_in_line_and_resumes() {
        let mut queue = queue_of(1);
        queue.take_ready();
        queue.ended(1, Err("connection lost".to_string()));
        assert!(!queue.job(1).unwrap().resume);

        queue.retry(1);

        let job = queue.job(1).unwrap();
        assert_eq!(job.state, JobState::Waiting);
        assert!(job.resume);
        assert_eq!(queue.take_ready(), [1]);
    }

    #[test]
    fn a_finished_transfer_cannot_be_retried() {
        let mut queue = queue_of(1);
        queue.take_ready();
        queue.ended(1, done(3));

        queue.retry(1);

        assert_eq!(queue.job(1).unwrap().state, JobState::Done(3));
    }

    #[test]
    fn the_queue_is_idle_once_everything_is_over() {
        let mut queue = queue_of(2);
        assert!(!queue.is_idle());
        queue.take_ready();
        queue.ended(1, done(1));
        assert!(!queue.is_idle());

        queue.ended(2, done(1));

        assert!(queue.is_idle());
    }

    #[test]
    fn clearing_forgets_only_what_is_over() {
        let mut queue = queue_of(3);
        queue.take_ready();
        queue.ended(1, done(1));
        queue.cancel(2);

        queue.clear_finished();

        assert_eq!(queue.jobs.len(), 1);
        assert_eq!(queue.jobs[0].id, 3);
    }
}
