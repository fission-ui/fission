use std::hash::{Hash, Hasher};
use std::sync::Arc;

use fission_core::internal::{InternalLowerer, InternalLoweringCx, InternalRenderNode};
use fission_core::ui::Widget;
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::{ActionEntry, ActionSet, ActionTrigger, CanvasTarget, Op, Role, Semantics};

#[derive(Debug, Clone)]
pub(crate) struct CanvasInteractionRegion {
    pub id: WidgetId,
    pub child: Widget,
    pub identifier: String,
    pub target: CanvasTarget,
    pub on_activate: Option<ActionEnvelope>,
    pub on_drag: Option<ActionEnvelope>,
}

impl InternalLowerer for CanvasInteractionRegion {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        cx.push_scope(self.id);
        let child = fission_core::internal::lower_widget(&self.child, cx);
        cx.pop_scope();
        let mut entries = Vec::new();
        if let Some(action) = &self.on_activate {
            entries.push(entry(action, ActionTrigger::Default));
        }
        if let Some(action) = &self.on_drag {
            entries.extend([
                entry(action, ActionTrigger::DragStart),
                entry(action, ActionTrigger::DragUpdate),
                entry(action, ActionTrigger::DragEnd),
            ]);
        }
        cx.insert_node(
            self.id,
            Op::Semantics(Semantics {
                role: Role::Generic,
                identifier: Some(self.identifier.clone()),
                actions: ActionSet { entries },
                canvas_target: Some(self.target.clone()),
                focusable: self.on_activate.is_some(),
                draggable: self.on_drag.is_some(),
                ..Semantics::default()
            }),
            vec![child],
        )
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(WidgetId::derived(self.id.as_u128(), &[0xCA11]))
    }

    fn stable_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        self.identifier.hash(&mut hasher);
        self.target.hash(&mut hasher);
        hash_action(&self.on_activate, &mut hasher);
        hash_action(&self.on_drag, &mut hasher);
        hasher.finish()
    }
}

fn hash_action(action: &Option<ActionEnvelope>, hasher: &mut impl Hasher) {
    action
        .as_ref()
        .map(|action| action.id.as_u128())
        .hash(hasher);
    action.as_ref().map(|action| &action.payload).hash(hasher);
}

impl From<CanvasInteractionRegion> for Widget {
    fn from(region: CanvasInteractionRegion) -> Self {
        fission_core::internal::custom_render_widget(InternalRenderNode {
            debug_tag: "InfiniteCanvasInteractionRegion".into(),
            lowerer: Some(Arc::new(region)),
            render_object: None,
        })
    }
}

fn entry(action: &ActionEnvelope, trigger: ActionTrigger) -> ActionEntry {
    ActionEntry {
        trigger,
        action_id: action.id.as_u128(),
        payload_data: Some(action.payload.clone()),
    }
}
