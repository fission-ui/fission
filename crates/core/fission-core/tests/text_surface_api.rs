use fission_core::env::{Env, RuntimeState, TextSelectionHandleKind};
use fission_core::lowering::LoweringContext;
use fission_core::ui::widgets::text::{RichTextChild, RichTextSpan, TextScaler, WidgetSpan};
use fission_core::ui::widgets::text_input::{
    DragStartBehavior, SpellCheckConfiguration, TextAlignVertical, TextContextMenuAction,
    TextInputRuntimeConfig, TextMagnifierConfiguration, TextUndoController,
};
use fission_core::ui::{Button, Container, Node, RichText, RichTextRun, Spacer, Text, TextInput};
use fission_core::{ActionEnvelope, ActionId};
use fission_ir::op::{
    decode_inline_widget_marker, Color, Fill, LayoutOp, MouseCursor, Op, PaintOp,
    RichTextAnnotation, TextAlign, TextDirection, TextHeightBehavior, TextOverflow, TextWidthBasis,
};
use fission_ir::semantics::ActionTrigger;
use fission_ir::{CoreIR, FlexDirection};

fn lower_node(node: Node) -> CoreIR {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime, None, None);
    let root = node.lower(&mut cx);
    cx.ir.root = Some(root);
    cx.ir
}

fn lower_node_with_runtime(node: Node, runtime: RuntimeState) -> CoreIR {
    let env = Env::default();
    let mut cx = LoweringContext::new(&env, &runtime, None, None);
    let root = node.lower(&mut cx);
    cx.ir.root = Some(root);
    cx.ir
}

fn test_text_input_selection_handle_id(
    input_id: fission_ir::NodeId,
    kind: TextSelectionHandleKind,
) -> fission_ir::NodeId {
    let suffix = match kind {
        TextSelectionHandleKind::Caret => 0,
        TextSelectionHandleKind::Start => 1,
        TextSelectionHandleKind::End => 2,
    };
    fission_ir::NodeId::derived(input_id.as_u128(), &[900, suffix])
}

