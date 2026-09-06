use fission_core::internal::{lower_widget, BuildCtx, InternalLoweringCx};
use fission_core::ui::Container;
use fission_core::{build, Env, RuntimeState, ScrollStateMap, View, Widget};
use fission_layout::{LayoutEngine, LayoutRect};
use fission_render::DisplayOp;
use fission_shell_winit::Pipeline;
use fission_widgets::ProgressBar;

fn progress_ir(env: &Env, value: f32) -> fission_ir::CoreIR {
    let runtime = RuntimeState::default();
    let view = View::new(&(), &runtime, env, None);
    let mut build_ctx = BuildCtx::<()>::new();
    let widget: Widget = build::enter(&mut build_ctx, &view, || {
        Container::new(ProgressBar { value }).width(200.0).into()
    });

    let mut lower = InternalLoweringCx::new(env, &runtime, None, None);
    let root = lower_widget(&widget, &mut lower);
    lower.ir.root = Some(root);
    lower.ir
}

fn progress_rects(value: f32) -> (Vec<LayoutRect>, f32) {
    let env = Env::default();
    let expected_height = env
        .theme
        .components
        .progress
        .track_style
        .height
        .unwrap_or(env.theme.components.progress.height);
    let viewport = LayoutRect::new(0.0, 0.0, 200.0, 40.0);
    let scroll = ScrollStateMap::default();
    let mut layout = LayoutEngine::new();
    let mut pipeline = Pipeline::new();

    pipeline.replace_ir(progress_ir(&env, value), &env);
    pipeline
        .ensure_layout(viewport, &mut layout, &scroll)
        .expect("progress layout");
    pipeline
        .prepare_current(
            viewport.size,
            viewport.size,
            false,
            &scroll,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .expect("progress display list");

    let display_list = pipeline.retained_scene().expect("retained scene").flatten();
    let rects = display_list
        .ops
        .iter()
        .filter_map(|op| match op {
            DisplayOp::DrawRect {
                rect,
                fill: Some(_),
                ..
            } => Some(*rect),
            _ => None,
        })
        .collect::<Vec<_>>();

    (rects, expected_height)
}

fn assert_nondegenerate(rects: &[LayoutRect], expected_height: f32) {
    assert!(
        rects
            .iter()
            .all(|rect| rect.size.width > 0.0 && rect.size.height > 0.0),
        "progress display rectangles must be nondegenerate: {rects:?}"
    );
    assert!(
        rects
            .iter()
            .all(|rect| (rect.size.height - expected_height).abs() < 0.01),
        "track and fill must retain the themed height: {rects:?}"
    );
}

#[test]
fn progress_track_and_fill_emit_nondegenerate_display_rects() {
    let (mut rects, expected_height) = progress_rects(0.5);
    rects.sort_by(|lhs, rhs| lhs.size.width.total_cmp(&rhs.size.width));

    assert_eq!(rects.len(), 2, "expected one track and one progress fill");
    assert_nondegenerate(&rects, expected_height);
    assert!((rects[0].size.width - 100.0).abs() < 0.01);
    assert!((rects[1].size.width - 200.0).abs() < 0.01);
}

#[test]
fn empty_progress_omits_the_zero_width_fill() {
    let (rects, expected_height) = progress_rects(0.0);

    assert_eq!(rects.len(), 1, "empty progress should paint only its track");
    assert_nondegenerate(&rects, expected_height);
    assert!((rects[0].size.width - 200.0).abs() < 0.01);
}
