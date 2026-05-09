use fission_core::env::{Env, RuntimeState};
use fission_core::lowering::LoweringContext;
use fission_core::ui::widgets::text::RichTextSpan;
use fission_core::ui::{Button, Container, Node, RichText, RichTextRun, Spacer, Text, TextInput};
use fission_ir::op::{
    decode_text_paragraph_style, Color, Fill, LayoutOp, Op, PaintOp, TextAlign, TextOverflow,
};
use fission_ir::{CoreIR, FlexDirection};

fn lower_node(node: Node) -> CoreIR {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime, None, None);
    let root = node.lower(&mut cx);
    cx.ir.root = Some(root);
    cx.ir
}

fn paint_ops(ir: &CoreIR) -> impl Iterator<Item = &PaintOp> {
    ir.nodes.values().filter_map(|node| match &node.op {
        Op::Paint(op) => Some(op),
        _ => None,
    })
}

fn layout_ops(ir: &CoreIR) -> impl Iterator<Item = &LayoutOp> {
    ir.nodes.values().filter_map(|node| match &node.op {
        Op::Layout(op) => Some(op),
        _ => None,
    })
}

#[test]
fn advanced_text_styles_lower_to_rich_text() {
    let ir = lower_node(
        Text::new("Headline")
            .family("Inter")
            .weight(600)
            .italic(true)
            .line_height(24.0)
            .letter_spacing(0.5)
            .into_node(),
    );

    let runs = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { runs, .. } => Some(runs),
            _ => None,
        })
        .expect("rich text paint op");

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].style.font_family.as_deref(), Some("Inter"));
    assert_eq!(runs[0].style.font_weight, 600);
    assert_eq!(runs[0].style.font_style, fission_ir::op::FontStyle::Italic);
    assert_eq!(runs[0].style.line_height, Some(24.0));
    assert_eq!(runs[0].style.letter_spacing, 0.5);
}

#[test]
fn rich_text_widget_lowers_multiple_runs() {
    let ir = lower_node(
        RichText::new(vec![
            RichTextRun::new("Hello ").family("Inter").weight(600),
            RichTextRun::new("world")
                .family("Space Grotesk")
                .italic(true),
        ])
        .into_node(),
    );

    let runs = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { runs, .. } => Some(runs),
            _ => None,
        })
        .expect("rich text paint op");

    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].style.font_family.as_deref(), Some("Inter"));
    assert_eq!(runs[0].style.font_weight, 600);
    assert_eq!(runs[1].style.font_family.as_deref(), Some("Space Grotesk"));
    assert_eq!(runs[1].style.font_style, fission_ir::op::FontStyle::Italic);
}

#[test]
fn container_background_fill_accepts_gradients() {
    let gradient = Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (200.0, 0.0),
        stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)],
    };

    let ir = lower_node(
        Container::new(
            Spacer {
                width: Some(40.0),
                height: Some(12.0),
                ..Default::default()
            }
            .into_node(),
        )
        .bg_fill(gradient.clone())
        .into_node(),
    );

    let fill = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRect { fill, .. } => fill.as_ref(),
            _ => None,
        })
        .expect("rect fill");

    assert_eq!(fill, &gradient);
}

#[test]
fn button_background_fill_and_text_override_lower() {
    let gradient = Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (240.0, 0.0),
        stops: vec![
            (
                0.0,
                Color {
                    r: 64,
                    g: 39,
                    b: 255,
                    a: 255,
                },
            ),
            (
                1.0,
                Color {
                    r: 0,
                    g: 212,
                    b: 255,
                    a: 255,
                },
            ),
        ],
    };

    let ir = lower_node(
        Button {
            child: Some(Box::new(Text::new("Continue").into_node())),
            background_fill: Some(gradient.clone()),
            text_color: Some(Color::WHITE),
            ..Default::default()
        }
        .into_node(),
    );

    let fill = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRect { fill, .. } => fill.as_ref(),
            _ => None,
        })
        .expect("button background fill");
    assert_eq!(fill, &gradient);

    let text_color = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawText { text, color, .. } if text == "Continue" => Some(*color),
            _ => None,
        })
        .expect("button label");
    assert_eq!(text_color, Color::WHITE);
}

