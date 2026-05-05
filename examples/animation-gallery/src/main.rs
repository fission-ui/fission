use fission_core::ui::{
    Button, ButtonVariant, Column, Composite, Container, Node, Row, Scroll, Text,
};
use fission_core::{
    op::Color as IrColor, AnimationPropertyId, AnimationRequest, AnimationStartValue, AppState,
    BuildCtx, FlexDirection, Handler, View, Widget, WidgetNodeId,
};
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use fission_widgets::Transition;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref OPACITY_ID: WidgetNodeId = WidgetNodeId::explicit("animation_gallery.opacity");
    static ref TRANSLATE_ID: WidgetNodeId = WidgetNodeId::explicit("animation_gallery.translate");
    static ref SCALE_ID: WidgetNodeId = WidgetNodeId::explicit("animation_gallery.scale");
    static ref ROTATION_ID: WidgetNodeId = WidgetNodeId::explicit("animation_gallery.rotation");
    static ref CLIP_ID: WidgetNodeId = WidgetNodeId::explicit("animation_gallery.clip");
    static ref CUSTOM_ID: WidgetNodeId = WidgetNodeId::explicit("animation_gallery.custom");
    static ref CUSTOM_PULSE: AnimationPropertyId = AnimationPropertyId::custom("pulse");
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnimationGalleryState {
    scene_active: bool,
    custom_active: bool,
}

impl Default for AnimationGalleryState {
    fn default() -> Self {
        Self {
            scene_active: true,
            custom_active: true,
        }
    }
}

impl AppState for AnimationGalleryState {}

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleScene;

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleCustom;

struct AnimationGalleryApp;

