use fission_core::authoring::{lower_widget, LoweringContext};
use fission_core::env::{Env, RuntimeState, VideoStateMap, WebStateMap};
use fission_core::ui::{TextInput, Widget};
use fission_core::ScrollStateMap;
use fission_ir::{CoreIR, Op, PaintOp, WidgetId};
use fission_layout::{LayoutEngine, LayoutRect, LayoutSize, ResolvedParagraphLayout};
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
    let widget: Widget = TextInput {
        id: Some(WidgetId::explicit("caret-layout")),
        value: value.into(),
        font_size: Some(size),
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
            } if runs.iter().map(|run| run.text.as_str()).collect::<String>() == "hello" => Some((
                *caret_index,
                *bounds,
                resolved_layout.clone().expect("retained text layout"),
            )),
            DisplayOp::DrawText {
                text,
                caret_index,
                bounds,
                resolved_layout,
                ..
            } if text == "hello" => Some((
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
