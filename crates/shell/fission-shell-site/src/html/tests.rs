use super::*;
use fission_core::authoring::BuildCtx;
use fission_core::ui::widgets::text::{RichTextChild, RichTextSpan, WidgetSpan};
use fission_core::ui::{
    Checkbox, Column, Grid, Radio, RichText, SemanticsRegion, Switch, Text, TextInput, Widget,
};
use fission_core::{build, ActionEnvelope, ActionId, Env, RuntimeState, View};
use fission_ir::{
    ActionEntry, ActionSet, CompositeScalar, CompositeStyle, CoreIR, CoreNode, LayoutDirection, Op,
    Semantics, WidgetId,
};
use fission_widgets::{
    FormControlLayout, FormDescription, FormError, FormLabel, MarkdownContent, Select, SelectItem,
};

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
    let mut lowering = fission_core::internal::LoweringContext::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.set_root(root);
    render_ir_to_html(lowering.ir(), &HtmlRenderOptions::default()).unwrap()
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
fn right_to_left_ir_sets_direction_on_the_rendered_root() {
    let root = WidgetId::explicit("rtl-root");
    let mut ir = CoreIR::new();
    ir.layout_direction = LayoutDirection::RightToLeft;
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        Vec::new(),
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    assert!(rendered
        .body_html
        .starts_with("<div class=\"fission-site-root\" dir=\"rtl\">"));
}

#[test]
fn interaction_inert_subtrees_remain_visual_but_are_hidden_from_interaction_and_at() {
    let root = WidgetId::explicit("interaction-inert");
    let action = WidgetId::explicit("exiting-action");
    let text = WidgetId::explicit("exiting-text");
    let mut ir = CoreIR::new();
    ir.add_node(
        text,
        Op::Paint(PaintOp::DrawText {
            text: "Exiting content".into(),
            size: 14.0,
            color: Color::BLACK,
            underline: false,
            locale: None,
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
        vec![],
    );
    ir.add_node(
        action,
        Op::Semantics(Semantics {
            role: Role::Button,
            actions: ActionSet {
                entries: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: 7,
                    payload_data: Some(vec![1]),
                }],
            },
            ..Semantics::default()
        }),
        vec![text],
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::InteractionInert { stable_hash: 1 }),
        vec![action],
    );
    ir.set_root(root);

    validate_static_ir(&ir, false).expect("inert actions are not interactive static output");
    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    assert!(rendered.html.contains("Exiting content"));
    assert!(rendered.html.contains("aria-hidden=\"true\" inert"));
    assert!(rendered.css.contains("pointer-events:none"));
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
fn packaged_fonts_are_omitted_by_default() {
    let root = WidgetId::explicit("root");
    let mut ir = CoreIR::new();
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        Vec::new(),
    );
    ir.set_root(root);
    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    assert!(!rendered.css.contains("@font-face"));

    assert_eq!(
        packaged_font_css(&TEST_FONT[0], WebFontDelivery::default()),
        None
    );
}

#[test]
fn packaged_fonts_can_use_external_cacheable_assets() {
    let css = packaged_font_css(&TEST_FONT[0], WebFontDelivery::External).unwrap();
    assert!(css.contains("font-family:'Test Sans'"));
    assert!(css.contains("font-weight:600"));
    assert!(css.contains("font-style:italic"));
    assert!(css.contains("font-variation-settings:'wght' 612"));
    assert!(css.contains("src:url('__fission/fonts/"));
    assert!(!css.contains("data:"));
}

