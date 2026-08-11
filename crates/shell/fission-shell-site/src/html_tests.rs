use super::*;
use fission_core::internal::BuildCtx;
use fission_core::ui::widgets::text::{RichTextChild, RichTextSpan, WidgetSpan};
use fission_core::ui::{Column, Grid, RichText, SemanticsRegion, Text, Widget};
use fission_core::{build, Env, RuntimeState, View};
use fission_ir::{
    ActionEntry, ActionSet, CompositeScalar, CompositeStyle, CoreIR, CoreNode, Op, Semantics,
    WidgetId,
};
use fission_widgets::MarkdownContent;

static TEST_FONT: [PackagedFont; 1] = [PackagedFont {
    family: "Test Sans",
    weight: 600,
    style: PackagedFontStyle::Italic,
    format: "truetype",
    data: b"font-bytes",
    axes: &[fission_theme::FontVariationAxis {
        tag: *b"wght",
        value: 612.0,
    }],
}];

fn render_test_widget(widget: impl Into<Widget>) -> RenderedHtml {
    let widget = widget.into();
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut lowering = fission_core::internal::InternalLoweringCx::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.ir.set_root(root);
    render_ir_to_html(&lowering.ir, &HtmlRenderOptions::default()).unwrap()
}

fn render_test_component(build_widget: impl FnOnce() -> Widget) -> RenderedHtml {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let state = ();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<()>::new();
    let widget = build::enter(&mut ctx, &view, build_widget);
    render_test_widget(widget)
}

fn generic_list_item(label: &str) -> Widget {
    SemanticsRegion::new(Text::new(label))
        .role(Role::ListItem)
        .into()
}

fn rendered_list_parts(html: &str) -> (&str, &str) {
    let (_, list) = html.split_once("<ul").expect("rendered list element");
    let (opening, after_opening) = list.split_once('>').expect("list opening tag");
    let (contents, _) = after_opening.split_once("</ul>").expect("list closing tag");
    (opening, contents)
}

#[test]
fn spotlight_layout_emits_browser_geometry_metadata() {
    let root = WidgetId::explicit("root");
    let anchor = WidgetId::explicit("tour-anchor");
    let spotlight = WidgetId::explicit("tour-spotlight");
    let regions = vec![
        WidgetId::explicit("tour-region-top"),
        WidgetId::explicit("tour-region-bottom"),
        WidgetId::explicit("tour-region-left"),
        WidgetId::explicit("tour-region-right"),
        WidgetId::explicit("tour-region-focus"),
    ];
    let mut ir = CoreIR::new();
    ir.add_node(
        anchor,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        Vec::new(),
    );
    for region in &regions {
        ir.add_node(
            *region,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 2 }),
            Vec::new(),
        );
    }
    ir.add_node(
        spotlight,
        Op::Layout(LayoutOp::Spotlight {
            anchor,
            padding: 12.0,
        }),
        regions,
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 3 }),
        vec![anchor, spotlight],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered
        .html
        .contains(&format!("data-fission-spotlight-anchor=\"{anchor}\"")));
    assert!(rendered
        .html
        .contains("data-fission-spotlight-padding=\"12\""));
    for region in [
        WidgetId::explicit("tour-region-top"),
        WidgetId::explicit("tour-region-bottom"),
        WidgetId::explicit("tour-region-left"),
        WidgetId::explicit("tour-region-right"),
        WidgetId::explicit("tour-region-focus"),
    ] {
        assert!(rendered
            .html
            .contains(&format!("data-fission-node=\"{region}\"")));
    }
}

#[test]
fn embeds_packaged_font_faces_in_site_css() {
    let root = WidgetId::explicit("root");
    let mut ir = CoreIR::new();
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        Vec::new(),
    );
    ir.set_root(root);
    let rendered = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            font_faces: &TEST_FONT,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(rendered.css.contains("@font-face"));
    assert!(rendered.css.contains("font-family:'Test Sans'"));
    assert!(rendered.css.contains("font-weight:600"));
    assert!(rendered.css.contains("font-style:italic"));
    assert!(rendered.css.contains("font-variation-settings:'wght' 612"));
    assert!(rendered.css.contains("base64,Zm9udC1ieXRlcw=="));
}

