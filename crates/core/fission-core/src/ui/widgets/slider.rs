use crate::internal::InternalLower;
use crate::lowering::wrap_zstack_child;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ActionEnvelope;
use fission_ir::{
    op::{Color, Fill, GridTrack, LayoutOp, Op, PaintOp},
    WidgetId,
};
use serde::{Deserialize, Serialize};

/// A continuous value selector rendered as a horizontal track with a draggable
/// thumb.
///
/// The thumb position is determined by `value` within the `[min, max]` range.
/// Dragging dispatches the `on_change` action with the new value carried as
/// pointer input (see [`crate::ActionInput::as_pointer`]).
///
/// # Example
///
/// ```rust,ignore
/// Slider {
///     value: view.state().volume,
///     min: 0.0,
///     max: 1.0,
///     on_change: Some(ctx.bind(
///         VolumeChanged,
///         reduce_with!(handle_volume),
///     )),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slider {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// Stable identifier exposed on the slider's interactive semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
    /// Current value (clamped to `[min, max]`).
    pub value: f32,
    /// Minimum value (default: 0.0).
    pub min: f32,
    /// Maximum value (default: 1.0).
    pub max: f32,
    /// Visual track height in layout points. Defaults to `4.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_height: Option<f32>,
    /// Visual thumb diameter in layout points. Defaults to `16.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb_size: Option<f32>,
    /// Optional track fill. Defaults to the active theme's strong border colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_fill: Option<Fill>,
    /// Optional thumb fill. Defaults to the active theme primary colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb_fill: Option<Fill>,
    /// Action dispatched when the user drags the thumb.
    pub on_change: Option<ActionEnvelope>,
}

impl Slider {
    /// Sets the stable identifier exposed to accessibility and test tooling.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self {
            id: None,
            semantics_identifier: None,
            value: 0.0,
            min: 0.0,
            max: 1.0,
            track_height: None,
            thumb_size: None,
            track_fill: None,
            thumb_fill: None,
            on_change: None,
        }
    }
}

impl InternalLower for Slider {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let tokens = &cx.env.theme.tokens;
        let thumb_size = self.thumb_size.unwrap_or(16.0).max(1.0);
        let track_height = self.track_height.unwrap_or(4.0).max(1.0);
        let control_height = thumb_size.max(track_height);

        let range = (self.max - self.min).max(0.0001);
        let pct = ((self.value - self.min) / range).clamp(0.0, 1.0) * 100.0;

        let layout_id = cx.next_node_id();
        let stack_id = cx.next_node_id();

        let track_layer = {
            let p_y = (control_height - track_height) / 2.0;

            let mut track_container = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box {
                    width: None,
                    height: Some(control_height),
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

            let inner_paint = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(
                        self.track_fill
                            .clone()
                            .unwrap_or(Fill::Solid(tokens.colors.border_strong)),
                    ),
                    stroke: None,
                    corner_radius: track_height / 2.0,
                    shadow: None,
                }),
            )
            .build(cx);

            let mut inner_box =
                InternalIrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
            inner_box.add_child(inner_paint);
            let inner_id = inner_box.build(cx);

            track_container.add_child(inner_id);
            track_container.build(cx)
        };

        // Layer 2: Thumb Grid
        let thumb_layer = {
            let thumb_paint = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(
                        self.thumb_fill
                            .clone()
                            .unwrap_or(Fill::Solid(tokens.colors.primary)),
                    ),
                    stroke: None,
                    corner_radius: thumb_size / 2.0,
                    shadow: Some(fission_ir::op::BoxShadow {
                        spread_radius: 0.0,
                        inset: false,
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

            let mut thumb_box = InternalIrBuilder::new(
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
            let thumb_box_id = thumb_box.build(cx);

            // Grid placement positions the thumb's left edge at the value
            // percentage. Translate the visual thumb so its centre sits on the
            // track point the user clicked or dragged to.
            let mut transformed_thumb = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Transform {
                    transform: [
                        1.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        1.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        1.0,
                        0.0,
                        -thumb_size / 2.0,
                        (control_height - thumb_size) / 2.0,
                        0.0,
                        1.0,
                    ],
                }),
            );
            transformed_thumb.add_child(thumb_box_id);
            let transformed_thumb_id = transformed_thumb.build(cx);

            let mut grid = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Grid {
                    columns: vec![
                        GridTrack::Percent(pct),
                        GridTrack::Points(thumb_size),
                        GridTrack::Fr(1.0),
                    ],
                    rows: vec![GridTrack::Points(control_height)],
                    column_gap: None,
                    row_gap: None,
                    padding: [0.0; 4],
                }),
            );

            // Thumb item at col 2
            let mut item = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::GridItem {
                    row_start: fission_ir::op::GridPlacement::Line(1),
                    row_end: fission_ir::op::GridPlacement::Auto,
                    col_start: fission_ir::op::GridPlacement::Line(2),
                    col_end: fission_ir::op::GridPlacement::Auto,
                }),
            );
            item.add_child(transformed_thumb_id);
            let item_id = item.build(cx);

            grid.add_child(item_id);
            grid.build(cx)
        };

        cx.push_scope(stack_id);
        let track_wrapped = wrap_zstack_child(cx, track_layer);
        let thumb_wrapped = wrap_zstack_child(cx, thumb_layer);
        cx.pop_scope();

        let mut zstack = InternalIrBuilder::new(stack_id, Op::Layout(LayoutOp::ZStack));
        zstack.add_child(track_wrapped);
        zstack.add_child(thumb_wrapped);
        zstack.build(cx);

        let mut layout = InternalIrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Box {
                width: None,
                height: Some(control_height),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 1.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
        );
        layout.add_child(stack_id);
        layout.build(cx);

        cx.pop_scope();

        let mut semantics = fission_ir::Semantics {
            role: fission_ir::Role::Slider,
            label: None,
            identifier: self.semantics_identifier.clone(),
            value: Some(format!("{:.2}", self.value)),
            hyperlink: None,
            popover_target: None,
            actions: Default::default(),
            canvas_target: None,
            action_scope_id: None,
            focusable: true,
            focus_policy: fission_ir::FocusPolicy::FocusOnPointer,
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            ime_preedit_cursor_range: None,
            text_selection: None,
            selectable_text: false,
            context_menu: false,
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

        let mut sem_node = InternalIrBuilder::new(id, Op::Semantics(semantics));
        sem_node.add_child(layout_id);
        sem_node.build(cx)
    }
}
