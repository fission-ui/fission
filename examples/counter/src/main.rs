use anyhow;
use fission_core::action::{Action, ActionId};
use fission_core::env::{VideoState, VideoStatus};
use fission_core::op::PaintOp;
use fission_core::ui::{
    Button, Column, CustomNode, Image, Node, Overlay, Row, Scroll, Stack, Text, TextContent,
    TextInput, Video,
};
use fission_core::{
    op::Color as IrColor, ActionEnvelope, AnimationPropertyId, AnimationRequest,
    AnimationStartValue, AppState, BuildCtx, FlexDirection, LowerDyn, LoweringContext, NodeBuilder,
    NodeId, Selector, View, Widget, WidgetNodeId,
};
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use fission_widgets::{canvas, checkbox, spacer, CheckboxProps, Portal};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

lazy_static! {
    static ref STATUS_WIDGET_ID: WidgetNodeId = WidgetNodeId::explicit("status_indicator");
    static ref DEMO_VIDEO_WIDGET_ID: WidgetNodeId = WidgetNodeId::explicit("demo_video");
    static ref STATUS_PULSE_PROPERTY: AnimationPropertyId =
        AnimationPropertyId::custom("pulse_intensity");
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CounterState {
    value: i32,
    #[serde(default)]
    text_value: String,
    #[serde(default)]
    checked: bool,
    #[serde(default)]
    show_modal: bool,
}

impl AppState for CounterState {}

#[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Increment;

fn on_increment(state: &mut CounterState, _action: Increment) {
    state.value += 1;
    println!("Counter incremented to: {}", state.value);
}

#[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
struct UpdateText(String);

#[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleChecked;

#[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ToggleModal;

fn on_toggle_modal(state: &mut CounterState, _action: ToggleModal) {
    state.show_modal = !state.show_modal;
}

struct CounterVM {
    label: String,
    is_even: bool,
    anim_val: f32,
    text_value: String,
}

impl Selector<CounterState> for CounterVM {
    type Output = CounterVM;

    fn select(view: &View<CounterState>) -> Self::Output {
        let anim_val = view.animation_value(*STATUS_WIDGET_ID, &STATUS_PULSE_PROPERTY);

        CounterVM {
            label: format!("Count: {}", view.state.value),
            is_even: view.state.value % 2 == 0,
            anim_val,
            text_value: view.state.text_value.clone(),
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
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0; 4],
            }),
        );
        layout_builder.add_child(paint_id);
        layout_builder.build(cx)
    }

    fn stable_key(&self) -> u64 {
        let mut key = self.id.as_u128();
        if self.active {
            key ^= 1;
        }
        u64::from_le_bytes(key.to_le_bytes()[0..8].try_into().unwrap())
    }
}

struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        let vm = view.select::<CounterVM>();

        ctx.anim_for(*STATUS_WIDGET_ID).request(AnimationRequest {
            property: STATUS_PULSE_PROPERTY.clone(),
            from: AnimationStartValue::Current,
            to: if vm.is_even { 1.0 } else { 0.0 },
            duration_ms: 250,
        });

        let video_state: VideoState = view
            .video_state(*DEMO_VIDEO_WIDGET_ID)
            .cloned()
            .unwrap_or_default();

        let video_controls = ctx.video_controls(*DEMO_VIDEO_WIDGET_ID);

        let half_duration = video_state.duration_ms.unwrap_or(0) / 2;

        let mut children = vec![
            Image {
                source: "docs/fission_logo.png".into(),
                width: Some(150.0),
                height: Some(150.0),
                ..Default::default()
            }
            .into(),
            // Canvas demo
            canvas(Some(220.0), Some(60.0), |cx| {
                use fission_core::op::{Color as IrColor, Fill};
                let r1 = NodeBuilder::new(
                    cx.next_node_id(),
                    fission_core::Op::Paint(PaintOp::DrawRect {
                        fill: Some(Fill {
                            color: IrColor {
                                r: 30,
                                g: 144,
                                b: 255,
                                a: 255,
                            },
                        }),
                        stroke: None,
                        corner_radius: 6.0,
                        shadow: None,
                    }),
                )
                .build(cx);
                let r2 = NodeBuilder::new(
                    cx.next_node_id(),
                    fission_core::Op::Paint(PaintOp::DrawRect {
                        fill: Some(Fill {
                            color: IrColor {
                                r: 46,
                                g: 204,
                                b: 113,
                                a: 255,
                            },
                        }),
                        stroke: None,
                        corner_radius: 6.0,
                        shadow: None,
                    }),
                )
                .build(cx);
                vec![r1, r2]
            }),
            spacer(Some(0.0), Some(8.0)),
            Video {
                id: Some(*DEMO_VIDEO_WIDGET_ID),
                source: "docs/video1.mp4".into(),
                width: Some(300.0),
                height: Some(200.0),
                autoplay: true,
                loop_playback: true,
                ..Default::default()
            }
            .build(ctx, view),
            Text {
                content: TextContent::Literal(vm.label.clone()),
                font_size: Some(24.0),
                ..Default::default()
            }
            .into(),
            Row {
                children: vec![
                    // Checkbox demo
                    checkbox(CheckboxProps {
                        checked: view.state.checked,
                        on_toggle: Some(ctx.bind(
                            ToggleChecked,
                            |state: &mut CounterState, _action: ToggleChecked| {
                                state.checked = !state.checked;
                                println!("Checked: {}", state.checked);
                            },
                        )),
                        label: Some("Enable feature".into()),
                    }),
                    spacer(Some(16.0), None),
                    Button {
                        on_press: Some(ctx.bind(ToggleModal, on_toggle_modal)),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal(if view.state.show_modal {
                                    "Hide Modal".into()
                                } else {
                                    "Show Modal".into()
                                }),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(140.0),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            Row {
                children: vec![
                    Button {
                        on_press: Some(video_controls.play()),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Play Video".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(140.0),
                        ..Default::default()
                    }
                    .into(),
                    Button {
                        on_press: Some(video_controls.pause()),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Pause Video".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(140.0),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            Row {
                children: vec![
                    Button {
                        on_press: Some(video_controls.stop()),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Stop".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(140.0),
                        ..Default::default()
                    }
                    .into(),
                    Button {
                        on_press: Some(video_controls.seek_to(0)),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Seek Start".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(140.0),
                        ..Default::default()
                    }
                    .into(),
                    Button {
                        on_press: Some(video_controls.seek_to(half_duration)),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Seek Mid".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(140.0),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            Row {
                children: vec![
                    Button {
                        on_press: Some(video_controls.set_rate(1.0)),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Rate 1x".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(140.0),
                        ..Default::default()
                    }
                    .into(),
                    Button {
                        on_press: Some(video_controls.set_rate(1.5)),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Rate 1.5x".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(140.0),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            Row {
                children: vec![
                    Button {
                        on_press: Some(video_controls.set_volume(0.0)),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Vol 0".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(120.0),
                        ..Default::default()
                    }
                    .into(),
                    Button {
                        on_press: Some(video_controls.set_volume(0.5)),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Vol 50%".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(120.0),
                        ..Default::default()
                    }
                    .into(),
                    Button {
                        on_press: Some(video_controls.set_volume(1.0)),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal("Vol 100%".into()),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(120.0),
                        ..Default::default()
                    }
                    .into(),
                    Button {
                        on_press: Some(video_controls.set_muted(!video_state.muted)),
                        child: Some(Box::new(
                            Text {
                                content: TextContent::Literal(
                                    if video_state.muted { "Unmute" } else { "Mute" }.into(),
                                ),
                                ..Default::default()
                            }
                            .into(),
                        )),
                        width: Some(120.0),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            Text {
                content: TextContent::Literal(format!(
                    "Video status: {:?} at {}ms / {:?} (rate {:.1}x, vol {:.0}%, muted {})",
                    video_state.status,
                    video_state.position_ms,
                    video_state.duration_ms,
                    video_state.rate,
                    video_state.volume * 100.0,
                    video_state.muted
                )),
                ..Default::default()
            }
            .into(),
            TextInput {
                value: vm.text_value.clone(),
                placeholder: Some("Type something...".into()),
                on_change: Some(ctx.bind(
                    UpdateText(String::new()),
                    |state: &mut CounterState, action: UpdateText| {
                        state.text_value = action.0;
                        println!("Text updated: {}", state.text_value);
                    },
                )),
                width: Some(200.0),
                ..Default::default()
            }
            .into(),
            // Live echo of input below the field
            Text {
                content: TextContent::Literal(format!("Echo: {}", vm.text_value)),
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
            Text {
                content: TextContent::Literal("Scroll down to see more...".into()),
                ..Default::default()
            }
            .into(),
        ];

        // Show a modal using Portal + Overlay layering above everything
        if view.state.show_modal {
            let modal = Overlay {
                id: None,
                content: Box::new(Node::Column(Column {
                    children: vec![],
                    ..Default::default()
                })),
                overlay: Box::new(Node::Stack(Stack {
                    children: vec![
                        // Dim background
                        Node::Custom(CustomNode {
                            debug_tag: "Dimmer".into(),
                            lowerer: Some(Arc::new(ModalDimLowerer)),
                        }),
                        // Modal box content
                        Node::Custom(CustomNode {
                            debug_tag: "Modal".into(),
                            lowerer: Some(Arc::new(ModalBoxLowerer)),
                        }),
                    ],
                    ..Default::default()
                })),
            };
            children.push(
                Portal {
                    child: Node::Overlay(modal),
                }
                .build(ctx, view),
            );
        }

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

#[derive(Debug)]
struct ModalDimLowerer;

impl LowerDyn for ModalDimLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        // Full-screen dim overlay (AbsoluteFill)
        let paint = NodeBuilder::new(
            cx.next_node_id(),
            fission_core::Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_core::op::Fill {
                    color: IrColor {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 150,
                    },
                }),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
        )
        .build(cx);
        let mut fill = NodeBuilder::new(
            cx.next_node_id(),
            fission_core::Op::Layout(fission_core::LayoutOp::AbsoluteFill),
        );
        fill.add_child(paint);
        fill.build(cx)
    }
}

#[derive(Debug)]
struct ModalBoxLowerer;

impl LowerDyn for ModalBoxLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        // Centered box using AbsoluteFill + inner Box
        let content = NodeBuilder::new(
            cx.next_node_id(),
            fission_core::Op::Paint(PaintOp::DrawRect {
                fill: Some(fission_core::op::Fill {
                    color: IrColor::WHITE,
                }),
                stroke: None,
                corner_radius: 8.0,
                shadow: None,
            }),
        )
        .build(cx);
        let mut inner = NodeBuilder::new(
            cx.next_node_id(),
            fission_core::Op::Layout(fission_core::LayoutOp::Box {
                width: Some(260.0),
                height: Some(120.0),
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [16.0; 4],
            }),
        );
        inner.add_child(content);
        let inner_id = inner.build(cx);

        // Wrap in AbsoluteFill to overlay and roughly center via padding
        let mut fill = NodeBuilder::new(
            cx.next_node_id(),
            fission_core::Op::Layout(fission_core::LayoutOp::AbsoluteFill),
        );
        fill.add_child(inner_id);
        fill.build(cx)
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(CounterApp);
    app.run()
}
