use fission_core::ui::{Button, Text, Row, Column, Scroll, Image, Video, TextContent, Node, CustomNode};
use fission_core::{AppState, ActionEnvelope, op::Color as IrColor, LoweringContext, LowerDyn, FlexDirection, Widget, View, BuildCtx, Selector, NodeId}; 
use fission_core::action::{Action, ActionId};
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use serde::{Serialize, Deserialize};
use lazy_static::lazy_static; 
use anyhow; 
use std::sync::Arc;
use fission_core::op::PaintOp;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CounterState {
    value: i32,
}

impl AppState for CounterState {}

#[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Increment;

fn on_increment(state: &mut CounterState, _action: Increment) {
    state.value += 1;
    println!("Counter incremented to: {}", state.value);
}

struct CounterVM {
    label: String,
    is_even: bool,
    anim_val: f32,
}

impl Selector<CounterState> for CounterVM {
    type Output = CounterVM;

    fn select(view: &View<CounterState>) -> Self::Output {
        // Animation logic temporarily removed due to baseline cleanup
        let val = 0.0; 
        
        CounterVM {
            label: format!("Count: {}", view.state.value),
            is_even: view.state.value % 2 == 0,
            anim_val: val,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusIndicator {
    active: bool,
    anim_val: f32,
}

impl Widget<CounterState> for StatusIndicator {
    fn build(&self, _ctx: &mut BuildCtx<CounterState>, _view: &View<CounterState>) -> Node {
        Node::Custom(CustomNode {
            debug_tag: "StatusIndicator".to_string(),
            lowerer: Some(Arc::new(StatusIndicatorLowerer { active: self.active, anim_val: self.anim_val })),
        })
    }
}

#[derive(Debug)]
struct StatusIndicatorLowerer {
    active: bool,
    anim_val: f32,
}

impl LowerDyn for StatusIndicatorLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = cx.next_node_id();
        let base_color = if self.active { IrColor::GREEN } else { IrColor::RED };
        
        let r = (base_color.r as f32 * (1.0 - self.anim_val) + 0.0 * self.anim_val) as u8;
        let g = (base_color.g as f32 * (1.0 - self.anim_val) + 0.0 * self.anim_val) as u8;
        let b = (base_color.b as f32 * (1.0 - self.anim_val) + 255.0 * self.anim_val) as u8;
        let color = IrColor { r, g, b, a: 255 };
        
        cx.add_node(
            layout_id,
            fission_core::Op::Layout(fission_core::LayoutOp::Box { width: Some(20.0), height: Some(20.0), padding: [0.0; 4] }), 
            vec![]
        );
        
        let paint_id = NodeId::explicit("status_anim");
        cx.add_node(
            paint_id, 
            fission_core::Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_core::op::Fill { color }),
                stroke: None,
                corner_radius: 10.0,
                shadow: None, 
            }), 
            vec![]
        );
        
        if let Some(node) = cx.ir.nodes.get_mut(&layout_id) {
            node.children.push(paint_id);
        }
        if let Some(node) = cx.ir.nodes.get_mut(&paint_id) {
            node.parent = Some(layout_id);
        }
        
        layout_id
    }
}

struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        let vm = view.select::<CounterVM>();

        let mut children = vec![
            Image {
                source: "docs/fission_logo.png".into(),
                width: Some(150.0),
                height: Some(150.0),
                ..Default::default()
            }.into(),
            
            Video {
                source: "docs/video1.mp4".into(),
                width: Some(300.0),
                height: Some(200.0),
                autoplay: true,
                loop_playback: true,
                ..Default::default()
            }.into(),
            
            Text { 
                content: TextContent::Literal(vm.label),
                font_size: Some(24.0),
                ..Default::default() 
            }.into(),
            
            StatusIndicator { active: vm.is_even, anim_val: vm.anim_val }.build(ctx, view),
            
            Button { 
                on_press: Some(ctx.bind(Increment, on_increment)), 
                child: Some(Box::new(Text { 
                    content: TextContent::Literal("Increment".into()), 
                    color: Some(IrColor::WHITE),
                    ..Default::default() 
                }.into())),
                width: Some(120.0),
                ..Default::default() 
            }.into(),
            
            // Animation button removed to stabilize build
            
            Text { content: TextContent::Literal("Scroll down to see more...".into()), ..Default::default() }.into(),
        ];

        for i in 0..20 {
            children.push(Text { 
                content: TextContent::Literal(format!("Item {}", i)), 
                ..Default::default() 
            }.into());
        }

        Scroll {
            direction: FlexDirection::Column,
            width: Some(600.0), 
            height: Some(400.0),
            show_scrollbar: true,
            child: Some(Box::new(Column {
                children,
                flex_grow: 1.0,
                ..Default::default()
            }.into())),
            ..Default::default()
        }.into()
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(CounterApp);
    app.run()
}
