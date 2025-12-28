use fission_core::{action::{ActionEnvelope, AppState}, op::{Color, Fill, LayoutOp, Op, PaintOp, Stroke}, ui::{Button, Column, Row, Scroll, Stack, Text, TextContent}, BuildCtx, CustomNode, LowerDyn, LoweringContext, Node, NodeBuilder, NodeId, View, Widget, WidgetNodeId};
use fission_ir::semantics::{ActionEntry, ActionSet, Role, Semantics};
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};

static DROPDOWN_BUTTON_ID: LazyLock<WidgetNodeId> = LazyLock::new(|| {
    WidgetNodeId::explicit("dropdown_button")
});

// --- LOCAL WIDGETS for this example ---

#[derive(Debug)]
struct SizedBoxLowerer {
    width: Option<f32>,
    height: Option<f32>,
    child: Node,
}

impl LowerDyn for SizedBoxLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let child_id = self.child.lower(cx);
        let mut box_node = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
            }),
        );
        box_node.add_child(child_id);
        box_node.build(cx)
    }
}

fn sized_box(child: Node, width: Option<f32>, height: Option<f32>) -> Node {
    Node::Custom(fission_core::CustomNode {
        debug_tag: "SizedBox".into(),
        lowerer: Some(Arc::new(SizedBoxLowerer { width, height, child })),
    })
}

#[derive(Debug)]
struct PanelLowerer {
    child: Node,
}

impl LowerDyn for PanelLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let background_paint = {
            let paint_op = Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill { color: Color::WHITE }),
                stroke: Some(Stroke { color: Color { r: 200, g: 200, b: 200, a: 255 }, width: 1.0 }),
                corner_radius: 4.0,
                shadow: None,
            });
            NodeBuilder::new(cx.next_node_id(), paint_op).build(cx)
        };

        let child_id = self.child.lower(cx);

        let mut stack_builder = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::Stack));
        stack_builder.add_child(background_paint);
        stack_builder.add_child(child_id);
        stack_builder.build(cx)
    }
}

fn panel(child: Node) -> Node {
    Node::Custom(fission_core::CustomNode {
        debug_tag: "Panel".into(),
        lowerer: Some(Arc::new(PanelLowerer { child })),
    })
}

#[derive(Debug)]
struct FlyoutLowerer {
    anchor: NodeId,
    content: Node,
}

impl LowerDyn for FlyoutLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let content_id = self.content.lower(cx);
        let flyout_op = Op::Layout(LayoutOp::Flyout { anchor: self.anchor, content: content_id });
        NodeBuilder::new(cx.next_node_id(), flyout_op).build(cx);
        content_id
    }
}

fn flyout(anchor: NodeId, content: Node) -> Node {
    Node::Custom(fission_core::CustomNode {
        debug_tag: "Flyout".into(),
        lowerer: Some(Arc::new(FlyoutLowerer { anchor, content })),
    })
}

#[derive(Debug)]
struct DismissLayerLowerer {
    on_dismiss: ActionEnvelope,
}

impl LowerDyn for DismissLayerLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let fill_node = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill)).build(cx);
        let semantics = Semantics {
            role: Role::Button,
            label: None,
            value: None,
            focusable: true,
            actions: ActionSet {
                entries: vec![ActionEntry {
                    action_id: self.on_dismiss.id.as_u128(),
                    payload_data: Some(self.on_dismiss.payload.clone()),
                }],
            },
            multiline: false,
            masked: false,
            input_mask: None,
            ime_preedit_range: None,
            checked: None,
            disabled: false,
        };
        let mut semantics_node = NodeBuilder::new(cx.next_node_id(), Op::Semantics(semantics));
        semantics_node.add_child(fill_node);
        semantics_node.build(cx)
    }
}

fn dismiss_layer(on_dismiss: ActionEnvelope) -> Node {
    Node::Custom(fission_core::CustomNode {
        debug_tag: "DismissLayer".into(),
        lowerer: Some(Arc::new(DismissLayerLowerer { on_dismiss })),
    })
}

