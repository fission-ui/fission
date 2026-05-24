use crate::date_picker::DatePicker;
use crate::stack::HStack;
use chrono::NaiveDate;
use fission_core::ui::{Node, Text};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget, WidgetNodeId};
use std::sync::Arc;

pub struct DateRangePicker {
    pub id_start: WidgetNodeId,
    pub id_end: WidgetNodeId,
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

impl<S: fission_core::AppState> Widget<S> for DateRangePicker {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let cb = self.on_change.clone();
        let s = self.start;
        let e = self.end;

        HStack {
            spacing: Some(8.0),
            children: vec![
                DatePicker {
                    id: self.id_start,
                    value: self.start,
                    is_open: self.is_start_open,
                    width: None,
                    view_year: None,
                    view_month: None,
                    on_navigate: None,
                    on_change: cb.clone().map(|f| {
                        Arc::new(move |d| f(Some(d), e))
                            as Arc<dyn Fn(NaiveDate) -> ActionEnvelope + Send + Sync>
                    }),
                    on_toggle: self.on_toggle_start.clone(),
                    on_close: self.on_close_start.clone(),
                }
                .build(_ctx, view),
                Text::new("-").into_node(),
                DatePicker {
                    id: self.id_end,
                    value: self.end,
                    is_open: self.is_end_open,
                    width: None,
                    view_year: None,
                    view_month: None,
                    on_navigate: None,
                    on_change: cb.map(|f| {
                        Arc::new(move |d| f(s, Some(d)))
                            as Arc<dyn Fn(NaiveDate) -> ActionEnvelope + Send + Sync>
                    }),
                    on_toggle: self.on_toggle_end.clone(),
                    on_close: self.on_close_end.clone(),
                }
                .build(_ctx, view),
            ],
        }
        .into_node()
    }
}
