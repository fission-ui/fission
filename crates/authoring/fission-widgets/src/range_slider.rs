use std::hash::{Hash, Hasher};
use std::sync::Arc;

use fission_core::input::range_slider::RangeSliderRuntimeConfig;
use fission_core::internal::{
    wrap_zstack_child, InternalIrBuilder, InternalLowerer, InternalLoweringCx,
};
use fission_core::ui::Widget;
use fission_core::ActionEnvelope;
use fission_ir::op::{Color, Fill, GridPlacement, GridTrack, LayoutOp, Op, PaintOp};
use fission_ir::{ActionEntry, ActionTrigger, FocusPolicy, Role, Semantics, WidgetId};
use serde::{Deserialize, Serialize};

const CONTROL_PATH: u32 = 0x5241_4e47;
const START_THUMB_PATH: u32 = 0x5354_4152;
const END_THUMB_PATH: u32 = 0x454e_4421;
const IMPLICIT_ID_SALT: u32 = 0x5241_4e53;

/// Controlled two-thumb slider selecting an inclusive numeric interval.
///
/// Pointer, keyboard, accessibility, and test input propose a new range through
/// `on_change`. The action's application payload is preserved; reducers read
/// the proposed values from [`fission_core::ActionInput::range_slider_change`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RangeSlider {
    /// Optional stable identity for retained focus and pointer-capture state.
    pub id: Option<WidgetId>,
    /// Stable identifier exposed on the complete range control. Its two thumbs
    /// use the same identifier with `.start` and `.end` suffixes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
    /// Current controlled lower value.
    pub start: f32,
    /// Current controlled upper value.
    pub end: f32,
    /// Minimum value represented by the track.
    pub min: f32,
    /// Maximum value represented by the track.
    pub max: f32,
    /// Optional increment used for pointer snapping and keyboard adjustment.
    /// Without one, pointer movement is continuous and keyboard input moves by
    /// one percent of the configured range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f32>,
    /// Action dispatched when interaction proposes a changed range.
    pub on_change: Option<ActionEnvelope>,
}

impl Default for RangeSlider {
    fn default() -> Self {
        Self {
            id: None,
            semantics_identifier: None,
            start: 0.0,
            end: 1.0,
            min: 0.0,
            max: 1.0,
            step: None,
            on_change: None,
        }
    }
}

impl RangeSlider {
    /// Sets the stable semantic identifier of the range and its two thumbs.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    /// Sets the positive increment used for snapping and keyboard adjustment.
    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }
}

impl From<RangeSlider> for Widget {
    fn from(mut component: RangeSlider) -> Self {
        let node_id = fission_core::build::current_widget_id()
            .or(component.id)
            .or_else(|| fission_core::build::next_implicit_widget_id(IMPLICIT_ID_SALT))
            .unwrap_or_else(|| WidgetId::explicit("fission_widgets::RangeSlider"));
        component.id = Some(node_id);
        let start_thumb_id = WidgetId::derived(node_id.as_u128(), &[START_THUMB_PATH]);
        let end_thumb_id = WidgetId::derived(node_id.as_u128(), &[END_THUMB_PATH]);
        let config = RangeSliderRuntimeConfig {
            node_id,
            start_thumb_id,
            end_thumb_id,
            start: component.start,
            end: component.end,
            min: component.min,
            max: component.max,
            step: component.step,
            on_change: component.on_change.clone(),
        };

        fission_core::internal::custom_render_widget(fission_core::internal::InternalRenderNode {
            debug_tag: "RangeSlider".into(),
            lowerer: Some(Arc::new(RangeSliderLowerer {
                component,
                node_id,
                start_thumb_id,
                end_thumb_id,
            })),
            render_object: Some(Arc::new(config)),
        })
    }
}

#[derive(Debug)]
struct RangeSliderLowerer {
    component: RangeSlider,
    node_id: WidgetId,
    start_thumb_id: WidgetId,
    end_thumb_id: WidgetId,
}

