use crate::internal::InternalLower;
use crate::lowering::{wrap_zstack_child, InternalIrBuilder, InternalLoweringCx};
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

impl InternalLower for Switch {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let tokens = &cx.env.theme.tokens;
        let width = 36.0;
        let height = 20.0;
        let thumb_size = 16.0;
        let padding = 2.0;

        let track_color = if self.disabled {
            tokens.colors.surface_sunken
        } else if self.checked {
            tokens.colors.primary
        } else {
            tokens.colors.border
        };
        let thumb_color = if self.disabled {
            tokens.colors.text_muted
        } else {
            tokens.colors.on_primary
        };

        // Track
        let track_paint = Op::Paint(PaintOp::DrawRect {
            fill: Some(fission_ir::op::Fill::Solid(track_color)),
            stroke: None,
            corner_radius: height / 2.0,
            shadow: None,
        });
        let track_node = InternalIrBuilder::new(cx.next_node_id(), track_paint).build(cx);

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
        });
        let thumb_paint_node = InternalIrBuilder::new(cx.next_node_id(), thumb_paint).build(cx);

        let left_padding = if self.checked {
            width - thumb_size - padding
        } else {
            padding
        };

        let mut thumb_wrapper = InternalIrBuilder::new(
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
        thumb_wrapper.add_child(thumb_paint_node);
        let thumb_id = thumb_wrapper.build(cx);

        // ZStack for Track + Content
        let layout_id = cx.next_node_id();
        let bg_id = {
            let mut bg_fill =
                InternalIrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
            bg_fill.add_child(track_node);
            bg_fill.build(cx)
        };

        let content_id = {
            let mut thumb_track = InternalIrBuilder::new(
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

        cx.push_scope(layout_id);
        let bg_wrapped = wrap_zstack_child(cx, bg_id);
        let content_wrapped = wrap_zstack_child(cx, content_id);
        cx.pop_scope();

        let mut root = InternalIrBuilder::new(layout_id, Op::Layout(LayoutOp::ZStack));
        root.add_child(bg_wrapped);
        root.add_child(content_wrapped);
        root.build(cx);

        cx.pop_scope();

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

        let mut sem_node = InternalIrBuilder::new(id, Op::Semantics(semantics));
        sem_node.add_child(layout_id);
        sem_node.build(cx)
    }
}
