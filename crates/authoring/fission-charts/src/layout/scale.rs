use std::f32;

#[derive(Debug, Clone)]
pub enum Scale {
    Linear(LinearScale),
    Category(CategoryScale),
}

#[derive(Debug, Clone)]
pub struct LinearScale {
    pub min: f32,
    pub max: f32,
    pub ticks: Vec<f32>,
}

impl LinearScale {
    pub fn nice(min: f32, max: f32, max_ticks: usize) -> Self {
        if min == max {
            return Self {
                min: min - 1.0,
                max: max + 1.0,
                ticks: vec![min - 1.0, min, max + 1.0],
            };
        }

        let range = nice_num(max - min, false);
        let tick_spacing = nice_num(range / (max_ticks.max(1) as f32 - 1.0), true);

        let nice_min = (min / tick_spacing).floor() * tick_spacing;
        let nice_max = (max / tick_spacing).ceil() * tick_spacing;

        let mut ticks = Vec::new();
        let mut t = nice_min;
        // Float precision safeguard
        while t <= nice_max + 0.1 * tick_spacing {
            ticks.push(t);
            t += tick_spacing;
        }

        Self {
            min: nice_min,
            max: nice_max,
            ticks,
        }
    }

    pub fn map(&self, value: f32, range_min: f32, range_max: f32) -> f32 {
        let p = (value - self.min) / (self.max - self.min).max(f32::EPSILON);
        range_min + p * (range_max - range_min)
    }
}

fn nice_num(range: f32, round: bool) -> f32 {
    let exponent = range.log10().floor();
    let fraction = range / 10f32.powf(exponent);

    let nice_fraction = if round {
        if fraction < 1.5 {
            1.0
        } else if fraction < 3.0 {
            2.0
        } else if fraction < 7.0 {
            5.0
        } else {
            10.0
        }
    } else {
        if fraction <= 1.0 {
            1.0
        } else if fraction <= 2.0 {
            2.0
        } else if fraction <= 5.0 {
            5.0
        } else {
            10.0
        }
    };

    nice_fraction * 10f32.powf(exponent)
}

#[derive(Debug, Clone)]
pub struct CategoryScale {
    pub labels: Vec<String>,
}

impl CategoryScale {
    pub fn new(labels: Vec<String>) -> Self {
        Self { labels }
    }

    pub fn map(&self, index: usize, range_min: f32, range_max: f32) -> f32 {
        let count = self.labels.len().max(1) as f32;
        let step = (range_max - range_min) / count;
        range_min + (index as f32 + 0.5) * step
    }

    pub fn band_width(&self, range_min: f32, range_max: f32) -> f32 {
        let count = self.labels.len().max(1) as f32;
        (range_max - range_min) / count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nice_numbers_linear_scale() {
        let scale = LinearScale::nice(12.0, 87.0, 5);
        assert_eq!(scale.min, 0.0);
        assert_eq!(scale.max, 100.0);
        assert_eq!(scale.ticks, vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    }

    #[test]
    fn test_linear_scale_mapping() {
        let scale = LinearScale::nice(0.0, 100.0, 5);
        // Map 50 to range 0..200 -> should be 100
        let mapped = scale.map(50.0, 0.0, 200.0);
        assert!((mapped - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_category_scale_mapping() {
        let scale = CategoryScale::new(vec!["A".into(), "B".into(), "C".into(), "D".into()]);
        let band = scale.band_width(0.0, 100.0);
        assert_eq!(band, 25.0);

        let center_b = scale.map(1, 0.0, 100.0); // Index 1 is "B"
        assert_eq!(center_b, 37.5); // 25 + 12.5
    }
}