impl InternalLowerer for RangeSliderLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let control_id = WidgetId::derived(self.node_id.as_u128(), &[CONTROL_PATH]);
        cx.push_scope(control_id);

        let tokens = &cx.env.theme.tokens;
        let thumb_size = 16.0;
        let track_height = 4.0;
        let (min, max, start, end) = normalized_values(&self.component);
        let range = (max - min).max(f32::EPSILON);
        let start_pct = (start - min) / range * 100.0;
        let end_pct = (end - min) / range * 100.0;

        let track_layer = track_layer(cx, thumb_size, track_height, tokens.colors.border_strong);
        let selected_layer = selected_track_layer(
            cx,
            start_pct,
            end_pct,
            thumb_size,
            track_height,
            tokens.colors.primary,
        );
        let action = self.component.on_change.as_ref();
        let start_thumb = thumb_layer(
            cx,
            self.start_thumb_id,
            start_pct,
            thumb_size,
            tokens.colors.primary,
            self.component
                .semantics_identifier
                .as_ref()
                .map(|id| format!("{id}.start")),
            min,
            end,
            start,
            action,
        );
        let end_thumb = thumb_layer(
            cx,
            self.end_thumb_id,
            end_pct,
            thumb_size,
            tokens.colors.primary,
            self.component
                .semantics_identifier
                .as_ref()
                .map(|id| format!("{id}.end")),
            start,
            max,
            end,
            action,
        );

        let stack_id = cx.next_node_id();
        cx.push_scope(stack_id);
        let track_wrapped = wrap_zstack_child(cx, track_layer);
        let selected_wrapped = wrap_zstack_child(cx, selected_layer);
        let start_wrapped = wrap_zstack_child(cx, start_thumb);
        let end_wrapped = wrap_zstack_child(cx, end_thumb);
        cx.pop_scope();
        let mut stack = InternalIrBuilder::new(stack_id, Op::Layout(LayoutOp::ZStack));
        stack.add_child(track_wrapped);
        stack.add_child(selected_wrapped);
        stack.add_child(start_wrapped);
        stack.add_child(end_wrapped);
        stack.build(cx);

        let layout_id = cx.next_node_id();
        let mut layout = InternalIrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Box {
                width: None,
                height: Some(thumb_size),
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
        let semantics = Semantics {
            role: Role::Generic,
            identifier: self.component.semantics_identifier.clone(),
            value: Some(format!("{start}–{end}")),
            ..Semantics::default()
        };
        let mut control = InternalIrBuilder::new(control_id, Op::Semantics(semantics));
        control.add_child(layout_id);
        control.build(cx)
    }

    fn stable_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.node_id.hash(&mut hasher);
        self.component.start.to_bits().hash(&mut hasher);
        self.component.end.to_bits().hash(&mut hasher);
        self.component.min.to_bits().hash(&mut hasher);
        self.component.max.to_bits().hash(&mut hasher);
        self.component.step.map(f32::to_bits).hash(&mut hasher);
        self.component.semantics_identifier.hash(&mut hasher);
        if let Some(action) = &self.component.on_change {
            action.id.as_u128().hash(&mut hasher);
            action.payload.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(self.node_id)
    }
}

fn track_layer(
    cx: &mut InternalLoweringCx,
    control_height: f32,
    track_height: f32,
    color: Color,
) -> WidgetId {
    let paint = InternalIrBuilder::new(
        cx.next_node_id(),
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(color)),
            stroke: None,
            corner_radius: track_height / 2.0,
            shadow: None,
        }),
    )
    .build(cx);
    let mut fill = InternalIrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
    fill.add_child(paint);
    let fill = fill.build(cx);
    let vertical = (control_height - track_height) / 2.0;
    let mut container = InternalIrBuilder::new(
        cx.next_node_id(),
        Op::Layout(LayoutOp::Box {
            width: None,
            height: Some(control_height),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, vertical, vertical],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        }),
    );
    container.add_child(fill);
    container.build(cx)
}

