use fission_core::ui::{Node, Text};
use fission_core::{BuildCtx, View, Widget, ActionEnvelope};
use crate::stack::HStack;
use crate::number_input::NumberInput;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

pub struct TimePicker {
    pub hour: u32, // 0-23
    pub minute: u32, // 0-59
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

impl<S: fission_core::AppState> Widget<S> for TimePicker {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let cb = self.on_change.as_ref();
        let h = self.hour;
        let m = self.minute;

        // Hour Envelopes
        let h_inc = cb.map(|f| f((h + 1) % 24, m));
        let h_dec = cb.map(|f| f(if h == 0 { 23 } else { h - 1 }, m));
        
        // Minute Envelopes
        let m_inc = cb.map(|f| f(h, (m + 1) % 60));
        let m_dec = cb.map(|f| f(h, if m == 0 { 59 } else { m - 1 }));

        HStack {
            spacing: Some(4.0),
            children: vec![
                NumberInput {
                    value: h as f32,
                    min: Some(0.0), max: Some(23.0),
                    step: 1.0,
                    on_increment: h_inc,
                    on_decrement: h_dec,
                    ..Default::default()
                }.build(_ctx, view),
                
                Text::new(":").into_node(),
                
                NumberInput {
                    value: m as f32,
                    min: Some(0.0), max: Some(59.0),
                    step: 1.0,
                    on_increment: m_inc,
                    on_decrement: m_dec,
                    ..Default::default()
                }.build(_ctx, view),
            ]
        }.into_node()
    }
}
