use crate::number_input::NumberInput;
use crate::stack::HStack;
use fission_core::ui::{Text, Widget};
use fission_core::ActionEnvelope;
use std::sync::Arc;

/// Controlled 24-hour time picker with increment and decrement controls.
///
/// The application owns the selected time. `on_change` receives the proposed
/// hour and minute and should dispatch an action that updates application state.
pub struct TimePicker {
    /// Hour in the inclusive range `0..=23`.
    pub hour: u32,
    /// Minute in the inclusive range `0..=59`.
    pub minute: u32,
    /// Factory used to create an action for a proposed `(hour, minute)` value.
    pub on_change: Option<Arc<dyn Fn(u32, u32) -> ActionEnvelope + Send + Sync>>,
}

// Manual Debug
impl std::fmt::Debug for TimePicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimePicker")
            .field("hour", &self.hour)
            .field("minute", &self.minute)
            .finish()
    }
}

impl From<TimePicker> for Widget {
    fn from(component: TimePicker) -> Self {
        let this = &component;

        let cb = this.on_change.as_ref();
        let h = this.hour;
        let m = this.minute;

        // Hour Envelopes
        let h_inc = cb.map(|f| f((h + 1) % 24, m));
        let h_dec = cb.map(|f| f(if h == 0 { 23 } else { h - 1 }, m));

        // Minute Envelopes
        let m_inc = cb.map(|f| f(h, (m + 1) % 60));
        let m_dec = cb.map(|f| f(h, if m == 0 { 59 } else { m - 1 }));

        HStack {
            spacing: Some(8.0),
            children: vec![
                NumberInput {
                    value: h as f32,
                    display_text: Some(format!("{:02}", h)),
                    min: Some(0.0),
                    max: Some(23.0),
                    step: 1.0,
                    field_width: Some(56.0),
                    button_size: Some(32.0),
                    gap: Some(4.0),
                    on_increment: h_inc,
                    on_decrement: h_dec,
                    ..Default::default()
                }
                .into(),
                Text::new(":").size(16.0).into(),
                NumberInput {
                    value: m as f32,
                    display_text: Some(format!("{:02}", m)),
                    min: Some(0.0),
                    max: Some(59.0),
                    step: 1.0,
                    field_width: Some(56.0),
                    button_size: Some(32.0),
                    gap: Some(4.0),
                    on_increment: m_inc,
                    on_decrement: m_dec,
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into()
    }
}
