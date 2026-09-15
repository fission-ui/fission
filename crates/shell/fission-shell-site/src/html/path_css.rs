//! Static HTML and CSS for path drawing and path motion: view boxes, stroke dash offset, stroke
//! trim and placement along a path.
//!
//! The site has no runtime, so animated values become `@keyframes`. Movement along a path is
//! sampled into keyframes with the easing already applied, because CSS applies a timing function
//! between each pair of keyframes rather than across the whole animation.

use super::*;
use fission_ir::op::{PathOffset, StrokeTrim};
use fission_ir::path::{point_along_svg_path, svg_path_length};

/// Keyframes used to approximate movement along a path.
const PATH_SAMPLES: usize = 48;

impl HtmlRenderer<'_> {
    /// Renders a `DrawPath` paint node as inline SVG.
    ///
    /// Paths with a view box fill their box in that coordinate space; paths without one keep the
    /// icon convention of a 24-unit square.
    pub(super) fn render_svg_path(
        &mut self,
        node: &CoreNode,
        path: &str,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
        view_box: Option<[f32; 2]>,
    ) -> String {
        let mut style = self.svg_paint_style(fill, stroke);
        let mut animated = false;
        if let Some(stroke) = stroke {
            let (stroke_style, stroke_animated) = self.path_stroke_style(node.parent, path, stroke);
            style.extend(stroke_style);
            animated = stroke_animated;
        }
        let base = if animated {
            "fission-site-svg-path fission-site-animated"
        } else {
            "fission-site-svg-path"
        };
        let path_class = self.class_name(base, style);
        let (svg_class, view_box) = match view_box {
            Some([width, height]) => (
                self.class_name(
                    "fission-site-vector-path",
                    vec![
                        "display:block".to_string(),
                        "width:100%".to_string(),
                        "height:100%".to_string(),
                        "overflow:visible".to_string(),
                    ],
                ),
                format!(
                    "0 0 {} {}\" preserveAspectRatio=\"none",
                    px(width),
                    px(height)
                ),
            ),
            None => ("fission-site-svg".to_string(), "0 0 24 24".to_string()),
        };
        format!(
            "<svg class=\"{}\" viewBox=\"{}\" aria-hidden=\"true\" data-fission-node=\"{}\"><path class=\"{}\" d=\"{}\"></path></svg>",
            escape_attr(&svg_class),
            view_box,
            node.id,
            escape_attr(&path_class),
            escape_attr(path)
        )
    }

    /// Dash offset and trim declarations for a stroke, and keyframes when they are animated.
    fn path_stroke_style(
        &mut self,
        motion_id: Option<WidgetId>,
        path: &str,
        stroke: &Stroke,
    ) -> (Vec<String>, bool) {
        let track = |renderer: &Self, property| {
            motion_id.and_then(|id| renderer.motion_track_for_target(id, property))
        };
        let dash_track = track(self, MotionPropertyId::StrokeDashOffset);
        let start_track = track(self, MotionPropertyId::PathTrimStart);
        let end_track = track(self, MotionPropertyId::PathTrimEnd);
        let mut style = Vec::new();

        if stroke.trim.is_some() || start_track.is_some() || end_track.is_some() {
            // A trim is drawn as one dash as long as the selected part of the path. It replaces
            // any dash pattern, which CSS cannot combine with it.
            let Some(length) = svg_path_length(path) else {
                return (style, false);
            };
            let base = stroke.trim.unwrap_or(StrokeTrim::new(0.0, 1.0));
            let resolve = |track: &Option<MotionTrack>, fallback: f32| {
                track
                    .as_ref()
                    .map(|track| {
                        (
                            animation_start_scalar(track, fallback).unwrap_or(fallback),
                            motion_expr_scalar_value(&track.to).unwrap_or(fallback),
                        )
                    })
                    .unwrap_or((fallback, fallback))
            };
            let (start_from, start_to) = resolve(&start_track, base.start);
            let (end_from, end_to) = resolve(&end_track, base.end);
            style.extend(trim_declarations(length, start_to, end_to));
            // Start and end share one timing on the site: the end track's, or the start track's.
            if let Some(timing) = end_track.as_ref().or(start_track.as_ref()) {
                let from = trim_declarations(length, start_from, end_from).join(";");
                let to = trim_declarations(length, start_to, end_to).join(";");
                let animation =
                    self.keyframes_animation("trim", vec![(0.0, from), (100.0, to)], timing, false);
                style.push(format!("animation:{animation}"));
                self.register_reduced_motion_rule();
                return (style, true);
            }
            return (style, false);
        }

        let (from, to) = match dash_track.as_ref() {
            Some(track) => (
                animation_start_scalar(track, stroke.dash_offset).unwrap_or(stroke.dash_offset),
                motion_expr_scalar_value(&track.to).unwrap_or(stroke.dash_offset),
            ),
            None => (stroke.dash_offset, stroke.dash_offset),
        };
        if to != 0.0 {
            style.push(format!("stroke-dashoffset:{}px", px(to)));
        }
        if let Some(track) = dash_track {
            let animation = self.keyframes_animation(
                "dash",
                vec![
                    (0.0, format!("stroke-dashoffset:{}px", px(from))),
                    (100.0, format!("stroke-dashoffset:{}px", px(to))),
                ],
                &track,
                false,
            );
            style.push(format!("animation:{animation}"));
            self.register_reduced_motion_rule();
            return (style, true);
        }
        (style, false)
    }

    /// Places a node with a path offset, adding its animation to `animations` when the distance
    /// is animated.
    pub(super) fn path_offset_style(
        &mut self,
        offset: &PathOffset,
        style: &mut Vec<String>,
        animations: &mut Vec<String>,
    ) {
        let request = self.animation_request(&offset.distance, MotionPropertyId::PathDistance);
        let resting = request
            .as_ref()
            .and_then(|track| motion_expr_scalar_value(&track.to))
            .unwrap_or(offset.distance.base);
        style.extend(path_placement(offset, resting));
        let Some(track) = request else {
            return;
        };
        let from = animation_start_scalar(&track, offset.distance.base).unwrap_or(resting);
        let (_, _, easing, _) = transition_css_parts(&track.transition);
        let frames = (0..=PATH_SAMPLES)
            .map(|sample| {
                let progress = sample as f32 / PATH_SAMPLES as f32;
                let distance = from + (resting - from) * easing.apply(progress);
                (progress * 100.0, path_placement(offset, distance).join(";"))
            })
            .collect();
        animations.push(self.keyframes_animation("path", frames, &track, true));
    }

    /// Registers a keyframes rule and returns the `animation` value that plays it.
    ///
    /// With `linear_timing` the frames already carry the easing, so the animation runs linearly.
    fn keyframes_animation(
        &mut self,
        kind: &str,
        frames: Vec<(f32, String)>,
        track: &MotionTrack,
        linear_timing: bool,
    ) -> String {
        let (duration_ms, delay_ms, easing, repeat) = transition_css_parts(&track.transition);
        let body = frames
            .iter()
            .map(|(percent, declarations)| format!("{}%{{{declarations}}}", px(*percent)))
            .collect::<String>();
        let key = format!("{kind}:{body}:{duration_ms}:{delay_ms}:{easing:?}:{repeat}");
        let name = format!("fission_path_{:016x}", stable_hash(key.as_bytes()));
        self.styles
            .raw_rule(name.clone(), format!("@keyframes {name}{{{body}}}\n"));
        let timing = if linear_timing {
            "linear".to_string()
        } else {
            easing_css(&easing)
        };
        format!(
            "{name} {duration_ms}ms {timing} {delay_ms}ms {} normal both",
            if repeat { "infinite" } else { "1" }
        )
    }

    pub(super) fn register_reduced_motion_rule(&mut self) {
        self.styles.raw_rule(
            "fission-site-reduced-motion-animations",
            "@media (prefers-reduced-motion:reduce){.fission-site-animated{animation-duration:0ms!important;animation-delay:0ms!important;animation-iteration-count:1!important;}}\n",
        );
    }
}

