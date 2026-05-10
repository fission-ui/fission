use crate::lowering::wrap_zstack_child;
use crate::ActionEnvelope;
use crate::{Lower, LoweringContext, NodeBuilder};
use fission_ir::{
    op::{Color, Fill, GridTrack, LayoutOp, Op, PaintOp},
    FlexDirection, NodeId,
};
use serde::{Deserialize, Serialize};

/// A continuous value selector rendered as a horizontal track with a draggable
/// thumb.
///
/// The thumb position is determined by `value` within the `[min, max]` range.
/// Dragging dispatches the `on_change` action with the new value carried as
/// pointer input (see [`ActionInput::as_pointer`]).
///
/// # Example
///
/// ```rust,ignore
/// Slider {
///     value: view.state.volume,
///     min: 0.0,
///     max: 1.0,
///     on_change: Some(ctx.bind(
///         VolumeChanged,
///         handle_volume as fn(&mut S, VolumeChanged),
///     )),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slider {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// Current value (clamped to `[min, max]`).
    pub value: f32,
    /// Minimum value (default: 0.0).
    pub min: f32,
    /// Maximum value (default: 1.0).
    pub max: f32,
    /// Action dispatched when the user drags the thumb.
    pub on_change: Option<ActionEnvelope>,
}

impl Slider {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Slider(self)
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self {
            id: None,
            value: 0.0,
            min: 0.0,
            max: 1.0,
            on_change: None,
        }
    }
}

impl Lower for Slider {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let tokens = &cx.env.theme.tokens;
        let thumb_size = 16.0;
        let track_height = 4.0;

        let range = (self.max - self.min).max(0.0001);
        let pct = ((self.value - self.min) / range).clamp(0.0, 1.0) * 100.0;

        // Visual Structure:
        // Grid [Percent(pct), Fixed(thumb), Auto]
        // Row 1: Height(max(track, thumb))

        // Track Background: DrawRect on the main container (centered vertically?)
        // Actually, we want the track to be vertically centered.
        // And Thumb centered.

        // Let's make the root a Grid.
        // It has a background PaintOp for the Track line?
        // If we paint on Root, it fills the whole area.
        // We want track to be thin.
        // So we need a child for Track?
        // But Track must span the whole width.
        // Use ZStack.
        // Layer 1: Track (Centered vertically).
        // Layer 2: Thumb Grid.

        let layout_id = cx.next_node_id();