// --- STATE ---

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxAppState {
    pub dropdown_open: bool,
    pub selected_option: Option<String>,
}

impl AppState for InboxAppState {}

// --- ACTIONS ---

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OnToggleDropDown;

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OnSelectOption(String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DismissDropdown;


// --- APP WIDGET ---

#[derive(Default)]
pub struct InboxApp;

impl Widget<InboxAppState> for InboxApp {
    fn build(&self, ctx: &mut BuildCtx<InboxAppState>, view: &View<InboxAppState>) -> Node {
        ctx.bind(DismissDropdown, |state: &mut InboxAppState, _action| {
            state.dropdown_open = false;
        });

        let options = vec![
            "High Priority".to_string(),
            "Medium Priority".to_string(),
            "Low Priority".to_string(),
        ];

        let dropdown_button = Button {
            id: Some((*DROPDOWN_BUTTON_ID).into()),
            child: Some(Box::new(
                Text {
                    content: TextContent::Literal(view.state.selected_option.as_deref().unwrap_or("Select an option").into()),
                    ..Default::default()
                }
                .into(),
            )),
            on_press: Some(ctx.bind(
                OnToggleDropDown,
                |state: &mut InboxAppState, _action: OnToggleDropDown| {
                    state.dropdown_open = !state.dropdown_open;
                },
            )),
            ..Default::default()
        }
        .into();

        let mut layers = vec![];
        let main_content_children = vec![
            Text { content: TextContent::Literal("Email Content".into()), ..Default::default() }.into(),
            Text { content: TextContent::Literal("From: ...".into()), ..Default::default() }.into(),
            Text { content: TextContent::Literal("Subject: ...".into()), ..Default::default() }.into(),
            dropdown_button,
            Text { content: TextContent::Literal("Body: ...".into()), ..Default::default() }.into(),
        ];

        if view.state.dropdown_open {
            let dismiss_action = ctx.bind(DismissDropdown, |_, _| {});
            layers.push(dismiss_layer(dismiss_action));

            let mut option_nodes = vec![];
            for option in &options {
                option_nodes.push(
                    Button {
                        child: Some(Box::new(Text { content: TextContent::Literal(option.clone().into()), ..Default::default() }.into())),
                        on_press: Some(ctx.bind(
                            OnSelectOption(option.clone()),
                            |state: &mut InboxAppState, action: OnSelectOption| {
                                state.selected_option = Some(action.0.clone());
                                state.dropdown_open = false;
                            },
                        )),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            let panel_content = panel(Column { children: option_nodes, ..Default::default() }.into());
            layers.push(flyout((*DROPDOWN_BUTTON_ID).into(), panel_content));
        }

        let sidebar = sized_box(
            Column {
                children: vec![
                    Text { content: TextContent::Literal("Folders".into()), ..Default::default() }.into(),
                    Button { child: Some(Box::new(Text{content: TextContent::Literal("Inbox".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                    Button { child: Some(Box::new(Text{content: TextContent::Literal("Sent".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                    Button { child: Some(Box::new(Text{content: TextContent::Literal("Trash".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                ],
                ..Default::default()
            }
            .into(),
            Some(200.0),
            None
        );

        let email_list = sized_box(
            Scroll {
                child: Some(Box::new(Column {
                    children: vec![
                        Text { content: TextContent::Literal("Emails".into()), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 1".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 2".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 3".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 4".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 5".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 6".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 7".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 8".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 9".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                        Button { child: Some(Box::new(Text{content: TextContent::Literal("Email 10".into()), ..Default::default()}.into())), ..Default::default() }.into(),
                    ],
                    ..Default::default()
                }.into())),
                ..Default::default()
            }
            .into(),
            Some(300.0),
            None
        );

        let email_content = Column {
            children: main_content_children,
            ..Default::default()
        }
        .into();

        let main_ui = Row {
            children: vec![
                sidebar,
                email_list,
                email_content,
            ],
            ..Default::default()
        }
        .into();
        
        layers.insert(0, main_ui);

        Stack { children: layers, ..Default::default() }.into()
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::new(InboxApp::default()).run()
}