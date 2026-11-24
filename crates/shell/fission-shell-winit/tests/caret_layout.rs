use fission_core::authoring::{lower_widget, LoweringContext};
use fission_core::env::{Env, RuntimeState, VideoStateMap, WebStateMap};
use fission_core::ui::{TextInput, Widget};
use fission_core::ScrollStateMap;
use fission_core::{TextAffinity, TextEditingValue, TextSelection};
use fission_ir::op::TextDisplayBreak;
use fission_ir::{CoreIR, Op, PaintOp, WidgetId};
use fission_layout::{LayoutEngine, LayoutPoint, LayoutRect, LayoutSize, ResolvedParagraphLayout};
use fission_render::{DisplayOp, RenderScene, Renderer};
use fission_render_vello::{parley::FontContext, VelloTextMeasurer};
use fission_shell_winit::Pipeline;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct CapturingRenderer {
    ops: Vec<DisplayOp>,
}

impl Renderer for CapturingRenderer {
    fn render_scene(&mut self, scene: &RenderScene) -> anyhow::Result<()> {
        self.ops = scene.flatten().ops;
        Ok(())
    }
}

fn input_ir(env: &Env, runtime: &RuntimeState, value: &str, size: f32) -> CoreIR {
    input_ir_with_breaks(env, runtime, value, size, Vec::new(), None, false)
}

fn input_ir_with_breaks(
    env: &Env,
    runtime: &RuntimeState,
    value: &str,
    size: f32,
    display_breaks: Vec<TextDisplayBreak>,
    editing_value: Option<TextEditingValue>,
    multiline: bool,
) -> CoreIR {
    let widget: Widget = TextInput {
        id: Some(WidgetId::explicit("caret-layout")),
        value: value.into(),
        font_size: Some(size),
        multiline,
        display_breaks,
        editing_value,
        ..Default::default()
    }
    .into();
    let mut cx = LoweringContext::new(env, runtime, None, None);
    let root = lower_widget(&widget, &mut cx);
    cx.set_root(root);
    cx.into_ir()
}

fn painted_input(
    renderer: &CapturingRenderer,
) -> (Option<usize>, LayoutRect, ResolvedParagraphLayout) {
    painted_input_for(renderer, "hello")
}

fn painted_input_for(
    renderer: &CapturingRenderer,
    expected_text: &str,
) -> (Option<usize>, LayoutRect, ResolvedParagraphLayout) {
    renderer
        .ops
        .iter()
        .find_map(|op| match op {
            DisplayOp::DrawRichText {
                runs,
                caret_index,
                bounds,
                resolved_layout,
                ..
            } if runs.iter().map(|run| run.text.as_str()).collect::<String>() == expected_text => {
                Some((
                    *caret_index,
                    *bounds,
                    resolved_layout.clone().expect("retained text layout"),
                ))
            }
            DisplayOp::DrawText {
                text,
                caret_index,
                bounds,
                resolved_layout,
                ..
            } if text == expected_text => Some((
                *caret_index,
                *bounds,
                resolved_layout.clone().expect("retained text layout"),
            )),
            _ => None,
        })
        .expect("input text must be painted")
}

#[test]
fn caret_blinks_repaint_using_the_retained_paragraph() {
    let env = Env::default();
    let mut runtime = RuntimeState::default();
    let id = WidgetId::explicit("caret-layout");
    runtime.interaction.set_focused(Some(id));
    runtime.caret_visible.insert(id, true);
    let measurer = Arc::new(VelloTextMeasurer::new(Arc::new(Mutex::new(
        FontContext::new(),
    ))));
    let mut engine = LayoutEngine::new().with_measurer(measurer);
    let mut renderer = CapturingRenderer::default();
    let mut pipeline = Pipeline::new();
    let viewport = LayoutSize::new(640.0, 480.0);
    let initial = pipeline
        .render(
            input_ir(&env, &runtime, "hello", 16.0),
            viewport,
            &mut engine,
            &ScrollStateMap::default(),
            &mut renderer,
            &VideoStateMap::default(),
            &WebStateMap::default(),
            &env,
        )
        .expect("production pipeline frame");
    assert!(initial.layout_updates > 0);
    let (caret, bounds, paragraph) = painted_input(&renderer);
    assert_eq!(caret, Some(5));
    assert!(!paragraph.caret_stops.is_empty());

    for visible in [false, true, false, true] {
        runtime.caret_visible.insert(id, visible);
        let stats = pipeline
            .render(
                input_ir(&env, &runtime, "hello", 16.0),
                viewport,
                &mut engine,
                &ScrollStateMap::default(),
                &mut renderer,
                &VideoStateMap::default(),
                &WebStateMap::default(),
                &env,
            )
            .expect("caret frame");
        assert_eq!(stats.layout_updates, 0, "blinking must not run layout");
        let (caret, next_bounds, next_paragraph) = painted_input(&renderer);
        assert_eq!(caret, visible.then_some(5));
        assert_eq!(next_bounds, bounds);
        assert_eq!(
            next_paragraph, paragraph,
            "caret geometry must retain shaping"
        );
    }

    let mut moved = input_ir(&env, &runtime, "hello", 16.0);
    for node in moved.nodes.values_mut() {
        match &mut node.op {
            Op::Paint(PaintOp::DrawText { caret_index, .. })
            | Op::Paint(PaintOp::DrawRichText { caret_index, .. }) => *caret_index = Some(2),
            _ => {}
        }
    }
    let stats = pipeline
        .render(
            moved,
            viewport,
            &mut engine,
            &ScrollStateMap::default(),
            &mut renderer,
            &VideoStateMap::default(),
            &WebStateMap::default(),
            &env,
        )
        .expect("caret movement frame");
    assert_eq!(stats.layout_updates, 0);
    let (caret, moved_bounds, moved_paragraph) = painted_input(&renderer);
    assert_eq!(caret, Some(2));
    assert_eq!(moved_bounds, bounds);
    assert_eq!(moved_paragraph, paragraph);

    for (value, size) in [("hello world", 16.0), ("hello world", 24.0)] {
        let stats = pipeline
            .render(
                input_ir(&env, &runtime, value, size),
                viewport,
                &mut engine,
                &ScrollStateMap::default(),
                &mut renderer,
                &VideoStateMap::default(),
                &WebStateMap::default(),
                &env,
            )
            .expect("text change frame");
        assert!(
            stats.layout_updates > 0,
            "text and font edits must run layout"
        );
    }
}

