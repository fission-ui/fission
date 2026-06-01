use crate::calendar::Calendar;
use crate::popover::Popover;
use chrono::{Datelike, NaiveDate};
use fission_core::ui::TextInput;
use fission_core::{ActionEnvelope, BuildCtx, View, Widget, WidgetNodeId};
use std::sync::Arc;

#[derive(Clone)]
pub struct DatePicker {
    pub id: WidgetNodeId,
    pub value: Option<NaiveDate>,
    pub is_open: bool,
    pub width: Option<f32>,
    pub on_change: Option<Arc<dyn Fn(NaiveDate) -> ActionEnvelope + Send + Sync>>,
    pub on_toggle: Option<ActionEnvelope>,
    pub on_close: Option<ActionEnvelope>,
}

impl std::fmt::Debug for DatePicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatePicker")
            .field("id", &self.id)
            .field("value", &self.value)
            .field("is_open", &self.is_open)
            .finish()
    }
}

impl<S: fission_core::AppState> Widget<S> for DatePicker {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget({
            let text = self
                .value
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            let viewport = view.viewport_size();
            let preferred_width = self.width.unwrap_or(164.0);
            let clamped_width = if viewport.width.is_finite() && viewport.width > 0.0 {
                preferred_width.min((viewport.width - 48.0).max(120.0))
            } else {
                preferred_width
            };

            let _trigger = TextInput {
                value: text.clone(),
                placeholder: Some("YYYY-MM-DD".into()),
                on_change: None, // Read-only via text for now, or parse?
                // If we want to toggle on click, we need to wrap it or use a Button disguised as Input.
                // TextInput captures focus.
                // Better: Button with TextInput look?
                // Or TextInput with `disabled: true` (but styles might look disabled).
                // Or TextInput with `on_focus` triggering open?
                // Let's use Button for Trigger for MVP stability.
                ..Default::default()
            }
            .into_node();

            // Wrap trigger in GestureDetector to handle click if TextInput consumes it?
            // Actually, let's use a Button for the trigger to ensure click works.
            use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Text};
            let trigger_btn = Button::<fission_core::ui::Node> {
                variant: ButtonVariant::Outline,
                child: Some(Box::new(
                    Text::new(if text.is_empty() {
                        "Select Date".to_string()
                    } else {
                        text
                    })
                    .into_node(),
                )),
                on_press: self.on_toggle.clone(),
                width: Some(clamped_width),
                height: Some(36.0),
                padding: Some([12.0, 12.0, 8.0, 8.0]),
                content_align: ButtonContentAlign::Start,
                ..Default::default()
            }
            .into_node();

            let content = if self.is_open {
                let today = chrono::Local::now().date_naive();
                let display_date = self.value.unwrap_or(today);

                Box::new(fission_core::view::lower_widget_to_node(
                    &Calendar {
                        year: display_date.year(),
                        month: display_date.month(),
                        selected_date: self.value,
                        on_select: self.on_change.clone(), // When selected, close? logic handles that
                        on_navigate: None, // TODO: Wiring navigation state requires DatePicker to own month state?
                        cell_size: None,
                        padding: None,
                        // Yes, DatePicker needs `view_month` state separate from `value`.
                        // For MVP, we navigate relative to `value` or `today`.
                        // Calendar needs `on_navigate` to update `view_month`.
                        // DatePicker doesn't store `view_month` in this struct.
                        // It relies on AppState.
                        // User must provide `view_month` in AppState?
                        // Yes, standard Fission pattern.
                    },
                    _ctx,
                    view,
                ))
            } else {
                Box::new(fission_core::ui::widgets::spacer::Spacer::default().into_node())
            };

            fission_core::view::lower_widget_to_node(
                &Popover {
                    id: self.id,
                    is_open: self.is_open,
                    on_toggle: self.on_toggle.clone(),
                    on_close: self.on_close.clone(),
                    trigger: Box::new(trigger_btn),
                    content,
                },
                _ctx,
                view,
            )
        })
    }
}
