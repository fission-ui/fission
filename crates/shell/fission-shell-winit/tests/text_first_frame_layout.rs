use fission_core::env::{Env, RuntimeState, VideoStateMap, WebStateMap};
use fission_core::internal::{lower_widget, InternalLoweringCx};
use fission_core::ui::{Column, Container, Text, Widget};
use fission_core::ScrollStateMap;
use fission_ir::op::Length;
use fission_layout::{LayoutEngine, LayoutSize};
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

#[test]
fn first_frame_pipeline_measures_and_paints_descendant_max_width_identically() {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let heading = "A deliberately long heading that wraps at the capped width";
    let sibling = "This sibling begins below every heading line.";
    let widget: Widget = Column {
        children: vec![
            Container::new(Text::new(heading).size(36.0).max_lines(3))
                .width_length(Length::percent(100.0))
                .max_width_length(Length::points(260.0))
                .into(),
            Text::new(sibling).into(),
        ],
        gap: Some(12.0),
        ..Default::default()
    }
    .into();
    let root = lower_widget(&widget, &mut cx);
    cx.ir.root = Some(root);

    let measurer = Arc::new(VelloTextMeasurer::new(Arc::new(Mutex::new(
        FontContext::new(),
    ))));
    let mut layout_engine = LayoutEngine::new().with_measurer(measurer);
    let mut renderer = CapturingRenderer::default();
    let mut pipeline = Pipeline::new();
    pipeline
        .render(
            cx.ir,
            LayoutSize::new(1000.0, 800.0),
            &mut layout_engine,
            &ScrollStateMap::default(),
            &mut renderer,
            &VideoStateMap::default(),
            &WebStateMap::default(),
            &env,
        )
        .expect("first production pipeline frame");

    let mut heading_bounds = None;
    let mut sibling_bounds = None;
    let mut heading_resolved = None;
    for op in &renderer.ops {
        match op {
            DisplayOp::DrawText {
                text,
                bounds,
                resolved_layout,
                ..
            } if text == heading => {
                heading_bounds = Some(*bounds);
                heading_resolved = resolved_layout.clone();
            }
            DisplayOp::DrawText { text, bounds, .. } if text == sibling => {
                sibling_bounds = Some(*bounds)
            }
            DisplayOp::DrawRichText {
                runs,
                bounds,
                resolved_layout,
                ..
            } => {
                let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
                if text == heading {
                    heading_bounds = Some(*bounds);
                    heading_resolved = resolved_layout.clone();
                } else if text == sibling {
                    sibling_bounds = Some(*bounds);
                }
            }
            _ => {}
        }
    }
    let heading_bounds = heading_bounds.expect("heading paint bounds");
    let sibling_bounds = sibling_bounds.expect("sibling paint bounds");
    assert!(heading_bounds.width() <= 260.0);
    let heading_resolved = heading_resolved.expect("paint must consume retained paragraph layout");
    assert_eq!(heading_resolved.constraint_width, Some(260.0));
    assert!(heading_resolved.lines.len() > 1);
    assert!(
        heading_bounds.height() > 43.2,
        "heading must reserve multiple lines"
    );
    assert!(sibling_bounds.y() >= heading_bounds.bottom() + 12.0);
}