#[test]
fn packaged_fonts_embed_only_when_explicitly_requested() {
    let css = packaged_font_css(&TEST_FONT[0], WebFontDelivery::Inline).unwrap();
    assert!(css.contains("src:url('data:font/ttf;base64,Zm9udC1ieXRlcw==')"));
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
                                        extend: Default::default(),
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
                corner_radii: None,
                border_sides: None,
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
            locale: Some("en-GB".into()),
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: Some(fission_ir::op::TextParagraphStyle {
                text_direction: fission_ir::op::TextDirection::Ltr,
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
    assert!(rendered.html.contains("lang=\"en-GB\""));
    assert!(rendered.html.contains("dir=\"ltr\""));
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
                dash_offset: 0.0,
                trim: None,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
            }),
            view_box: None,
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
fn web_embed_uses_its_semantic_label_as_the_iframe_title() {
    let web_widget = WidgetId::explicit("web-widget");
    let web_node = WidgetId::explicit("web-node");
    let wrapper = WidgetId::explicit("web-wrapper");
    let semantics = WidgetId::explicit("web-semantics");
    let mut ir = CoreIR::new();
    ir.add_node(
        web_node,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Web,
            widget_id: web_widget,
            width: None,
            height: Some(640.0),
        }),
        Vec::new(),
    );
    ir.add_node(
        wrapper,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![web_node],
    );
    ir.add_node(
        semantics,
        Op::Semantics(Semantics {
            role: Role::Group,
            label: Some("Interactive Rust GUI example".into()),
            ..Default::default()
        }),
        vec![wrapper],
    );
    ir.set_root(semantics);

    let mut options = HtmlRenderOptions::default();
    options.web_registrations.insert(
        web_widget,
        WebRegistration {
            node_id: web_widget,
            url: "/showcase-app/".into(),
            user_agent: None,
        },
    );

    let rendered = render_ir_to_html(&ir, &options).unwrap();
    assert!(rendered
        .html
        .contains("title=\"Interactive Rust GUI example\""));
}

#[test]
fn text_controls_lower_complete_browser_input_contract() {
    use fission_ir::semantics::{
        TextCapitalization, TextFieldValidationState, TextInputAction, TextInputType,
    };

    let input = WidgetId::explicit("email-field");
    let mut ir = CoreIR::new();
    ir.add_node(
        input,
        Op::Semantics(Semantics {
            role: Role::TextInput,
            label: Some("Email".into()),
            identifier: Some("email-semantics".into()),
            text_field_name: Some("email".into()),
            autofill_group: Some("account".into()),
            text_input_type: TextInputType::EmailAddress,
            text_input_action: TextInputAction::Next,
            text_capitalization: TextCapitalization::None,
            required: true,
            min_length: Some(3),
            max_length: Some(320),
            validation_pattern: Some("[^@]+@[^@]+".into()),
            validation_state: TextFieldValidationState::Invalid,
            validation_message: Some("Enter a valid email".into()),
            autocorrect: false,
            spell_check: false,
            autofill_hints: vec!["email".into()],
            ..Default::default()
        }),
        Vec::new(),
    );
    ir.set_root(input);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    for attribute in [
        "name=\"email\"",
        "type=\"email\"",
        "inputmode=\"email\"",
        "enterkeyhint=\"next\"",
        "autocomplete=\"section-account email\"",
        "autocapitalize=\"none\"",
        "autocorrect=\"off\"",
        "spellcheck=\"false\"",
        "required",
        "minlength=\"3\"",
        "maxlength=\"320\"",
        "pattern=\"[^@]+@[^@]+\"",
        "aria-invalid=\"true\"",
        "aria-errormessage=\"fission-text-validation-",
        "data-fission-autofill-group=\"account\"",
    ] {
        assert!(rendered.html.contains(attribute), "missing {attribute}");
    }
    assert!(rendered.html.contains("Enter a valid email"));
}

#[test]
fn native_text_controls_replace_their_painted_editor_but_keep_form_messages() {
    let rendered = render_test_component(|| {
        FormControlLayout::new(
            WidgetId::explicit("account-email-field"),
            TextInput::default()
                .semantics_identifier("account-email")
                .required(true),
        )
        .label(FormLabel::new("Email"))
        .description(FormDescription::new("Used for account recovery"))
        .into()
    });

    assert_eq!(rendered.html.matches("<input ").count(), 1);
    assert!(rendered.html.contains("Used for account recovery"));
    assert!(!rendered.html.contains("aria-invalid=\"true\""));

    let invalid = render_test_component(|| {
        FormControlLayout::new(
            WidgetId::explicit("invalid-email-field"),
            TextInput::default().semantics_identifier("invalid-email"),
        )
        .label(FormLabel::new("Email"))
        .error(FormError::new("Enter a valid email"))
        .into()
    });

    assert_eq!(invalid.html.matches("<input ").count(), 1);
    assert!(invalid.html.contains("Enter a valid email"));
    assert!(invalid.html.contains("aria-invalid=\"true\""));
    assert!(!invalid.html.contains("fission-site-control-validation"));
}

