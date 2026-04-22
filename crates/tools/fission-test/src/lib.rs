use anyhow::Result;
use fission_core::env::Env;
use fission_core::lowering::build_layout_tree;
use fission_core::{ActionEnvelope, AppState, BuildCtx, Clock, CurrentTime, Node, View, Widget, WidgetNodeId, NodeId};
use fission_ir::CoreIR;
use fission_layout::{LayoutEngine, LayoutRect, LayoutSnapshot, TextMeasurer, LayoutUnit, LineMetric};
use fission_render::{DisplayList, DisplayOp, Renderer};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct HeadlessApp<S: AppState> {
    pub state: S,
    pub root: Box<dyn Widget<S>>,
    pub clock: Clock,
    pub env: Env,
    pub layout_engine: LayoutEngine,
    pub action_registry: fission_core::registry::ActionRegistry<S>,
    pub last_snapshot: Option<LayoutSnapshot>,
    pub last_ir: Option<CoreIR>,
}

impl<S: AppState> HeadlessApp<S> {
    pub fn new(state: S, root: impl Widget<S> + 'static) -> Self {
        let measurer: Arc<dyn TextMeasurer> = Arc::new(MockTextMeasurer);
        let mut env = Env::default();
        // Env doesn't have measurer field in clean version
        Self {
            state,
            root: Box::new(root),
            clock: Clock::default(),
            env,
            layout_engine: LayoutEngine::new().with_measurer(measurer),
            action_registry: fission_core::registry::ActionRegistry::new(),
            last_snapshot: None,
            last_ir: None,
        }
    }

    pub fn tick(&mut self, dt_ms: u64) -> Result<()> {
        self.clock.advance_by(dt_ms)?;
        Ok(())
    }
}

struct MockTextMeasurer;
impl TextMeasurer for MockTextMeasurer {
    fn measure(&self, text: &str, _font_size: f32, avail: Option<f32>) -> (f32, f32) {
        let char_width = 10.0;
        let line_height = 20.0;
        let full_width = text.len() as f32 * char_width;
        
        if let Some(w) = avail {
            if full_width > w {
                let safe_w = w.max(char_width); 
                let lines = (full_width / safe_w).ceil();
                return (w, lines * line_height);
            }
        }
        (full_width, line_height)
    }

    fn measure_rich_text(&self, runs: &[fission_ir::op::TextRun], available_width: Option<f32>) -> (f32, f32) {
        let full_w: f32 = runs.iter().map(|r| r.text.len() as f32 * 10.0).sum();
        let char_width = 10.0;
        let line_height = 20.0;
        
        if let Some(w) = available_width {
            if full_w > w {
                 let safe_w = w.max(char_width);
                 let lines = (full_w / safe_w).ceil();
                 return (w, lines * line_height);
            }
        }
        (full_w.max(10.0), line_height)
    }

    fn hit_test(&self, _text: &str, _font_size: f32, _available_width: Option<f32>, _x: f32, _y: f32) -> usize {
        0
    }

    fn get_line_metrics(&self, _text: &str, _font_size: f32, _available_width: Option<f32>) -> Vec<LineMetric> {
        vec![]
    }

    fn get_caret_position(&self, _text: &str, _font_size: f32, _available_width: Option<f32>, _caret_index: usize) -> (f32, f32) {
        (0.0, 0.0)
    }
}

const DEFAULT_TEST_FONT_FAMILY: &str = "Fission Default";

fn should_use_mock_measurer() -> bool {
    let env_mock = std::env::var("FISSION_TEST_USE_MOCK_MEASURER")
        .map(|v| {
            let v = v.to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes"
        })
        .unwrap_or(false);
    let env_kind = std::env::var("FISSION_TEST_MEASURER")
        .map(|v| v.to_ascii_lowercase())
        .ok();
    env_mock || matches!(env_kind.as_deref(), Some("mock"))
}

pub struct TestRenderer {
    pub display_list: Option<DisplayList>,
}

impl TestRenderer {
    pub fn new() -> Self {
        Self { display_list: None }
    }
}

impl Renderer for TestRenderer {
    fn render(&mut self, display_list: &DisplayList) -> Result<()> {
        self.display_list = Some(display_list.clone());
        Ok(())
    }
}