#[test]
fn lowers_interaction_motion_to_css_pseudo_states() {
    let motion = WidgetId::explicit("motion");
    let pressable = WidgetId::explicit("pressable");
    let styled_box = WidgetId::explicit("pressable-style");
    let mut ir = CoreIR::new();
    ir.add_node(
        styled_box,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle::default(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        Vec::new(),
    );
    ir.add_node(
        pressable,
        Op::Semantics(Semantics {
            role: Role::Button,
            focusable: true,
            ..Default::default()
        }),
        vec![styled_box],
    );
    ir.add_node(
        motion,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![pressable],
    );
    ir.set_root(motion);
    let rendered = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            motion_declarations: vec![MotionDeclaration {
                id: motion,
                kind: MotionDeclarationKind::Tracks {
                    tracks: vec![
                        MotionTrack::paint(
                            MotionPropertyId::BackgroundColor,
                            MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Color(
                                Color::BLACK,
                            ))),
                            MotionExpr::If {
                                predicate: MotionPredicate::Hovered(pressable),
                                then_expr: Box::new(MotionExpr::Value(MotionValue::Color(
                                    Color::WHITE,
                                ))),
                                else_expr: Box::new(MotionExpr::Value(MotionValue::Color(
                                    Color::BLACK,
                                ))),
                            },
                        )
                        .transition(MotionTransition::ease_out(160)),
                        MotionTrack::paint(
                            MotionPropertyId::BackgroundFill,
                            MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Fill(
                                Fill::Solid(Color::BLACK),
                            ))),
                            MotionExpr::If {
                                predicate: MotionPredicate::Hovered(pressable),
                                then_expr: Box::new(MotionExpr::Value(MotionValue::Fill(
                                    Fill::LinearGradient {
                                        start: (0.0, 0.0),
                                        end: (1.0, 0.0),
                                        stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)],
                                    },
                                ))),
                                else_expr: Box::new(MotionExpr::Value(MotionValue::Fill(
                                    Fill::Solid(Color::BLACK),
                                ))),
                            },
                        )
                        .transition(MotionTransition::Instant),
                        MotionTrack::paint(
                            MotionPropertyId::BoxShadows,
                            MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Shadows(
                                Vec::new(),
                            ))),
                            MotionExpr::If {
                                predicate: MotionPredicate::Hovered(pressable),
                                then_expr: Box::new(MotionExpr::Value(MotionValue::Shadows(vec![
                                    BoxShadow {
                                        color: Color::BLACK,
                                        offset: (0.0, 4.0),
                                        blur_radius: 12.0,
                                        spread_radius: 2.0,
                                        inset: false,
                                    },
                                ]))),
                                else_expr: Box::new(MotionExpr::Value(MotionValue::Shadows(
                                    Vec::new(),
                                ))),
                            },
                        )
                        .transition(MotionTransition::Instant),
                    ],
                },
            }],
            ..Default::default()
        },
    )
    .expect("render interaction motion");

    assert!(rendered.css.contains(":has("));
    assert!(rendered.css.contains(":hover"));
    assert!(rendered.css.contains("background-color:#ffffff"));
    assert!(rendered
        .css
        .contains("transition:background-color 160ms ease-out 0ms"));
    assert!(rendered.css.contains("background:linear-gradient("));
    assert!(rendered.css.contains("box-shadow:0px 4px 12px 2px #000000"));
}

#[test]
fn coalesces_ordered_shadows_into_one_css_shadow_list() {
    let root = WidgetId::explicit("shadow-root");
    let outer = WidgetId::explicit("outer-shadow");
    let inset = WidgetId::explicit("inset-shadow");
    let mut ir = CoreIR::new();
    for (id, shadow) in [
        (
            outer,
            BoxShadow {
                color: Color::BLACK,
                offset: (0.0, 4.0),
                blur_radius: 12.0,
                spread_radius: 2.0,
                inset: false,
            },
        ),
        (
            inset,
            BoxShadow {
                color: Color::WHITE,
                offset: (0.0, 1.0),
                blur_radius: 2.0,
                spread_radius: 0.0,
                inset: true,
            },
        ),
    ] {
        ir.add_node(
            id,
            Op::Paint(PaintOp::DrawRect {
                fill: None,
                stroke: None,
                corner_radius: 8.0,
                shadow: Some(shadow),
            }),
            Vec::new(),
        );
    }
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Box {
            width: Some(100.0),
            height: Some(40.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        }),
        vec![outer, inset],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    assert!(rendered
        .css
        .contains("box-shadow:0px 4px 12px 2px #000000,inset 0px 1px 2px 0px #ffffff"));
}

