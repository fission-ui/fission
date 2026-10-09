use fission_core::{
    diff::diff_ir,
    env::{Env, RuntimeState},
    internal::{lower_widget, LoweringContext},
    ui::{TextInput, Widget},
};
use fission_ir::{CoreIR, Op, PaintOp, WidgetId};

fn input_ir(state: &RuntimeState, value: &str, size: f32) -> CoreIR {
    let env = Env::default();
    let widget: Widget = TextInput {
        id: Some(WidgetId::explicit("caret-regression")),
        value: value.into(),
        font_size: Some(size),
        ..Default::default()
    }
    .into();
    let mut cx = LoweringContext::new(&env, state, None, None);
    let root = lower_widget(&widget, &mut cx);
    cx.set_root(root);
    cx.into_ir()
}

fn caret(ir: &CoreIR) -> Option<usize> {
    ir.nodes.values().find_map(|n| match &n.op {
        Op::Paint(PaintOp::DrawText { caret_index, .. })
        | Op::Paint(PaintOp::DrawRichText { caret_index, .. }) => *caret_index,
        _ => None,
    })
}

#[test]
fn real_focused_text_input_blinks_without_invalidating_layout() {
    let id = WidgetId::explicit("caret-regression");
    let mut state = RuntimeState::default();
    state.interaction.set_focused(Some(id));
    state.caret_visible.insert(id, true);
    let visible = input_ir(&state, "hello", 16.0);
    assert_eq!(caret(&visible), Some(5));
    state.caret_visible.insert(id, false);
    let hidden = input_ir(&state, "hello", 16.0);
    assert_eq!(caret(&hidden), None);
    for (before, after) in [(&visible, &hidden), (&hidden, &visible)] {
        let diff = diff_ir(before, after);
        assert!(
            diff.dirty_layout.is_empty(),
            "caret blink changed layout: {:?}",
            diff.dirty_layout
        );
        assert!(!diff.dirty_paint.is_empty(), "caret blink must repaint");
        assert!(diff.dirty_composite.is_empty());
    }
}

#[test]
fn input_text_and_font_changes_still_invalidate_layout() {
    let id = WidgetId::explicit("caret-regression");
    let mut state = RuntimeState::default();
    state.interaction.set_focused(Some(id));
    let before = input_ir(&state, "hello", 16.0);
    for after in [
        input_ir(&state, "hello world", 16.0),
        input_ir(&state, "hello", 24.0),
    ] {
        assert!(!diff_ir(&before, &after).dirty_layout.is_empty());
    }
}

fn text_ir(op: PaintOp) -> CoreIR {
    let id = WidgetId::explicit("text-op");
    let mut ir = CoreIR::new();
    ir.add_node(id, Op::Paint(op), vec![]);
    ir.set_root(id);
    ir
}

#[test]
fn plain_and_rich_text_caret_changes_are_paint_only() {
    use fission_ir::op::Color;
    let plain = PaintOp::DrawText {
        text: "hello".into(),
        size: 16.0,
        color: Color::BLACK,
        underline: false,
        locale: None,
        wrap: true,
        caret_index: Some(5),
        caret_color: None,
        caret_width: None,
        caret_height: None,
        caret_radius: None,
        paragraph_style: None,
    };
    let mut state = RuntimeState::default();
    let id = WidgetId::explicit("caret-regression");
    state.interaction.set_focused(Some(id));
    state.caret_visible.insert(id, true);
    let rich = input_ir(&state, "hello", 16.0)
        .nodes
        .into_values()
        .find_map(|node| match node.op {
            Op::Paint(op @ PaintOp::DrawRichText { .. }) => Some(op),
            _ => None,
        })
        .expect("real text input emits rich text");
    for op in [plain, rich] {
        let before = text_ir(op.clone());
        for field in 0..6 {
            let mut changed = op.clone();
            match &mut changed {
                PaintOp::DrawText {
                    caret_index,
                    caret_color,
                    caret_width,
                    caret_height,
                    caret_radius,
                    ..
                }
                | PaintOp::DrawRichText {
                    caret_index,
                    caret_color,
                    caret_width,
                    caret_height,
                    caret_radius,
                    ..
                } => match field {
                    0 => *caret_index = None,
                    1 => *caret_index = Some(2),
                    2 => *caret_color = Some(Color::WHITE),
                    3 => *caret_width = Some(3.0),
                    4 => *caret_height = Some(20.0),
                    5 => *caret_radius = Some(2.0),
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
            let after = text_ir(changed);
            for (prev, next) in [(&before, &after), (&after, &before)] {
                let diff = diff_ir(prev, next);
                assert!(diff.dirty_layout.is_empty(), "caret field {field}");
                assert_eq!(diff.dirty_paint.len(), 1);
            }
        }
    }
}

#[test]
fn wrapping_and_paragraph_changes_still_invalidate_layout() {
    use fission_ir::op::TextParagraphStyle;
    let before = input_ir(&RuntimeState::default(), "hello", 16.0);
    for paragraph in [false, true] {
        let mut after = before.clone();
        let text = after
            .nodes
            .values_mut()
            .find_map(|node| match &mut node.op {
                Op::Paint(op @ (PaintOp::DrawText { .. } | PaintOp::DrawRichText { .. })) => {
                    Some(op)
                }
                _ => None,
            })
            .expect("input text");
        match text {
            PaintOp::DrawText {
                wrap,
                paragraph_style,
                ..
            }
            | PaintOp::DrawRichText {
                wrap,
                paragraph_style,
                ..
            } => {
                if paragraph {
                    *paragraph_style = Some(TextParagraphStyle::default());
                } else {
                    *wrap = !*wrap;
                }
            }
            _ => unreachable!(),
        }
        assert!(!diff_ir(&before, &after).dirty_layout.is_empty());
    }
}
