use crate::date_picker::DatePicker;
use crate::stack::HStack;
use chrono::NaiveDate;
use fission_core::ui::{Text, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use std::sync::Arc;

pub struct DateRangePicker {
    pub id_start: WidgetId,
    pub id_end: WidgetId,
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    pub is_start_open: bool,
    pub is_end_open: bool,
    pub on_change:
        Option<Arc<dyn Fn(Option<NaiveDate>, Option<NaiveDate>) -> ActionEnvelope + Send + Sync>>,
    pub on_toggle_start: Option<ActionEnvelope>,
    pub on_toggle_end: Option<ActionEnvelope>,
    pub on_close_start: Option<ActionEnvelope>,
    pub on_close_end: Option<ActionEnvelope>,
}

impl std::fmt::Debug for DateRangePicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DateRangePicker")
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

impl From<DateRangePicker> for Widget {
    fn from(component: DateRangePicker) -> Self {
        let this = &component;

        let cb = this.on_change.clone();
        let s = this.start;
        let e = this.end;

        HStack {
            spacing: Some(8.0),
            children: vec![
                DatePicker {
                    id: this.id_start,
                    value: this.start,
                    is_open: this.is_start_open,
                    width: None,
                    on_change: cb.clone().map(|f| {
                        Arc::new(move |d| f(Some(d), e))
                            as Arc<dyn Fn(NaiveDate) -> ActionEnvelope + Send + Sync>
                    }),
                    on_toggle: this.on_toggle_start.clone(),
                    on_close: this.on_close_start.clone(),
                }
                .into(),
                Text::new("-").into(),
                DatePicker {
                    id: this.id_end,
                    value: this.end,
                    is_open: this.is_end_open,
                    width: None,
                    on_change: cb.map(|f| {
                        Arc::new(move |d| f(s, Some(d)))
                            as Arc<dyn Fn(NaiveDate) -> ActionEnvelope + Send + Sync>
                    }),
                    on_toggle: this.on_toggle_end.clone(),
                    on_close: this.on_close_end.clone(),
                }
                .into(),
            ],
        }
        .into()
    }
}
