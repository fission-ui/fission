//! Text editing state kept in step with focus changes.

use super::*;

impl Runtime {
    pub(super) fn clear_text_pending_on_blur(
        &mut self,
        old_focus: Option<WidgetId>,
        new_focus: Option<WidgetId>,
    ) {
        if old_focus == new_focus {
            return;
        }
        if let Some(old_id) = old_focus {
            if let Some(st) = self.runtime_state.text_edit.states.get_mut(&old_id) {
                st.pending_model_sync = false;
                st.clear_preedit();
            }
        }
    }

    pub(super) fn text_input_value(&self, ir: &CoreIR, id: WidgetId) -> crate::TextEditingValue {
        self.runtime_state
            .text_edit
            .get(id)
            .map(|state| state.editing_value())
            .unwrap_or_else(|| {
                let text = ir
                    .nodes
                    .get(&id)
                    .and_then(|node| match &node.op {
                        Op::Semantics(semantics) => semantics.value.clone(),
                        _ => None,
                    })
                    .unwrap_or_default();
                crate::TextEditingValue::from_text(text)
            })
    }
}
