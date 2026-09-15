use crate::authoring::Lower;
use crate::lowering::{IrBuilder, LoweringContext};
use crate::ActionEnvelope;
use fission_ir::{
    op::{LayoutOp, Op, PaintOp},
    WidgetId,
};
use serde::{Deserialize, Serialize};

/// A single-select radio button with a circular indicator and optional label.
///
/// Radio buttons are typically used in a group where exactly one is selected
/// at a time. The `on_select` action is dispatched when the user taps the
/// button; the application updates which option is selected in the reducer.
///
/// # Example
///
/// ```rust,ignore
/// for (i, option) in options.iter().enumerate() {
///     let on_select = ctx.bind(
///         SelectOption { index: i },
///         reduce_with!(handle_select),
///     );
///     children.push(Radio {
///         checked: view.state().selected == i,
///         on_select: Some(on_select),
///         label: Some(option.clone()),
///         ..Default::default()
///     }.into());
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Radio {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// Stable identifier exposed on the radio's interactive semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
    /// Whether this radio button is currently selected.
    pub checked: bool,
    /// Action dispatched when this radio button is tapped.
    pub on_select: Option<ActionEnvelope>,
    /// Optional text label rendered next to the indicator.
    pub label: Option<String>,
    /// Whether the radio button is visibly disabled and cannot receive focus
    /// or dispatch its selection action.
    #[serde(default)]
    pub disabled: bool,
}

impl Radio {
    /// Sets the stable identifier exposed to accessibility and test tooling.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }

    /// Sets whether the radio button is disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Lower for Radio {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        let layout_id = cx.with_scope(id, |cx| {
            let tokens = &cx.env.theme.tokens;
            let size = 18.0;
            let dot_size = size * 0.5;
            let _radius = size / 2.0;
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
            let text_color = if self.disabled {
                tokens.colors.text_muted
            } else {
                tokens.colors.text_primary
            };

            // The ring's colour and weight ease between states when widget motion is on.
            let ring_color = match cx
                .runtime_state
                .motion
                .values
                .get(&(id, crate::motion::MotionPropertyId::BorderColor))
            {
                Some(crate::motion::MotionValue::Color(color)) => *color,
                _ if self.checked => active_color,
                _ => border_color,
            };
            let ring_width = match cx
                .runtime_state
                .motion
                .values
                .get(&(id, crate::motion::MotionPropertyId::BorderWidth))
            {
                Some(crate::motion::MotionValue::Px(width)) => *width,
                _ if self.checked => RING_CHECKED_WIDTH,
                _ => RING_WIDTH,
            };
            let bg_paint = Op::Paint(PaintOp::DrawRect {
                fill: None,
                stroke: Some(fission_ir::op::Stroke {
                    fill: fission_ir::op::Fill::Solid(ring_color),
                    width: ring_width,
                    dash_array: None,
                    dash_offset: 0.0,
                    trim: None,
                    line_cap: fission_ir::op::LineCap::Butt,
                    line_join: fission_ir::op::LineJoin::Miter,
                }),
                corner_radius: size / 2.0,
                shadow: None,
                corner_radii: None,
                border_sides: None,
            });
            let outer_node = IrBuilder::new(cx.next_node_id(), bg_paint).build(cx);

            // The dot is always present so it can fade and grow in and out.
            let dot_motion = WidgetId::derived(id.as_u128(), &[DOT_MOTION_PATH]);
            let dot_node = {
                let dot = IrBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: Some(fission_ir::op::Fill::Solid(active_color)),
                        stroke: None,
                        corner_radius: dot_size / 2.0,
                        shadow: None,
                        corner_radii: None,
                        border_sides: None,
                    }),
                )
                .build(cx);
                let mut dot_box = IrBuilder::new(
                    cx.next_node_id(),
                    Op::Layout(LayoutOp::Box {
                        width: Some(dot_size),
                        height: Some(dot_size),
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
                            .motion(dot_motion),
                    ),
                    scale: Some(
                        fission_ir::op::CompositeScalar::new(if self.checked {
                            1.0
                        } else {
                            DOT_HIDDEN_SCALE
                        })
                        .motion(dot_motion),
                    ),
                    ..Default::default()
                });
                dot_box.add_child(dot);
                let dot_box_id = dot_box.build(cx);
                let mut dot_align = IrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Align));
                dot_align.add_child(dot_box_id);
                let dot_align_id = dot_align.build(cx);
                let mut dot_container = IrBuilder::new(
                    cx.next_node_id(),
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
                dot_container.add_child(dot_align_id);
                dot_container.build(cx)
            };

            let mut radio_box = IrBuilder::new(
                cx.next_node_id(),
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
            radio_box.add_child(outer_node);
            radio_box.add_child(dot_node);
            let radio_final = radio_box.build(cx);

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
            row.add_child(radio_final);
            if let Some(l) = label_id {
                row.add_child(l);
            }
            row.build(cx);

            layout_id
        });

        let mut semantics = fission_ir::Semantics {
            role: fission_ir::Role::Radio,
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
            ..fission_ir::Semantics::default()
        };
        if !self.disabled {
            if let Some(action) = &self.on_select {
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

/// Path from a radio's id to its dot's motion identity.
const DOT_MOTION_PATH: u32 = 0xD07_0001;
/// The scale a radio dot grows from and shrinks to.
const DOT_HIDDEN_SCALE: f32 = 0.4;
/// The ring's stroke width when unselected and when selected.
const RING_WIDTH: f32 = 1.5;
const RING_CHECKED_WIDTH: f32 = 2.0;

impl Radio {
    /// Registers the tracks that ease the ring and dot between states.
    pub(crate) fn register_motion_declarations(&self, id: WidgetId) {
        use crate::ui::widgets::checkbox::{register_tracks, shown_tracks, toggle_transition};
        let Some(env) = crate::build::try_current_env() else {
            return;
        };
        let Some(transition) = toggle_transition(env) else {
            return;
        };
        let colors = &env.theme.tokens.colors;
        let ring = if self.disabled {
            colors.text_muted
        } else if self.checked {
            colors.primary
        } else {
            colors.text_secondary
        };
        register_tracks(
            id,
            vec![
                crate::motion::MotionTrack::paint(
                    crate::motion::MotionPropertyId::BorderColor,
                    crate::motion::MotionStartValue::Current,
                    crate::motion::color(ring),
                )
                .transition(transition.clone()),
                crate::motion::MotionTrack::paint(
                    crate::motion::MotionPropertyId::BorderWidth,
                    crate::motion::MotionStartValue::Current,
                    crate::motion::px(if self.checked {
                        RING_CHECKED_WIDTH
                    } else {
                        RING_WIDTH
                    }),
                )
                .transition(transition.clone()),
            ],
        );
        register_tracks(
            WidgetId::derived(id.as_u128(), &[DOT_MOTION_PATH]),
            shown_tracks(self.checked, DOT_HIDDEN_SCALE, transition),
        );
    }
}
