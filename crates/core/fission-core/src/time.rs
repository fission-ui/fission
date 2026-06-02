use crate::GlobalState;
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub type CurrentTime = u64; // Milliseconds, monotonic

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clock {
    current_time: CurrentTime,
}

impl Default for Clock {
    fn default() -> Self {
        Self { current_time: 0 }
    }
}

impl GlobalState for Clock {
    // as_any and as_any_mut are provided by the Downcast trait now.
}

impl Clock {
    pub fn current_time(&self) -> CurrentTime {
        self.current_time
    }

    // Advance the clock by a duration `dt`.
    // This should only be called by the runtime in response to a Tick action.
    pub fn advance_by(&mut self, dt: CurrentTime) -> Result<()> {
        // Enforce monotonicity and non-negative dt
        if dt == 0 {
            return Ok(());
        }
        // Check for overflow (unlikely for u64 milliseconds in practice for reasonable durations)
        self.current_time = self
            .current_time
            .checked_add(dt)
            .ok_or_else(|| anyhow::anyhow!("Clock overflow"))?;
        Ok(())
    }

    // Set the clock to a specific time.
    // This should only be called by the runtime in response to an AdvanceTo action.
    pub fn set_to(&mut self, new_time: CurrentTime) -> Result<()> {
        if new_time < self.current_time {
            anyhow::bail!("Cannot set clock to a time before current time (time regression).");
        }
        self.current_time = new_time;
        Ok(())
    }
}
