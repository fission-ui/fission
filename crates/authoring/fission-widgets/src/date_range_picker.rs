use crate::date_picker::DatePicker;
use crate::stack::HStack;
use chrono::NaiveDate;
use fission_core::ui::{Text, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use std::sync::Arc;

/// Controlled inclusive start/end date range composed from two date pickers.
pub struct DateRangePicker {
    /// Stable identity of the start-date field and popup.
    pub id_start: WidgetId,
    /// Stable identity of the end-date field and popup.
    pub id_end: WidgetId,
    /// Optional inclusive start date.
    pub start: Option<NaiveDate>,
    /// Optional inclusive end date.
    pub end: Option<NaiveDate>,
    /// Whether the controlled start-date popup is open.
    pub is_start_open: bool,
    /// Whether the controlled end-date popup is open.
    pub is_end_open: bool,
    /// Factory producing an action for the proposed complete range.
    pub on_change:
        Option<Arc<dyn Fn(Option<NaiveDate>, Option<NaiveDate>) -> ActionEnvelope + Send + Sync>>,
    /// Action requesting a start-popup state toggle.
    pub on_toggle_start: Option<ActionEnvelope>,
    /// Action requesting an end-popup state toggle.
    pub on_toggle_end: Option<ActionEnvelope>,
    /// Action dispatched when the start popup requests dismissal.
    pub on_close_start: Option<ActionEnvelope>,
    /// Action dispatched when the end popup requests dismissal.
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
                    view_year: None,
                    view_month: None,
                    on_navigate: None,
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
                    view_year: None,
                    view_month: None,
                    on_navigate: None,
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