#[test]
fn native_text_controls_preserve_the_resolved_placeholder_attribute() {
    let rendered = render_test_component(|| {
        FormControlLayout::new(
            WidgetId::explicit("account-email-field"),
            TextInput {
                placeholder: Some("you@example.com & more".into()),
                ..Default::default()
            },
        )
        .label(FormLabel::new("Email address"))
        .into()
    });

    assert!(rendered
        .html
        .contains("placeholder=\"you@example.com &amp; more\""));
    assert!(rendered.html.contains("Email address"));

    let filled = render_test_component(|| {
        FormControlLayout::new(
            WidgetId::explicit("filled-email-field"),
            TextInput {
                value: "person@example.com".into(),
                placeholder: Some("you@example.com".into()),
                ..Default::default()
            },
        )
        .label(FormLabel::new("Email address"))
        .into()
    });

    assert!(filled.html.contains("placeholder=\"you@example.com\""));
    assert!(filled.html.contains("value=\"person@example.com\""));
}

#[test]
fn native_text_controls_do_not_render_the_retained_editor_subtree() {
    let input = WidgetId::explicit("native-input");
    let painted_editor = WidgetId::explicit("painted-editor");
    let mut ir = CoreIR::new();
    ir.add_node(
        painted_editor,
        Op::Paint(PaintOp::DrawText {
            text: "retained editor duplicate".into(),
            size: 14.0,
            color: Color::BLACK,
            underline: false,
            locale: None,
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
        input,
        Op::Semantics(Semantics {
            role: Role::TextInput,
            value: Some("browser value".into()),
            ..Default::default()
        }),
        vec![painted_editor],
    );
    ir.set_root(input);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    assert!(rendered.html.contains("value=\"browser value\""));
    assert!(!rendered.html.contains("retained editor duplicate"));
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
fn retained_radio_uses_one_visual_and_binds_its_default_action() {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let state = ();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<()>::new();
    let widget = build::enter(&mut ctx, &view, || {
        Radio {
            label: Some("Express shipping".into()),
            on_select: Some(ActionEnvelope {
                id: ActionId::from_name("site-radio-test::select"),
                payload: vec![0xde, 0xad],
            }),
            ..Default::default()
        }
        .into()
    });
    let mut lowering = fission_core::internal::LoweringContext::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.set_root(root);

    let rendered = render_ir_to_html(
        lowering.ir(),
        &HtmlRenderOptions {
            browser_action_bindings: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(!rendered
        .html
        .contains("fission-site-control-label\">Express shipping"));
    assert!(rendered.html.contains("fission-site-retained-control"));
    assert!(rendered
        .html
        .contains("fission-site-retained-control-input"));
    assert!(rendered
        .html
        .contains("data-fission-browser-action=\"true\""));
    assert!(rendered
        .html
        .contains("data-fission-action-payload=\"dead\""));
    // The dot's own alignment element must fill the ring and center on
    // both axes. Check its generated rule rather than declaration order:
    // StyleRegistry canonicalizes CSS properties for deduplication.
    let align_class = rendered
        .html
        .split_once("class=\"fission-site-node fission-site-align ")
        .expect("the retained radio dot needs an alignment element")
        .1
        .split_once('"')
        .unwrap()
        .0;
    let align_style = rendered
        .css
        .split_once(&format!(".{align_class}{{"))
        .unwrap_or_else(|| panic!("missing alignment rule for {align_class}: {}", rendered.css))
        .1
        .split_once('}')
        .unwrap()
        .0;
    let declarations = align_style.split(';').collect::<Vec<_>>();
    for expected in [
        "display:flex",
        "width:100%",
        "height:100%",
        "align-items:center",
        "justify-content:center",
    ] {
        assert!(
            declarations.contains(&expected),
            "radio dot alignment is missing {expected}: {align_style}"
        );
    }
}

#[test]
fn disabled_toggle_widgets_render_checked_native_controls() {
    let rendered = render_test_component(|| {
        Column {
            children: vec![
                Checkbox {
                    checked: true,
                    disabled: true,
                    semantics_identifier: Some("disabled-checkbox".into()),
                    ..Default::default()
                }
                .into(),
                Radio {
                    checked: true,
                    disabled: true,
                    semantics_identifier: Some("disabled-radio".into()),
                    ..Default::default()
                }
                .into(),
                Switch {
                    checked: true,
                    disabled: true,
                    semantics_identifier: Some("disabled-switch".into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    });

    fn input_tag<'a>(html: &'a str, identifier: &str) -> &'a str {
        let marker = format!("data-fission-semantics=\"{identifier}\"");
        let marker_offset = html.find(&marker).expect("semantic identifier");
        let start = html[..marker_offset].rfind("<input").expect("input start");
        let end = marker_offset
            + html[marker_offset..]
                .find('>')
                .expect("input closing bracket")
            + 1;
        &html[start..end]
    }

    let checkbox = input_tag(&rendered.html, "disabled-checkbox");
    assert!(checkbox.contains("type=\"checkbox\""));
    assert!(checkbox.contains(" checked"));
    assert!(checkbox.contains(" disabled"));

    let radio = input_tag(&rendered.html, "disabled-radio");
    assert!(radio.contains("type=\"radio\""));
    assert!(radio.contains(" checked"));
    assert!(radio.contains(" disabled"));

    let switch = input_tag(&rendered.html, "disabled-switch");
    assert!(switch.contains("type=\"checkbox\""));
    assert!(switch.contains("role=\"switch\""));
    assert!(switch.contains(" checked"));
    assert!(switch.contains(" disabled"));
}

#[test]
fn checkbox_form_values_preserve_boolean_and_repeated_field_contracts() {
    let rendered = render_test_component(|| {
        Column {
            children: vec![
                Checkbox {
                    name: Some("terms".into()),
                    form_id: Some("connect".into()),
                    checked: true,
                    ..Default::default()
                }
                .into(),
                Checkbox {
                    name: Some("regions".into()),
                    form_id: Some("connect".into()),
                    form_value: Some("eu-west".into()),
                    checked: true,
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    });

    assert!(rendered.html.contains("name=\"terms\""));
    assert!(!rendered.html.contains("name=\"terms\" value="));
    assert!(rendered
        .html
        .contains("name=\"regions\" form=\"connect\" value=\"eu-west\" checked"));
    assert!(!rendered.html.contains("fission-site-retained-control"));
    assert!(!rendered
        .html
        .contains("fission-site-retained-control-input"));
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
    assert!(rendered.css.contains("animation-duration:0ms!important"));
    assert!(rendered
        .css
        .contains("animation-iteration-count:1!important"));
    assert!(!rendered.css.contains("animation:none!important"));
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
                    locale: Some("ar&amp;\"test".into()),
                    font_weight: 700,
                    font_style: FontStyle::Normal,
                    line_height: Some(28.0),
                    letter_spacing: 0.0,
                    background_color: None,
                    typography: Default::default(),
                },
            }],
            display_breaks: Vec::new(),
            paragraph_styles: Vec::new(),
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: Some(fission_ir::op::TextParagraphStyle {
                text_align: TextAlign::Center,
                text_direction: fission_ir::op::TextDirection::Rtl,
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
    assert!(rendered.html.contains("dir=\"rtl\""));
    assert!(rendered.html.contains("lang=\"ar&amp;amp;&quot;test\""));
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

mod semantics;