fn test_text_input_toolbar_button_id(
    input_id: fission_ir::NodeId,
    action: TextContextMenuAction,
) -> fission_ir::NodeId {
    let suffix = match action {
        TextContextMenuAction::Copy => 0,
        TextContextMenuAction::Cut => 1,
        TextContextMenuAction::Paste => 2,
        TextContextMenuAction::SelectAll => 3,
    };
    fission_ir::NodeId::derived(input_id.as_u128(), &[901, suffix])
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

fn rich_text_annotations(ir: &CoreIR) -> Option<(fission_ir::NodeId, &Vec<RichTextAnnotation>)> {
    ir.nodes.iter().find_map(|(id, node)| match &node.op {
        Op::Paint(PaintOp::DrawRichText { .. }) => {
            ir.custom_render_objects
                .get(id)
                .and_then(|annotation_sidecar| {
                    annotation_sidecar
                        .as_ref()
                        .downcast_ref::<Vec<RichTextAnnotation>>()
                        .map(|annotations| (*id, annotations))
                })
        }
        _ => None,
    })
}

fn test_action(name: &str, payload: &'static [u8]) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: payload.to_vec(),
    }
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
                .italic(true)
                .text_scaler(TextScaler::linear(1.25)),
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
    assert_eq!(
        runs[1].style.font_size,
        Env::default().theme.tokens.typography.body_medium_size * 1.25
    );
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
            label: Some("Email".into()),
            helper_text: Some("We never share your address.".into()),
            counter_text: Some("custom counter".into()),
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

    assert!(paint_ops(&ir).any(|op| match op {
        PaintOp::DrawText { text, .. } => text == "Email",
        PaintOp::DrawRichText { runs, .. } => runs.iter().any(|run| run.text == "Email"),
        _ => false,
    }));
    assert!(paint_ops(&ir).any(|op| match op {
        PaintOp::DrawText { text, .. } => text == "We never share your address.",
        PaintOp::DrawRichText { runs, .. } => runs
            .iter()
            .any(|run| run.text == "We never share your address."),
        _ => false,
    }));
    assert!(paint_ops(&ir).any(|op| match op {
        PaintOp::DrawText { text, .. } => text == "custom counter",
        PaintOp::DrawRichText { runs, .. } => runs.iter().any(|run| run.text == "custom counter"),
        _ => false,
    }));
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
            enable_suggestions: true,
            spell_check: true,
            smart_dashes: true,
            smart_quotes: true,
            autofill_hints: Vec::new(),
            drag_start_behavior: DragStartBehavior::Down,
            undo_controller: Some(TextUndoController { capacity: 7 }),
            spell_check_configuration: Some(SpellCheckConfiguration {
                enabled: false,
                underline_color: Some(Color {
                    r: 255,
                    g: 59,
                    b: 48,
                    a: 255,
                }),
                show_suggestions: false,
            }),
            restoration_id: Some("email-field".into()),
            on_submit: Some(fission_core::ActionEnvelope {
                id: fission_core::ActionId::from_name("tests::submit"),
                payload: br#""payload""#.to_vec(),
            }),
            on_tap_outside: Some(fission_core::ActionEnvelope {
                id: fission_core::ActionId::from_name("tests::tap_outside"),
                payload: br#""outside""#.to_vec(),
            }),
            cursor_color: Some(Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            }),
            cursor_width: Some(3.0),
            cursor_height: Some(18.0),
            cursor_radius: Some(2.0),
            text_align: TextAlign::Center,
            text_align_vertical: TextAlignVertical::Bottom,
            locale: Some("fr-GB".into()),
            text_scale: Some(1.25),
            text_direction: TextDirection::Rtl,
            strut_line_height: Some(24.0),
            mouse_cursor: Some(fission_ir::semantics::MouseCursor::Text),
            scroll_padding: Some([12.0, 13.0, 14.0, 15.0]),
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
    assert_eq!(semantics.scroll_padding, Some([12.0, 13.0, 14.0, 15.0]));
    assert_eq!(
        semantics.input_formatters,
        vec![fission_ir::semantics::InputFormatter::AsciiOnly]
    );
    assert!(!semantics.autocorrect);
    assert!(!semantics.enable_suggestions);
    assert!(!semantics.spell_check);
    assert!(semantics
        .actions
        .entries
        .iter()
        .any(|entry| entry.as_hover_cursor() == Some(fission_ir::semantics::MouseCursor::Text)));
    assert!(semantics
        .actions
        .entries
        .iter()
        .any(|entry| entry.trigger == fission_ir::semantics::ActionTrigger::Submit));
    assert!(semantics
        .actions
        .entries
        .iter()
        .any(|entry| { entry.trigger == fission_ir::semantics::ActionTrigger::TapOutside }));

    let semantics_id = ir
        .nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Semantics(semantics) if semantics.role == fission_ir::Role::TextInput => Some(*id),
            _ => None,
        })
        .expect("text input id");
    let runtime_config = ir
        .custom_render_objects
        .get(&semantics_id)
        .and_then(|sidecar| sidecar.as_ref().downcast_ref::<TextInputRuntimeConfig>())
        .expect("text input runtime config");
    assert_eq!(runtime_config.drag_start_behavior, DragStartBehavior::Down);
    assert_eq!(
        runtime_config.undo_controller,
        Some(TextUndoController { capacity: 7 })
    );
    assert_eq!(
        runtime_config.restoration_id.as_deref(),
        Some("email-field")
    );
    assert_eq!(
        runtime_config
            .spell_check_configuration
            .as_ref()
            .map(|cfg| cfg.show_suggestions),
        Some(false)
    );

    let caret = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText {
                caret_color,
                caret_width,
                caret_height,
                caret_radius,
                paragraph_style,
                ..
            } => Some((
                caret_color,
                caret_width,
                caret_height,
                caret_radius,
                paragraph_style,
            )),
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
    assert_eq!(caret.2, &Some(18.0));
    assert_eq!(caret.3, &Some(2.0));
    assert_eq!(
        caret.4,
        &Some(fission_ir::op::TextParagraphStyle {
            text_align: TextAlign::Center,
            max_lines: None,
            overflow: TextOverflow::Visible,
            text_direction: TextDirection::Rtl,
            text_width_basis: TextWidthBasis::Parent,
            strut_line_height: Some(24.0),
            text_height_behavior: TextHeightBehavior::default(),
        })
    );

    let value_run = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { runs, .. } => runs.iter().find(|run| run.text == "hello"),
            _ => None,
        })
        .expect("value run");
    assert_eq!(value_run.style.locale.as_deref(), Some("fr-GB"));
    assert_eq!(value_run.style.font_size, 21.25);
}

