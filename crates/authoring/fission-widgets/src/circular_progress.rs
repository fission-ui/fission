use fission_core::internal::{InternalIrBuilder, InternalLowerer, InternalLoweringCx};
use fission_core::ui::{Composite, Widget};
use fission_core::{
    AnimationPropertyId, AnimationRequest, AnimationStartValue, EasingFunction, WidgetId,
};
use fission_ir::{op::Color, LayoutOp, Op, PaintOp};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

const SPIN_DURATION_MS: u64 = 900;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircularProgress {
    pub id: WidgetId,
    pub value: Option<f32>, // 0.0 to 1.0. If None, indeterminate (spinner).
    pub size: f32,
    pub color: Option<Color>,
    pub track_color: Option<Color>,
    pub thickness: f32,
    #[serde(default = "circular_progress_default_animated")]
    pub animated: bool,
}

impl Default for CircularProgress {
    fn default() -> Self {
        Self {
            id: WidgetId::explicit("fission.widgets.circular_progress"),
            value: None,
            size: 40.0,
            color: None,
            track_color: None,
            thickness: 4.0,
            animated: true,
        }
    }
}

impl From<CircularProgress> for Widget {
    fn from(component: CircularProgress) -> Self {
        let (ctx, view) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let color = this.color.unwrap_or(tokens.colors.primary);
        let track_color = this.track_color.unwrap_or(tokens.colors.border);

        let node = fission_core::internal::custom_render_widget(
            fission_core::internal::InternalRenderNode {
                debug_tag: "CircularProgress".into(),
                lowerer: Some(std::sync::Arc::new(CircularProgressLowerer {
                    value: this.value,
                    size: this.size,
                    color,
                    track_color,
                    thickness: this.thickness,
                })),
                render_object: None,
            },
        );

        if this.value.is_none() && this.animated {
            ctx.anim_for(this.id).request(AnimationRequest {
                property: AnimationPropertyId::Rotation,
                from: AnimationStartValue::Explicit(0.0),
                to: PI * 2.0,
                duration_ms: SPIN_DURATION_MS,
                repeat: true,
                delay_ms: 0,
                frame_interval_ms: None,
                easing: EasingFunction::Linear,
            });
            Composite::new(node)
                .repaint_boundary(true)
                .animated_rotation(this.id, 0.0)
                .into()
        } else {
            node
        }
    }
}

const fn circular_progress_default_animated() -> bool {
    true
}

#[derive(Debug)]
struct CircularProgressLowerer {
    value: Option<f32>,
    size: f32,
    color: Color,
    track_color: Color,
    thickness: f32,
}

impl InternalLowerer for CircularProgressLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = cx.next_node_id();

        // Track Circle
        // Keep the stroked arc inside the widget bounds so retained texture
        // edges do not clip the antialiased stroke into square artifacts.
        let r = (self.size * 0.5 - (self.thickness * 0.5 + 1.0)).max(0.0);
        let cx_pt = self.size / 2.0;
        let cy_pt = self.size / 2.0;

        // Full circle path for track
        let track_path = format!(
            "M {cx} {cy} m -{r}, 0 a {r},{r} 0 1,0 {d},0 a {r},{r} 0 1,0 -{d},0",
            cx = cx_pt,
            cy = cy_pt,
            r = r,
            d = r * 2.0
        );

        let track = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawPath {
                path: track_path,
                fill: None,
                stroke: Some(fission_ir::op::Stroke {
                    fill: fission_ir::op::Fill::Solid(self.track_color),
                    width: self.thickness,
                    dash_array: None,
                    line_cap: fission_ir::op::LineCap::Round,
                    line_join: fission_ir::op::LineJoin::Round,
                }),
            }),
        )
        .build(cx);

        // Value Arc
        let val = self.value.unwrap_or(0.25);

        let angle = val * 2.0 * PI;
        // Arc from -PI/2 (top) to -PI/2 + angle.

        // Simple SVG path for arc is complex to generate manually here without trig.
        // M start_x start_y A r r 0 large_arc sweep end_x end_y

        let start_angle = -PI / 2.0;
        let end_angle = start_angle + angle;

        let x1 = cx_pt + r * start_angle.cos();
        let y1 = cy_pt + r * start_angle.sin();
        let x2 = cx_pt + r * end_angle.cos();
        let y2 = cy_pt + r * end_angle.sin();

        let large_arc = if angle > PI { 1 } else { 0 };
        let sweep = 1;

        let arc_path = format!(
            "M {x1} {y1} A {r} {r} 0 {large_arc} {sweep} {x2} {y2}",
            x1 = x1,
            y1 = y1,
            r = r,
            large_arc = large_arc,
            sweep = sweep,
            x2 = x2,
            y2 = y2
        );

        let indicator = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawPath {
                path: arc_path,
                fill: None,
                stroke: Some(fission_ir::op::Stroke {
                    fill: fission_ir::op::Fill::Solid(self.color),
                    width: self.thickness,
                    dash_array: None,
                    line_cap: fission_ir::op::LineCap::Round,
                    line_join: fission_ir::op::LineJoin::Round,
                }),
            }),
        )
        .build(cx);

        let mut layout = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: Some(self.size),
                height: Some(self.size),
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
        layout.add_child(track);
        layout.add_child(indicator);
        layout.build(cx)
    }
}
