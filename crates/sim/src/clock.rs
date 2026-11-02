//! Wall-clock scheduling for the 20 Hz authoritative simulation.

pub const TICK_MILLIS: u64 = 50;
/// Maximum work in one scheduler wake. Larger gaps require recovery, not fabricated play.
pub const MAX_CATCH_UP_TICKS: u64 = 20;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Clock { next_tick_ms: u64 }

impl Clock {
    pub fn start(now_ms: u64) -> Result<Self, &'static str> {
        Ok(Self { next_tick_ms: now_ms.checked_add(TICK_MILLIS).ok_or("clock_overflow")? })
    }

    pub fn next_tick_ms(&self) -> u64 { self.next_tick_ms }

    /// Returns the number of ticks due without moving the deadline on an overrun.
    pub fn due(&mut self, now_ms: u64) -> Result<u64, &'static str> {
        if now_ms < self.next_tick_ms { return Ok(0); }
        let count = (now_ms - self.next_tick_ms) / TICK_MILLIS + 1;
        if count > MAX_CATCH_UP_TICKS { return Err("clock_overrun"); }
        self.next_tick_ms = self.next_tick_ms.checked_add(count * TICK_MILLIS).ok_or("clock_overflow")?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ticks_follow_elapsed_time_and_refuse_large_gaps() {
        let mut clock = Clock::start(1_000).unwrap();
        assert_eq!(clock.due(1_049), Ok(0));
        assert_eq!(clock.due(1_050), Ok(1));
        assert_eq!(clock.due(1_200), Ok(3));
        assert_eq!(clock.due(2_250), Err("clock_overrun"));
        assert_eq!(clock.due(1_250), Ok(1));
    }
}