#[test]
fn text_lowers_paragraph_controls_for_alignment_and_ellipsis() {
    let height_behavior = TextHeightBehavior {
        apply_height_to_first_ascent: false,
        apply_height_to_last_descent: true,
    };
    let ir = lower_node(
        Text::new("Paragraph parity")
            .size(16.0)
            .text_align(TextAlign::Center)
            .text_direction(TextDirection::Rtl)
            .max_lines(2)
            .overflow(TextOverflow::Ellipsis)
            .strut_line_height(24.0)
            .text_height_behavior(height_behavior)
            .into_node(),
    );

    let paragraph = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawText {
                paragraph_style: Some(paragraph_style),
                ..
            } => Some(*paragraph_style),
            _ => None,
        })
        .expect("paragraph metadata");

    assert_eq!(paragraph.text_align, TextAlign::Center);
    assert_eq!(paragraph.max_lines, Some(2));
    assert_eq!(paragraph.overflow, TextOverflow::Ellipsis);
    assert_eq!(paragraph.text_direction, TextDirection::Rtl);
    assert_eq!(paragraph.text_width_basis, TextWidthBasis::Parent);
    assert_eq!(paragraph.strut_line_height, Some(24.0));
    assert_eq!(paragraph.text_height_behavior, height_behavior);

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
                    }) if (height - 48.0).abs() < 0.001
                )
        })
        .expect("clipped layout box");

    assert!(matches!(clipped_box.op, Op::Layout(LayoutOp::Box { .. })));
}

#[test]
fn rich_text_lowers_paragraph_controls_for_line_capping() {
    let height_behavior = TextHeightBehavior {
        apply_height_to_first_ascent: false,
        apply_height_to_last_descent: false,
    };
    let ir = lower_node(
        RichText::new(vec![
            RichTextRun::new("Hello ").size(14.0).line_height(18.0),
            RichTextRun::new("world").size(20.0).line_height(24.0),
        ])
        .text_align(TextAlign::End)
        .text_direction(TextDirection::Ltr)
        .max_lines(3)
        .overflow(TextOverflow::Clip)
        .strut_line_height(28.0)
        .text_height_behavior(height_behavior)
        .into_node(),
    );

    let paragraph = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText {
                paragraph_style: Some(paragraph_style),
                ..
            } => Some(*paragraph_style),
            _ => None,
        })
        .expect("paragraph metadata");

    assert_eq!(paragraph.text_align, TextAlign::End);
    assert_eq!(paragraph.max_lines, Some(3));
    assert_eq!(paragraph.overflow, TextOverflow::Clip);
    assert_eq!(paragraph.text_direction, TextDirection::Ltr);
    assert_eq!(paragraph.text_width_basis, TextWidthBasis::Parent);
    assert_eq!(paragraph.strut_line_height, Some(28.0));
    assert_eq!(paragraph.text_height_behavior, height_behavior);

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
                    }) if (height - 84.0).abs() < 0.001
                )
        })
        .expect("clipped layout box");

    assert!(matches!(clipped_box.op, Op::Layout(LayoutOp::Box { .. })));
}

#[test]
fn text_lowers_longest_line_width_basis() {
    let ir = lower_node(
        Text::new("Width basis")
            .text_width_basis(TextWidthBasis::LongestLine)
            .into_node(),
    );

    let paragraph = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawText {
                paragraph_style: Some(paragraph_style),
                ..
            } => Some(*paragraph_style),
            _ => None,
        })
        .expect("paragraph metadata");

    assert_eq!(paragraph.text_width_basis, TextWidthBasis::LongestLine);
}

