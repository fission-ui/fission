use anyhow;
use fission_core::action::{Action, ActionId};
use fission_core::op::PaintOp;
use fission_core::ui::{
    Button, Column, CustomNode, Image, Node, Row, Scroll, Text, TextContent, Video,
};
use fission_core::{
    op::Color as IrColor, ActionEnvelope, AnimationPropertyId, AnimationRequest,
    AnimationStartValue, AppState, BuildCtx, FlexDirection, LowerDyn, LoweringContext, NodeBuilder,
    NodeId, Selector, View, Widget, WidgetNodeId,
};
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

lazy_static! {
    static ref STATUS_WIDGET_ID: WidgetNodeId = WidgetNodeId::explicit("status_indicator");
    static ref DEMO_VIDEO_WIDGET_ID: WidgetNodeId = WidgetNodeId::explicit("demo_video");
    static ref STATUS_PULSE_PROPERTY: AnimationPropertyId =
        AnimationPropertyId::new("pulse_intensity");
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CounterState {
    value: i32,
    pulse_generation: u64,
}

impl AppState for CounterState {}

#[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Increment;

fn on_increment(state: &mut CounterState, _action: Increment) {
    state.value += 1;
    state.pulse_generation += 1;
    println!("Counter incremented to: {}", state.value);
}

struct CounterVM {
    label: String,
    is_even: bool,
    anim_val: f32,
    pulse_generation: u64,
}

impl Selector<CounterState> for CounterVM {
    type Output = CounterVM;

    fn select(view: &View<CounterState>) -> Self::Output {
        let anim_val = view
            .animation_value(*STATUS_WIDGET_ID, &STATUS_PULSE_PROPERTY)
            .unwrap_or(0.0);

        CounterVM {
            label: format!("Count: {}", view.state.value),
            is_even: view.state.value % 2 == 0,
            anim_val,
            pulse_generation: view.state.pulse_generation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusIndicator {
    id: WidgetNodeId,
    active: bool,
    anim_val: f32,
}

impl Widget<CounterState> for StatusIndicator {
    fn build(&self, _ctx: &mut BuildCtx<CounterState>, _view: &View<CounterState>) -> Node {
        Node::Custom(CustomNode {
            debug_tag: "StatusIndicator".to_string(),
            lowerer: Some(Arc::new(StatusIndicatorLowerer {
                id: self.id,
                active: self.active,
                anim_val: self.anim_val,
            })),
        })
    }
}

#[derive(Debug)]
struct StatusIndicatorLowerer {
    id: WidgetNodeId,
    active: bool,
    anim_val: f32,
}

impl LowerDyn for StatusIndicatorLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = cx.widget_node_id(self.id);
        let base_color = if self.active {
            IrColor::GREEN
        } else {
            IrColor::RED
        };

        let r = (base_color.r as f32 * (1.0 - self.anim_val) + 0.0 * self.anim_val) as u8;
        let g = (base_color.g as f32 * (1.0 - self.anim_val) + 0.0 * self.anim_val) as u8;
        let b = (base_color.b as f32 * (1.0 - self.anim_val) + 255.0 * self.anim_val) as u8;
        let color = IrColor { r, g, b, a: 255 };

        let paint_id = NodeBuilder::new(
            NodeId::derived(layout_id.as_u128(), &[1]),
            fission_core::Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_core::op::Fill { color }),
                stroke: None,
                corner_radius: 10.0,
                shadow: None,
            }),
        )
        .build(cx);

        let mut layout_builder = NodeBuilder::new(
            layout_id,
            fission_core::Op::Layout(fission_core::LayoutOp::Box {
                width: Some(20.0),
                height: Some(20.0),
                padding: [0.0; 4],
            }),
        );
        layout_builder.add_child(paint_id);
        layout_builder.build(cx)
    }
}

struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        let vm = view.select::<CounterVM>();

        ctx.request_animation(AnimationRequest {
            target: *STATUS_WIDGET_ID,
            property: STATUS_PULSE_PROPERTY.clone(),
            from: AnimationStartValue::Current,
            to: if vm.is_even { 1.0 } else { 0.0 },
            duration_ms: 250,
        });

        let mut children = vec![
            Image {
                source: "docs/fission_logo.png".into(),
                width: Some(150.0),
                height: Some(150.0),
                ..Default::default()
            }
            .into(),
            Video {
                id: Some(*DEMO_VIDEO_WIDGET_ID),
                source: "docs/video1.mp4".into(),
                width: Some(300.0),
                height: Some(200.0),
                autoplay: true,
                loop_playback: true,
                ..Default::default()
            }
            .into(),
            Text {
                content: TextContent::Literal(vm.label.clone()),
                font_size: Some(24.0),
                ..Default::default()
            }
            .into(),
            StatusIndicator {
                id: *STATUS_WIDGET_ID,
                active: vm.is_even,
                anim_val: vm.anim_val,
            }
            .build(ctx, view),
            Button {
                on_press: Some(ctx.bind(Increment, on_increment)),
                child: Some(Box::new(
                    Text {
                        content: TextContent::Literal("Increment".into()),
                        color: Some(IrColor::WHITE),
                        ..Default::default()
                    }
                    .into(),
                )),
                width: Some(120.0),
                ..Default::default()
            }
            .into(),
            // Animation button removed to stabilize build
            Text {
                content: TextContent::Literal("Scroll down to see more...".into()),
                ..Default::default()
            }
            .into(),
        ];

        for i in 0..20 {
            children.push(
                Text {
                    content: TextContent::Literal(format!("Item {}", i)),
                    ..Default::default()
                }
                .into(),
            );
        }
        Scroll {
            direction: FlexDirection::Column,
            width: Some(600.0),
            height: Some(600.0),
            show_scrollbar: true,
            child: Some(Box::new(
                Column {
                    children,
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
            )),
            ..Default::default()
        }
        .into()
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(CounterApp);
    app.run()
}