#[test]
fn renders_text_from_core_ir() {
    let root = WidgetId::explicit("root");
    let text = WidgetId::explicit("text");
    let mut ir = CoreIR::new();
    ir.add_node(
        text,
        Op::Paint(PaintOp::DrawText {
            text: "Hello <site>".into(),
            size: 16.0,
            color: Color::BLACK,
            underline: false,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![text],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    assert!(rendered.html.contains("Hello &lt;site&gt;"));
    assert!(!rendered.html.contains("style=\""));
    assert!(rendered.css.contains(".fs_"));
}

#[test]
fn renders_typed_image_sources_to_img_elements() {
    let root = WidgetId::explicit("root");
    let image = WidgetId::explicit("image");
    let mut ir = CoreIR::new();
    ir.add_node(
        image,
        Op::Paint(PaintOp::DrawImage {
            request: fission_ir::op::ImageRequest {
                source: ImageSource::Network {
                    url: "https://cdn.example.com/product.webp".into(),
                    headers: Vec::new(),
                    cache_policy: fission_ir::op::ImageCachePolicy::Default,
                },
                semantic_label: Some("Product photo".into()),
                ..Default::default()
            },
            fit: ImageFit::Cover,
            alignment: fission_ir::op::ImageAlignment::Center,
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![image],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered
        .html
        .contains("src=\"https://cdn.example.com/product.webp\""));
    assert!(rendered.html.contains("alt=\"Product photo\""));
    assert!(rendered.css.contains("object-fit:cover"));
}

#[test]
fn data_images_alignment_and_svg_dash_arrays_lower_to_html() {
    let root = WidgetId::explicit("root");
    let image = WidgetId::explicit("image");
    let path = WidgetId::explicit("path");
    let mut ir = CoreIR::new();
    ir.add_node(
        image,
        Op::Paint(PaintOp::DrawImage {
            request: fission_ir::op::ImageRequest {
                source: ImageSource::SvgText {
                    content: "<svg viewBox=\"0 0 1 1\"></svg>".into(),
                },
                semantic_label: Some("Inline icon".into()),
                ..Default::default()
            },
            fit: ImageFit::Contain,
            alignment: ImageAlignment::BottomEnd,
        }),
        Vec::new(),
    );
    ir.add_node(
        path,
        Op::Paint(PaintOp::DrawPath {
            path: "M0 0 L10 10".into(),
            fill: None,
            stroke: Some(Stroke {
                fill: Fill::Solid(Color::BLACK),
                width: 2.0,
                dash_array: Some(vec![4.0, 2.0]),
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
            }),
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![image, path],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("data:image/svg+xml;base64,"));
    assert!(rendered.css.contains("object-position:right bottom"));
    assert!(rendered.css.contains("stroke-dasharray:4 2"));
}

#[test]
fn embeds_and_native_controls_lower_without_static_rejection() {
    let root = WidgetId::explicit("root");
    let video_node = WidgetId::explicit("video-node");
    let video_widget = WidgetId::explicit("video-widget");
    let input = WidgetId::explicit("search-input");
    let mut ir = CoreIR::new();
    ir.add_node(
        video_node,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Video,
            widget_id: video_widget,
            width: Some(320.0),
            height: Some(180.0),
        }),
        Vec::new(),
    );
    ir.add_node(
        input,
        Op::Semantics(Semantics {
            role: Role::TextInput,
            label: Some("Search".into()),
            identifier: Some("search".into()),
            value: Some("fission".into()),
            ..Default::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![video_node, input],
    );
    ir.set_root(root);
    let mut options = HtmlRenderOptions::default();
    options.video_registrations.insert(
        video_widget,
        VideoRegistration {
            node_id: video_widget,
            source: "/media/demo.mp4".into(),
            autoplay: true,
            loop_playback: true,
            audio: Default::default(),
        },
    );

    let rendered = render_ir_to_html(&ir, &options).unwrap();

    assert!(rendered.html.contains("<video"));
    assert!(rendered.html.contains("src=\"media/demo.mp4\""));
    assert!(rendered.html.contains("autoplay muted"));
    assert!(rendered.html.contains("loop"));
    assert!(rendered.html.contains("<input"));
    assert!(rendered.html.contains("value=\"fission\""));
}

#[test]
fn radio_semantics_render_as_checked_native_radio_input() {
    let radio = WidgetId::explicit("shipping-express");
    let mut ir = CoreIR::new();
    ir.add_node(
        radio,
        Op::Semantics(Semantics {
            role: Role::Radio,
            label: Some("Express shipping".into()),
            identifier: Some("shipping.express".into()),
            checked: Some(true),
            focusable: true,
            ..Default::default()
        }),
        Vec::new(),
    );
    ir.set_root(radio);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("class=\"fission-site-radio\""));
    assert!(rendered.html.contains("type=\"radio\""));
    assert!(rendered.html.contains(" checked"));
    assert!(rendered
        .html
        .contains("data-fission-semantics=\"shipping.express\""));
    assert!(!rendered.html.contains("type=\"checkbox\""));
}

#[test]
fn layout_and_paint_motion_lower_to_css_keyframes() {
    let root = WidgetId::explicit("root");
    let panel = WidgetId::explicit("motion-panel");
    let mut ir = CoreIR::new();
    ir.add_node(
        panel,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 7 }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![panel],
    );
    ir.set_root(root);
    let options = HtmlRenderOptions {
        motion_declarations: vec![MotionDeclaration {
            id: panel,
            kind: MotionDeclarationKind::Tracks {
                tracks: vec![
                    MotionTrack {
                        property: MotionPropertyId::Width,
                        phase: fission_core::MotionPhase::Layout,
                        from: MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Px(0.0))),
                        to: MotionExpr::Value(MotionValue::Px(240.0)),
                        transition: MotionTransition::tween(180, MotionEasing::EaseOut),
                    },
                    MotionTrack {
                        property: MotionPropertyId::BackgroundColor,
                        phase: fission_core::MotionPhase::Paint,
                        from: MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Color(
                            Color::WHITE,
                        ))),
                        to: MotionExpr::Value(MotionValue::Color(Color::BLACK)),
                        transition: MotionTransition::tween(180, MotionEasing::EaseOut),
                    },
                ],
            },
        }],
        ..Default::default()
    };

    let rendered = render_ir_to_html(&ir, &options).unwrap();

    assert!(rendered.html.contains("fission-site-animated"));
    assert!(rendered.css.contains("width:0px"));
    assert!(rendered.css.contains("width:240px"));
    assert!(rendered.css.contains("background-color:#ffffff"));
    assert!(rendered.css.contains("background-color:#000000"));
    assert!(rendered.css.matches("@keyframes fission_anim_").count() >= 2);
}

#[test]
fn style_registry_deduplicates_normalized_styles() {
    let mut styles = StyleRegistry::default();
    let first = styles
        .class_for(vec!["color:red".to_string(), "display:block".to_string()])
        .unwrap();
    let second = styles
        .class_for(vec!["display:block;".to_string(), "color:red".to_string()])
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(styles.to_css().matches(".fs_").count(), 1);
}

#[test]
fn style_registry_keeps_last_declaration_for_duplicate_properties() {
    let mut styles = StyleRegistry::default();
    let class_name = styles
        .class_for(vec![
            "overflow:auto".to_string(),
            "display:block".to_string(),
            "overflow:hidden".to_string(),
        ])
        .unwrap();
    let css = styles.to_css();

    assert!(css.contains(&format!(".{class_name}")));
    assert!(css.contains("display:block;overflow:hidden"));
    assert!(!css.contains("overflow:auto"));
}

#[test]
fn repeated_rotation_animation_lowers_to_css_keyframes() {
    let root = WidgetId::explicit("root");
    let spinner = WidgetId::explicit("spinner-node");
    let target = WidgetId::explicit("spinner-animation");
    let mut ir = CoreIR::new();
    ir.nodes.insert(
        spinner,
        CoreNode {
            id: spinner,
            op: Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 7 }),
            composite: CompositeStyle {
                rotation: Some(CompositeScalar::new(0.0).motion(target)),
                ..Default::default()
            },
            children: Vec::new(),
            parent: None,
            hash: 0,
        },
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![spinner],
    );
    ir.set_root(root);
    let options = HtmlRenderOptions {
        motion_declarations: vec![MotionDeclaration {
            id: target,
            kind: MotionDeclarationKind::Tracks {
                tracks: vec![MotionTrack {
                    property: MotionPropertyId::Rotation,
                    phase: fission_core::MotionPhase::Composite,
                    from: MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Deg(0.0))),
                    to: MotionExpr::Value(MotionValue::Deg(360.0)),
                    transition: MotionTransition::tween(7000, MotionEasing::Linear)
                        .repeat(true)
                        .delay_ms(120),
                }],
            },
        }],
        ..Default::default()
    };

    let rendered = render_ir_to_html(&ir, &options).unwrap();

    assert!(rendered.html.contains("fission-site-animated"));
    assert!(rendered.css.contains("@keyframes fission_anim_"));
    assert!(rendered.css.contains("rotate:0deg"));
    assert!(rendered.css.contains("rotate:360deg"));
    assert!(rendered
        .css
        .contains("7000ms linear 120ms infinite normal both"));
    assert!(rendered.css.contains("prefers-reduced-motion:reduce"));
}

