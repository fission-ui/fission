use anyhow::Result;
use fission_core::{Runtime, Action, ActionId, AppState, CurrentTime, AdvanceTo, Tick, Lower, LoweringContext, InputEvent, ActionEnvelope, Env, Widget, View, Node, BuildCtx};
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

pub struct TestHarness<S: AppState> {
    pub runtime: Runtime,
    pub renderer: MockRenderer,
    pub layout_engine: LayoutEngine,
    pub last_snapshot: Option<LayoutSnapshot>,
    pub last_ir: Option<CoreIR>, 
    pub root_widget: Option<Box<dyn Widget<S>>>,
    pub env: Env,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: AppState> TestHarness<S> {
    pub fn new(initial_state: S) -> Self {
        let mut runtime = Runtime::default();
        runtime.add_app_state(Box::new(initial_state)).expect("Failed to add initial state");
        
        Self {
            runtime,
            renderer: MockRenderer::default(),
            layout_engine: LayoutEngine::new(),
            last_snapshot: None,
            last_ir: None,
            root_widget: None,
            env: Env::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_root_widget<W: Widget<S> + 'static>(mut self, widget: W) -> Self {
        self.root_widget = Some(Box::new(widget));
        self
    }

    pub fn register_reducer(
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
        // 1. Build & Lower
        let mut layout_input_nodes = Vec::new();
        
        if let Some(root) = &self.root_widget {
            // Build
            let node_tree = {
                let state = self.runtime.get_app_state::<S>().expect("App state missing");
                let view = View::new(state, &self.runtime.runtime_state, &self.env);
                let mut ctx = BuildCtx::new();
                let tree = root.build(&mut ctx, &view);
                
                self.runtime.clear_reducers();
                self.runtime.absorb_registry(ctx.registry);
                tree
            };

            // Lower
            let mut cx = LoweringContext::new(&self.env, &self.runtime.runtime_state);
            let root_id = node_tree.lower(&mut cx);
            cx.ir.root = Some(root_id);
            
            layout_input_nodes = build_layout_tree(&cx.ir);
            self.last_ir = Some(cx.ir); 
            
            // 2. Layout
            let viewport = LayoutSize { width: 800.0, height: 600.0 };
            let snapshot = self.layout_engine.compute_layout(&layout_input_nodes, root_id, viewport)?;
            self.last_snapshot = Some(snapshot.clone());
        }

        // 3. Render
        let display_list = DisplayList::new(LayoutRect::new(0.0, 0.0, 800.0, 600.0));
        self.renderer.render(&display_list)?;

        Ok(())
    }
    
    pub fn get_last_display_list(&self) -> Option<DisplayList> {
        self.renderer.last_display_list.lock().unwrap().clone()
    }
}
