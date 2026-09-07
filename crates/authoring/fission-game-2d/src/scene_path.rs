use std::sync::Arc;

use fission_core::internal::{
    InternalIrBuilder, InternalLowerer, InternalLoweringCx, InternalRenderNode,
};
use fission_core::ui::Widget;
use fission_core::{LayoutOp, Op, WidgetId};
use fission_game::SceneNodeId;
use fission_ir::op::{BoxStyle, Fill, Length, PaintOp, Stroke};
use fission_ir::{CompositeScalar, CompositeStyle};

// The scene identity belongs to the interaction wrapper. Give the visual its
// own stable identity so an interactive path cannot produce duplicate IR IDs.
const SCENE_PATH_VISUAL_SLOT: u32 = 0x5041_5448;
const SCENE_PATH_LAYOUT_SLOT: u32 = 0;
const SCENE_PATH_PAINT_SLOT: u32 = 1;

#[derive(Clone, Debug)]
pub(crate) struct ScenePathLayer {
    id: WidgetId,
    path: String,
    width: f32,
    height: f32,
    fill: Option<Fill>,
    stroke: Option<Stroke>,
    opacity: f32,
}

impl ScenePathLayer {
    pub(crate) fn new(
        scene_id: &SceneNodeId,
        path: String,
        width: f32,
        height: f32,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
        opacity: f32,
    ) -> Self {
        Self {
            id: WidgetId::derived(scene_id.widget_id().as_u128(), &[SCENE_PATH_VISUAL_SLOT]),
            path,
            width,
            height,
            fill,
            stroke,
            opacity,
        }
    }
}

impl InternalLowerer for ScenePathLayer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let paint = InternalIrBuilder::new(
            WidgetId::derived(self.id.as_u128(), &[SCENE_PATH_PAINT_SLOT]),
            Op::Paint(PaintOp::DrawPath {
                path: self.path.clone(),
                fill: self.fill.clone(),
                stroke: self.stroke.clone(),
            }),
        )
        .build(cx);

        let mut layout = InternalIrBuilder::new(
            WidgetId::derived(self.id.as_u128(), &[SCENE_PATH_LAYOUT_SLOT]),
            Op::Layout(LayoutOp::StyledBox {
                style: BoxStyle {
                    width: Some(Length::points(self.width)),
                    height: Some(Length::points(self.height)),
                    ..Default::default()
                },
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
        )
        .composite(CompositeStyle {
            opacity: (self.opacity < 1.0)
                .then(|| CompositeScalar::new(self.opacity.clamp(0.0, 1.0))),
            clip_to_bounds: true,
            ..Default::default()
        });
        layout.add_child(paint);
        layout.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(self.id)
    }

    fn stable_key(&self) -> u64 {
        // The wrapper's shape is invariant. Its layout, paint, and composite
        // values live on stable child nodes and must be diffed there.
        u64::from(SCENE_PATH_VISUAL_SLOT)
    }
}