#[test]
fn scale_and_opacity_animations_share_one_dom_node() {
    let root = WidgetId::explicit("root");
    let pulse = WidgetId::explicit("pulse-node");
    let target = WidgetId::explicit("pulse-animation");
    let mut ir = CoreIR::new();
    ir.nodes.insert(
        pulse,
        CoreNode {
            id: pulse,
            op: Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 7 }),
            composite: CompositeStyle {
                opacity: Some(CompositeScalar::new(0.72).motion(target)),
                scale: Some(CompositeScalar::new(0.92).motion(target)),
                ..Default::default()
            },
            children: Vec::new(),
            parent: None,
            hash: 0,
        },
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![pulse],
    );
    ir.set_root(root);
    let options = HtmlRenderOptions {
        motion_declarations: vec![MotionDeclaration {
            id: target,
            kind: MotionDeclarationKind::Tracks {
                tracks: vec![
                    MotionTrack {
                        property: MotionPropertyId::Opacity,
                        phase: fission_core::MotionPhase::Composite,
                        from: MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Scalar(
                            0.72,
                        ))),
                        to: MotionExpr::Value(MotionValue::Scalar(1.0)),
                        transition: MotionTransition::tween(1400, MotionEasing::EaseInOut)
                            .repeat(true),
                    },
                    MotionTrack {
                        property: MotionPropertyId::Scale,
                        phase: fission_core::MotionPhase::Composite,
                        from: MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Scalar(
                            0.92,
                        ))),
                        to: MotionExpr::Value(MotionValue::Scalar(1.08)),
                        transition: MotionTransition::tween(1400, MotionEasing::EaseInOut)
                            .repeat(true),
                    },
                ],
            },
        }],
        ..Default::default()
    };

    let rendered = render_ir_to_html(&ir, &options).unwrap();

    assert!(rendered.css.contains("opacity:0.720"));
    assert!(rendered.css.contains("opacity:1"));
    assert!(rendered.css.contains("scale:0.920"));
    assert!(rendered.css.contains("scale:1.080"));
    assert!(rendered.css.matches("@keyframes fission_anim_").count() >= 2);
    assert!(rendered.css.contains(",fission_anim_"));
}

