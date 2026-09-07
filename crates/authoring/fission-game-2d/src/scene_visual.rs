use std::sync::Arc;

use fission_core::internal::{
    InternalIrBuilder, InternalLowerer, InternalLoweringCx, InternalRenderNode,
};
use fission_core::ui::Widget;
use fission_core::{Op, WidgetId};
use fission_ir::{Role, Semantics, StructuralOp};

const ACTIONLESS_VISUAL_SLOT: u32 = 0x5649_5355;
const RETAINED_METADATA_VISUAL_SLOT: u32 = 0x4D45_5441;
const SCENE_SEMANTICS_OWNER_SLOT: u32 = 0x5345_4D41;

/// Retains decorative scene paint while excluding its complete subtree from
/// coordinate hit testing. The outer custom-widget node keeps the scene
/// object's stable identity; this marker is its stable, derived child.
#[derive(Clone, Debug)]
pub(crate) struct ActionlessSceneVisual {
    id: WidgetId,
    child: Widget,
}

impl ActionlessSceneVisual {
    pub(crate) fn new(id: WidgetId, child: Widget) -> Self {
        Self { id, child }
    }

    /// Wraps a child that already owns `retained_id` without shadowing that
    /// semantic identity with the custom widget's structural owner.
    pub(crate) fn preserving_child_identity(retained_id: WidgetId, child: Widget) -> Self {
        Self::new(
            WidgetId::derived(retained_id.as_u128(), &[RETAINED_METADATA_VISUAL_SLOT]),
            child,
        )
    }
}

impl InternalLowerer for ActionlessSceneVisual {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let marker_id = WidgetId::derived(self.id.as_u128(), &[ACTIONLESS_VISUAL_SLOT]);

        cx.push_scope(marker_id);
        let child_root = cx.next_node_id();
        let child_id = fission_core::internal::lower_widget_with_root(&self.child, cx, child_root);
        cx.pop_scope();

        let mut marker =
            InternalIrBuilder::new(marker_id, Op::Structural(StructuralOp::PointerTransparent));
        marker.add_child(child_id);
        marker.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(self.id)
    }

    fn stable_key(&self) -> u64 {
        u64::from(ACTIONLESS_VISUAL_SLOT)
    }
}

impl From<ActionlessSceneVisual> for Widget {
    fn from(visual: ActionlessSceneVisual) -> Self {
        fission_core::internal::custom_render_widget(InternalRenderNode {
            debug_tag: "ActionlessSceneVisual".into(),
            lowerer: Some(Arc::new(visual)),
            render_object: None,
        })
    }
}

/// Retains non-tap scene metadata with a generic role, including disabled
/// state, without presenting the object as a button.
#[derive(Clone, Debug)]
pub(crate) struct SceneObjectSemantics {
    owner_id: WidgetId,
    semantic_id: WidgetId,
    label: String,
    identifier: Option<String>,
    disabled: bool,
    child: Widget,
}

impl SceneObjectSemantics {
    pub(crate) fn new(
        semantic_id: WidgetId,
        label: String,
        identifier: Option<String>,
        disabled: bool,
        child: Widget,
    ) -> Self {
        Self {
            owner_id: WidgetId::derived(semantic_id.as_u128(), &[SCENE_SEMANTICS_OWNER_SLOT]),
            semantic_id,
            label,
            identifier,
            disabled,
            child,
        }
    }
}

impl InternalLowerer for SceneObjectSemantics {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        cx.push_scope(self.semantic_id);
        let child_root = cx.next_node_id();
        let child_id = fission_core::internal::lower_widget_with_root(&self.child, cx, child_root);
        cx.pop_scope();

        let mut semantic = InternalIrBuilder::new(
            self.semantic_id,
            Op::Semantics(Semantics {
                role: Role::Generic,
                label: Some(self.label.clone()),
                identifier: self.identifier.clone(),
                disabled: self.disabled,
                ..Default::default()
            }),
        );
        semantic.add_child(child_id);
        semantic.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(self.owner_id)
    }

    fn stable_key(&self) -> u64 {
        u64::from(SCENE_SEMANTICS_OWNER_SLOT)
    }
}

impl From<SceneObjectSemantics> for Widget {
    fn from(semantics: SceneObjectSemantics) -> Self {
        fission_core::internal::custom_render_widget(InternalRenderNode {
            debug_tag: "SceneObjectSemantics".into(),
            lowerer: Some(Arc::new(semantics)),
            render_object: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use fission_core::env::{Env, RuntimeState};
    use fission_core::ui::Container;
    use fission_ir::op::Color;

    use super::*;

    #[test]
    fn actionless_visual_retains_identity_behind_pointer_transparent_marker() {
        let owner_id = WidgetId::explicit("scene-decoration");
        let widget: Widget = ActionlessSceneVisual::new(
            owner_id,
            Container::default().bg(Color::BLUE).size(40.0, 30.0).into(),
        )
        .into();
        let env = Env::default();
        let runtime = RuntimeState::default();
        let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);

        let root = fission_core::internal::lower_widget(&widget, &mut cx);

        assert_eq!(root, owner_id);
        let owner = cx.ir.nodes.get(&root).expect("retained scene owner");
        assert_eq!(owner.children.len(), 1);
        let marker = cx
            .ir
            .nodes
            .get(&owner.children[0])
            .expect("pointer-transparent marker");
        assert!(matches!(
            &marker.op,
            Op::Structural(StructuralOp::PointerTransparent)
        ));
        assert_eq!(marker.children.len(), 1);
    }
}
