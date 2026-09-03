use crate::calendar::Calendar;
use crate::popover::Popover;
use chrono::{Datelike, NaiveDate};
use fission_core::ui::{TextInput, Widget};
use fission_core::{ActionEnvelope, WidgetId};
use std::sync::Arc;

/// Controlled date field with an anchored calendar popup.
pub struct DatePicker {
    /// Stable identity used for the field and popup anchor.
    pub id: WidgetId,
    /// Current selected date, or `None` for an empty field.
    pub value: Option<NaiveDate>,
    /// Whether the controlled calendar popup is open.
    pub is_open: bool,
    /// Optional preferred logical field and popup width.
    pub width: Option<f32>,
    /// Optional year displayed by the popup, independent of the selected date.
    pub view_year: Option<i32>,
    /// Optional one-based month displayed by the popup.
    pub view_month: Option<u32>,
    /// Factory producing an action for calendar month navigation.
    pub on_navigate: Option<Arc<dyn Fn(i32, u32) -> ActionEnvelope + Send + Sync>>,
    /// Factory producing an action for a newly selected date.
    pub on_change: Option<Arc<dyn Fn(NaiveDate) -> ActionEnvelope + Send + Sync>>,
    /// Action dispatched when the field requests an open-state toggle.
    pub on_toggle: Option<ActionEnvelope>,
    /// Action dispatched when popup behavior requests dismissal.
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

impl From<DatePicker> for Widget {
    fn from(component: DatePicker) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let text = this
            .value
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        let viewport = view.viewport_size();
        let preferred_width = this.width.unwrap_or(164.0);
        let clamped_width = if viewport.width.is_finite() && viewport.width > 0.0 {
            preferred_width.min((viewport.width - 48.0).max(120.0))
        } else {
            preferred_width
        };

        let _trigger: Widget = TextInput {
            value: text.clone(),
            placeholder: Some("YYYY-MM-DD".into()),
            on_input: None, // Read-only via text for now, or parse?
            // If we want to toggle on click, we need to wrap it or use a Button disguised as Input.
            // TextInput captures focus.
            // Better: Button with TextInput look?
            // Or TextInput with `disabled: true` (but styles might look disabled).
            // Or TextInput with `on_focus` triggering open?
            // Let's use Button for Trigger for MVP stability.
            ..Default::default()
        }
        .into();

        // Wrap trigger in GestureDetector to handle click if TextInput consumes it?
        // Actually, let's use a Button for the trigger to ensure click works.
        use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Text};
        let trigger_btn = Button {
            variant: ButtonVariant::Outline,
            child: Some(
                Text::new(if text.is_empty() {
                    "Select Date".to_string()
                } else {
                    text
                })
                .into(),
            ),
            on_press: this.on_toggle.clone(),
            width: Some(clamped_width),
            height: Some(36.0),
            padding: Some([12.0, 12.0, 8.0, 8.0]),
            content_align: ButtonContentAlign::Start,
            ..Default::default()
        }
        .into();

        let content = if this.is_open {
            let today = chrono::Local::now().date_naive();
            let display_date = this.value.unwrap_or(today);

            Calendar {
                year: this.view_year.unwrap_or(display_date.year()),
                month: this.view_month.unwrap_or(display_date.month()),
                selected_date: this.value,
                on_select: this.on_change.clone(),
                on_navigate: this.on_navigate.clone(),
                cell_size: None,
                padding: None,
            }
            .into()
        } else {
            fission_core::ui::widgets::spacer::Spacer::default().into()
        };

        Popover {
            id: this.id,
            is_open: this.is_open,
            on_close: this.on_close.clone(),
            trigger: trigger_btn,
            content,
            motion: None,
        }
        .into()
    }
}
