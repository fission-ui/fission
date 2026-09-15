use crate::authoring::Lower;
use crate::lowering::{wrap_zstack_child, IrBuilder, LoweringContext};
use crate::ActionEnvelope;
use fission_ir::{
    op::{Color, LayoutOp, Op, PaintOp},
    WidgetId,
};
use serde::{Deserialize, Serialize};

/// A boolean toggle rendered as a sliding thumb on a track.
///
/// Visually similar to iOS/Material "switch" controls. The `on_toggle` action
/// is dispatched when the user taps the switch; the application toggles
/// `checked` in the reducer.
///
/// # Example
///
/// ```rust,ignore
/// Switch {
///     checked: view.state().dark_mode,
///     on_toggle: Some(ctx.bind(ToggleDarkMode, reduce_with!(handler))),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Switch {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// Stable identifier exposed on the switch's interactive semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
    /// Current on/off state.
    pub checked: bool,
    /// Action dispatched when the switch is tapped.
    pub on_toggle: Option<ActionEnvelope>,
    /// Whether the switch is visibly disabled and cannot receive focus or
    /// dispatch its toggle action.
    #[serde(default)]
    pub disabled: bool,
}

impl Switch {
    /// Sets the stable identifier exposed to accessibility and test tooling.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    /// Sets whether the switch is disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Lower for Switch {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        let layout_id = cx.with_scope(id, |cx| {
            let tokens = &cx.env.theme.tokens;
            let geometry = SwitchGeometry::from_tokens(tokens);
            let width = geometry.width;
            let height = geometry.height;
            let thumb_size = geometry.thumb;
            let padding = geometry.padding;

            let track_color = if self.disabled {
                tokens.colors.surface_sunken
            } else if self.checked {
                tokens.colors.accent
            } else {
                tokens.colors.border
            };
            // The track colour eases between states when widget motion is on.
            let track_color = match cx
                .runtime_state
                .motion
                .values
                .get(&(id, crate::motion::MotionPropertyId::BackgroundColor))
            {
                Some(crate::motion::MotionValue::Color(color)) => *color,
                _ => track_color,
            };
            let thumb_color = if self.disabled {
                tokens.colors.text_muted
            } else {
                tokens.colors.on_accent
            };

            // Track
            let track_paint = Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_ir::op::Fill::Solid(track_color)),
                stroke: None,
                corner_radius: height / 2.0,
                shadow: None,
                corner_radii: None,
                border_sides: None,
            });
            let track_node = IrBuilder::new(cx.next_node_id(), track_paint).build(cx);

            // Thumb
            let thumb_paint = Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_ir::op::Fill::Solid(thumb_color)),
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
                corner_radii: None,
                border_sides: None,
            });
            let thumb_paint_node = IrBuilder::new(cx.next_node_id(), thumb_paint).build(cx);

            // The thumb is laid out in the off position and slides to its state.
            let left_padding = padding;
            let thumb_motion = WidgetId::derived(id.as_u128(), &[THUMB_MOTION_PATH]);

            let mut thumb_wrapper = IrBuilder::new(
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
            )
            .composite(fission_ir::op::CompositeStyle {
                translate_x: Some(
                    fission_ir::op::CompositeScalar::new(if self.checked {
                        geometry.thumb_travel()
                    } else {
                        0.0
                    })
                    .motion(thumb_motion),
                ),
                ..Default::default()
            });
            thumb_wrapper.add_child(thumb_paint_node);
            let thumb_id = thumb_wrapper.build(cx);

            // ZStack for Track + Content
            let layout_id = cx.next_node_id();
            let bg_id = {
                let mut bg_fill =
                    IrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
                bg_fill.add_child(track_node);
                bg_fill.build(cx)
            };

            let content_id = {
                let mut thumb_track = IrBuilder::new(
                    cx.next_node_id(),
                    Op::Layout(LayoutOp::Box {
                        width: Some(width),
                        height: Some(height),
                        min_width: None,
                        max_width: None,
                        min_height: None,
                        max_height: None,
                        padding: [left_padding, 0.0, padding, 0.0],
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        aspect_ratio: None,
                    }),
                );
                thumb_track.add_child(thumb_id);
                thumb_track.build(cx)
            };

            let (bg_wrapped, content_wrapped) = cx.with_scope(layout_id, |cx| {
                let bg_wrapped = wrap_zstack_child(cx, bg_id);
                let content_wrapped = wrap_zstack_child(cx, content_id);
                (bg_wrapped, content_wrapped)
            });

            let mut root = IrBuilder::new(layout_id, Op::Layout(LayoutOp::ZStack));
            root.add_child(bg_wrapped);
            root.add_child(content_wrapped);
            root.build(cx);

            layout_id
        });

        let mut semantics = fission_ir::Semantics {
            role: fission_ir::Role::Switch,
            identifier: self.semantics_identifier.clone(),
            value: Some(if self.checked {
                "true".into()
            } else {
                "false".into()
            }),
            focusable: !self.disabled,
            checked: Some(self.checked),
            disabled: self.disabled,
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

/// A switch's track and thumb, sized from the design system: the track is as
/// tall as a medium icon, the thumb sits inside it with a two-pixel inset, and
/// the track is long enough for the thumb to travel most of its own width.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SwitchGeometry {
    width: f32,
    height: f32,
    thumb: f32,
    padding: f32,
}

impl SwitchGeometry {
    fn from_tokens(tokens: &fission_theme::Tokens) -> Self {
        let height = tokens.sizing.icon_md.max(12.0);
        let padding = tokens.spacing.xxs.max(0.0);
        let thumb = (height - padding * 2.0).max(4.0);
        let width = (height * 1.8).round();
        Self {
            width,
            height,
            thumb,
            padding,
        }
    }

    /// How far the thumb slides between the off and on positions.
    fn thumb_travel(self) -> f32 {
        self.width - self.thumb - self.padding * 2.0
    }
}
/// Path from a switch's id to its thumb's motion identity.
const THUMB_MOTION_PATH: u32 = 0x7B0_0001;

impl Switch {
    /// Registers the tracks that slide the thumb and ease the track colour.
    pub(crate) fn register_motion_declarations(&self, id: WidgetId) {
        use crate::ui::widgets::checkbox::{register_tracks, toggle_transition};
        let Some(env) = crate::build::try_current_env() else {
            return;
        };
        let Some(transition) = toggle_transition(env) else {
            return;
        };
        let colors = &env.theme.tokens.colors;
        let track = if self.disabled {
            colors.surface_sunken
        } else if self.checked {
            colors.accent
        } else {
            colors.border
        };
        register_tracks(
            id,
            vec![crate::motion::MotionTrack::paint(
                crate::motion::MotionPropertyId::BackgroundColor,
                crate::motion::MotionStartValue::Current,
                crate::motion::color(track),
            )
            .transition(transition.clone())],
        );
        register_tracks(
            WidgetId::derived(id.as_u128(), &[THUMB_MOTION_PATH]),
            vec![crate::motion::MotionTrack::composite(
                crate::motion::MotionPropertyId::TranslateX,
                crate::motion::MotionStartValue::Current,
                crate::motion::px(if self.checked {
                    SwitchGeometry::from_tokens(&env.theme.tokens).thumb_travel()
                } else {
                    0.0
                }),
            )
            .transition(transition)],
        );
    }
}
