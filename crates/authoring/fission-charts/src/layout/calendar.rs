use chrono::{Datelike, NaiveDate};

#[derive(Debug, Clone)]
pub struct CalendarGrid {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub cell_size: f32,
    pub cell_padding: f32,
}

impl CalendarGrid {
    pub fn new(
        start_date: NaiveDate,
        end_date: NaiveDate,
        cell_size: f32,
        cell_padding: f32,
    ) -> Self {
        Self {
            start_date,
            end_date,
            cell_size,
            cell_padding,
        }
    }

    /// Maps a given date to an [x, y] position in a 7-day grid.
    /// x represents the week column, y represents the day of the week row.
    pub fn map_date(&self, date: NaiveDate) -> Option<[f32; 2]> {
        if date < self.start_date || date > self.end_date {
            return None;
        }

        let days_since_start = (date - self.start_date).num_days() as i64;
        let start_dow = self.start_date.weekday().num_days_from_sunday() as i64;

        let total_days_offset = start_dow + days_since_start;
        let week_col = total_days_offset / 7;
        let day_row = total_days_offset % 7;

        let first_col_offset = start_dow / 7;

        let col = week_col - first_col_offset;
        let row = day_row;

        let step = self.cell_size + self.cell_padding;
        let x = col as f32 * step;
        let y = row as f32 * step;

        Some([x, y])
    }

    pub fn map_timestamp(&self, timestamp_secs: i64) -> Option<[f32; 2]> {
        let date = chrono::DateTime::from_timestamp(timestamp_secs, 0)?.date_naive();
        self.map_date(date)
    }
}
