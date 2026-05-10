use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ActionEnvelope;
use fission_ir::{
    op::{LayoutOp, Op, PaintOp},
    NodeId,
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
///         handle_select as fn(&mut S, SelectOption),
///     );
///     children.push(Radio {
///         checked: view.state.selected == i,
///         on_select: Some(on_select),
///         label: Some(option.clone()),
///         ..Default::default()
///     }.into_node().into());
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Radio {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// Whether this radio button is currently selected.
    pub checked: bool,
    /// Action dispatched when this radio button is tapped.
    pub on_select: Option<ActionEnvelope>,
    /// Optional text label rendered next to the indicator.
    pub label: Option<String>,
}

impl Radio {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Radio(self)
    }
}

impl Lower for Radio {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let tokens = &cx.env.theme.tokens;
        let size = 18.0;
        let dot_size = size * 0.5;
        let _radius = size / 2.0;
        let border_color = tokens.colors.text_secondary;
        let active_color = tokens.colors.primary;
        let text_color = tokens.colors.text_primary;

        // Outer Circle
        let bg_paint = if self.checked {
            Op::Paint(PaintOp::DrawRect {
                fill: None,
                stroke: Some(fission_ir::op::Stroke {
                    fill: fission_ir::op::Fill::Solid(active_color),
                    width: 2.0,
                    dash_array: None,
                    line_cap: fission_ir::op::LineCap::Butt,
                    line_join: fission_ir::op::LineJoin::Miter,
                }),
                corner_radius: size / 2.0,
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
                corner_radius: size / 2.0,
                shadow: None,
            })
        };
        let outer_node = NodeBuilder::new(cx.next_node_id(), bg_paint).build(cx);

        // Dot
        let dot_node = if self.checked {
            let dot = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(fission_ir::op::Fill::Solid(active_color)),
                    stroke: None,
                    corner_radius: dot_size / 2.0,
                    shadow: None,
                }),
            )
            .build(cx);
            let mut dot_box = NodeBuilder::new(
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
            );
            dot_box.add_child(dot);
            let dot_box_id = dot_box.build(cx);
            let mut dot_align = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Align));
            dot_align.add_child(dot_box_id);
            let dot_align_id = dot_align.build(cx);
            let mut dot_container = NodeBuilder::new(
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
            Some(dot_container.build(cx))
        } else {
            None
        };

        let mut radio_box = NodeBuilder::new(
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
        if let Some(d) = dot_node {
            radio_box.add_child(d);
        }
        let radio_final = radio_box.build(cx);

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
                    caret_color: None,
                    caret_width: None,
                    caret_height: None,
                    caret_radius: None,
                    paragraph_style: None,
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
        row.add_child(radio_final);
        if let Some(l) = label_id {
            row.add_child(l);
        }
        row.build(cx);

        cx.pop_scope();

        let mut semantics = fission_ir::Semantics {
            role: fission_ir::Role::Checkbox, // Reuse Checkbox for Radio behavior?
            label: self.label.clone(),
            identifier: None,
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
            read_only: false,
            autofocus: false,
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
        if let Some(action) = &self.on_select {
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
