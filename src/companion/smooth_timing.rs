#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SmoothSemanticClock {
    interval: Duration,
    next_due: Instant,
    tick_index: u64,
}

impl SmoothSemanticClock {
    pub fn new(started_at: Instant, interval: Duration) -> Self {
        Self {
            interval,
            next_due: started_at + interval,
            tick_index: 0,
        }
    }

    pub fn consume_due_tick(&mut self, now: Instant) -> Option<u64> {
        if now < self.next_due {
            return None;
        }
        self.tick_index = self.tick_index.saturating_add(1);
        self.next_due = now + self.interval;
        Some(self.tick_index)
    }

    pub fn tick_index(&self) -> u64 {
        self.tick_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_semantic_clock_waits_until_interval_elapses() {
        let start = Instant::now();
        let mut clock = SmoothSemanticClock::new(start, Duration::from_millis(250));

        assert_eq!(
            clock.consume_due_tick(start + Duration::from_millis(249)),
            None
        );
        assert_eq!(
            clock.consume_due_tick(start + Duration::from_millis(250)),
            Some(1)
        );
        assert_eq!(clock.tick_index(), 1);
    }

    #[test]
    fn smooth_semantic_clock_drops_missed_intervals_instead_of_catching_up() {
        let start = Instant::now();
        let mut clock = SmoothSemanticClock::new(start, Duration::from_millis(250));

        assert_eq!(
            clock.consume_due_tick(start + Duration::from_secs(3)),
            Some(1)
        );
        assert_eq!(
            clock.consume_due_tick(start + Duration::from_secs(3) + Duration::from_millis(1)),
            None
        );
        assert_eq!(clock.tick_index(), 1);
    }
}