        // Layer 1: Track
        // A Box with height `track_height`, centered?
        // ZStack stretches children.
        // We can use a Column with `Justify: Center` inside ZStack layer?
        // Or just use `padding` on a box to squish the paint?
        // `PaintOp` `DrawRect` fills the node layout.
        // If we want a thin line, the node layout must be thin.
        // `LayoutOp::Flex` (Column) with `AlignItems: Stretch` and `JustifyContent: Center`.
        // Add a child Box with height `track_height`.
        let track_layer = {
            let track_paint = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(Fill::Solid(tokens.colors.border)), // Inactive track
                    stroke: None,
                    corner_radius: track_height / 2.0,
                    shadow: None,
                }),
            )
            .build(cx);

            let mut track_box = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box {
                    width: None, // Auto width
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
            let _track_box_id = track_box.build(cx);

            // Center vertically
            let _center_col = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Flex {
                    direction: FlexDirection::Row,
                    wrap: fission_ir::op::FlexWrap::NoWrap,
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    padding: [0.0; 4],
                    gap: None,
                    align_items: fission_ir::op::AlignItems::Center,
                    justify_content: fission_ir::op::JustifyContent::Start,
                }),
            );
            // We need `justify_content: center`. `LayoutOp::Flex` maps to `AlignItems: Center` (cross axis) but justification?
            // `fission-layout` hardcodes `justify_content: FlexStart`.
            // So we can't center easily using Flex properties exposed currently.
            // Workaround: Use `Grid` 1x1 with `AlignItems: Center`?
            // `fission-layout` Grid mapping doesn't expose alignment yet.

            // Workaround 2: Use Padding? We don't know height.
            // Workaround 3: Make the Thumb layer determine height, and Track stretches?
            // No, Track is thin.

            // Workaround 4: Paint the track as a `DrawRect` on the ROOT node, but use `stroke` instead of `fill`?
            // Stroke is centered on border? No, stroke is usually inset or centered.
            // If we have `PaintOp::DrawLine`? No.

            // Let's assume the Slider height IS the thumb size.
            // We paint the track by `DrawRect` with custom logic? No, standard ops.

            // Best approach given constraints:
            // Use `LayoutOp::Box` with top/bottom padding calculated to center the track?
            // `padding_top = (thumb_size - track_height) / 2`.
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

            let inner_paint = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(Fill::Solid(tokens.colors.border)),
                    stroke: None,
                    corner_radius: track_height / 2.0,
                    shadow: None,
                }),
            )
            .build(cx);

            // We need inner box to have height `track_height`.
            // The padding pushes the content in.
            // The inner box fills the remaining height.
            // If container height is `thumb_size`, and padding is `(thumb-track)/2`, remaining is `track`.
            // But container height is `Auto` (driven by ZStack constraint?).
            // ZStack constraints are "largest child".
            // If Thumb layer is `thumb_size` height, Root is `thumb_size`.
            // Track container fills Root.

            // So yes, Padding approach works if Root height is constrained by Thumb.

            // But `inner_paint` needs a layout node to fill?
            let mut inner_box =
                NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill)); // Fill the padded area
            inner_box.add_child(inner_paint);
            let inner_id = inner_box.build(cx);

            track_container.add_child(inner_id);
            track_container.build(cx)
        };

        // Layer 2: Thumb Grid
        let thumb_layer = {
            let thumb_paint = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(Fill::Solid(tokens.colors.primary)),
                    stroke: None,
                    corner_radius: thumb_size / 2.0,
                    shadow: Some(fission_ir::op::BoxShadow {
                        color: Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 50,
                        },
                        blur_radius: 2.0,
                        offset: (0.0, 1.0),
                    }),
                }),
            )
            .build(cx);

            let mut thumb_box = NodeBuilder::new(
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
            thumb_box.add_child(thumb_paint);
            let thumb_id = thumb_box.build(cx);

            let mut grid = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Grid {
                    columns: vec![
                        GridTrack::Percent(pct),
                        GridTrack::Points(thumb_size),
                        GridTrack::Fr(1.0),
                    ],
                    rows: vec![GridTrack::Points(thumb_size)],
                    column_gap: None,
                    row_gap: None,
                    padding: [0.0; 4],
                }),
            );

            // Thumb item at col 2
            let mut item = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::GridItem {
                    row_start: fission_ir::op::GridPlacement::Line(1),
                    row_end: fission_ir::op::GridPlacement::Auto,
                    col_start: fission_ir::op::GridPlacement::Line(2),
                    col_end: fission_ir::op::GridPlacement::Auto,
                }),
            );
            item.add_child(thumb_id);
            let item_id = item.build(cx);

            grid.add_child(item_id);
            grid.build(cx)
        };

        cx.push_scope(layout_id);
        let track_wrapped = wrap_zstack_child(cx, track_layer);
        let thumb_wrapped = wrap_zstack_child(cx, thumb_layer);
        cx.pop_scope();

        let mut zstack = NodeBuilder::new(layout_id, Op::Layout(LayoutOp::ZStack));
        zstack.add_child(track_wrapped);
        zstack.add_child(thumb_wrapped);
        zstack.build(cx);

        cx.pop_scope();

        let mut semantics = fission_ir::Semantics {
            role: fission_ir::Role::Slider,
            label: None,
            identifier: None,
            value: Some(format!("{:.2}", self.value)),
            actions: Default::default(),
            focusable: true,
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: None,
            disabled: false,
            read_only: false,
            autofocus: false,
            draggable: true,
            scrollable_x: false,
            scrollable_y: false,
            min_value: Some(self.min),
            max_value: Some(self.max),
            current_value: Some(self.value),
            is_focus_scope: false,
            is_focus_barrier: false,
            drag_payload: None,
            hero_tag: None,
            focus_index: None,
            text_input_type: fission_ir::semantics::TextInputType::Text,
            text_input_action: fission_ir::semantics::TextInputAction::Done,
            text_capitalization: fission_ir::semantics::TextCapitalization::None,
            max_length: None,
            max_length_enforcement: fission_ir::semantics::MaxLengthEnforcement::Enforced,
            input_formatters: Vec::new(),
            autocorrect: true,
            enable_suggestions: true,
            spell_check: true,
            smart_dashes: true,
            smart_quotes: true,
            autofill_hints: Vec::new(),
            scroll_padding: None,
            capture_tab: false,
            auto_indent: false,
        };

        if let Some(action) = &self.on_change {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Change,
                action_id: action.id.as_u128(),
                payload_data: Some(action.payload.clone()),
            });
        }

        let mut sem_node = NodeBuilder::new(id, Op::Semantics(semantics));
        sem_node.add_child(layout_id);
        sem_node.build(cx)
    }
}