#[test]
fn centered_rich_text_lowers_to_width_bearing_block() {
    let root = WidgetId::explicit("root");
    let text = WidgetId::explicit("centered-text");
    let mut ir = CoreIR::new();
    ir.add_node(
        text,
        Op::Paint(PaintOp::DrawRichText {
            runs: vec![TextRun {
                text: "Centered\ncopy".into(),
                style: fission_ir::op::TextStyle {
                    font_size: 24.0,
                    color: Color::WHITE,
                    underline: false,
                    font_family: None,
                    locale: None,
                    font_weight: 700,
                    font_style: FontStyle::Normal,
                    line_height: Some(28.0),
                    letter_spacing: 0.0,
                    background_color: None,
                },
            }],
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: Some(fission_ir::op::TextParagraphStyle {
                text_align: TextAlign::Center,
                ..Default::default()
            }),
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![text],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.css.contains("display:block"));
    assert!(rendered.css.contains("width:100%"));
    assert!(rendered.css.contains("text-align:center"));
    assert!(!rendered
        .css
        .contains("display:inline;white-space:pre-wrap;text-align:center"));
}

#[test]
fn relative_hrefs_are_derived_from_current_route() {
    assert_eq!(
        relative_href_for_route("/docs/learn/quickstart/", "/reference/widgets/button/#api"),
        "../../../reference/widgets/button/#api"
    );
    assert_eq!(
        relative_href_for_route("/", "/docs/learn/overview/"),
        "docs/learn/overview/"
    );
    assert_eq!(
        relative_href_for_route("/docs/learn/quickstart/", "/"),
        "../../../"
    );
    assert!(site_link_is_current_page("/support", "/support/"));
    assert!(site_link_is_current_page(
        "/support?source=footer",
        "/support/?source=navigation"
    ));
    assert!(site_link_is_current_page("/", "/"));
    assert!(!site_link_is_current_page("#support", "/support"));
    assert!(!site_link_is_current_page(
        "https://example.test/support",
        "/support"
    ));
}

#[test]
fn rejects_interactive_actions() {
    let root = WidgetId::explicit("root");
    let mut semantics = Semantics::default();
    semantics.actions = ActionSet {
        entries: vec![ActionEntry {
            trigger: fission_ir::semantics::ActionTrigger::Default,
            action_id: 1,
            payload_data: None,
        }],
    };
    let mut ir = CoreIR::new();
    ir.nodes.insert(
        root,
        CoreNode {
            id: root,
            op: Op::Semantics(semantics),
            composite: Default::default(),
            children: Vec::new(),
            parent: None,
            hash: 0,
        },
    );
    ir.set_root(root);
    let error = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap_err();
    assert!(error.to_string().contains("interactive actions"));
}

#[test]
fn server_action_options_render_signed_post_form() {
    let root = WidgetId::explicit("server-action");
    let mut semantics = Semantics {
        role: Role::Button,
        ..Default::default()
    };
    semantics.actions = ActionSet {
        entries: vec![ActionEntry {
            trigger: fission_ir::semantics::ActionTrigger::Default,
            action_id: 7,
            payload_data: Some(vec![1, 2, 3]),
        }],
    };
    let mut ir = CoreIR::new();
    ir.nodes.insert(
        root,
        CoreNode {
            id: root,
            op: Op::Semantics(semantics),
            composite: Default::default(),
            children: Vec::new(),
            parent: None,
            hash: 0,
        },
    );
    ir.set_root(root);
    let mut options = HtmlRenderOptions {
        server_action_post_path: Some("/__fission/action".to_string()),
        ..Default::default()
    };
    options
        .server_action_tokens
        .insert((root, 7), "signed-token".to_string());

    let rendered = render_ir_to_html(&ir, &options).unwrap();
    assert!(rendered.html.contains("method=\"post\""));
    assert!(rendered.html.contains("action=\"/__fission/action\""));
    assert!(rendered.html.contains("name=\"token\""));
    assert!(rendered.html.contains("signed-token"));
}

#[test]
fn site_form_semantics_render_native_post_form() {
    let root = WidgetId::explicit("static-form");
    let semantics = Semantics {
        identifier: Some("site-form:contact".to_string()),
        label: Some("Contact".to_string()),
        value: Some(
            r#"{
                "action": "/contact/submit",
                "method": "post",
                "submitLabel": "Send",
                "fields": [
                    {"kind": "email", "name": "email", "label": "Email", "required": true, "maxLength": 320},
                    {"kind": "textarea", "name": "message", "label": "Message", "rows": 4},
                    {"kind": "checkbox", "name": "agree", "label": "Agree"}
                ]
            }"#
            .to_string(),
        ),
        ..Default::default()
    };
    let mut ir = CoreIR::new();
    ir.nodes.insert(
        root,
        CoreNode {
            id: root,
            op: Op::Semantics(semantics),
            composite: Default::default(),
            children: Vec::new(),
            parent: None,
            hash: 0,
        },
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            current_route_path: "/contact/".to_string(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(rendered.html.contains("<form"));
    assert!(rendered.html.contains("method=\"post\""));
    assert!(rendered.html.contains("action=\"../contact/submit\""));
    assert!(rendered
        .html
        .contains("data-fission-semantics=\"site-form:contact\""));
    assert!(rendered.html.contains("type=\"email\""));
    assert!(rendered.html.contains("name=\"message\""));
    assert!(rendered.html.contains("<textarea"));
    assert!(rendered.html.contains("type=\"checkbox\""));
    assert!(rendered.html.contains(">Send</button>"));
}

#[test]
fn escape_script_data_is_case_insensitive() {
    let escaped = escape_script_data(
        "<script>{\"value\":\"</script\",\"alt\":\"</Script\",\"upper\":\"</SCRIPT\"}</script>",
    );
    assert_eq!(
        escaped,
        "<script>{\"value\":\"<\\/script\",\"alt\":\"<\\/script\",\"upper\":\"<\\/script\"}<\\/script>",
    );
    assert_eq!(escaped.matches("<\\/script").count(), 4);
}

#[test]
fn browser_action_options_render_client_binding_attributes() {
    let root = WidgetId::explicit("browser-action");
    let mut semantics = Semantics {
        role: Role::Button,
        ..Default::default()
    };
    semantics.actions = ActionSet {
        entries: vec![ActionEntry {
            trigger: fission_ir::semantics::ActionTrigger::Default,
            action_id: 9,
            payload_data: Some(vec![0xde, 0xad]),
        }],
    };
    let mut ir = CoreIR::new();
    ir.nodes.insert(
        root,
        CoreNode {
            id: root,
            op: Op::Semantics(semantics),
            composite: Default::default(),
            children: Vec::new(),
            parent: None,
            hash: 0,
        },
    );
    ir.set_root(root);
    let options = HtmlRenderOptions {
        browser_action_bindings: true,
        ..Default::default()
    };

    let rendered = render_ir_to_html(&ir, &options).unwrap();

    assert!(rendered
        .html
        .contains("data-fission-browser-action=\"true\""));
    assert!(rendered.html.contains("data-fission-action-id=\"9\""));
    assert!(rendered
        .html
        .contains("data-fission-action-payload=\"dead\""));
}

#[test]
fn semantic_list_absorbs_column_layout_for_direct_list_items() {
    let rendered = render_test_widget(
        SemanticsRegion::new(Column {
            gap: Some(8.0),
            children: vec![
                generic_list_item("First item"),
                generic_list_item("Second item"),
            ],
            ..Default::default()
        })
        .identifier("sample-list")
        .label("Sample items")
        .role(Role::List),
    );

    let (opening, contents) = rendered_list_parts(&rendered.html);
    assert!(opening.contains("fission-site-column"));
    assert!(opening.contains("data-fission-semantics=\"sample-list\""));
    assert!(opening.contains("aria-label=\"Sample items\""));
    assert!(contents.starts_with("<li "));
    assert_eq!(contents.matches("<li ").count(), 2);
    assert!(contents.contains("</li><li "));
    assert!(!contents.starts_with("<div"));
    assert!(rendered.css.contains("display:flex"));
    assert!(rendered.css.contains("gap:8px"));
}

#[test]
fn semantic_list_absorbs_grid_layout_for_direct_list_items() {
    let rendered = render_test_widget(
        SemanticsRegion::new(Grid {
            columns: vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)],
            column_gap: Some(12.0),
            children: vec![
                generic_list_item("First result"),
                generic_list_item("Second result"),
            ],
            ..Default::default()
        })
        .role(Role::List),
    );

    let (opening, contents) = rendered_list_parts(&rendered.html);
    assert!(opening.contains("fission-site-grid"));
    assert!(contents.starts_with("<li "));
    assert_eq!(contents.matches("<li ").count(), 2);
    assert!(contents.contains("</li><li "));
    assert!(!contents.starts_with("<div"));
    assert!(rendered.css.contains("display:grid"));
    assert!(rendered.css.contains("grid-template-columns:1fr 1fr"));
    assert!(rendered.css.contains("column-gap:12px"));
}

#[test]
fn semantic_list_preserves_layout_inside_meaningful_list_item() {
    let nested_item = SemanticsRegion::new(Column {
        children: vec![Text::new("Nested item detail").into()],
        ..Default::default()
    })
    .identifier("nested-item")
    .role(Role::ListItem);
    let rendered = render_test_widget(
        SemanticsRegion::new(Column {
            children: vec![nested_item.into()],
            ..Default::default()
        })
        .role(Role::List),
    );

    let (_, contents) = rendered_list_parts(&rendered.html);
    assert!(contents.starts_with("<li "));
    assert!(contents.contains("data-fission-semantics=\"nested-item\""));
    let (_, item_contents) = contents.split_once('>').expect("list item opening tag");
    assert!(item_contents.starts_with("<div "));
    assert!(item_contents.contains("fission-site-column"));
}

#[test]
fn markdown_table_separates_header_and_body_rows() {
    let rendered = render_test_component(|| {
        MarkdownContent::new("| Package | Status |\n| --- | --- |\n| example | Ready |").into()
    });

    assert!(rendered.html.contains("<table "));
    assert!(rendered.html.contains("<thead><tr "));
    assert!(rendered.html.contains("<th "));
    assert!(rendered.html.contains("</thead><tbody><tr "));
    assert!(rendered.html.contains("<td "));
}

#[test]
fn markdown_rich_text_preserves_inline_links_and_emphasis() {
    let rendered = render_test_component(|| {
        MarkdownContent::new(
            "Read the [support guide](/support) and **keep this visible** with `sample-code`.",
        )
        .into()
    });

    assert!(rendered
        .body_html
        .contains("class=\"fission-site-link fission-site-markdown-link\""));
    assert!(rendered.html.contains("href=\"support\""));
    assert_eq!(
        rendered
            .body_html
            .matches("class=\"fission-site-link fission-site-markdown-link\"")
            .count(),
        1
    );
    let (_, link_and_after) = rendered
        .body_html
        .split_once("class=\"fission-site-link fission-site-markdown-link\"")
        .expect("inline Markdown link");
    let (_, link_content) = link_and_after
        .split_once('>')
        .expect("inline Markdown link opening tag");
    let (link_content, _) = link_content
        .split_once("</a>")
        .expect("inline Markdown link closing tag");
    assert!(link_content.contains("support guide"));
    assert!(!link_content.contains("Read the"));
    assert!(!link_content.contains("keep this visible"));
    assert!(rendered.css.contains("font-weight:700"));
    assert!(rendered.html.contains("sample-code"));
    assert!(rendered.css.contains("background:"));
}

#[test]
fn external_markdown_links_preserve_targets_and_add_safe_relationships() {
    let rendered = render_test_component(|| {
        MarkdownContent::new(
            "Visit the [reference](HTTPS://example.com/docs) or its [mirror](//cdn.example.com/docs).",
        )
        .into()
    });

    assert!(rendered
        .body_html
        .contains("href=\"HTTPS://example.com/docs\" rel=\"noopener noreferrer\""));
    assert!(rendered
        .body_html
        .contains("href=\"//cdn.example.com/docs\" rel=\"noopener noreferrer\""));
    assert!(!rendered.body_html.contains("target=\"_blank\""));
}

#[test]
fn rich_text_annotations_keep_inline_widgets_in_text_order() {
    let rendered = render_test_widget(RichText::from_span(
        RichTextSpan::new("Before ")
            .semantics_identifier("markdown-link:/guide")
            .children(vec![
                RichTextChild::from(WidgetSpan::new(Text::new("badge"), 40.0, 16.0)),
                RichTextChild::from(RichTextSpan::new(" after").weight(700)),
            ]),
    ));

    let before = rendered
        .body_html
        .find("Before ")
        .expect("text before marker");
    let badge = rendered
        .body_html
        .find("badge")
        .expect("inline widget marker payload");
    let after = rendered
        .body_html
        .find(" after")
        .expect("text after marker");
    assert!(before < badge);
    assert!(badge < after);
    assert_eq!(rendered.body_html.matches("href=\"guide\"").count(), 2);
}

#[test]
fn nested_rich_text_annotations_keep_parent_semantics_outermost() {
    let rendered = render_test_widget(RichText::from_span(
        RichTextSpan::new("")
            .semantics_label("Read documentation")
            .children([
                RichTextSpan::new("documentation").semantics_identifier("markdown-link:/docs")
            ]),
    ));

    assert!(rendered.body_html.contains(
        "<span aria-label=\"Read documentation\"><a class=\"fission-site-link fission-site-markdown-link\" href=\"docs\""
    ));
}

#[test]
fn client_action_semantics_render_generic_site_button() {
    let rendered = render_test_widget(
        SemanticsRegion::new(Text::new("Open preferences"))
            .identifier("site-client-action:open-preferences")
            .label("Open preferences")
            .role(Role::Generic),
    );

    assert!(rendered.html.contains("<button "));
    assert!(rendered
        .html
        .contains("data-fission-client-action=\"open-preferences\""));
    assert!(rendered
        .html
        .contains("data-fission-semantics=\"site-client-action:open-preferences\""));
    assert!(rendered.html.contains("aria-label=\"Open preferences\""));
    assert!(!rendered.html.contains(" disabled"));
}

#[test]
fn site_address_and_current_page_link_use_native_html_semantics() {
    let rendered = render_test_widget(Column {
        children: vec![
            SemanticsRegion::new(Text::new("Example Company"))
                .identifier("site-address")
                .role(Role::Generic)
                .into(),
            SemanticsRegion::new(Text::new("Home"))
                .identifier("site-link:/")
                .role(Role::Link)
                .into(),
        ],
        ..Default::default()
    });

    assert!(rendered.html.contains("<address "));
    assert!(rendered
        .html
        .contains("data-fission-semantics=\"site-address\""));
    assert!(rendered.html.contains("aria-current=\"page\""));
}

#[test]
fn responsive_css_preserves_first_match_precedence() {
    let root = WidgetId::explicit("responsive");
    let first = WidgetId::explicit("first");
    let second = WidgetId::explicit("second");
    let fallback = WidgetId::explicit("fallback");
    let mut ir = CoreIR::new();
    for child in [first, second, fallback] {
        ir.add_node(
            child,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            Vec::new(),
        );
    }
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Responsive {
            query: fission_ir::op::ResponsiveQuery::Viewport,
            cases: vec![
                fission_ir::op::ResponsiveCondition {
                    min_width: None,
                    max_width: Some(900.0),
                },
                fission_ir::op::ResponsiveCondition {
                    min_width: None,
                    max_width: Some(600.0),
                },
            ],
        }),
        vec![first, second, fallback],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    let later_case = rendered
        .css
        .find("(max-width:599.990px)")
        .expect("later responsive case");
    let first_case = rendered
        .css
        .find("(max-width:899.990px)")
        .expect("first responsive case");

    assert!(
        later_case < first_case,
        "the first case must be emitted last so equal-specificity CSS wins"
    );
}