#[test]
fn rich_text_lowers_longest_line_width_basis() {
    let ir = lower_node(
        RichText::new(vec![RichTextRun::new("Width basis")])
            .text_width_basis(TextWidthBasis::LongestLine)
            .into_node(),
    );

    let paragraph = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText {
                paragraph_style: Some(paragraph_style),
                ..
            } => Some(*paragraph_style),
            _ => None,
        })
        .expect("paragraph metadata");

    assert_eq!(paragraph.text_width_basis, TextWidthBasis::LongestLine);
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
fn rich_text_inline_widgets_lower_marker_runs_and_child_nodes() {
    let ir = lower_node(
        RichText::from_spans(vec![
            RichTextChild::from(RichTextSpan::new("Before ")),
            RichTextChild::from(
                WidgetSpan::new(
                    Spacer {
                        width: Some(18.0),
                        height: Some(10.0),
                        ..Default::default()
                    }
                    .into_node(),
                    18.0,
                    10.0,
                )
                .semantics_label("[badge]"),
            ),
            RichTextChild::from(RichTextSpan::new(" after")),
        ])
        .into_node(),
    );

    let (paint_node_id, runs) = ir
        .nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => Some((*id, runs)),
            _ => None,
        })
        .expect("rich text paint op");

    assert_eq!(ir.nodes.get(&paint_node_id).unwrap().children.len(), 1);
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].text, "Before ");
    assert_eq!(runs[2].text, " after");

    let marker = runs
        .iter()
        .find_map(|run| {
            if run.text.is_empty() {
                decode_inline_widget_marker(run.style.font_family.as_deref())
            } else {
                None
            }
        })
        .expect("inline widget marker");

    assert_eq!(marker.id, 0);
    assert_eq!(marker.width, 18.0);
    assert_eq!(marker.height, 10.0);
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
    assert_eq!(semantics.role, fission_ir::Role::Text);
    assert!(semantics.multiline);
}

#[test]
fn rich_text_span_interactions_lower_to_annotation_sidecar() {
    let tap = test_action("tests::span_tap", br#""tap""#);
    let hover_enter = test_action("tests::span_hover_enter", br#""enter""#);
    let secondary = test_action("tests::span_secondary", br#""context""#);
    let ir = lower_node(
        RichText::from_span(
            RichTextSpan::new("Read ")
                .on_tap(tap.clone())
                .mouse_cursor(MouseCursor::Pointer)
                .child(
                    RichTextSpan::new("docs")
                        .semantics_label("documentation")
                        .semantics_identifier("docs-link")
                        .on_hover_enter(hover_enter.clone())
                        .on_secondary_click(secondary.clone()),
                ),
        )
        .into_node(),
    );

    let runs = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { runs, .. } => Some(runs),
            _ => None,
        })
        .expect("rich text paint op");
    assert_eq!(
        runs.len(),
        1,
        "interaction metadata should not split shaping runs"
    );
    assert_eq!(runs[0].text, "Read docs");

    let (_, annotations) = rich_text_annotations(&ir).expect("annotation sidecar");
    let parent = annotations
        .iter()
        .find(|annotation| annotation.range == (0..9))
        .expect("parent annotation");
    let child = annotations
        .iter()
        .find(|annotation| annotation.range == (5..9))
        .expect("child annotation");

    assert_eq!(parent.mouse_cursor, Some(MouseCursor::Pointer));
    assert!(parent.actions.iter().any(|action| {
        action.trigger == ActionTrigger::Default && action.action_id == tap.id.as_u128()
    }));

    assert_eq!(child.semantics_label.as_deref(), Some("documentation"));
    assert_eq!(child.semantics_identifier.as_deref(), Some("docs-link"));
    assert_eq!(child.mouse_cursor, None);
    assert!(child.actions.iter().any(|action| {
        action.trigger == ActionTrigger::HoverEnter && action.action_id == hover_enter.id.as_u128()
    }));
    assert!(child.actions.iter().any(|action| {
        action.trigger == ActionTrigger::SecondaryClick
            && action.action_id == secondary.id.as_u128()
    }));
}

#[test]
fn rich_text_run_spell_out_lowers_to_annotation_sidecar() {
    let ir = lower_node(
        RichText::new(vec![
            RichTextRun::new("NASA").spell_out(true),
            RichTextRun::new(" launch"),
        ])
        .into_node(),
    );

    let runs = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { runs, .. } => Some(runs),
            _ => None,
        })
        .expect("rich text paint op");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "NASA launch");

    let (_, annotations) = rich_text_annotations(&ir).expect("annotation sidecar");
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].range, 0..4);
    assert_eq!(annotations[0].spell_out, Some(true));
}

