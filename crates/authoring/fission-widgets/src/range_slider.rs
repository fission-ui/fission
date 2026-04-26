use fission_core::lowering::wrap_zstack_child;
use fission_core::ui::Node;
use fission_core::{
    ActionEnvelope, BuildCtx, LowerDyn, LoweringContext, NodeBuilder, View, Widget,
};
use fission_ir::{
    op::{Fill, Stroke},
    FlexDirection, LayoutOp, NodeId, Op, PaintOp,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RangeSlider {
    pub id: Option<NodeId>,
    pub start: f32,
    pub end: f32,
    pub min: f32,
    pub max: f32,
    pub on_change: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for RangeSlider {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Custom(fission_core::ui::CustomNode {
            debug_tag: "RangeSlider".into(),
            lowerer: Some(std::sync::Arc::new(RangeSliderLowerer {
                id: self.id,
                start: self.start,
                end: self.end,
                min: self.min,
                max: self.max,
            })),
            render_object: None,
        })
    }
}

#[derive(Debug)]
struct RangeSliderLowerer {
    id: Option<NodeId>,
    start: f32,
    end: f32,
    min: f32,
    max: f32,
}

impl LowerDyn for RangeSliderLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let tokens = &cx.env.theme.tokens;
        let thumb_size = 16.0;
        let track_height = 4.0;

        let range = (self.max - self.min).max(0.0001);
        let start_pct = ((self.start - self.min) / range).clamp(0.0, 1.0) * 100.0;
        let end_pct = ((self.end - self.min) / range).clamp(0.0, 1.0) * 100.0;

        // Track layer
        let track_layer = {
            let track_paint = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(fission_ir::op::Fill::Solid(
                        tokens.colors.border,
                    )),
                    stroke: None,
                    corner_radius: track_height / 2.0,
                    shadow: None,
                }),
            )
            .build(cx);

            let mut track_box = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box {
                    width: None,
                    height: Some(track_height),
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    aspect_ratio: None,
                }),
            );
            track_box.add_child(track_paint);
            let track_box_id = track_box.build(cx);

            let p_y = (thumb_size - track_height) / 2.0;
            let mut track_container = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box {
                    width: None,
                    height: None,
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0, 0.0, p_y, p_y],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    aspect_ratio: None,
                }),
            );
            track_container.add_child(track_box_id); // Wait, previous slider impl used Inner AbsoluteFill inside padded box.
                                                     // Here track_box IS the inner box. Wait, track_box is fixed height.
                                                     // If I put it in padded box, it works if container constrains width?
                                                     // Actually, ZStack constrains width.
            track_container.build(cx)
        };

        // Thumbs Layer (Grid with 5 columns: space, thumb1, space, thumb2, space)
        // Col 1: start_pct
        // Col 2: thumb_size
        // Col 3: end_pct - start_pct - thumb_size (approx)
        // Actually percent tracks are easier.
        // Col 1: start_pct
        // Col 2: thumb_size
        // Col 3: (end_pct - start_pct)
        // Col 4: thumb_size
        // Col 5: 1fr (remaining)

        // Problem: Pct is relative to total width.
        // We want absolute positioning relative to width.
        // Grid with percentages works.
        // But spacing between thumbs?
        // Let's use `GridTrack::Percent`.

        let mut grid = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Grid {
                columns: vec![
                    fission_ir::op::GridTrack::Percent(start_pct),
                    fission_ir::op::GridTrack::Points(thumb_size),
                    fission_ir::op::GridTrack::Percent(end_pct - start_pct),
                    fission_ir::op::GridTrack::Points(thumb_size),
                    fission_ir::op::GridTrack::Fr(1.0),
                ],
                rows: vec![fission_ir::op::GridTrack::Points(thumb_size)],
                column_gap: None,
                row_gap: None,
                padding: [0.0; 4],
            }),
        );

        // Thumb 1
        let thumb1 = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_ir::op::Fill::Solid(
                    tokens.colors.primary,
                )),
                stroke: None,
                corner_radius: thumb_size / 2.0,
                shadow: None,
            }),
        )
        .build(cx);
        let mut item1 = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::GridItem {
                row_start: fission_ir::op::GridPlacement::Line(1),
                row_end: fission_ir::op::GridPlacement::Auto,
                col_start: fission_ir::op::GridPlacement::Line(2),
                col_end: fission_ir::op::GridPlacement::Auto,
            }),
        );
        let mut box1 = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: Some(thumb_size),
                height: Some(thumb_size),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        );
        box1.add_child(thumb1);
        item1.add_child(box1.build(cx));
        grid.add_child(item1.build(cx));

        // Thumb 2
        let thumb2 = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_ir::op::Fill::Solid(
                    tokens.colors.primary,
                )),
                stroke: None,
                corner_radius: thumb_size / 2.0,
                shadow: None,
            }),
        )
        .build(cx);
        let mut item2 = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::GridItem {
                row_start: fission_ir::op::GridPlacement::Line(1),
                row_end: fission_ir::op::GridPlacement::Auto,
                col_start: fission_ir::op::GridPlacement::Line(4),
                col_end: fission_ir::op::GridPlacement::Auto,
            }),
        );
        let mut box2 = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: Some(thumb_size),
                height: Some(thumb_size),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        );
        box2.add_child(thumb2);
        item2.add_child(box2.build(cx));
        grid.add_child(item2.build(cx));

        let thumb_layer = grid.build(cx);

        let zstack_id = cx.next_node_id();
        cx.push_scope(zstack_id);
        let track_wrapped = wrap_zstack_child(cx, track_layer);
        let thumb_wrapped = wrap_zstack_child(cx, thumb_layer);
        cx.pop_scope();

        let mut zstack = NodeBuilder::new(zstack_id, Op::Layout(LayoutOp::ZStack));
        zstack.add_child(track_wrapped);
        zstack.add_child(thumb_wrapped);

        cx.pop_scope();
        zstack.build(cx)
    }

    fn stable_key(&self) -> u64 {
        // Hash inputs
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        (self.start as u32).hash(&mut hasher);
        (self.end as u32).hash(&mut hasher);
        hasher.finish()
    }
}