#[test]
fn site_shell_sizes_stretched_container_query_children() {
    let root = WidgetId::explicit("stretch-box");
    let responsive = WidgetId::explicit("responsive");
    let fallback = WidgetId::explicit("fallback");
    let background = WidgetId::explicit("background");
    let mut ir = CoreIR::new();
    ir.add_node(
        fallback,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        Vec::new(),
    );
    ir.add_node(
        responsive,
        Op::Layout(LayoutOp::Responsive {
            query: fission_ir::op::ResponsiveQuery::Container,
            cases: Vec::new(),
        }),
        vec![fallback],
    );
    ir.add_node(
        background,
        Op::Paint(PaintOp::DrawRect {
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle {
                alignment: fission_ir::op::BoxAlignment::Stretch,
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![responsive, background],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered
        .html
        .contains("fission-site-box-stretch-auto-width"));
    assert!(rendered.html.contains("fission-site-responsive"));
    assert!(rendered.html.contains("container-type:inline-size"));
    assert!(crate::site_base_css()
        .contains(".fission-site-box-stretch-auto-width > .fission-site-node"));
}

#[test]
fn site_shell_preserves_explicit_width_on_stretch_children() {
    let root = WidgetId::explicit("stretch-box-static");
    let child = WidgetId::explicit("explicit-child");
    let mut ir = CoreIR::new();
    ir.add_node(
        child,
        Op::Layout(LayoutOp::Box {
            width: Some(240.0),
            height: Some(80.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle {
                alignment: fission_ir::op::BoxAlignment::Stretch,
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![child],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(!rendered
        .html
        .contains("fission-site-box-stretch-auto-width"));
    assert!(rendered.css.contains("width:240px"));
}

#[test]
fn site_shell_stretches_auto_width_grid_children() {
    let root = WidgetId::explicit("stretch-box-grid");
    let child = WidgetId::explicit("auto-grid");
    let background = WidgetId::explicit("grid-background");
    let mut ir = CoreIR::new();
    ir.add_node(
        child,
        Op::Layout(LayoutOp::Grid {
            columns: vec![fission_ir::op::GridTrack::Fr(1.0)],
            rows: Vec::new(),
            column_gap: None,
            row_gap: None,
            padding: [0.0; 4],
        }),
        Vec::new(),
    );
    ir.add_node(
        background,
        Op::Paint(PaintOp::DrawRect {
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle {
                alignment: fission_ir::op::BoxAlignment::Stretch,
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![child, background],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered
        .html
        .contains("fission-site-box-stretch-auto-width"));
}

#[test]
fn site_shell_preserves_intrinsic_width_inside_links() {
    let link = WidgetId::explicit("site-link");
    let container = WidgetId::explicit("link-container");
    let child = WidgetId::explicit("link-label");
    let mut ir = CoreIR::new();
    ir.add_node(
        child,
        Op::Paint(PaintOp::DrawText {
            text: "Open details".into(),
            size: 16.0,
            color: Color::BLACK,
            underline: false,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
        Vec::new(),
    );
    ir.add_node(
        container,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle {
                alignment: fission_ir::op::BoxAlignment::Stretch,
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![child],
    );
    ir.add_node(
        link,
        Op::Semantics(Semantics {
            role: Role::Link,
            identifier: Some("site-link:#details".into()),
            ..Default::default()
        }),
        vec![container],
    );
    ir.set_root(link);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("href=\"#details\""));
    assert!(!rendered
        .html
        .contains("fission-site-box-stretch-auto-width"));
}

#[test]
fn site_semantics_emit_native_landmarks_headings_and_anchors() {
    let root = WidgetId::explicit("root");
    let heading_text = WidgetId::explicit("semantic-heading-text");
    let identifiers = [
        "site-header",
        "site-main",
        "site-navigation",
        "site-section:features",
        "site-heading-2:page-title",
        "site-anchor:details",
        "site-footer",
    ];
    let mut ir = CoreIR::new();
    let mut semantic_nodes = Vec::new();
    let node_ids = [
        "semantic-header",
        "semantic-main",
        "semantic-navigation",
        "semantic-section",
        "semantic-heading",
        "semantic-anchor",
        "semantic-footer",
    ];
    ir.add_node(
        heading_text,
        Op::Paint(PaintOp::DrawText {
            text: "Page heading".into(),
            size: 24.0,
            color: Color::BLACK,
            underline: false,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
        Vec::new(),
    );
    for (identifier, node_id) in identifiers.into_iter().zip(node_ids) {
        let id = WidgetId::explicit(node_id);
        let node_children = if identifier == "site-heading-2:page-title" {
            vec![heading_text]
        } else {
            Vec::new()
        };
        ir.add_node(
            id,
            Op::Semantics(Semantics {
                role: Role::Generic,
                identifier: Some(identifier.to_string()),
                ..Default::default()
            }),
            node_children,
        );
        semantic_nodes.push(id);
    }
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        semantic_nodes,
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("<header "));
    assert!(rendered.html.contains("<main "));
    assert!(rendered.html.contains("<nav "));
    assert!(rendered.html.contains("<section "));
    assert!(rendered.html.contains("id=\"features\""));
    assert!(rendered.html.contains("<h2 "));
    assert!(rendered.html.contains("id=\"page-title\""));
    assert!(rendered.html.contains("Page heading"));
    assert!(rendered.html.contains("id=\"details\""));
    assert!(rendered.html.contains("<footer "));
    assert!(crate::site_base_css().contains("h2.fission-site-semantics"));
}

#[test]
fn site_link_semantics_emit_an_ordinary_anchor() {
    let root = WidgetId::explicit("site-link");
    let mut ir = CoreIR::new();
    ir.add_node(
        root,
        Op::Semantics(Semantics {
            role: Role::Link,
            identifier: Some("site-link:#details".into()),
            label: Some("View details".into()),
            ..Default::default()
        }),
        Vec::new(),
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("<a "));
    assert!(rendered.html.contains("href=\"#details\""));
    assert!(rendered.html.contains("aria-label=\"View details\""));
    assert!(!rendered.html.contains("<form"));
}