#[test]
fn rich_text_span_spell_out_preserves_nested_overrides() {
    let ir = lower_node(
        RichText::from_span(
            RichTextSpan::new("Call ")
                .spell_out(true)
                .child(RichTextSpan::new("911").spell_out(false)),
        )
        .into_node(),
    );

    let (_, annotations) = rich_text_annotations(&ir).expect("annotation sidecar");
    let parent = annotations
        .iter()
        .find(|annotation| annotation.range == (0..8))
        .expect("parent annotation");
    let child = annotations
        .iter()
        .find(|annotation| annotation.range == (5..8))
        .expect("child annotation");

    assert_eq!(parent.spell_out, Some(true));
    assert_eq!(child.spell_out, Some(false));
}

#[test]
fn text_semantics_actions_keep_text_role_and_focusability() {
    let tap = test_action("tests::text_tap", br#""tap""#);
    let hover = test_action("tests::text_hover", br#""hover""#);
    let secondary = test_action("tests::text_secondary", br#""secondary""#);

    let ir = lower_node(
        Text::new("Docs")
            .semantics_label("Read docs")
            .on_tap(tap.clone())
            .on_hover_enter(hover.clone())
            .on_secondary_click(secondary.clone())
            .into_node(),
    );

    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.label.as_deref() == Some("Read docs") => {
                Some(semantics)
            }
            _ => None,
        })
        .expect("text semantics");

    assert_eq!(semantics.role, fission_ir::Role::Text);
    assert!(semantics.focusable);
    assert!(!semantics.multiline);
    assert!(semantics.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::Default && entry.action_id == tap.id.as_u128()
    }));
    assert!(semantics.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::HoverEnter && entry.action_id == hover.id.as_u128()
    }));
    assert!(semantics.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::SecondaryClick && entry.action_id == secondary.id.as_u128()
    }));
}

#[test]
fn text_locale_scale_selection_and_identifier_lower_to_rich_text() {
    let ir = lower_node(
        Text::new("Visible text")
            .locale("fr-FR")
            .text_scaler(TextScaler::linear(1.25))
            .selection_range((0, 7))
            .selection_color(Color {
                r: 1,
                g: 2,
                b: 3,
                a: 255,
            })
            .selection_text_color(Color::WHITE)
            .semantics_identifier("hero-copy")
            .into_node(),
    );

    let runs = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { runs, .. } => Some(runs),
            _ => None,
        })
        .expect("rich text paint op");
    let base_size = Env::default().theme.tokens.typography.body_medium_size;

    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].text, "Visible");
    assert_eq!(runs[0].style.locale.as_deref(), Some("fr-FR"));
    assert_eq!(runs[0].style.font_size, base_size * 1.25);
    assert_eq!(
        runs[0].style.background_color,
        Some(Color {
            r: 1,
            g: 2,
            b: 3,
            a: 255
        })
    );
    assert_eq!(runs[0].style.color, Color::WHITE);
    assert_eq!(runs[1].style.locale.as_deref(), Some("fr-FR"));

    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.identifier.is_some() => Some(semantics),
            _ => None,
        })
        .expect("text semantics");

    assert_eq!(semantics.identifier.as_deref(), Some("hero-copy"));
}

#[test]
fn rich_text_identifier_and_locale_propagate_from_nested_spans() {
    let ir = lower_node(
        RichText::from_span(
            RichTextSpan::new("")
                .locale("en-GB")
                .semantics_identifier("nested-copy")
                .child(RichTextSpan::new("Hello ").text_scaler(TextScaler::linear(1.5)))
                .child(RichTextSpan::new("world")),
        )
        .into_node(),
    );

    let runs = paint_ops(&ir)
        .find_map(|op| match op {
            PaintOp::DrawRichText { runs, .. } => Some(runs),
            _ => None,
        })
        .expect("rich text paint op");
    let base_size = Env::default().theme.tokens.typography.body_medium_size;

    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].style.locale.as_deref(), Some("en-GB"));
    assert_eq!(runs[1].style.locale.as_deref(), Some("en-GB"));
    assert_eq!(runs[0].style.font_size, base_size * 1.5);
    assert_eq!(runs[1].style.font_size, base_size);

    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.identifier.is_some() => Some(semantics),
            _ => None,
        })
        .expect("rich text semantics");

    assert_eq!(semantics.identifier.as_deref(), Some("nested-copy"));
}

