use fission_core::internal::{InternalLower, InternalLoweringCx};
use fission_core::{Action as CoreAction, ActionId, Env, RuntimeState};
use fission_ir::{ActionSet, LayoutOp, Op, Role, Semantics}; // Removed StructuralOp
use fission_widgets::{Button, Row, Text, TextContent, Widget};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

// Dummy Action for Button
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestClickAction {
    pub value: u32,
}

impl CoreAction for TestClickAction {
    fn static_id() -> ActionId {
        *TEST_CLICK_ACTION_ID
    }
}

lazy_static! {
    static ref TEST_CLICK_ACTION_ID: ActionId =
        ActionId::from_name("fission_widgets_test::TestClickAction");
}

#[test]
fn test_text_widget_default_and_lower() {
    let text_widget = Text::default();
    // content is default Literal("")

    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
    let node_id = text_widget.lower(&mut cx);

    assert!(cx.ir.nodes.contains_key(&node_id));
    let node = cx.ir.nodes.get(&node_id).unwrap();
    // Default Text maps to LayoutOp::Box
    assert!(matches!(node.op, Op::Layout(LayoutOp::Box { .. })));
}

#[test]
fn test_row_widget_children_lower() {
    let row_widget = Row {
        id: None,
        children: vec![
            Text {
                content: TextContent::Literal("Hello".into()),
                ..Default::default()
            }
            .into(),
            Text {
                content: TextContent::Literal("World".into()),
                ..Default::default()
            }
            .into(),
        ],
        semantics: None,
        ..Default::default()
    };

    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
    let row_node_id = row_widget.lower(&mut cx);

    assert!(cx.ir.nodes.contains_key(&row_node_id));
    let row_node = cx.ir.nodes.get(&row_node_id).unwrap();
    assert!(matches!(row_node.op, Op::Layout(LayoutOp::Flex { .. })));
    assert_eq!(row_node.children.len(), 2);
}

#[test]
fn test_button_widget_lower_with_child_and_semantics() {
    let button_widget = Button {
        id: None,
        child: Some(
            Text {
                content: TextContent::Literal("Click Me".into()),
                ..Default::default()
            }
            .into(),
        ),
        on_press: Some(TestClickAction { value: 1 }.into()),
        semantics: Some(Semantics {
            role: Role::Button,
            label: Some("My Button".into()),
            identifier: None,
            value: None,
            actions: ActionSet::default(),
            action_scope_id: None,
            focusable: true,
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: None,
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
            capture_tab: false,
            auto_indent: false,
            scroll_padding: None,
        }),
        ..Default::default()
    };

    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
    let button_node_id = button_widget.lower(&mut cx);

    assert!(cx.ir.nodes.contains_key(&button_node_id));

    // In new model, Button.lower returns button_layout_id.
    // If semantics are present, it wraps it.
    // So the returned ID should be the semantics node.
    let semantics_node = cx.ir.nodes.get(&button_node_id).unwrap();
    assert!(matches!(semantics_node.op, Op::Semantics(_)));

    if let Op::Semantics(s_op) = &semantics_node.op {
        assert_eq!(s_op.actions.entries.len(), 1);
        assert_eq!(
            s_op.actions.entries[0].action_id,
            TEST_CLICK_ACTION_ID.as_u128()
        );
        assert!(s_op.actions.entries[0].payload_data.is_some());
    }
}

#[test]
fn test_node_enum_lower() {
    let node = Widget::from(Text::default());
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
    fission_core::internal::lower_widget(&node, &mut cx);
    assert!(!cx.ir.nodes.is_empty());
}