impl From<ScenePathLayer> for Widget {
    fn from(layer: ScenePathLayer) -> Self {
        fission_core::internal::custom_render_widget(InternalRenderNode {
            debug_tag: "Scene2DPathLayer".into(),
            lowerer: Some(Arc::new(layer)),
            render_object: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use fission_core::env::{Env, RuntimeState};
    use fission_ir::op::{Color, LineCap, LineJoin};

    use super::*;

    #[test]
    fn path_layer_uses_distinct_ids_and_preserves_path_paint() {
        let scene_id = SceneNodeId::from_key(&41_u32);
        let owner_id = scene_id.widget_id();
        let fill = Fill::Solid(Color::BLUE);
        let stroke = Stroke {
            fill: Fill::Solid(Color::BLACK),
            width: 2.5,
            dash_array: Some(vec![3.0, 1.0]),
            line_cap: LineCap::Round,
            line_join: LineJoin::Bevel,
        };
        let widget: Widget = ScenePathLayer::new(
            &scene_id,
            "M0 0 L30 0 L15 12 Z".into(),
            30.0,
            12.0,
            Some(fill.clone()),
            Some(stroke.clone()),
            0.4,
        )
        .into();
        let env = Env::default();
        let runtime = RuntimeState::default();
        let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);

        let root = fission_core::internal::lower_widget(&widget, &mut cx);

        assert_ne!(root, owner_id);
        assert_eq!(
            root,
            WidgetId::derived(owner_id.as_u128(), &[SCENE_PATH_VISUAL_SLOT])
        );
        let wrapper = cx.ir.nodes.get(&root).expect("path visual wrapper");
        assert_eq!(wrapper.children.len(), 1);
        let layout = cx.ir.nodes.get(&wrapper.children[0]).expect("path layout");
        assert_ne!(layout.id, owner_id);
        assert_eq!(layout.children.len(), 1);
        let paint = cx.ir.nodes.get(&layout.children[0]).expect("path paint");
        assert_ne!(paint.id, owner_id);
        assert_ne!(paint.id, layout.id);
        assert!(matches!(
            &paint.op,
            Op::Paint(PaintOp::DrawPath {
                path,
                fill: actual_fill,
                stroke: actual_stroke,
            }) if path == "M0 0 L30 0 L15 12 Z"
                && actual_fill.as_ref() == Some(&fill)
                && actual_stroke.as_ref() == Some(&stroke)
        ));
    }

    #[test]
    fn path_layer_clips_exact_bounds_and_only_composites_partial_opacity() {
        let scene_id = SceneNodeId::from_key(&42_u32);
        let lower = |opacity| {
            let widget: Widget = ScenePathLayer::new(
                &scene_id,
                "M-10 -10 L50 50".into(),
                31.0,
                23.0,
                None,
                Some(Stroke {
                    fill: Fill::Solid(Color::BLACK),
                    width: 1.0,
                    dash_array: None,
                    line_cap: LineCap::Butt,
                    line_join: LineJoin::Miter,
                }),
                opacity,
            )
            .into();
            let env = Env::default();
            let runtime = RuntimeState::default();
            let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
            let root = fission_core::internal::lower_widget(&widget, &mut cx);
            let layout_id = cx.ir.nodes[&root].children[0];
            cx.ir.nodes[&layout_id].clone()
        };

        let partial = lower(0.35);
        let Op::Layout(LayoutOp::StyledBox { style, .. }) = &partial.op else {
            panic!("path visual should lower through an exact-size styled box");
        };
        assert_eq!(style.width, Some(Length::points(31.0)));
        assert_eq!(style.height, Some(Length::points(23.0)));
        assert!(partial.composite.clip_to_bounds);
        assert_eq!(partial.composite.opacity, Some(CompositeScalar::new(0.35)));

        let opaque = lower(1.0);
        assert!(opaque.composite.clip_to_bounds);
        assert_eq!(opaque.composite.opacity, None);
    }

    #[test]
    fn successive_path_data_change_dirties_only_the_stable_paint_node() {
        let scene_id = SceneNodeId::from_key(&43_u32);
        let lower = |path: &str| {
            let widget: Widget = ScenePathLayer::new(
                &scene_id,
                path.into(),
                31.0,
                23.0,
                Some(Fill::Solid(Color::BLUE)),
                None,
                1.0,
            )
            .into();
            fission_core::internal::lower_widget_to_ir(&widget)
        };

        let previous = lower("M0 0 L31 0 L15 10 Z");
        let next = lower("M0 1 L31 3 L15 12 Z");
        let visual_id =
            WidgetId::derived(scene_id.widget_id().as_u128(), &[SCENE_PATH_VISUAL_SLOT]);
        let layout_id = WidgetId::derived(visual_id.as_u128(), &[SCENE_PATH_LAYOUT_SLOT]);
        let paint_id = WidgetId::derived(visual_id.as_u128(), &[SCENE_PATH_PAINT_SLOT]);

        assert_eq!(previous.root, Some(visual_id));
        assert_eq!(next.root, Some(visual_id));
        assert_eq!(previous.nodes[&visual_id].children, vec![layout_id]);
        assert_eq!(next.nodes[&visual_id].children, vec![layout_id]);
        assert_eq!(previous.nodes[&layout_id].children, vec![paint_id]);
        assert_eq!(next.nodes[&layout_id].children, vec![paint_id]);

        let diff = fission_core::diff::diff_ir(&previous, &next);
        assert!(
            diff.dirty_layout.is_empty(),
            "path-data-only updates must not invalidate layout: {:?}",
            diff.dirty_layout
        );
        assert!(
            diff.dirty_composite.is_empty(),
            "path-data-only updates must not invalidate compositing: {:?}",
            diff.dirty_composite
        );
        assert_eq!(
            diff.dirty_paint,
            std::collections::HashSet::from([paint_id])
        );
    }
}
