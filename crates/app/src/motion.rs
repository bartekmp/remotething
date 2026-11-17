//! Small animations: a value that glides from one number to another.
//!
//! A [`Tween`] is read with the time of the frame being drawn, so drawing stays
//! a pure function of the state. While one runs, the window asks for frames.

use std::time::{Duration, Instant};

/// How long a slide or a fade takes.
pub const DURATION: Duration = Duration::from_millis(180);
/// How long a tab that was dropped keeps glowing.
pub const GLOW: Duration = Duration::from_millis(500);

/// A number that moves from `from` to `to` over `duration`, fast at first and slowing down.
#[derive(Debug, Clone, Copy)]
pub struct Tween {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
}

impl Tween {
    /// A value that is already where it is going.
    pub fn at(value: f32) -> Self {
        Self::arrived(value, Instant::now())
    }

    /// A value that got to `value` a while before `now`.
    fn arrived(value: f32, now: Instant) -> Self {
        Self {
            from: value,
            to: value,
            started: now.checked_sub(DURATION).unwrap_or(now),
            duration: DURATION,
        }
    }

    /// Starts from `from` and moves to `to`.
    pub fn between(from: f32, to: f32, now: Instant, duration: Duration) -> Self {
        Self {
            from,
            to,
            started: now,
            duration,
        }
    }

    /// Where the value is going.
    pub fn target(&self) -> f32 {
        self.to
    }

    /// Sends the value to `to` from wherever it is now. Without `animate` it is there at once.
    pub fn retarget(&mut self, to: f32, now: Instant, animate: bool) {
        if !animate {
            *self = Self::arrived(to, now);
            return;
        }
        *self = Self::between(self.value(now), to, now, DURATION);
    }

    /// The value at `now`.
    pub fn value(&self, now: Instant) -> f32 {
        let progress = self.progress(now);
        self.from + (self.to - self.from) * ease_out(progress)
    }

    /// Whether the value has not arrived yet at `now`.
    pub fn is_running(&self, now: Instant) -> bool {
        self.progress(now) < 1.0
    }

    fn progress(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.started);
        (elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
    }
}

/// Fast at first, slow at the end.
fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tween_starts_at_its_origin_and_ends_at_its_target() {
        let start = Instant::now();
        let tween = Tween::between(10.0, 20.0, start, DURATION);

        assert_eq!(tween.value(start), 10.0);
        assert!(tween.is_running(start));
        assert_eq!(tween.value(start + DURATION), 20.0);
        assert!(!tween.is_running(start + DURATION));
    }

    #[test]
    fn it_slows_down_towards_the_end() {
        let start = Instant::now();
        let tween = Tween::between(0.0, 1.0, start, DURATION);

        let halfway = tween.value(start + DURATION / 2);

        // Past the midpoint of the way after half the time, and still short of the end.
        assert!(halfway > 0.5 && halfway < 1.0, "{halfway}");
    }

    #[test]
    fn a_clock_that_is_behind_the_start_changes_nothing() {
        let start = Instant::now() + Duration::from_secs(5);
        let tween = Tween::between(3.0, 9.0, start, DURATION);

        assert_eq!(tween.value(Instant::now()), 3.0);
    }

    #[test]
    fn retargeting_continues_from_where_the_value_is() {
        let start = Instant::now();
        let mut tween = Tween::between(0.0, 100.0, start, DURATION);
        let middle = start + DURATION / 2;
        let here = tween.value(middle);

        tween.retarget(0.0, middle, true);

        assert_eq!(tween.value(middle), here);
        assert_eq!(tween.target(), 0.0);
        assert_eq!(tween.value(middle + DURATION), 0.0);
    }

    #[test]
    fn without_animation_the_value_is_at_its_target_at_once() {
        let now = Instant::now();
        let mut tween = Tween::at(0.0);

        tween.retarget(1.0, now, false);

        assert_eq!(tween.value(now), 1.0);
        assert!(!tween.is_running(now));
    }
}