/// `stroke-dasharray` and `stroke-dashoffset` that draw only the trimmed part of a path.
fn trim_declarations(length: f32, start: f32, end: f32) -> Vec<String> {
    let (start, end) = (start.clamp(0.0, 1.0), end.clamp(0.0, 1.0));
    let visible = (end - start).max(0.0) * length;
    vec![
        format!("stroke-dasharray:{}px {}px", px(visible), px(length + 1.0)),
        format!("stroke-dashoffset:{}px", px(-start * length)),
    ]
}

/// `translate` and `rotate` declarations that put a node's centre on its path at `distance`.
fn path_placement(offset: &PathOffset, distance: f32) -> Vec<String> {
    let Some(point) = point_along_svg_path(&offset.path, distance) else {
        return Vec::new();
    };
    let mut declarations = vec![format!(
        "translate:calc({}px - 50%) calc({}px - 50%)",
        px(point.x),
        px(point.y)
    )];
    if offset.rotate {
        declarations.push(format!("rotate:{}deg", px(point.angle_degrees)));
    }
    declarations
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use fission_core::motion::{
        MotionEasing, MotionExpr, MotionStartValue, MotionTransition, MotionValue,
    };
    use fission_ir::op::{BoxStyle, CompositeStyle, PathOffset};

    fn value(value: f32) -> MotionExpr {
        MotionExpr::Value(MotionValue::Scalar(value))
    }

    fn looping(property: MotionPropertyId, from: f32, to: f32) -> MotionTrack {
        MotionTrack::paint(property, MotionStartValue::Explicit(value(from)), value(to))
            .transition(MotionTransition::tween(800, MotionEasing::EaseInOut).repeat(true))
    }

    fn styled_box() -> Op {
        Op::Layout(LayoutOp::StyledBox {
            style: BoxStyle::default(),
            flex_grow: 0.0,
            flex_shrink: 0.0,
        })
    }

    fn stroke() -> Stroke {
        Stroke {
            fill: Fill::Solid(Color::BLACK),
            width: 2.0,
            dash_array: None,
            dash_offset: 0.0,
            trim: None,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
        }
    }

    #[test]
    fn animated_trim_draws_the_path_on_with_keyframes() {
        let edge = WidgetId::explicit("edge");
        let path = WidgetId::explicit("edge-path");
        let mut ir = CoreIR::new();
        ir.add_node(
            path,
            Op::Paint(PaintOp::DrawPath {
                path: "M0 0 H100".into(),
                fill: None,
                stroke: Some(stroke()),
                view_box: Some([100.0, 10.0]),
            }),
            Vec::new(),
        );
        ir.add_node(edge, styled_box(), vec![path]);
        ir.set_root(edge);
        let options = HtmlRenderOptions {
            motion_declarations: vec![MotionDeclaration {
                id: edge,
                kind: MotionDeclarationKind::Tracks {
                    tracks: vec![looping(MotionPropertyId::PathTrimEnd, 0.0, 1.0)],
                },
            }],
            ..Default::default()
        };

        let rendered = render_ir_to_html(&ir, &options).unwrap();

        assert!(rendered
            .html
            .contains("viewBox=\"0 0 100 10\" preserveAspectRatio=\"none\""));
        assert!(rendered.css.contains(
            "0%{stroke-dasharray:0px 101px;stroke-dashoffset:0px}100%{stroke-dasharray:100px 101px;stroke-dashoffset:0px}"
        ));
        assert!(rendered.css.contains("infinite"));
    }

    #[test]
    fn dash_offset_is_static_or_animated() {
        let edge = WidgetId::explicit("flow");
        let path = WidgetId::explicit("flow-path");
        let mut ir = CoreIR::new();
        ir.add_node(
            path,
            Op::Paint(PaintOp::DrawPath {
                path: "M0 0 H40".into(),
                fill: None,
                stroke: Some(Stroke {
                    dash_array: Some(vec![6.0, 4.0]),
                    dash_offset: 3.0,
                    ..stroke()
                }),
                view_box: None,
            }),
            Vec::new(),
        );
        ir.add_node(edge, styled_box(), vec![path]);
        ir.set_root(edge);

        let still = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
        assert!(still.css.contains("stroke-dashoffset:3px"));
        assert!(still.html.contains("viewBox=\"0 0 24 24\""));

        let options = HtmlRenderOptions {
            motion_declarations: vec![MotionDeclaration {
                id: edge,
                kind: MotionDeclarationKind::Tracks {
                    tracks: vec![looping(MotionPropertyId::StrokeDashOffset, 10.0, 0.0)],
                },
            }],
            ..Default::default()
        };
        let animated = render_ir_to_html(&ir, &options).unwrap();
        assert!(animated
            .css
            .contains("0%{stroke-dashoffset:10px}100%{stroke-dashoffset:0px}"));
    }

    #[test]
    fn path_offset_places_and_moves_the_node_along_its_path() {
        let dot = WidgetId::explicit("dot");
        let mut ir = CoreIR::new();
        ir.add_node_with_composite(
            dot,
            styled_box(),
            CompositeStyle {
                path_offset: Some(PathOffset {
                    path: "M0 0 H100".into(),
                    distance: CompositeScalar::new(0.5).motion(dot),
                    rotate: true,
                }),
                ..Default::default()
            },
            Vec::new(),
        );
        ir.set_root(dot);

        let still = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
        assert!(still
            .css
            .contains("translate:calc(50px - 50%) calc(0px - 50%)"));
        assert!(!still.css.contains("@keyframes"));

        let track = MotionTrack::composite(
            MotionPropertyId::PathDistance,
            MotionStartValue::Explicit(value(0.0)),
            value(1.0),
        )
        .transition(MotionTransition::tween(1_000, MotionEasing::EaseIn));
        let options = HtmlRenderOptions {
            motion_declarations: vec![MotionDeclaration {
                id: dot,
                kind: MotionDeclarationKind::Tracks {
                    tracks: vec![track],
                },
            }],
            ..Default::default()
        };
        let moving = render_ir_to_html(&ir, &options).unwrap();
        assert!(moving
            .css
            .contains("0%{translate:calc(0px - 50%) calc(0px - 50%);rotate:0deg}"));
        assert!(moving
            .css
            .contains("100%{translate:calc(100px - 50%) calc(0px - 50%);rotate:0deg}"));
        // Easing is baked into the samples, so the animation itself runs linearly.
        assert!(moving.css.contains("1000ms linear 0ms 1 normal both"));
    }
}
