use crate::authoring::Lower;
use crate::lowering::{IrBuilder, LoweringContext};
use crate::ActionEnvelope;
use fission_ir::{
    op::{LayoutOp, Op, PaintOp},
    WidgetId,
};
use serde::{Deserialize, Serialize};

/// A boolean toggle with a square check indicator and optional label.
///
/// When pressed, the `on_toggle` action is dispatched. The application is
/// responsible for toggling `checked` in the corresponding reducer.
///
/// # Example
///
/// ```rust,ignore
/// let on_toggle = ctx.bind(ToggleAgree, reduce_with!(handle_toggle));
///
/// Checkbox {
///     checked: view.state().agreed,
///     on_toggle: Some(on_toggle),
///     label: Some("I agree to the terms".into()),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Checkbox {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// Stable identifier exposed on the checkbox's interactive semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
    /// Semantic name submitted by server-rendered HTML forms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Logical form membership for server-rendered HTML submission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    /// Current checked state.
    pub checked: bool,
    /// Action dispatched when the checkbox is tapped.
    pub on_toggle: Option<ActionEnvelope>,
    /// Optional text label rendered next to the indicator.
    pub label: Option<String>,
    /// Whether the checkbox is visibly disabled and cannot receive focus or
    /// dispatch its toggle action.
    #[serde(default)]
    pub disabled: bool,
}

impl Checkbox {
    /// Sets the stable identifier exposed to accessibility and test tooling.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    /// Sets the successful-control name used by HTML form submission.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Associates this checkbox with one logical form.
    pub fn form_id(mut self, form_id: impl Into<String>) -> Self {
        self.form_id = Some(form_id.into());
        self
    }

    /// Sets whether the checkbox is disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Lower for Checkbox {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        let layout_id = cx.with_scope(id, |cx| {
            let tokens = &cx.env.theme.tokens;
            let size = 18.0;
            let radius = tokens.radii.small;
            let border_color = if self.disabled {
                tokens.colors.text_muted
            } else {
                tokens.colors.text_secondary
            };
            let active_color = if self.disabled {
                tokens.colors.text_muted
            } else {
                tokens.colors.primary
            };
            let indicator_color = if self.disabled {
                tokens.colors.surface
            } else {
                tokens.colors.on_primary
            };
            let text_color = if self.disabled {
                tokens.colors.text_muted
            } else {
                tokens.colors.text_primary
            };

            // Square indicator
            let square_id = cx.next_node_id();

