use crate::stack::{HStack, VStack};
use chrono::{Datelike, Local, NaiveDate};
use fission_core::ui::{Button, ButtonVariant, Container, Text};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget};
use std::sync::Arc;

pub struct Calendar {
    pub year: i32,
    pub month: u32,
    pub selected_date: Option<NaiveDate>,
    pub on_select: Option<Arc<dyn Fn(NaiveDate) -> ActionEnvelope + Send + Sync>>,
    pub on_navigate: Option<Arc<dyn Fn(i32, u32) -> ActionEnvelope + Send + Sync>>,
    pub cell_size: Option<f32>,
    pub padding: Option<f32>,
}

// Manual Debug
impl std::fmt::Debug for Calendar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Calendar")
            .field("year", &self.year)
            .field("month", &self.month)
            .field("selected", &self.selected_date)
            .finish()
    }
}

impl<S: fission_core::AppState> Widget<S> for Calendar {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::AnyWidget::from_node({
            let theme = &view.env.theme.components.calendar;
            let tokens = &view.env.theme.tokens;
            let cell_size = self.cell_size.unwrap_or(36.0);
            let padding = self.padding.unwrap_or(16.0);
            let weekday_text_size = if cell_size <= 32.0 { 12.0 } else { 13.0 };
            let day_text_size = if cell_size <= 32.0 { 13.0 } else { 14.0 };

            let first_day = NaiveDate::from_ymd_opt(self.year, self.month, 1).unwrap();
            let days_in_month = if self.month == 12 {
                NaiveDate::from_ymd_opt(self.year + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(self.year, self.month + 1, 1).unwrap()
            }
            .signed_duration_since(first_day)
            .num_days() as u32;

            let start_weekday = first_day.weekday().num_days_from_sunday(); // 0 = Sun

            // Header
            let prev_cb = self.on_navigate.clone();
            let next_cb = self.on_navigate.clone();
            let (prev_y, prev_m) = if self.month == 1 {
                (self.year - 1, 12)
            } else {
                (self.year, self.month - 1)
            };
            let (next_y, next_m) = if self.month == 12 {
                (self.year + 1, 1)
            } else {
                (self.year, self.month + 1)
            };

            let header = HStack {
                spacing: Some(8.0),
                children: vec![
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new("<").into_node())),
                        on_press: prev_cb.map(|f| f(prev_y, prev_m)),
                        width: Some(cell_size),
                        height: Some(cell_size),
                        ..Default::default()
                    }
                    .into_node(),
                    fission_core::ui::widgets::Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                    Text::new(first_day.format("%B %Y").to_string())
                        .size(tokens.typography.body_large_size)
                        .into_node(),
                    fission_core::ui::widgets::Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new(">").into_node())),
                        on_press: next_cb.map(|f| f(next_y, next_m)),
                        width: Some(cell_size),
                        height: Some(cell_size),
                        ..Default::default()
                    }
                    .into_node(),
                ],
            }
            .into_node();

            // Weekday labels
            let weekdays = ["S", "M", "T", "W", "T", "F", "S"];
            let labels = HStack {
                spacing: Some(0.0),
                children: weekdays
                    .iter()
                    .map(|d| {
                        Container::new(
                            Text::new(d.to_string())
                                .size(weekday_text_size)
                                .color(tokens.colors.text_secondary)
                                .into_node(),
                        )
                        .width(cell_size)
                        .height(cell_size)
                        .into_node()
                    })
                    .collect(),
            }
            .into_node();

            // Days
            let mut days = Vec::new();
            // Padding for start
            for _ in 0..start_weekday {
                days.push(fission_core::ui::widgets::spacer::Spacer::default().into_node());
            }

            for d in 1..=days_in_month {
                let date = NaiveDate::from_ymd_opt(self.year, self.month, d).unwrap();
                let is_selected = self.selected_date == Some(date);
                let is_today = date == Local::now().date_naive();
                let cb = self.on_select.clone();

                let day_button = Button {
                    variant: if is_selected {
                        ButtonVariant::Filled
                    } else {
                        ButtonVariant::Ghost
                    },
                    child: Some(Box::new(
                        Text::new(d.to_string())
                            .size(day_text_size)
                            .color(if is_selected {
                                theme.selected_text
                            } else {
                                tokens.colors.text_primary
                            })
                            .into_node(),
                    )),
                    on_press: cb.map(|f| f(date)),
                    width: Some(cell_size),
                    height: Some(cell_size),
                    padding: Some([0.0; 4]),
                    ..Default::default()
                }
                .into_node();

                let day_node = if is_today && !is_selected {
                    Container::new(day_button)
                        .border(theme.today_outline, 1.0)
                        .border_radius(cell_size / 2.0)
                        .into_node()
                } else {
                    day_button
                };

                days.push(day_node);
            }

            // Grid with 7 columns
            let day_grid = fission_core::ui::Grid {
                columns: vec![fission_ir::op::GridTrack::Points(cell_size); 7],
                rows: vec![],
                children: days,
                column_gap: Some(0.0),
                row_gap: Some(0.0),
                padding: [0.0; 4],
                ..Default::default()
            }
            .into_node();

            let mut c = Container::new(
                VStack {
                    spacing: Some(8.0),
                    children: vec![header, labels, day_grid],
                }
                .into_node(),
            )
            .padding_all(padding)
            .bg(theme.bg_color)
            .border(theme.border_color, 1.0)
            .border_radius(theme.radius);

            if let Some(s) = tokens.elevations.level2 {
                c = c.shadow(s);
            }

            c.into_node()
        })
    }
}
