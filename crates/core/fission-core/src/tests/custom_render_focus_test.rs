use crate::env::{Env, RuntimeState};
use crate::internal::{
    CustomRenderObject, InternalIrBuilder, InternalLowerer, InternalLoweringCx, InternalRenderNode,
};
use crate::Runtime;
use fission_ir::{LayoutOp, Op, WidgetId};
use std::sync::Arc;

#[derive(Debug)]
struct TextSurfaceLowerer {
    id: WidgetId,
}

impl InternalLowerer for TextSurfaceLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: Some(320.0),
                height: Some(180.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
        )
        .build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(self.id)
    }
}

#[derive(Debug)]
struct TextSurfaceRenderObject;

impl CustomRenderObject for TextSurfaceRenderObject {
    fn accepts_text_input(&self) -> bool {
        true
    }
}

fn lower_text_surface(id: WidgetId) -> fission_ir::CoreIR {
    let widget = crate::internal::custom_render_widget(InternalRenderNode {
        debug_tag: "TextSurface".into(),
        lowerer: Some(Arc::new(TextSurfaceLowerer { id })),
        render_object: Some(Arc::new(TextSurfaceRenderObject)),
    });
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
    let root = widget.lower(&mut cx);
    cx.ir.root = Some(root);
    cx.ir
}

#[test]
fn custom_text_input_keeps_focus_across_rebuilds() {
    let id = WidgetId::explicit("custom-text-surface");
    let first = lower_text_surface(id);
    let second = lower_text_surface(id);

    assert_eq!(first.root, Some(id));
    assert_eq!(second.root, Some(id));
    assert_eq!(second.custom_render_objects.len(), 1);
    assert!(second.custom_render_objects.contains_key(&id));

    let mut runtime = Runtime::default();
    runtime.runtime_state.interaction.set_focused(Some(id));

    assert!(!runtime.reconcile_focus(&second).unwrap());
    assert_eq!(runtime.runtime_state.interaction.focused, Some(id));
}