#[test]
fn rich_text_run_semantics_metadata_surfaces_without_nested_spans() {
    let hover_exit = test_action("tests::rich_text_hover_exit", br#""leave""#);
    let ir = lower_node(
        RichText::new(vec![
            RichTextRun::new("Tap ").semantics_identifier("footer-copy"),
            RichTextRun::new("here").semantics_label("link"),
            RichTextRun::new(" now"),
        ])
        .on_hover_exit(hover_exit.clone())
        .into_node(),
    );

    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.identifier.as_deref() == Some("footer-copy") => {
                Some(semantics)
            }
            _ => None,
        })
        .expect("rich text semantics");

    assert_eq!(semantics.role, fission_ir::Role::Text);
    assert_eq!(semantics.label.as_deref(), Some("Tap link now"));
    assert_eq!(semantics.identifier.as_deref(), Some("footer-copy"));
    assert!(semantics.multiline);
    assert!(semantics.actions.entries.iter().any(|entry| {
        entry.trigger == ActionTrigger::HoverExit && entry.action_id == hover_exit.id.as_u128()
    }));
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
    assert_eq!(semantics.role, fission_ir::Role::Text);
    assert!(!semantics.multiline);
}

#[test]
fn focused_text_input_lowers_toolbar_handles_and_magnifier_overlays() {
    let input_id = fission_ir::NodeId::derived(88, &[0]);
    let mut runtime = RuntimeState::default();
    runtime.interaction.set_focused(Some(input_id));
    let state = runtime.text_edit.get_mut_or_default(input_id);
    state.caret = 8;
    state.anchor = 2;
    state.affordances.toolbar_visible = true;
    state.affordances.toolbar_anchor = Some(fission_layout::LayoutPoint::new(40.0, 12.0));
    state.affordances.selection_start_handle = Some(fission_layout::LayoutPoint::new(18.0, 24.0));
    state.affordances.selection_end_handle = Some(fission_layout::LayoutPoint::new(96.0, 24.0));
    state.affordances.active_handle = Some(TextSelectionHandleKind::End);
    state.affordances.magnifier_visible = true;
    state.affordances.magnifier_anchor = Some(fission_layout::LayoutPoint::new(96.0, 24.0));

    let ir = lower_node_with_runtime(
        TextInput {
            id: Some(input_id),
            value: "abcdefghij".into(),
            magnifier_configuration: TextMagnifierConfiguration {
                diameter: 72.0,
                ..Default::default()
            },
            ..Default::default()
        }
        .into_node(),
        runtime,
    );

    assert!(ir.nodes.contains_key(&test_text_input_selection_handle_id(
        input_id,
        TextSelectionHandleKind::Start
    )));
    assert!(ir.nodes.contains_key(&test_text_input_selection_handle_id(
        input_id,
        TextSelectionHandleKind::End
    )));
    assert!(ir.nodes.contains_key(&test_text_input_toolbar_button_id(
        input_id,
        TextContextMenuAction::Copy
    )));

    let magnifier_box = ir
        .nodes
        .values()
        .find(|node| {
            matches!(
                node.op,
                Op::Layout(LayoutOp::Positioned {
                    width: Some(width),
                    height: Some(height),
                    ..
                }) if (width - 72.0).abs() < 0.001 && (height - 72.0).abs() < 0.001
            )
        })
        .expect("magnifier positioned overlay");
    assert!(matches!(
        magnifier_box.op,
        Op::Layout(LayoutOp::Positioned { .. })
    ));

    assert!(paint_ops(&ir).any(|op| matches!(op, PaintOp::DrawRichText { .. })));
}