impl Widget<AnimationGalleryState> for AnimationGalleryApp {
    fn build(
        &self,
        ctx: &mut BuildCtx<AnimationGalleryState>,
        view: &View<AnimationGalleryState>,
    ) -> Node {
        let tokens = &view.env.theme.tokens.colors;
        let scene_active = view.state.scene_active;
        let custom_active = view.state.custom_active;

        if custom_active {
            ctx.anim_for(*CUSTOM_ID).request(AnimationRequest {
                property: CUSTOM_PULSE.clone(),
                from: AnimationStartValue::Explicit(0.0),
                to: 1.0,
                duration_ms: 1400,
                delay_ms: 0,
                repeat: true,
            });
        }

        let pulse = if custom_active {
            view.animation_value(*CUSTOM_ID, &CUSTOM_PULSE)
        } else {
            0.0
        };

        let title = Column {
            gap: Some(8.0),
            children: vec![
                Text::new("Animation Gallery").size(28.0).into_node(),
                Text::new(
                    "Built-in compositor-driven opacity, translation, scale, rotation, clip, scroll, and a custom build-driven pulse.",
                )
                .size(14.0)
                .color(tokens.text_secondary)
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node();

        let controls = Row {
            gap: Some(12.0),
            children: vec![
                Button {
                    child: Some(Box::new(Text::new("Toggle scene").into_node())),
                    on_press: Some(ctx.bind(
                        ToggleScene,
                        (|state: &mut AnimationGalleryState, _, _| {
                            state.scene_active = !state.scene_active;
                        }) as Handler<AnimationGalleryState, ToggleScene>,
                    )),
                    ..Default::default()
                }
                .into_node(),
                Button {
                    child: Some(Box::new(Text::new("Toggle custom pulse").into_node())),
                    on_press: Some(ctx.bind(
                        ToggleCustom,
                        (|state: &mut AnimationGalleryState, _, _| {
                            state.custom_active = !state.custom_active;
                        }) as Handler<AnimationGalleryState, ToggleCustom>,
                    )),
                    variant: ButtonVariant::Outline,
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node();

        let demos = Column {
            gap: Some(18.0),
            children: vec![
                Row {
                    gap: Some(18.0),
                    children: vec![
                        demo_card(
                            "Opacity",
                            Transition {
                                id: *OPACITY_ID,
                                property: AnimationPropertyId::Opacity,
                                value: if scene_active { 1.0 } else { 0.18 },
                                duration: 550,
                                child: Box::new(sample_block("Fade", tokens.primary)),
                                ..Default::default()
                            }
                            .build(ctx, view),
                        ),
                        demo_card(
                            "Translate X",
                            Transition {
                                id: *TRANSLATE_ID,
                                property: AnimationPropertyId::TranslateX,
                                value: if scene_active { 0.0 } else { -88.0 },
                                duration: 550,
                                child: Box::new(sample_block("Slide", color(30, 136, 93, 255))),
                                ..Default::default()
                            }
                            .build(ctx, view),
                        ),
                        demo_card(
                            "Scale",
                            Transition {
                                id: *SCALE_ID,
                                property: AnimationPropertyId::Scale,
                                value: if scene_active { 1.0 } else { 0.68 },
                                duration: 550,
                                child: Box::new(sample_block("Zoom", color(222, 144, 35, 255))),
                                ..Default::default()
                            }
                            .build(ctx, view),
                        ),
                    ],
                    ..Default::default()
                }
                .into_node(),
                Row {
                    gap: Some(18.0),
                    children: vec![
                        demo_card(
                            "Rotation",
                            Transition {
                                id: *ROTATION_ID,
                                property: AnimationPropertyId::Rotation,
                                value: if scene_active { 0.0 } else { -0.40 },
                                duration: 650,
                                child: Box::new(sample_block("Rotate", color(54, 96, 168, 255))),
                                ..Default::default()
                            }
                            .build(ctx, view),
                        ),
                        demo_card(
                            "Clip + translate",
                            Composite::new(
                                Transition {
                                    id: *CLIP_ID,
                                    property: AnimationPropertyId::TranslateX,
                                    value: if scene_active { 28.0 } else { -64.0 },
                                    duration: 700,
                                    child: Box::new(
                                        Container::new(sample_block("Clipped", tokens.primary))
                                            .width(116.0)
                                            .height(64.0)
                                            .into_node(),
                                    ),
                                    ..Default::default()
                                }
                                .build(ctx, view),
                            )
                            .clip_to_bounds(true)
                            .repaint_boundary(true)
                            .into_node(),
                        ),
                        demo_card(
                            "Custom pulse",
                            custom_pulse_card(pulse, custom_active, tokens.primary),
                        ),
                    ],
                    ..Default::default()
                }
                .into_node(),
                wide_demo_card(
                    "Scroll translation",
                    Scroll {
                        direction: FlexDirection::Row,
                        width: Some(720.0),
                        height: Some(88.0),
                        show_scrollbar: false,
                        child: Some(Box::new(scroll_strip(
                            tokens.primary,
                            color(84, 110, 122, 255),
                        ))),
                        ..Default::default()
                    }
                    .into_node(),
                    762.0,
                ),
            ],
            ..Default::default()
        }
        .into_node();

        Container::new(
            Column {
                gap: Some(20.0),
                children: vec![title, controls, demos],
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(24.0)
        .bg(tokens.background)
        .into_node()
    }
}

fn demo_card(title: &str, body: Node) -> Node {
    sized_demo_card(title, body, 236.0)
}

fn wide_demo_card(title: &str, body: Node, width: f32) -> Node {
    sized_demo_card(title, body, width)
}

fn sized_demo_card(title: &str, body: Node, width: f32) -> Node {
    let header = Text::new(title).size(14.0).into_node();
    let frame = Composite::new(
        Container::new(body)
            .height(112.0)
            .padding_all(14.0)
            .border(color(120, 120, 140, 70), 1.0)
            .border_radius(16.0)
            .bg(color(250, 250, 252, 255))
            .into_node(),
    )
    .repaint_boundary(true)
    .into_node();

    Container::new(
        Column {
            gap: Some(10.0),
            children: vec![header, frame],
            ..Default::default()
        }
        .into_node(),
    )
    .width(width)
    .padding_all(14.0)
    .border(color(218, 219, 228, 255), 1.0)
    .border_radius(18.0)
    .bg(color(255, 255, 255, 255))
    .into_node()
}

fn sample_block(label: &str, color: IrColor) -> Node {
    Container::new(
        Text::new(label)
            .size(18.0)
            .color(IrColor::WHITE)
            .into_node(),
    )
    .width(96.0)
    .height(64.0)
    .padding_all(18.0)
    .border_radius(18.0)
    .bg(color)
    .into_node()
}

fn custom_pulse_card(pulse: f32, active: bool, base: IrColor) -> Node {
    let brightness = (180.0 + pulse * 70.0).clamp(0.0, 255.0) as u8;
    let width = 86.0 + pulse * 28.0;
    let height = 54.0 + pulse * 18.0;
    let label = if active {
        format!("Pulse {:.2}", pulse)
    } else {
        "Pulse paused".to_string()
    };

    Container::new(
        Text::new(label)
            .size(16.0)
            .color(IrColor::WHITE)
            .into_node(),
    )
    .width(width)
    .height(height)
    .padding_all(14.0)
    .border_radius(16.0)
    .bg(color(base.r, brightness, base.b, 255))
    .into_node()
}

fn scroll_strip(primary: IrColor, alt: IrColor) -> Node {
    let mut items = Vec::new();
    for i in 0..14 {
        let bg = if i % 2 == 0 { primary } else { alt };
        items.push(
            Container::new(
                Text::new(format!("Lane {}", i + 1))
                    .size(14.0)
                    .color(IrColor::WHITE)
                    .into_node(),
            )
            .width(112.0)
            .height(52.0)
            .padding_all(16.0)
            .border_radius(14.0)
            .bg(bg)
            .into_node(),
        );
    }

    Row {
        gap: Some(12.0),
        children: items,
        ..Default::default()
    }
    .into_node()
}

fn main() -> anyhow::Result<()> {
    DesktopApp::new(AnimationGalleryApp)
        .with_title("Fission Animation Gallery")
        .run()
}

fn color(r: u8, g: u8, b: u8, a: u8) -> IrColor {
    IrColor { r, g, b, a }
}
