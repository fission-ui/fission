use fission_widgets::{Button, Text, Row, TextContent, Node, Widget, View, BuildCtx, Selector, CustomNode}; 
use fission_core::{Action, AppState, ActionEnvelope, ActionId, op::Color as IrColor, LoweringContext, LowerDyn}; 
use fission_core::{Op, NodeId, op::{LayoutOp, PaintOp, Fill}}; 
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use serde::{Serialize, Deserialize};
use lazy_static::lazy_static; 
use anyhow; 
use std::sync::Arc;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CounterState {
    value: i32,
}

impl AppState for CounterState {}

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Increment;

fn on_increment(state: &mut CounterState, _action: Increment) {
    state.value += 1;
    println!("Counter incremented to: {}", state.value);
}

// 1. Define a Selector (View Model)
struct CounterVM {
    label: String,
    is_even: bool,
}

impl Selector<CounterState> for CounterVM {
    type Output = CounterVM;

    fn select(view: &View<CounterState>) -> Self::Output {
        CounterVM {
            label: format!("Count: {}", view.state.value),
            is_even: view.state.value % 2 == 0,
        }
    }
}

// 2. Define a Custom Widget using Node::Custom and LowerDyn
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusIndicator {
    active: bool,
}

impl Widget<CounterState> for StatusIndicator {
    fn build(&self, _ctx: &mut BuildCtx<CounterState>, _view: &View<CounterState>) -> Node {
        Node::Custom(CustomNode {
            debug_tag: "StatusIndicator".to_string(),
            lowerer: Some(Arc::new(StatusIndicatorLowerer { active: self.active })),
        })
    }
}

#[derive(Debug)]
struct StatusIndicatorLowerer {
    active: bool,
}

impl LowerDyn for StatusIndicatorLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = cx.next_node_id();
        let color = if self.active { IrColor::GREEN } else { IrColor::RED };
        
        // 1. Emit LayoutOp::Box (container)
        cx.add_node(
            layout_id,
            Op::Layout(LayoutOp::Box { width: Some(20.0), height: Some(20.0), padding: [0.0; 4] }), 
            vec![]
        );
        
        // 2. Emit PaintOp::DrawRect (circle)
        let paint_id = cx.next_node_id();
        cx.add_node(
            paint_id, 
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill { color }),
                stroke: None,
                corner_radius: 10.0, // Circle
                shadow: None, // Added
            }), 
            vec![]
        );
        
        // 3. Manually link parent/child
        if let Some(node) = cx.ir.nodes.get_mut(&layout_id) {
            node.children.push(paint_id);
        }
        if let Some(node) = cx.ir.nodes.get_mut(&paint_id) {
            node.parent = Some(layout_id);
        }
        
        layout_id
    }
}

// 3. Update App Widget
struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        // Use Selector
        let vm = view.select::<CounterVM>();

        Row {
            children: vec![
                Text { 
                    content: TextContent::Literal(vm.label), // Use VM data
                    // Removed width/height to test intrinsic sizing!
                    // But Taffy might collapse if no size? 
                    // No, TextMeasurer handles it.
                    // Text { ... }.into()
                    ..Default::default() 
                }.into(),
                
                // Add Custom Widget
                StatusIndicator { active: vm.is_even }.build(ctx, view),
                
                Button { 
                    on_press: Some(ctx.bind(Increment, on_increment)), 
                    child: Some(Box::new(Text { 
                        content: TextContent::Literal("Inc".into()), 
                        // Intrinsic size
                        ..Default::default() 
                    }.into())),
                    // Intrinsic size
                    ..Default::default() 
                }.into(),
            ],
            // Add some spacing to Row? 
            // Currently Row doesn't support gap.
            ..Default::default()
        }.into()
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(CounterApp);
    app.run()
}