#[test]
fn text_input_supports_decorations_and_typography_overrides() {
    let ir = lower_node(
        TextInput {
            value: "alice@example.com".into(),
            font_family: Some("Inter".into()),
            font_weight: Some(500),
            line_height: Some(22.0),
            letter_spacing: Some(0.25),
            prefix: Some(Box::new(Text::new("@").into_node())),
            suffix: Some(Box::new(Text::new(".com").into_node())),
            ..Default::default()
        }
        .into_node(),
    );

    assert!(layout_ops(&ir).any(|op| matches!(
        op,
        LayoutOp::Flex {
            direction: FlexDirection::Row,
            ..
        }
    )));
    assert!(layout_ops(&ir).any(|op| matches!(op, LayoutOp::Scroll { .. })));

    let runs = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { runs, .. }
                if runs
                    .iter()
                    .any(|run| run.text.contains("alice@example.com")) =>
            {
                Some(runs)
            }
            _ => None,
        })
        .expect("input text runs");

    let value_run = runs
        .iter()
        .find(|run| run.text.contains("alice@example.com"))
        .expect("value run");

    assert_eq!(value_run.style.font_family.as_deref(), Some("Inter"));
    assert_eq!(value_run.style.font_weight, 500);
    assert_eq!(value_run.style.line_height, Some(22.0));
    assert_eq!(value_run.style.letter_spacing, 0.25);
}

#[test]
fn text_input_lowers_cursor_and_semantics_overrides() {
    let ir = lower_node(
        TextInput {
            value: "hello".into(),
            placeholder: Some("Email".into()),
            read_only: true,
            enabled: false,
            autofocus: true,
            keyboard_type: fission_ir::semantics::TextInputType::EmailAddress,
            text_input_action: fission_ir::semantics::TextInputAction::Search,
            text_capitalization: fission_ir::semantics::TextCapitalization::Words,
            max_length: Some(24),
            input_formatters: vec![fission_ir::semantics::InputFormatter::AsciiOnly],
            autocorrect: false,
            enable_suggestions: false,
            spell_check: false,
            smart_dashes: true,
            smart_quotes: true,
            autofill_hints: Vec::new(),
            on_submit: Some(fission_core::ActionEnvelope {
                id: fission_core::ActionId::from_name("tests::submit"),
                payload: br#""payload""#.to_vec(),
            }),
            cursor_color: Some(Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            }),
            cursor_width: Some(3.0),
            ..Default::default()
        }
        .into_node(),
    );

    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.role == fission_ir::Role::TextInput => {
                Some(semantics)
            }
            _ => None,
        })
        .expect("text input semantics");

    assert_eq!(semantics.label.as_deref(), Some("Email"));
    assert!(semantics.read_only);
    assert!(semantics.disabled);
    assert!(!semantics.focusable);
    assert!(semantics.autofocus);
    assert_eq!(
        semantics.text_input_type,
        fission_ir::semantics::TextInputType::EmailAddress
    );
    assert_eq!(
        semantics.text_input_action,
        fission_ir::semantics::TextInputAction::Search
    );
    assert_eq!(
        semantics.text_capitalization,
        fission_ir::semantics::TextCapitalization::Words
    );
    assert_eq!(semantics.max_length, Some(24));
    assert_eq!(
        semantics.input_formatters,
        vec![fission_ir::semantics::InputFormatter::AsciiOnly]
    );
    assert!(!semantics.autocorrect);
    assert!(!semantics.enable_suggestions);
    assert!(semantics
        .actions
        .entries
        .iter()
        .any(|entry| entry.trigger == fission_ir::semantics::ActionTrigger::Submit));

    let caret = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText {
                caret_color,
                caret_width,
                ..
            } => Some((caret_color, caret_width)),
            _ => None,
        })
        .expect("input paint op");

    assert_eq!(
        caret.0,
        &Some(Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        })
    );
    assert_eq!(caret.1, &Some(3.0));
}