            // The box fill and border ease between states when widget motion is on.
            let animated = |property| match cx.runtime_state.motion.values.get(&(id, property)) {
                Some(crate::motion::MotionValue::Color(color)) => Some(*color),
                _ => None,
            };
            let fill_color = animated(crate::motion::MotionPropertyId::BackgroundColor).unwrap_or(
                if self.checked {
                    active_color
                } else {
                    fission_ir::op::Color::TRANSPARENT
                },
            );
            let stroke_color =
                animated(crate::motion::MotionPropertyId::BorderColor).unwrap_or(if self.checked {
                    active_color
                } else {
                    border_color
                });
            let bg_paint = Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_ir::op::Fill::Solid(fill_color)),
                stroke: Some(fission_ir::op::Stroke {
                    fill: fission_ir::op::Fill::Solid(stroke_color),
                    width: 1.5,
                    dash_array: None,
                    line_cap: fission_ir::op::LineCap::Butt,
                    line_join: fission_ir::op::LineJoin::Miter,
                }),
                corner_radius: radius,
                shadow: None,
                corner_radii: None,
                border_sides: None,
            });
            let bg_node = IrBuilder::new(cx.next_node_id(), bg_paint).build(cx);

            // The check mark is always present so it can fade and grow in and out. It is a stroked
            // tick drawn in the 10x10 box's own coordinates, so it reads as checked rather than as
            // a filled square inside the box.
            let check_motion = WidgetId::derived(id.as_u128(), &[CHECK_MOTION_PATH]);
            let check_node = {
                let check = IrBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawPath {
                        path: CHECK_MARK_PATH.to_string(),
                        fill: None,
                        stroke: Some(fission_ir::op::Stroke {
                            fill: fission_ir::op::Fill::Solid(indicator_color),
                            width: 1.8,
                            dash_array: None,
                            line_cap: fission_ir::op::LineCap::Round,
                            line_join: fission_ir::op::LineJoin::Round,
                        }),
                    }),
                )
                .build(cx);
                let mut check_box = IrBuilder::new(
                    cx.next_node_id(),
                    Op::Layout(LayoutOp::Box {
                        width: Some(10.0),
                        height: Some(10.0),
                        min_width: None,
                        max_width: None,
                        min_height: None,
                        max_height: None,
                        padding: [0.0; 4],
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        aspect_ratio: None,
                    }),
                )
                .composite(fission_ir::op::CompositeStyle {
                    opacity: Some(
                        fission_ir::op::CompositeScalar::new(if self.checked { 1.0 } else { 0.0 })
                            .motion(check_motion),
                    ),
                    scale: Some(
                        fission_ir::op::CompositeScalar::new(if self.checked {
                            1.0
                        } else {
                            CHECK_HIDDEN_SCALE
                        })
                        .motion(check_motion),
                    ),
                    ..Default::default()
                });
                check_box.add_child(check);
                let check_box_id = check_box.build(cx);
                let mut align = IrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Align));
                align.add_child(check_box_id);
                align.build(cx)
            };

            let mut square_box = IrBuilder::new(
                square_id,
                Op::Layout(LayoutOp::Box {
                    width: Some(size),
                    height: Some(size),
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
            square_box.add_child(bg_node);
            square_box.add_child(check_node);
            let square_final = square_box.build(cx);

            // Label
            let label_id = if let Some(text) = &self.label {
                let text_id = IrBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawText {
                        text: text.clone(),
                        size: tokens.typography.body_medium_size,
                        color: text_color,
                        underline: false,
                        locale: None,
                        wrap: false,
                        caret_index: None,
                        caret_color: None,
                        caret_width: None,
                        caret_height: None,
                        caret_radius: None,
                        paragraph_style: None,
                    }),
                )
                .build(cx);
                let mut layout = IrBuilder::new(
                    cx.next_node_id(),
                    Op::Layout(LayoutOp::Box {
                        width: None,
                        height: None,
                        min_width: None,
                        max_width: None,
                        min_height: None,
                        max_height: None,
                        padding: [tokens.spacing.s, 0.0, 0.0, 0.0],
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        aspect_ratio: None,
                    }),
                );
                layout.add_child(text_id);
                Some(layout.build(cx))
            } else {
                None
            };

            let layout_id = cx.next_node_id();
            let mut row = IrBuilder::new(
                layout_id,
                Op::Layout(LayoutOp::Flex {
                    direction: fission_ir::FlexDirection::Row,
                    wrap: fission_ir::op::FlexWrap::NoWrap,
                    flex_grow: 0.0,
                    flex_shrink: 1.0,
                    padding: [0.0; 4],
                    gap: Some(8.0),
                    line_gap: None,
                    align_items: fission_ir::op::AlignItems::Center,
                    justify_content: fission_ir::op::JustifyContent::Start,
                }),
            );
            row.add_child(square_final);
            if let Some(l) = label_id {
                row.add_child(l);
            }
            row.build(cx);

            layout_id
        });

        let mut semantics = fission_ir::Semantics {
            role: fission_ir::Role::Checkbox,
            label: self.label.clone(),
            identifier: self.semantics_identifier.clone(),
            value: Some(if self.checked {
                "true".into()
            } else {
                "false".into()
            }),
            focusable: !self.disabled,
            checked: Some(self.checked),
            disabled: self.disabled,
            text_field_name: self.name.clone(),
            text_form_id: self.form_id.clone(),
            ..fission_ir::Semantics::default()
        };
        if !self.disabled {
            if let Some(action) = &self.on_toggle {
                semantics.actions.entries.push(fission_ir::ActionEntry {
                    trigger: fission_ir::semantics::ActionTrigger::Default,
                    action_id: action.id.as_u128(),
                    payload_data: Some(action.payload.clone()),
                });
            }
        }

        let mut sem_node = IrBuilder::new(id, Op::Semantics(semantics));
        sem_node.add_child(layout_id);
        sem_node.build(cx)
    }
}

