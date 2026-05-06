use anyhow;
use fission_core::op::PaintOp;
use fission_core::ui::{
    Button, Column, Composite, Image, Node, Row, Scroll, Text, TextContent, TextInput,
};
use fission_core::{
    op::Color as IrColor, AnimationPropertyId, AnimationRequest, AnimationStartValue, AppState,
    BuildCtx, FlexDirection, Handler, NodeBuilder, ReducerContext, Selector, View, Widget,
    WidgetNodeId,
};
use fission_shell_desktop::DesktopApp;
use fission_widgets::{canvas, Checkbox, Modal, ModalAction, Spacer};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref STATUS_WIDGET_ID: WidgetNodeId = WidgetNodeId::explicit("status_indicator");
    static ref DEMO_VIDEO_WIDGET_ID: WidgetNodeId = WidgetNodeId::explicit("demo_video");
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

fn on_increment(
    state: &mut CounterState,
    _action: Increment,
    _ctx: &mut ReducerContext<CounterState>,
) {
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

fn on_toggle_modal(
    state: &mut CounterState,
    _action: ToggleModal,
    _ctx: &mut ReducerContext<CounterState>,
) {
    state.show_modal = !state.show_modal;
}

struct CounterVM {
    label: String,
    is_even: bool,
    text_value: String,
}

impl Selector<CounterState> for CounterVM {
    type Output = CounterVM;

    fn select(view: &View<CounterState>) -> Self::Output {
        CounterVM {
            label: format!("Count: {}", view.state.value),
            is_even: view.state.value % 2 == 0,
            text_value: view.state.text_value.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusIndicator {
    id: WidgetNodeId,
    active: bool,
}

impl Widget<CounterState> for StatusIndicator {
    fn build(&self, _ctx: &mut BuildCtx<CounterState>, _view: &View<CounterState>) -> Node {
        let dot = fission_core::ui::Container::new(Spacer::default().into_node())
            .width(20.0)
            .height(20.0)
            .border_radius(10.0)
            .bg(if self.active {
                IrColor::GREEN
            } else {
                IrColor::RED
            })
            .into_node();
        Composite::new(dot)
            .animated_scale(self.id, 1.0)
            .animated_opacity(self.id, if self.active { 1.0 } else { 0.72 })
            .into_node()
    }
}

struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        let vm = view.select::<CounterVM>();
        let viewport_width = view.viewport_size().width.max(0.0);
        let control_width = (viewport_width - 96.0).clamp(180.0, 320.0);

        ctx.anim_for(*STATUS_WIDGET_ID).request(AnimationRequest {
            property: AnimationPropertyId::Scale,
            from: AnimationStartValue::Current,
            to: if vm.is_even { 1.14 } else { 1.0 },
            duration_ms: 250,
            repeat: false,
            delay_ms: 0,
            frame_interval_ms: None,
            easing: Default::default(),
        });
        ctx.anim_for(*STATUS_WIDGET_ID).request(AnimationRequest {
            property: AnimationPropertyId::Opacity,
            from: AnimationStartValue::Current,
            to: if vm.is_even { 1.0 } else { 0.72 },
            duration_ms: 250,
            repeat: false,
            delay_ms: 0,
            frame_interval_ms: None,
            easing: Default::default(),
        });

        let mut children = vec![
            Image {
                source: "docs/fission_logo.png".into(),
                width: Some(150.0),
                height: Some(150.0),
                ..Default::default()
            }
            .into(),
            canvas(Some(220.0), Some(60.0), |cx| {
                use fission_core::op::{Color as IrColor, Fill};
                let r1 = NodeBuilder::new(
                    cx.next_node_id(),
                    fission_core::Op::Paint(PaintOp::DrawRect {
                        fill: Some(Fill::Solid(IrColor {
                            r: 46,
                            g: 204,
                            b: 113,
                            a: 255,
                        })),
                        stroke: None,
                        corner_radius: 6.0,
                        shadow: None,
                    }),
                )
                .build(cx);
                let r2 = NodeBuilder::new(
                    cx.next_node_id(),
                    fission_core::Op::Paint(PaintOp::DrawRect {
                        fill: Some(Fill::Solid(IrColor {
                            r: 46,
                            g: 204,
                            b: 113,
                            a: 255,
                        })),
                        stroke: None,
                        corner_radius: 6.0,
                        shadow: None,
                    }),
                )
                .build(cx);
                vec![r1, r2]
            }),
            Spacer {
                width: Some(0.0),
                height: Some(8.0),
                ..Default::default()
            }
            .into(),
            Text {
                content: TextContent::Literal(vm.label.clone()),
                font_size: Some(24.0),
                ..Default::default()
            }
            .into(),
            Row {
                children: vec![
                    Checkbox {
                        checked: view.state.checked,
                        on_toggle: Some(ctx.bind(
                            ToggleChecked,
                            (|state: &mut CounterState, _action: ToggleChecked, _| {
                                state.checked = !state.checked;
                                println!("Checked: {}", state.checked);
                            }) as Handler<CounterState, ToggleChecked>,
                        )),
                        label: Some("Enable feature".into()),
                        ..Default::default()
                    }
                    .build(ctx, view),
                    Spacer {
                        width: Some(16.0),
                        height: None,
                        ..Default::default()
                    }
                    .into(),
                    Button {
                        on_press: Some(ctx.bind(
                            ToggleModal,
                            on_toggle_modal as Handler<CounterState, ToggleModal>,
                        )),
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
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            TextInput {
                value: vm.text_value.clone(),
                placeholder: Some("Type something...".into()),
                on_change: Some(ctx.bind(
                    UpdateText(String::new()),
                    (|state: &mut CounterState, action: UpdateText, _| {
                        state.text_value = action.0;
                        println!("Text updated: {}", state.text_value);
                    }) as Handler<CounterState, UpdateText>,
                )),
                width: Some(control_width),
                ..Default::default()
            }
            .into(),
            Text {
                content: TextContent::Literal(format!("Echo: {}", vm.text_value)),
                ..Default::default()
            }
            .into(),
            StatusIndicator {
                id: *STATUS_WIDGET_ID,
                active: vm.is_even,
            }
            .build(ctx, view),
            Button {
                on_press: Some(
                    ctx.bind(Increment, on_increment as Handler<CounterState, Increment>),
                ),
                child: Some(Box::new(
                    Text {
                        content: TextContent::Literal("Increment".into()),
                        color: Some(IrColor::WHITE),
                        ..Default::default()
                    }
                    .into(),
                )),
                ..Default::default()
            }
            .into(),
            Text {
                content: TextContent::Literal("Scroll down to see more...".into()),
                ..Default::default()
            }
            .into(),
        ];

        if view.state.show_modal {
            children.push(
                Modal {
                    id: WidgetNodeId::explicit("counter_modal"),
                    title: "Counter Modal".into(),
                    content: Box::new(
                        Column {
                            children: vec![
                                Text::new("The modal overlay should dim the full surface.").into(),
                                Text::new(
                                    "This example is used as a portal/compositor regression.",
                                )
                                .into(),
                            ],
                            gap: Some(8.0),
                            ..Default::default()
                        }
                        .into(),
                    ),
                    is_open: true,
                    on_dismiss: Some(ctx.bind(
                        ToggleModal,
                        on_toggle_modal as Handler<CounterState, ToggleModal>,
                    )),
                    actions: vec![ModalAction {
                        label: "Close".into(),
                        on_press: Some(ctx.bind(
                            ToggleModal,
                            on_toggle_modal as Handler<CounterState, ToggleModal>,
                        )),
                        is_primary: true,
                    }],
                    width: Some((viewport_width - 48.0).clamp(280.0, 420.0)),
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
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
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
