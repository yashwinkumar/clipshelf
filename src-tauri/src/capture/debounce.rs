//! Pure change detection and debounce logic. No AppKit, so it is unit tested.
//!
//! Mirrors the behaviour observed in a commercial clipboard manager: a repeating
//! poll compares the pasteboard change count with a stored value, and on a change
//! the delay is restarted, so a burst of changes yields one read.
//! The 250 ms poll and 300 ms delay are those observed values. See ARCHITECTURE.md.

use std::time::{Duration, Instant};

pub const POLL_INTERVAL: Duration = Duration::from_millis(250);
pub const DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, PartialEq, Eq)]
pub struct Fired {
    /// The change count that was current when the debounce expired.
    pub change_count: i64,
}

pub struct Debouncer {
    last_count: i64,
    deadline: Option<Instant>,
    delay: Duration,
}

impl Debouncer {
    /// `initial_count` is the count at start up, so launching does not fire.
    pub fn new(initial_count: i64, delay: Duration) -> Self {
        Self {
            last_count: initial_count,
            deadline: None,
            delay,
        }
    }

    /// Feed one poll sample. Returns `Some` when a debounced change is ready.
    pub fn observe(&mut self, now: Instant, count: i64) -> Option<Fired> {
        if count != self.last_count {
            self.last_count = count;
            // Restart the wait on every new change.
            self.deadline = Some(now + self.delay);
        }
        match self.deadline {
            Some(deadline) if now >= deadline => {
                self.deadline = None;
                Some(Fired {
                    change_count: count,
                })
            }
            _ => None,
        }
    }
}

impl Debouncer {
    /// Record that this app wrote to the pasteboard. `produced` is the change
    /// count our write created, `current` is the count read right after it.
    /// Only when they match is the write ours alone: the stored count moves to
    /// it and any pending wait is dropped, so our write is not captured again.
    /// If they differ, another app wrote in between and nothing is suppressed.
    /// Returns whether the write was acknowledged.
    pub fn acknowledge_write(&mut self, produced: i64, current: i64) -> bool {
        if produced == current {
            self.last_count = current;
            self.deadline = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    #[test]
    fn no_change_never_fires() {
        let t = Instant::now();
        let mut d = Debouncer::new(10, DEBOUNCE);
        for i in 0..20 {
            assert_eq!(d.observe(ms(t, i * 250), 10), None);
        }
    }

    #[test]
    fn single_change_fires_once_after_delay() {
        let t = Instant::now();
        let mut d = Debouncer::new(10, DEBOUNCE);
        assert_eq!(d.observe(ms(t, 0), 11), None);
        assert_eq!(d.observe(ms(t, 250), 11), None);
        assert_eq!(d.observe(ms(t, 500), 11), Some(Fired { change_count: 11 }));
        assert_eq!(d.observe(ms(t, 750), 11), None);
    }

    #[test]
    fn burst_restarts_wait_and_fires_once() {
        let t = Instant::now();
        let mut d = Debouncer::new(10, DEBOUNCE);
        assert_eq!(d.observe(ms(t, 0), 11), None);
        assert_eq!(d.observe(ms(t, 250), 12), None); // restarts
        assert_eq!(d.observe(ms(t, 500), 13), None); // restarts
        assert_eq!(d.observe(ms(t, 750), 13), None); // 250 ms since last change
        assert_eq!(d.observe(ms(t, 800), 13), Some(Fired { change_count: 13 }));
        assert_eq!(d.observe(ms(t, 1050), 13), None);
    }

    #[test]
    fn two_separate_changes_fire_twice() {
        let t = Instant::now();
        let mut d = Debouncer::new(0, DEBOUNCE);
        assert_eq!(d.observe(ms(t, 0), 1), None);
        assert_eq!(d.observe(ms(t, 300), 1), Some(Fired { change_count: 1 }));
        assert_eq!(d.observe(ms(t, 1000), 2), None);
        assert_eq!(d.observe(ms(t, 1300), 2), Some(Fired { change_count: 2 }));
    }

    #[test]
    fn own_write_is_not_fired() {
        let t = Instant::now();
        let mut d = Debouncer::new(10, DEBOUNCE);
        assert!(d.acknowledge_write(11, 11));
        for i in 0..6 {
            assert_eq!(d.observe(ms(t, i * 250), 11), None);
        }
    }

    #[test]
    fn external_change_after_own_write_still_fires() {
        let t = Instant::now();
        let mut d = Debouncer::new(10, DEBOUNCE);
        assert!(d.acknowledge_write(11, 11));
        assert_eq!(d.observe(ms(t, 0), 12), None);
        assert_eq!(d.observe(ms(t, 300), 12), Some(Fired { change_count: 12 }));
    }

    #[test]
    fn write_with_interleaved_external_change_is_not_acknowledged() {
        let t = Instant::now();
        let mut d = Debouncer::new(10, DEBOUNCE);
        // our write produced 11, but another app moved the count to 12
        assert!(!d.acknowledge_write(11, 12));
        assert_eq!(d.observe(ms(t, 0), 12), None);
        assert_eq!(d.observe(ms(t, 300), 12), Some(Fired { change_count: 12 }));
    }

    #[test]
    fn own_write_drops_pending_wait() {
        let t = Instant::now();
        let mut d = Debouncer::new(10, DEBOUNCE);
        assert_eq!(d.observe(ms(t, 0), 11), None); // external change pending
        assert!(d.acknowledge_write(12, 12));
        assert_eq!(d.observe(ms(t, 400), 12), None);
    }
}