/// Path from a checkbox's id to its check mark's motion identity.
const CHECK_MOTION_PATH: u32 = 0xC4EC_0001;
/// The scale a check mark grows from and shrinks to.
/// A tick from the lower left, down to a point near the bottom, then up to the upper right, in the
/// check mark box's 10x10 coordinates.
const CHECK_MARK_PATH: &str = "M 1.5 5.2 L 4 7.7 L 8.6 2.4";
const CHECK_HIDDEN_SCALE: f32 = 0.6;

impl Checkbox {
    /// Registers the tracks that ease the box and check mark between states.
    pub(crate) fn register_motion_declarations(&self, id: WidgetId) {
        let Some(env) = crate::build::try_current_env() else {
            return;
        };
        let Some(transition) = toggle_transition(env) else {
            return;
        };
        let colors = &env.theme.tokens.colors;
        let active = if self.disabled {
            colors.text_muted
        } else {
            colors.primary
        };
        let border = if self.disabled {
            colors.text_muted
        } else {
            colors.text_secondary
        };
        let (fill, stroke) = if self.checked {
            (active, active)
        } else {
            (fission_ir::op::Color::TRANSPARENT, border)
        };
        register_tracks(
            id,
            vec![
                crate::motion::MotionTrack::paint(
                    crate::motion::MotionPropertyId::BackgroundColor,
                    crate::motion::MotionStartValue::Current,
                    crate::motion::color(fill),
                )
                .transition(transition.clone()),
                crate::motion::MotionTrack::paint(
                    crate::motion::MotionPropertyId::BorderColor,
                    crate::motion::MotionStartValue::Current,
                    crate::motion::color(stroke),
                )
                .transition(transition.clone()),
            ],
        );
        register_tracks(
            WidgetId::derived(id.as_u128(), &[CHECK_MOTION_PATH]),
            shown_tracks(self.checked, CHECK_HIDDEN_SCALE, transition),
        );
    }
}

/// The transition toggles ease between states with, or `None` when the app has
/// turned built-in widget motion off.
pub(crate) fn toggle_transition(env: &crate::Env) -> Option<crate::motion::MotionTransition> {
    env.widget_motion.is_on().then(|| {
        let motion = &env.theme.tokens.motion;
        crate::ui::widgets::button::component_motion_transition(fission_theme::ComponentMotion {
            duration_ms: motion.duration_fast_ms,
            easing: motion.easing_standard.clone(),
        })
    })
}

/// Opacity and scale tracks that show an indicator when `shown` and hide it,
/// shrunk to `hidden_scale`, otherwise.
pub(crate) fn shown_tracks(
    shown: bool,
    hidden_scale: f32,
    transition: crate::motion::MotionTransition,
) -> Vec<crate::motion::MotionTrack> {
    vec![
        crate::motion::MotionTrack::composite(
            crate::motion::MotionPropertyId::Opacity,
            crate::motion::MotionStartValue::Current,
            crate::motion::scalar(if shown { 1.0 } else { 0.0 }),
        )
        .transition(transition.clone()),
        crate::motion::MotionTrack::composite(
            crate::motion::MotionPropertyId::Scale,
            crate::motion::MotionStartValue::Current,
            crate::motion::scalar(if shown { 1.0 } else { hidden_scale }),
        )
        .transition(transition),
    ]
}

/// Registers `tracks` for the motion identity `id`.
pub(crate) fn register_tracks(id: WidgetId, tracks: Vec<crate::motion::MotionTrack>) {
    crate::build::try_register_motion(crate::motion::MotionDeclaration {
        id,
        kind: crate::motion::MotionDeclarationKind::Tracks { tracks },
    });
}
