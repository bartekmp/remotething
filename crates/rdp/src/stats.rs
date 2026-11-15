//! What a session counts about itself, for the connection details.

use std::time::{Duration, Instant};

/// How often the window is told the counts.
const REPORT_EVERY: Duration = Duration::from_secs(1);

/// Totals since the session began. Rates are the window's to work out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Bytes of RDP data received from the server, without TLS overhead.
    pub received: u64,
    /// Frames passed to the window.
    pub frames: u64,
}

/// Counts as the session runs and says when the totals are due.
pub(crate) struct Counter {
    totals: Stats,
    last_report: Instant,
}

impl Counter {
    pub(crate) fn new() -> Self {
        Self {
            totals: Stats::default(),
            last_report: Instant::now(),
        }
    }

    pub(crate) fn received(&mut self, bytes: usize) {
        self.totals.received += bytes as u64;
    }

    pub(crate) fn frame_sent(&mut self) {
        self.totals.frames += 1;
    }

    /// The totals, once per report interval.
    pub(crate) fn due(&mut self) -> Option<Stats> {
        if self.last_report.elapsed() < REPORT_EVERY {
            return None;
        }
        self.last_report = Instant::now();
        Some(self.totals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_add_up() {
        let mut counter = Counter::new();
        counter.received(100);
        counter.received(50);
        counter.frame_sent();

        assert_eq!(
            counter.totals,
            Stats {
                received: 150,
                frames: 1
            }
        );
    }

    #[test]
    fn nothing_is_due_right_after_the_start() {
        assert_eq!(Counter::new().due(), None);
    }

    #[test]
    fn totals_are_due_after_the_interval() {
        let mut counter = Counter::new();
        counter.received(7);
        counter.last_report = Instant::now() - REPORT_EVERY;

        assert_eq!(
            counter.due(),
            Some(Stats {
                received: 7,
                frames: 0
            })
        );
        assert_eq!(counter.due(), None);
    }
}
