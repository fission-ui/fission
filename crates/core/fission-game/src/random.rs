//! Small explicit deterministic random stream for simulation and replay.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomSnapshot {
    state: u64,
}

#[derive(Clone, Debug)]
pub struct DeterministicRandom {
    state: u64,
}

impl DeterministicRandom {
    pub fn seeded(seed: u64) -> Self {
        Self {
            state: nonzero_seed(seed),
        }
    }

    pub fn snapshot(&self) -> RandomSnapshot {
        RandomSnapshot { state: self.state }
    }

    pub fn from_snapshot(snapshot: RandomSnapshot) -> Self {
        Self {
            state: nonzero_seed(snapshot.state),
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        value
    }

    pub fn next_f32(&mut self) -> f32 {
        const SCALE: f32 = 1.0 / ((1_u32 << 24) as f32);
        ((self.next_u64() >> 40) as u32) as f32 * SCALE
    }

    pub fn range_u32(&mut self, range: std::ops::Range<u32>) -> u32 {
        assert!(range.start < range.end, "random range must not be empty");
        range.start + (self.next_u64() % u64::from(range.end - range.start)) as u32
    }
}

fn nonzero_seed(seed: u64) -> u64 {
    if seed == 0 {
        0xF155_10A5_D37E_4D1C
    } else {
        seed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_stream_repeats_exact_values() {
        let mut random = DeterministicRandom::seeded(42);
        let _ = random.next_u64();
        let snapshot = random.snapshot();
        let expected = [random.next_u64(), random.next_u64(), random.next_u64()];
        let mut restored = DeterministicRandom::from_snapshot(snapshot);
        assert_eq!(
            expected,
            [
                restored.next_u64(),
                restored.next_u64(),
                restored.next_u64()
            ]
        );
    }
}