fn selected_track_layer(
    cx: &mut InternalLoweringCx,
    start_pct: f32,
    end_pct: f32,
    control_height: f32,
    track_height: f32,
    color: Color,
) -> WidgetId {
    let paint = InternalIrBuilder::new(
        cx.next_node_id(),
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(color)),
            stroke: None,
            corner_radius: track_height / 2.0,
            shadow: None,
        }),
    )
    .build(cx);
    let vertical = (control_height - track_height) / 2.0;
    let mut segment = InternalIrBuilder::new(
        cx.next_node_id(),
        Op::Layout(LayoutOp::Box {
            width: None,
            height: Some(control_height),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, vertical, vertical],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        }),
    );
    segment.add_child(paint);
    let segment = segment.build(cx);
    let mut grid = InternalIrBuilder::new(
        cx.next_node_id(),
        Op::Layout(LayoutOp::Grid {
            columns: vec![
                GridTrack::Percent(start_pct),
                GridTrack::Percent((end_pct - start_pct).max(0.0)),
                GridTrack::Fr(1.0),
            ],
            rows: vec![GridTrack::Points(control_height)],
            column_gap: None,
            row_gap: None,
            padding: [0.0; 4],
        }),
    );
    let mut item = InternalIrBuilder::new(
        cx.next_node_id(),
        Op::Layout(LayoutOp::GridItem {
            row_start: GridPlacement::Line(1),
            row_end: GridPlacement::Auto,
            col_start: GridPlacement::Line(2),
            col_end: GridPlacement::Auto,
        }),
    );
    item.add_child(segment);
    grid.add_child(item.build(cx));
    grid.build(cx)
}

#[allow(clippy::too_many_arguments)]
fn thumb_layer(
    cx: &mut InternalLoweringCx,
    semantics_id: WidgetId,
    pct: f32,
    thumb_size: f32,
    color: Color,
    identifier: Option<String>,
    min: f32,
    max: f32,
    current: f32,
    action: Option<&ActionEnvelope>,
) -> WidgetId {
    let paint = InternalIrBuilder::new(
        cx.next_node_id(),
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(color)),
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
    thumb_box.add_child(paint);
    let thumb_box = thumb_box.build(cx);

    let mut semantics = Semantics {
        role: Role::Slider,
        identifier,
        value: Some(current.to_string()),
        focusable: true,
        focus_policy: FocusPolicy::FocusOnPointer,
        min_value: Some(min),
        max_value: Some(max),
        current_value: Some(current),
        ..Semantics::default()
    };
    if let Some(action) = action {
        semantics.actions.entries.push(ActionEntry {
            trigger: ActionTrigger::Change,
            action_id: action.id.as_u128(),
            payload_data: Some(action.payload.clone()),
        });
    }
    let mut semantic_thumb = InternalIrBuilder::new(semantics_id, Op::Semantics(semantics));
    semantic_thumb.add_child(thumb_box);
    let semantic_thumb = semantic_thumb.build(cx);

    let mut translated = InternalIrBuilder::new(
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
                0.0,
                0.0,
                1.0,
            ],
        }),
    );
    translated.add_child(semantic_thumb);
    let translated = translated.build(cx);
    let mut grid = InternalIrBuilder::new(
        cx.next_node_id(),
        Op::Layout(LayoutOp::Grid {
            columns: vec![
                GridTrack::Percent(pct.clamp(0.0, 100.0)),
                GridTrack::Points(thumb_size),
                GridTrack::Fr(1.0),
            ],
            rows: vec![GridTrack::Points(thumb_size)],
            column_gap: None,
            row_gap: None,
            padding: [0.0; 4],
        }),
    );
    let mut item = InternalIrBuilder::new(
        cx.next_node_id(),
        Op::Layout(LayoutOp::GridItem {
            row_start: GridPlacement::Line(1),
            row_end: GridPlacement::Auto,
            col_start: GridPlacement::Line(2),
            col_end: GridPlacement::Auto,
        }),
    );
    item.add_child(translated);
    grid.add_child(item.build(cx));
    grid.build(cx)
}

fn normalized_values(range: &RangeSlider) -> (f32, f32, f32, f32) {
    let min = range.min.min(range.max);
    let max = range.min.max(range.max);
    let first = range.start.clamp(min, max);
    let second = range.end.clamp(min, max);
    (min, max, first.min(second), first.max(second))
}
