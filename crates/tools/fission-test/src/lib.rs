use anyhow::Result;
use fission_core::{Runtime, Action, ActionId, AppState, CurrentTime, AdvanceTo, Tick};
use fission_ir::NodeId;
use fission_layout::{LayoutSnapshot, LayoutSize, LayoutEngine};
use fission_render::{Renderer, DisplayList, LayoutRect};
use std::sync::{Arc, Mutex};

// A mock renderer that captures the display list for inspection.
#[derive(Default, Clone)]
pub struct MockRenderer {
    pub last_display_list: Arc<Mutex<Option<DisplayList>>>,
}

impl Renderer for MockRenderer {
    fn render(&mut self, display_list: &DisplayList) -> Result<()> {
        let mut lock = self.last_display_list.lock().unwrap();
        *lock = Some(display_list.clone());
        Ok(())
    }
}

pub struct TestHarness {
    pub runtime: Runtime,
    pub renderer: MockRenderer,
    pub layout_engine: LayoutEngine,
    // Helper to track the last rendered frame/snapshot
    pub last_snapshot: Option<LayoutSnapshot>,
}

impl Default for TestHarness {
    fn default() -> Self {
        Self {
            runtime: Runtime::default(),
            renderer: MockRenderer::default(),
            layout_engine: LayoutEngine::new(),
            last_snapshot: None,
        }
    }
}

impl TestHarness {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_app_state<S: AppState + 'static>(mut self, state: S) -> Self {
        self.runtime.add_app_state(Box::new(state)).expect("Failed to add app state");
        self
    }

    pub fn register_reducer<S: AppState + 'static>(
        mut self,
        action_id: ActionId,
        reducer: fn(&mut S, &dyn Action, NodeId) -> Result<()>,
    ) -> Self {
        self.runtime.register_reducer::<S>(action_id, reducer).unwrap();
        self
    }

    pub fn dispatch(&mut self, action: impl Action + 'static) -> Result<()> {
        // For simple tests, we assume a root target ID (0).
        let target = NodeId::derived(0, &[0]);
        self.runtime.dispatch(Box::new(action), target)
    }

    pub fn tick(&mut self, dt: CurrentTime) -> Result<()> {
        let action = Tick { dt };
        self.dispatch(action)
    }

    pub fn advance_to(&mut self, time: CurrentTime) -> Result<()> {
        self.dispatch(AdvanceTo { time })
    }

    pub fn current_time(&self) -> CurrentTime {
        self.runtime.clock().current_time()
    }

    // A simulated "frame" evaluation
    pub fn pump(&mut self) -> Result<()> {
        // 1. In a real app, we'd lower the Authoring Tree here to get IR.
        // 2. Then run Layout.
        let viewport = LayoutSize { width: 800.0, height: 600.0 };
        // We'll use a dummy node list for now as we haven't integrated the full authoring->IR pipeline in the harness yet.
        let nodes = vec![]; 
        let snapshot = self.layout_engine.compute_layout(&nodes, viewport)?;
        self.last_snapshot = Some(snapshot.clone());

        // 3. Then Render.
        // Again, dummy display list generation based on snapshot.
        let display_list = DisplayList::new(LayoutRect::new(0.0, 0.0, 800.0, 600.0));
        self.renderer.render(&display_list)?;

        Ok(())
    }
    
    pub fn get_last_display_list(&self) -> Option<DisplayList> {
        self.renderer.last_display_list.lock().unwrap().clone()
    }
}