use fission_core::ui::{Node, TextInput};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope, WidgetNodeId};
use crate::popover::Popover;
use crate::calendar::Calendar;
use chrono::{Datelike, NaiveDate};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

pub struct DatePicker {
    pub id: WidgetNodeId,
    pub value: Option<NaiveDate>,
    pub is_open: bool,
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
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let text = self.value.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
        
        let trigger = TextInput {
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
        }.into_node();
        
        // Wrap trigger in GestureDetector to handle click if TextInput consumes it?
        // Actually, let's use a Button for the trigger to ensure click works.
        use fission_core::ui::{Button, ButtonVariant, Text};
        let trigger_btn = Button {
            variant: ButtonVariant::Outline,
            child: Some(Box::new(Text::new(if text.is_empty() { "Select Date".to_string() } else { text }).into_node())),
            on_press: self.on_toggle.clone(),
            width: Some(150.0),
            ..Default::default()
        }.into_node();

        let content = if self.is_open {
            let today = chrono::Local::now().date_naive();
            let display_date = self.value.unwrap_or(today);
            
            Box::new(Calendar {
                year: display_date.year(),
                month: display_date.month(),
                selected_date: self.value,
                on_select: self.on_change.clone(), // When selected, close? logic handles that
                on_navigate: None, // TODO: Wiring navigation state requires DatePicker to own month state?
                // Yes, DatePicker needs `view_month` state separate from `value`.
                // For MVP, we navigate relative to `value` or `today`.
                // Calendar needs `on_navigate` to update `view_month`.
                // DatePicker doesn't store `view_month` in this struct.
                // It relies on AppState.
                // User must provide `view_month` in AppState?
                // Yes, standard Fission pattern.
            }.build(_ctx, view))
        } else {
            Box::new(fission_core::ui::widgets::spacer::Spacer::default().into_node())
        };

        Popover {
            id: self.id,
            is_open: self.is_open,
            on_toggle: self.on_toggle.clone(),
            on_close: self.on_close.clone(),
            trigger: Box::new(trigger_btn),
            content,
        }.build(_ctx, view)
    }
}
