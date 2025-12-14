use anyhow::Result;
use fission_core::{Runtime, Action, ActionId, AppState, CurrentTime, AdvanceTo, Tick, Desugar, LoweringContext, InputEvent, ActionEnvelope};
use fission_core::lowering::build_layout_tree;
use fission_ir::{NodeId, CoreIR};
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
    pub last_snapshot: Option<LayoutSnapshot>,
    pub last_ir: Option<CoreIR>, 
    pub root_widget: Option<Box<dyn Desugar>>,
}

impl Default for TestHarness {
    fn default() -> Self {
        Self {
            runtime: Runtime::default(),
            renderer: MockRenderer::default(),
            layout_engine: LayoutEngine::new(),
            last_snapshot: None,
            last_ir: None,
            root_widget: None,
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

    pub fn with_root_widget<W: Desugar + 'static>(mut self, widget: W) -> Self {
        self.root_widget = Some(Box::new(widget));
        self
    }

    pub fn register_reducer<S: AppState + 'static>(
        mut self,
        action_id: ActionId,
        reducer: fn(&mut S, &ActionEnvelope, NodeId) -> Result<()>,
    ) -> Self {
        self.runtime.register_reducer::<S>(action_id, reducer).unwrap();
        self
    }

    pub fn dispatch(&mut self, action: impl Action + 'static) -> Result<()> {
        let target = NodeId::derived(0, &[0]);
        let envelope: ActionEnvelope = action.into();
        self.runtime.dispatch(envelope, target)
    }

    pub fn send_event(&mut self, event: InputEvent) -> Result<()> {
        if let (Some(ir), Some(layout)) = (&self.last_ir, &self.last_snapshot) {
            self.runtime.handle_input(event, ir, layout)
        } else {
            anyhow::bail!("Cannot handle input: no frame pumped (missing IR/Layout). Call pump() first.");
        }
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

    pub fn pump(&mut self) -> Result<()> {
        // 1. Lowering
        let mut layout_input_nodes = Vec::new();
        
        let mut root_id = None;
        if let Some(root) = &self.root_widget {
            let mut cx = LoweringContext::new();
            let desugared_root = root.desugar(&mut cx);
            cx.ir.root = Some(desugared_root);
            
            layout_input_nodes = build_layout_tree(&cx.ir);
            root_id = Some(desugared_root);
            self.last_ir = Some(cx.ir); 
        }

        let root_id = root_id.ok_or_else(|| anyhow::anyhow!("pump() called without a root widget"))?;

        // 2. Layout
        let viewport = LayoutSize { width: 800.0, height: 600.0 };
        let snapshot = self
            .layout_engine
            .compute_layout(&layout_input_nodes, root_id, viewport)?;
        self.last_snapshot = Some(snapshot.clone());

        // 3. Render
        let display_list = DisplayList::new(LayoutRect::new(0.0, 0.0, 800.0, 600.0));
        self.renderer.render(&display_list)?;

        Ok(())
    }
    
    pub fn get_last_display_list(&self) -> Option<DisplayList> {
        self.renderer.last_display_list.lock().unwrap().clone()
    }
}
