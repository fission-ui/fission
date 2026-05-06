use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ActionEnvelope;
use fission_ir::{
    op::{LayoutOp, Op, PaintOp},
    NodeId,
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
/// let on_toggle = ctx.bind(ToggleAgree, handle_toggle as fn(&mut S, ToggleAgree));
///
/// Checkbox {
///     checked: view.state.agreed,
///     on_toggle: Some(on_toggle),
///     label: Some("I agree to the terms".into()),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Checkbox {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// Current checked state.
    pub checked: bool,
    /// Action dispatched when the checkbox is tapped.
    pub on_toggle: Option<ActionEnvelope>,
    /// Optional text label rendered next to the indicator.
    pub label: Option<String>,
}

impl Checkbox {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Checkbox(self)
    }
}

impl Lower for Checkbox {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let tokens = &cx.env.theme.tokens;
        let size = 18.0;
        let radius = tokens.radii.small;
        let border_color = tokens.colors.text_secondary;
        let active_color = tokens.colors.primary;
        let text_color = tokens.colors.text_primary;

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
        let bg_node = NodeBuilder::new(cx.next_node_id(), bg_paint).build(cx);

        // Checkmark
        let check_node = if self.checked {
            let check = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(fission_ir::op::Fill::Solid(tokens.colors.on_primary)),
                    stroke: None,
                    corner_radius: 1.0,
                    shadow: None,
                }),
            )
            .build(cx);
            let mut check_box = NodeBuilder::new(
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
            let mut align = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Align));
            align.add_child(check_box_id);
            Some(align.build(cx))
        } else {
            None
        };

        let mut square_box = NodeBuilder::new(
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
            let text_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText {
                    text: text.clone(),
                    size: tokens.typography.body_medium_size,
                    color: text_color,
                    underline: false,
                    wrap: false,
                    caret_index: None,
                }),
            )
            .build(cx);
            let mut layout = NodeBuilder::new(
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
        let mut row = NodeBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Flex {
                direction: fission_ir::FlexDirection::Row,
                wrap: fission_ir::op::FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: Some(8.0),
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
            value: Some(if self.checked {
                "true".into()
            } else {
                "false".into()
            }),
            actions: Default::default(),
            focusable: true,
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: Some(self.checked),
            disabled: false,
            draggable: false,
            scrollable_x: false,
            scrollable_y: false,
            min_value: None,
            max_value: None,
            current_value: None,
            is_focus_scope: false,
            is_focus_barrier: false,
            drag_payload: None,
            hero_tag: None,
            focus_index: None,
            capture_tab: false,
            auto_indent: false,
        };
        if let Some(action) = &self.on_toggle {
            semantics.actions.entries.push(fission_ir::ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Default,
                action_id: action.id.as_u128(),
                payload_data: Some(action.payload.clone()),
            });
        }

        let mut sem_node = NodeBuilder::new(id, Op::Semantics(semantics));
        sem_node.add_child(layout_id);
        sem_node.build(cx)
    }
}
