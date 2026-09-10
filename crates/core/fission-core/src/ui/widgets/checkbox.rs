use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
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

impl InternalLower for Checkbox {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

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

        let bg_paint = if self.checked {
            Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_ir::op::Fill::Solid(active_color)),
                stroke: None,
                corner_radius: radius,
                shadow: None,
            })
        } else {
            Op::Paint(PaintOp::DrawRect {
                fill: None,
                stroke: Some(fission_ir::op::Stroke {
                    fill: fission_ir::op::Fill::Solid(border_color),
                    width: 1.5,
                    dash_array: None,
                    line_cap: fission_ir::op::LineCap::Butt,
                    line_join: fission_ir::op::LineJoin::Miter,
                }),
                corner_radius: radius,
                shadow: None,
            })
        };
        let bg_node = InternalIrBuilder::new(cx.next_node_id(), bg_paint).build(cx);

        // Checkmark
        let check_node = if self.checked {
            let check = InternalIrBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(fission_ir::op::Fill::Solid(indicator_color)),
                    stroke: None,
                    corner_radius: 1.0,
                    shadow: None,
                }),
            )
            .build(cx);
            let mut check_box = InternalIrBuilder::new(
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
            );
            check_box.add_child(check);
            let check_box_id = check_box.build(cx);
            let mut align = InternalIrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Align));
            align.add_child(check_box_id);
            Some(align.build(cx))
        } else {
            None
        };

        let mut square_box = InternalIrBuilder::new(
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
        if let Some(c) = check_node {
            square_box.add_child(c);
        }
        let square_final = square_box.build(cx);

        // Label
        let label_id = if let Some(text) = &self.label {
            let text_id = InternalIrBuilder::new(
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
            let mut layout = InternalIrBuilder::new(
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
        let mut row = InternalIrBuilder::new(
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

        cx.pop_scope();

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

        let mut sem_node = InternalIrBuilder::new(id, Op::Semantics(semantics));
        sem_node.add_child(layout_id);
        sem_node.build(cx)
    }
}
