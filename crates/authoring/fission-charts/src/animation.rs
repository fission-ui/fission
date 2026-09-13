use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartEasing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartAnimationKind {
    None,
    Grow,
    Fade,
    Sweep,
    Pulse,
    Morph,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartAnimation {
    pub enabled: bool,
    pub kind: ChartAnimationKind,
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub stagger_ms: u64,
    pub easing: ChartEasing,
    pub reduced_motion_safe: bool,
    pub repeat: bool,
    /// Tweens series values from their previous data when the data changes.
    /// Applies to line and bar series in charts of at most a few hundred
    /// points; larger charts update immediately.
    #[serde(default)]
    pub animate_updates: bool,
}

impl Default for ChartAnimation {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: ChartAnimationKind::Grow,
            duration_ms: 450,
            delay_ms: 0,
            stagger_ms: 24,
            easing: ChartEasing::EaseOut,
            reduced_motion_safe: true,
            repeat: false,
            animate_updates: false,
        }
    }
}

impl ChartAnimation {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            kind: ChartAnimationKind::None,
            ..Self::default()
        }
    }

    pub fn enter(kind: ChartAnimationKind) -> Self {
        Self {
            enabled: true,
            kind,
            ..Self::default()
        }
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn delay_ms(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    pub fn stagger_ms(mut self, stagger_ms: u64) -> Self {
        self.stagger_ms = stagger_ms;
        self
    }

    pub fn easing(mut self, easing: ChartEasing) -> Self {
        self.easing = easing;
        self
    }

    pub fn reduced_motion_safe(mut self, safe: bool) -> Self {
        self.reduced_motion_safe = safe;
        self
    }

    pub fn repeat(mut self, repeat: bool) -> Self {
        self.repeat = repeat;
        self
    }

    /// Tweens values from the previous data whenever the data changes.
    pub fn updates(mut self, animate: bool) -> Self {
        self.animate_updates = animate;
        self
    }

    pub fn progress_at(&self, elapsed_ms: u64, item_index: usize) -> f32 {
        if !self.enabled || self.duration_ms == 0 {
            return 1.0;
        }
        let start = self
            .delay_ms
            .saturating_add(self.stagger_ms.saturating_mul(item_index as u64));
        if elapsed_ms <= start {
            return 0.0;
        }
        let raw = ((elapsed_ms - start) as f32 / self.duration_ms as f32).clamp(0.0, 1.0);
        match self.easing {
            ChartEasing::Linear => raw,
            ChartEasing::EaseIn => raw * raw,
            ChartEasing::EaseOut => 1.0 - (1.0 - raw) * (1.0 - raw),
            ChartEasing::EaseInOut => {
                if raw < 0.5 {
                    2.0 * raw * raw
                } else {
                    1.0 - (-2.0 * raw + 2.0).powi(2) / 2.0
                }
            }
        }
    }
}
