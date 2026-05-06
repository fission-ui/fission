#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeScale {
    pub start_ts: i64,
    pub end_ts: i64,
}

impl TimeScale {
    pub fn new(start_ts: i64, end_ts: i64) -> Self {
        Self { start_ts, end_ts }
    }

    /// Generates "nice" date/time ticks between start and end.
    /// Ticks are timestamps in seconds.
    pub fn ticks(&self, max_ticks: usize) -> Vec<i64> {
        let span = self.end_ts - self.start_ts;
        if span <= 0 || max_ticks < 2 {
            return vec![self.start_ts, self.end_ts];
        }

        let target_interval = span as f64 / (max_ticks - 1) as f64;

        let minute = 60.0;
        let hour = 3600.0;
        let day = 86400.0;
        let week = day * 7.0;
        let month = day * 30.0;
        let year = day * 365.0;

        let nice_intervals = [
            1.0,
            5.0,
            10.0,
            15.0,
            30.0,
            minute,
            minute * 5.0,
            minute * 15.0,
            minute * 30.0,
            hour,
            hour * 3.0,
            hour * 6.0,
            hour * 12.0,
            day,
            day * 2.0,
            week,
            month,
            month * 3.0,
            month * 6.0,
            year,
            year * 2.0,
            year * 5.0,
            year * 10.0,
        ];

        let mut best_interval = nice_intervals[0];
        let mut min_diff = f64::MAX;

        for &interval in &nice_intervals {
            let diff = (target_interval - interval).abs();
            if diff < min_diff {
                min_diff = diff;
                best_interval = interval;
            }
        }

        let mut ticks = Vec::new();
        let start_tick = (self.start_ts as f64 / best_interval).ceil() * best_interval;

        let mut current_tick = start_tick;
        while current_tick <= self.end_ts as f64 {
            ticks.push(current_tick as i64);
            current_tick += best_interval;
        }

        ticks
    }

    pub fn scale(&self, ts: i64, min_pos: f32, max_pos: f32) -> f32 {
        if self.start_ts == self.end_ts {
            return min_pos;
        }
        let t = (ts - self.start_ts) as f32 / (self.end_ts - self.start_ts) as f32;
        min_pos + t * (max_pos - min_pos)
    }
}
