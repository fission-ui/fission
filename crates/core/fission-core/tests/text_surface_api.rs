use fission_core::env::{Env, RuntimeState};
use fission_core::lowering::LoweringContext;
use fission_core::ui::{
    Button, Container, Node, RichText, RichTextRun, Spacer, Text, TextInput,
};
use fission_ir::op::{Color, Fill, LayoutOp, Op, PaintOp};
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
            RichTextRun::new("world").family("Space Grotesk").italic(true),
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
