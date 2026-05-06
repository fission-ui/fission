#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogScale {
    pub domain_min: f32,
    pub domain_max: f32,
    pub base: f32,
}

impl LogScale {
    pub fn new(domain_min: f32, domain_max: f32, base: f32) -> Self {
        let min = if domain_min <= 0.0 { 1e-6 } else { domain_min };
        let max = if domain_max <= 0.0 { 1.0 } else { domain_max };
        let actual_max = if max < min { min } else { max };

        Self {
            domain_min: min,
            domain_max: actual_max,
            base,
        }
    }

    /// Maps a value from the logarithmic domain to a linear range [range_min, range_max].
    pub fn scale(&self, value: f32, range_min: f32, range_max: f32) -> f32 {
        let val = value.max(self.domain_min);
        let log_min = self.domain_min.log(self.base);
        let log_max = self.domain_max.log(self.base);
        let log_val = val.log(self.base);

        if (log_max - log_min).abs() < f32::EPSILON {
            return range_min;
        }

        let t = (log_val - log_min) / (log_max - log_min);
        range_min + t * (range_max - range_min)
    }

    /// Generates ticks in base-10 (or self.base) format.
    pub fn ticks(&self) -> Vec<f32> {
        let mut ticks = Vec::new();

        let log_min = self.domain_min.log(self.base).floor() as i32;
        let log_max = self.domain_max.log(self.base).ceil() as i32;

        for exp in log_min..=log_max {
            let tick_val = self.base.powi(exp);
            if tick_val >= self.domain_min && tick_val <= self.domain_max {
                ticks.push(tick_val);
            }
        }

        ticks
    }
}