#[test]
fn text_lowers_paragraph_controls_for_alignment_and_ellipsis() {
    let ir = lower_node(
        Text::new("Paragraph parity")
            .size(16.0)
            .text_align(TextAlign::Center)
            .max_lines(2)
            .overflow(TextOverflow::Ellipsis)
            .into_node(),
    );

    let paragraph = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawText { caret_width, .. } => decode_text_paragraph_style(*caret_width),
            _ => None,
        })
        .expect("paragraph metadata");

    assert_eq!(paragraph.text_align, TextAlign::Center);
    assert_eq!(paragraph.max_lines, Some(2));
    assert_eq!(paragraph.overflow, TextOverflow::Ellipsis);

    let clipped_box = ir
        .nodes
        .values()
        .find(|node| {
            node.composite.clip_to_bounds
                && matches!(
                    node.op,
                    Op::Layout(LayoutOp::Box {
                        max_height: Some(height),
                        ..
                    }) if (height - 38.4).abs() < 0.001
                )
        })
        .expect("clipped layout box");

    assert!(matches!(clipped_box.op, Op::Layout(LayoutOp::Box { .. })));
}

#[test]
fn rich_text_lowers_paragraph_controls_for_line_capping() {
    let ir = lower_node(
        RichText::new(vec![
            RichTextRun::new("Hello ").size(14.0).line_height(18.0),
            RichTextRun::new("world").size(20.0).line_height(24.0),
        ])
        .text_align(TextAlign::End)
        .max_lines(3)
        .overflow(TextOverflow::Clip)
        .into_node(),
    );

    let paragraph = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { caret_width, .. } => decode_text_paragraph_style(*caret_width),
            _ => None,
        })
        .expect("paragraph metadata");

    assert_eq!(paragraph.text_align, TextAlign::End);
    assert_eq!(paragraph.max_lines, Some(3));
    assert_eq!(paragraph.overflow, TextOverflow::Clip);

    let clipped_box = ir
        .nodes
        .values()
        .find(|node| {
            node.composite.clip_to_bounds
                && matches!(
                    node.op,
                    Op::Layout(LayoutOp::Box {
                        max_height: Some(height),
                        ..
                    }) if (height - 72.0).abs() < 0.001
                )
        })
        .expect("clipped layout box");

    assert!(matches!(clipped_box.op, Op::Layout(LayoutOp::Box { .. })));
}

#[test]
fn nested_rich_text_spans_flatten_styles_in_order() {
    let ir = lower_node(
        RichText::from_span(
            RichTextSpan::new("Hello ")
                .color(Color {
                    r: 12,
                    g: 34,
                    b: 56,
                    a: 255,
                })
                .weight(600)
                .child(RichTextSpan::new("world").italic(true)),
        )
        .into_node(),
    );

    let runs = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { runs, .. } => Some(runs),
            _ => None,
        })
        .expect("rich text paint op");

    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].text, "Hello ");
    assert_eq!(
        runs[0].style.color,
        Color {
            r: 12,
            g: 34,
            b: 56,
            a: 255
        }
    );
    assert_eq!(runs[0].style.font_weight, 600);
    assert_eq!(runs[1].text, "world");
    assert_eq!(
        runs[1].style.color,
        Color {
            r: 12,
            g: 34,
            b: 56,
            a: 255
        }
    );
    assert_eq!(runs[1].style.font_weight, 600);
    assert_eq!(runs[1].style.font_style, fission_ir::op::FontStyle::Italic);
}

#[test]
fn rich_text_span_semantics_labels_wrap_accessible_text() {
    let ir = lower_node(
        RichText::from_span(
            RichTextSpan::new("FYI")
                .semantics_label("For your information")
                .child(RichTextSpan::new("!")),
        )
        .into_node(),
    );

    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.label.is_some() => Some(semantics),
            _ => None,
        })
        .expect("rich text semantics");

    assert_eq!(semantics.label.as_deref(), Some("For your information!"));
    assert!(semantics.multiline);
}

#[test]
fn text_semantics_label_builder_sets_label() {
    let ir = lower_node(
        Text::new("Visible")
            .semantics_label("Screen reader")
            .into_node(),
    );

    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.label.is_some() => Some(semantics),
            _ => None,
        })
        .expect("text semantics");

    assert_eq!(semantics.label.as_deref(), Some("Screen reader"));
    assert!(!semantics.multiline);
}