#[test]
fn text_input_display_break_keeps_byte_selection_and_moves_caret_and_hit_test() {
    let env = Env::default();
    let value = "abécd";
    let selection_start = 2;
    let page_break = "abé".len();
    let selection_end = value.len();
    let editing_value = TextEditingValue::new(
        value,
        TextSelection::new(
            value,
            selection_start,
            selection_end,
            TextAffinity::Downstream,
        )
        .expect("valid UTF-8 byte selection"),
        None,
    )
    .expect("valid text editing value");
    let mut runtime = RuntimeState::default();
    runtime
        .interaction
        .set_focused(Some(WidgetId::explicit("caret-layout")));
    runtime
        .caret_visible
        .insert(WidgetId::explicit("caret-layout"), true);

    let display_break = TextDisplayBreak {
        text_offset: page_break,
        gap_after: 28.0,
    };
    let mut broken_ir = input_ir_with_breaks(
        &env,
        &runtime,
        value,
        20.0,
        vec![display_break],
        Some(editing_value.clone()),
        true,
    );
    let semantics = broken_ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) => Some(semantics),
            _ => None,
        })
        .expect("TextInput semantics");
    assert_eq!(semantics.value.as_deref(), Some(value));
    assert_eq!(
        semantics.text_selection,
        Some((selection_start, selection_end))
    );

    let mut engine = LayoutEngine::new().with_measurer(Arc::new(VelloTextMeasurer::new(Arc::new(
        Mutex::new(FontContext::new()),
    ))));
    let mut renderer = CapturingRenderer::default();
    let mut pipeline = Pipeline::new();
    let viewport = LayoutSize::new(640.0, 480.0);
    pipeline
        .render(
            broken_ir,
            viewport,
            &mut engine,
            &ScrollStateMap::default(),
            &mut renderer,
            &VideoStateMap::default(),
            &WebStateMap::default(),
            &env,
        )
        .expect("TextInput with display break frame");

    let (caret, _, paragraph) = painted_input_for(&renderer, value);
    assert_eq!(caret, Some(selection_end));
    let painted_value = renderer
        .ops
        .iter()
        .find_map(|op| match op {
            DisplayOp::DrawRichText { runs, .. } => {
                Some(runs.iter().map(|run| run.text.as_str()).collect::<String>())
            }
            _ => None,
        })
        .expect("TextInput paint runs");
    assert_eq!(painted_value, value);
    assert_eq!(
        paragraph
            .clusters
            .iter()
            .map(|cluster| &value[cluster.start_index..cluster.end_index])
            .collect::<String>(),
        value
    );
    let upstream = paragraph
        .caret(page_break, true)
        .expect("upstream caret at the page transition");
    let downstream = paragraph
        .caret(page_break, false)
        .expect("downstream caret at the page transition");
    assert!(downstream.position.y > upstream.position.y + display_break.gap_after);
    assert_eq!(
        paragraph.hit_test(LayoutPoint::new(
            downstream.position.x + 0.5,
            downstream.position.y + downstream.height * 0.5,
        )),
        page_break
    );

    let first_page_selection = paragraph.selection_rects(selection_start, page_break);
    let second_page_selection = paragraph.selection_rects(page_break, selection_end);
    let gap = paragraph
        .display_breaks
        .first()
        .expect("resolved page gap geometry")
        .gap_rect;
    assert!(!first_page_selection.is_empty());
    assert!(!second_page_selection.is_empty());
    assert!(first_page_selection
        .iter()
        .any(|rect| rect.bottom() <= gap.y()));
    // The gap uses native line-block coordinates; authored leading can move
    // glyph/selection ink slightly above that block. Verify the exact page
    // translation of the same typography rather than treating ink top as the
    // block boundary.
    let first_ink_top = first_page_selection
        .iter()
        .map(|rect| rect.y())
        .fold(f32::INFINITY, f32::min);
    let second_ink_top = second_page_selection
        .iter()
        .map(|rect| rect.y())
        .fold(f32::INFINITY, f32::min);
    assert!(
        (second_ink_top - gap.bottom() - first_ink_top).abs() < 0.01,
        "page gap must translate selection ink without changing its leading: first={first_page_selection:?}, second={second_page_selection:?}, gap={gap:?}"
    );

    let baseline_ir = input_ir_with_breaks(
        &env,
        &runtime,
        value,
        20.0,
        Vec::new(),
        Some(editing_value),
        true,
    );
    pipeline
        .render(
            baseline_ir,
            viewport,
            &mut engine,
            &ScrollStateMap::default(),
            &mut renderer,
            &VideoStateMap::default(),
            &WebStateMap::default(),
            &env,
        )
        .expect("TextInput without display break frame");
    let (baseline_caret, _, baseline) = painted_input_for(&renderer, value);
    assert_eq!(baseline_caret, Some(selection_end));
    assert!(downstream.position.y > baseline.caret(page_break, false).unwrap().position.y);
}
