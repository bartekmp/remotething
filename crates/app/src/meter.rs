//! Turns a session's running totals into rates for the connection details.

use std::time::{Duration, Instant};

/// Running totals since the session began.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Totals {
    pub(crate) received: u64,
    pub(crate) sent: u64,
    /// Pictures shown; stays 0 for sessions without a picture.
    pub(crate) frames: u64,
}

/// What the details show about the traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reading {
    pub(crate) received_per_second: u64,
    pub(crate) sent_per_second: u64,
    pub(crate) frames_per_second: u64,
    /// Everything received and sent so far.
    pub(crate) totals: Totals,
    /// Time since the session started.
    pub(crate) uptime: Duration,
}

/// Remembers the previous totals so a rate can be taken between two reports.
pub(crate) struct Meter {
    started: Instant,
    last: (Duration, Totals),
    pub(crate) reading: Option<Reading>,
}

impl Meter {
    pub(crate) fn new() -> Self {
        Self {
            started: Instant::now(),
            last: (Duration::ZERO, Totals::default()),
            reading: None,
        }
    }

    pub(crate) fn sample(&mut self, totals: Totals) {
        self.sample_at(self.started.elapsed(), totals);
    }

    fn sample_at(&mut self, at: Duration, totals: Totals) {
        let (before, earlier) = self.last;
        let seconds = at.saturating_sub(before).as_secs_f64();
        if seconds <= 0.0 {
            return;
        }
        let per_second = |now: u64, then: u64| (now.saturating_sub(then) as f64 / seconds) as u64;
        self.reading = Some(Reading {
            received_per_second: per_second(totals.received, earlier.received),
            sent_per_second: per_second(totals.sent, earlier.sent),
            frames_per_second: per_second(totals.frames, earlier.frames),
            totals,
            uptime: at,
        });
        self.last = (at, totals);
    }
}

/// A size in bytes with a sensible unit: "512 B", "340 kB", "1.2 MB".
pub(crate) fn bytes(count: u64) -> String {
    const KILO: f64 = 1000.0;
    let count = count as f64;
    if count < KILO {
        format!("{count:.0} B")
    } else if count < KILO * KILO {
        format!("{:.0} kB", count / KILO)
    } else if count < KILO * KILO * KILO {
        format!("{:.1} MB", count / (KILO * KILO))
    } else {
        format!("{:.2} GB", count / (KILO * KILO * KILO))
    }
}

/// A duration as "2:05" or "1:02:05".
pub(crate) fn clock(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn totals(received: u64, frames: u64) -> Totals {
        Totals {
            received,
            sent: received / 10,
            frames,
        }
    }

    #[test]
    fn the_first_report_is_averaged_over_the_whole_session() {
        let mut meter = Meter::new();

        meter.sample_at(Duration::from_secs(2), totals(4000, 60));

        let reading = meter.reading.unwrap();
        assert_eq!(reading.received_per_second, 2000);
        assert_eq!(reading.sent_per_second, 200);
        assert_eq!(reading.frames_per_second, 30);
        assert_eq!(reading.totals.received, 4000);
    }

    #[test]
    fn later_reports_use_only_what_came_since_the_last_one() {
        let mut meter = Meter::new();
        meter.sample_at(Duration::from_secs(1), totals(1000, 10));

        meter.sample_at(Duration::from_secs(3), totals(1400, 30));

        let reading = meter.reading.unwrap();
        assert_eq!(reading.received_per_second, 200);
        assert_eq!(reading.frames_per_second, 10);
        assert_eq!(reading.uptime, Duration::from_secs(3));
    }

    #[test]
    fn two_reports_at_the_same_moment_change_nothing() {
        let mut meter = Meter::new();

        meter.sample_at(Duration::ZERO, totals(10, 1));

        assert_eq!(meter.reading, None);
    }

    #[test]
    fn sizes_get_a_unit() {
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(340_000), "340 kB");
        assert_eq!(bytes(1_200_000), "1.2 MB");
        assert_eq!(bytes(3_500_000_000), "3.50 GB");
    }

    #[test]
    fn a_clock_shows_hours_only_when_needed() {
        assert_eq!(clock(Duration::from_secs(125)), "2:05");
        assert_eq!(clock(Duration::from_secs(3725)), "1:02:05");
    }
}
