use crate::{
    motion::MotionDeclaration,
    registry::{
        ActionRegistry, IntoHandler, PortalEntry, PortalLayer, ResourceRegistry,
        RuntimeResourceDeclaration, VideoControlCtx, VideoRegistration, WebRegistration,
    },
    ui::Widget,
    Action, ActionEnvelope, ActionId, BoxedReducer, GlobalState,
};
use fission_ir::WidgetId;

/// `BuildCtx` is where widgets register side-effects that must survive beyond
/// the build phase:
///
/// - **Action binding** -- [`bind`](BuildCtx::bind) registers a handler and
///   returns an [`ActionEnvelope`] that can be stored in widget fields like
///   `on_press`.
/// - **Portals** -- [`register_portal`](BuildCtx::register_portal) places a
///   node in the global overlay stack (modals, toasts, flyouts).
/// - **Motion** -- [`register_motion`](BuildCtx::register_motion) registers
///   declarative motion declarations.
/// - **Video / WebView registration** -- [`register_video`](BuildCtx::register_video),
///   [`register_web_view`](BuildCtx::register_web_view).
///
/// # Example
///
/// ```rust,ignore
/// impl From<MyButton> for Widget {
///     fn from(_: MyButton) -> Widget {
///         let (ctx, _) = fission_core::build::current::<S>();
///         let on_press = ctx.bind(MyAction { .. }, reduce_with!(handler));
///         Button { on_press: Some(on_press), ..Default::default() }.into()
///     }
/// }
/// ```
pub struct BuildCtx<S: GlobalState> {
    /// The action registry that accumulates handlers during the build phase.
    pub registry: ActionRegistry<S>,
    /// Declarative runtime resources collected during the build phase.
    pub resources: ResourceRegistry,
    /// Declarative motion collected during the build phase.
    pub motion_declarations: Vec<MotionDeclaration>,
    /// Registered video nodes.
    pub video_nodes: Vec<VideoRegistration>,
    /// Registered web view nodes.
    pub web_nodes: Vec<WebRegistration>,
    /// Portal entries (overlays, modals, toasts).
    pub portals: Vec<PortalEntry>,
    portal_seq: u64,
}

impl<S: GlobalState> BuildCtx<S> {
    pub fn new() -> Self {
        Self {
            registry: ActionRegistry::new(),
            resources: ResourceRegistry::new(),
            motion_declarations: Vec::new(),
            video_nodes: Vec::new(),
            web_nodes: Vec::new(),
            portals: Vec::new(),
            portal_seq: 0,
        }
    }

    pub fn bind<A: Action, H>(&mut self, action: A, handler: H) -> ActionEnvelope
    where
        H: IntoHandler<S, A> + Send + Sync + 'static,
    {
        self.registry.register(handler);

        ActionEnvelope {
            id: A::static_id(),
            payload: action.encode(),
        }
    }

    pub fn register<A: Action, H>(&mut self, handler: H)
    where
        H: IntoHandler<S, A> + Send + Sync + 'static,
    {
        self.registry.register::<A, H>(handler);
    }

    pub(crate) fn register_runtime_reducer(&mut self, action_id: ActionId, reducer: BoxedReducer) {
        self.registry.register_runtime_reducer(action_id, reducer);
    }

    pub fn register_motion(&mut self, declaration: MotionDeclaration) {
        self.motion_declarations.push(declaration);
    }

    pub fn register_video(&mut self, registration: VideoRegistration) {
        self.video_nodes.push(registration);
    }

    pub fn register_web_view(&mut self, registration: WebRegistration) {
        self.web_nodes.push(registration);
    }

    pub fn take_motion_declarations(&mut self) -> Vec<MotionDeclaration> {
        std::mem::take(&mut self.motion_declarations)
    }

    pub fn take_video_registrations(&mut self) -> Vec<VideoRegistration> {
        std::mem::take(&mut self.video_nodes)
    }

    pub fn take_web_registrations(&mut self) -> Vec<WebRegistration> {
        std::mem::take(&mut self.web_nodes)
    }

    pub fn take_resources(&mut self) -> Vec<RuntimeResourceDeclaration> {
        self.resources.take()
    }

    pub fn register_portal(&mut self, node: Widget) {
        self.register_portal_with_layer(PortalLayer::Default, None, node);
    }

    pub fn register_portal_with_id(&mut self, id: WidgetId, node: Widget) {
        self.register_portal_with_layer(PortalLayer::Default, Some(id), node);
    }

    pub fn register_portal_with_layer(
        &mut self,
        layer: PortalLayer,
        id: Option<WidgetId>,
        node: Widget,
    ) {
        let seq = self.portal_seq_for_scoped_build();
        self.portals.push(PortalEntry {
            layer,
            seq,
            id,
            node,
        });
    }

    pub(crate) fn portal_seq_for_scoped_build(&mut self) -> u64 {
        let seq = self.portal_seq;
        self.portal_seq = self.portal_seq.wrapping_add(1);
        seq
    }

    pub fn take_portals(&mut self) -> Vec<(Option<WidgetId>, Widget)> {
        let mut entries = std::mem::take(&mut self.portals);
        entries.sort_by(|a, b| (a.layer, a.seq).cmp(&(b.layer, b.seq)));
        entries.into_iter().map(|e| (e.id, e.node)).collect()
    }

    pub fn video_controls(&self, target: WidgetId) -> VideoControlCtx {
        VideoControlCtx { target }
    }
}
