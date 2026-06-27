use crate::{build_context::BuildCtx, GlobalState, View};
use std::any::{type_name, Any, TypeId};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

type NextPortalSeq = unsafe fn(*mut ()) -> u64;
type RegisterRuntimeReducer = unsafe fn(*mut (), crate::ActionId, crate::BoxedReducer);

struct BuildScope {
    state_type: TypeId,
    state_name: &'static str,
    ctx: *mut (),
    view: *const (),
    resources: *mut crate::registry::ResourceRegistry,
    motion_declarations: *mut Vec<crate::motion::MotionDeclaration>,
    video_nodes: *mut Vec<crate::registry::VideoRegistration>,
    web_nodes: *mut Vec<crate::registry::WebRegistration>,
    portals: *mut Vec<crate::registry::PortalEntry>,
    next_portal_seq: NextPortalSeq,
    register_runtime_reducer: RegisterRuntimeReducer,
    runtime: *const crate::RuntimeState,
    env: *const crate::Env,
    layout: Option<*const crate::LayoutSnapshot>,
    local_state_ordinals: HashMap<(&'static str, &'static str), usize>,
    local_state_seen: HashSet<crate::state::LocalStateKey>,
    widget_id_stack: Vec<crate::WidgetId>,
    providers: HashMap<TypeId, Vec<Box<dyn Any + Send + Sync>>>,
}

thread_local! {
    static BUILD_SCOPES: RefCell<Vec<BuildScope>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug)]
pub struct BuildCtxHandle<S: GlobalState> {
    _state: PhantomData<fn() -> S>,
}

impl<S: GlobalState> Clone for BuildCtxHandle<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: GlobalState> Copy for BuildCtxHandle<S> {}

#[derive(Debug)]
pub struct ViewHandle<S: GlobalState> {
    _state: PhantomData<fn() -> S>,
}

impl<S: GlobalState> Clone for ViewHandle<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: GlobalState> Copy for ViewHandle<S> {}

#[doc(hidden)]
pub fn enter<S, R>(ctx: &mut BuildCtx<S>, view: &View<'_, S>, f: impl FnOnce() -> R) -> R
where
    S: GlobalState,
{
    BUILD_SCOPES.with(|scopes| {
        scopes.borrow_mut().push(BuildScope {
            state_type: TypeId::of::<S>(),
            state_name: type_name::<S>(),
            ctx: (ctx as *mut BuildCtx<S>).cast::<()>(),
            view: (view as *const View<'_, S>).cast::<()>(),
            resources: &mut ctx.resources,
            motion_declarations: &mut ctx.motion_declarations,
            video_nodes: &mut ctx.video_nodes,
            web_nodes: &mut ctx.web_nodes,
            portals: &mut ctx.portals,
            next_portal_seq: next_portal_seq::<S>,
            register_runtime_reducer: register_runtime_reducer::<S>,
            runtime: view.runtime(),
            env: view.env(),
            layout: view
                .layout()
                .map(|layout| layout as *const crate::LayoutSnapshot),
            local_state_ordinals: HashMap::new(),
            local_state_seen: HashSet::new(),
            widget_id_stack: Vec::new(),
            providers: HashMap::new(),
        });
    });

    struct PopGuard;
    impl Drop for PopGuard {
        fn drop(&mut self) {
            BUILD_SCOPES.with(|scopes| {
                let mut scopes = scopes.borrow_mut();
                let Some(scope) = scopes.pop() else {
                    return;
                };
                if scopes.is_empty() {
                    unsafe {
                        (*scope.runtime)
                            .local_widget_state
                            .retain_active(&scope.local_state_seen);
                    }
                } else if let Some(parent) = scopes.last_mut() {
                    parent.local_state_seen.extend(scope.local_state_seen);
                }
            });
        }
    }

    let _guard = PopGuard;
    f()
}

unsafe fn next_portal_seq<S: GlobalState>(ctx: *mut ()) -> u64 {
    let ctx = unsafe { &mut *ctx.cast::<BuildCtx<S>>() };
    ctx.portal_seq_for_scoped_build()
}

unsafe fn register_runtime_reducer<S: GlobalState>(
    ctx: *mut (),
    action_id: crate::ActionId,
    reducer: crate::BoxedReducer,
) {
    let ctx = unsafe { &mut *ctx.cast::<BuildCtx<S>>() };
    ctx.register_runtime_reducer(action_id, reducer);
}

pub(crate) fn resolve_local_state<T>(
    component: &'static str,
    field: &'static str,
    make_default: impl FnOnce() -> T,
) -> crate::StateField<T>
where
    T: Clone + Send + Sync + 'static,
{
    let (runtime, key) = BUILD_SCOPES.with(|scopes| {
        let mut scopes = scopes.borrow_mut();
        let Some(scope) = scopes.last_mut() else {
            panic!(
                "Fission local widget state field `{}` on `{}` was accessed outside an active build pass",
                field, component
            );
        };

        let key_path = scope
            .widget_id_stack
            .iter()
            .map(|id| id.as_u128().to_string())
            .collect::<Vec<_>>();
        let ordinal = if key_path.is_empty() {
            let next = scope
                .local_state_ordinals
                .entry((component, field))
                .and_modify(|next| *next += 1)
                .or_insert(0);
            *next
        } else {
            0
        };
        let key = crate::state::LocalStateKey::new_scoped(component, field, key_path, ordinal);
        if !scope.local_state_seen.insert(key.clone()) {
            panic!(
                "Duplicate Fission local widget state identity for `{}` on `{}`.",
                field, component
            );
        }
        (scope.runtime, key)
    });
    let value = unsafe { &*runtime }
        .local_widget_state
        .get_or_insert_with(key.clone(), make_default);
    crate::StateField::resolved(key, value)
}

pub fn with_widget_id<R>(id: crate::WidgetId, f: impl FnOnce() -> R) -> R {
    let pushed = BUILD_SCOPES.with(|scopes| {
        let mut scopes = scopes.borrow_mut();
        if let Some(scope) = scopes.last_mut() {
            scope.widget_id_stack.push(id);
            true
        } else {
            false
        }
    });

    struct PopGuard(bool);
    impl Drop for PopGuard {
        fn drop(&mut self) {
            if self.0 {
                BUILD_SCOPES.with(|scopes| {
                    if let Some(scope) = scopes.borrow_mut().last_mut() {
                        scope.widget_id_stack.pop();
                    }
                });
            }
        }
    }

    let _guard = PopGuard(pushed);
    f()
}

pub fn current_widget_id() -> Option<crate::WidgetId> {
    BUILD_SCOPES.with(|scopes| {
        scopes
            .borrow()
            .last()
            .and_then(|scope| scope.widget_id_stack.last().copied())
    })
}

pub fn provide<T, R>(value: T, f: impl FnOnce() -> R) -> R
where
    T: Clone + Send + Sync + 'static,
{
    BUILD_SCOPES.with(|scopes| {
        let mut scopes = scopes.borrow_mut();
        let Some(scope) = scopes.last_mut() else {
            panic!(
                "Fission build provider `{}` was installed outside an active build pass",
                type_name::<T>()
            );
        };
        scope
            .providers
            .entry(TypeId::of::<T>())
            .or_default()
            .push(Box::new(value));
    });

    struct PopGuard<T: 'static> {
        _provider: PhantomData<T>,
    }
    impl<T: 'static> Drop for PopGuard<T> {
        fn drop(&mut self) {
            BUILD_SCOPES.with(|scopes| {
                if let Some(scope) = scopes.borrow_mut().last_mut() {
                    let provider_type = TypeId::of::<T>();
                    if let Some(values) = scope.providers.get_mut(&provider_type) {
                        values.pop();
                        if values.is_empty() {
                            scope.providers.remove(&provider_type);
                        }
                    }
                }
            });
        }
    }

    let _guard = PopGuard::<T> {
        _provider: PhantomData,
    };
    f()
}

pub fn try_read<T>() -> Option<T>
where
    T: Clone + Send + Sync + 'static,
{
    BUILD_SCOPES.with(|scopes| {
        let scopes = scopes.borrow();
        scopes.iter().rev().find_map(|scope| {
            scope
                .providers
                .get(&TypeId::of::<T>())
                .and_then(|values| values.last())
                .and_then(|value| value.downcast_ref::<T>())
                .cloned()
        })
    })
}

pub fn read<T>() -> T
where
    T: Clone + Send + Sync + 'static,
{
    try_read::<T>().unwrap_or_else(|| {
        panic!(
            "Fission build provider `{}` was not found in the active build scope",
            type_name::<T>()
        )
    })
}

pub fn current<S>() -> (BuildCtxHandle<S>, ViewHandle<S>)
where
    S: GlobalState,
{
    assert_current_scope::<S>();
    (
        BuildCtxHandle {
            _state: PhantomData,
        },
        ViewHandle {
            _state: PhantomData,
        },
    )
}

pub fn try_register_video(registration: crate::registry::VideoRegistration) {
    let video_nodes =
        BUILD_SCOPES.with(|scopes| scopes.borrow().last().map(|scope| scope.video_nodes));
    if let Some(video_nodes) = video_nodes {
        unsafe {
            (*video_nodes).push(registration);
        }
    }
}

pub fn try_register_motion(declaration: crate::motion::MotionDeclaration) {
    let motion_declarations = BUILD_SCOPES.with(|scopes| {
        scopes
            .borrow()
            .last()
            .map(|scope| scope.motion_declarations)
    });
    if let Some(motion_declarations) = motion_declarations {
        unsafe {
            (*motion_declarations).push(declaration);
        }
    }
}

pub fn try_current_runtime_state() -> Option<&'static crate::RuntimeState> {
    BUILD_SCOPES.with(|scopes| {
        scopes
            .borrow()
            .last()
            .map(|scope| unsafe { &*scope.runtime })
    })
}

fn requested_common_scope<S: GlobalState>() -> bool {
    TypeId::of::<S>() == TypeId::of::<()>()
}

fn assert_current_scope<S: GlobalState>() {
    BUILD_SCOPES.with(|scopes| {
        let scopes = scopes.borrow();
        if requested_common_scope::<S>() {
            if scopes.is_empty() {
                panic!(
                    "Fission build context for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            }
            return;
        }

        let Some(scope) = scopes
            .iter()
            .rev()
            .find(|scope| scope.state_type == TypeId::of::<S>())
        else {
            panic!(
                "Fission build context for `{}` requested outside an active build pass",
                type_name::<S>()
            );
        };
        let _ = scope.state_name;
    });
}

fn exact_scope_index<S: GlobalState>(scopes: &[BuildScope]) -> Option<usize> {
    scopes
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, scope)| (scope.state_type == TypeId::of::<S>()).then_some(index))
}

impl<S: GlobalState> BuildCtxHandle<S> {
    fn with_exact_ctx<R>(&self, f: impl FnOnce(&mut BuildCtx<S>) -> R) -> R {
        let ctx = BUILD_SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let Some(index) = exact_scope_index::<S>(&scopes) else {
                panic!(
                    "Fission build context for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            };
            scopes[index].ctx.cast::<BuildCtx<S>>()
        });
        // Build scopes are entered and exited synchronously by the shell.
        // Handles only resolve while that scope is active, so this raw
        // pointer never outlives the build pass that created it.
        unsafe { f(&mut *ctx) }
    }

    pub fn bind<A, H>(&self, action: A, handler: H) -> crate::ActionEnvelope
    where
        A: crate::Action,
        H: crate::registry::IntoHandler<S, A> + Send + Sync + 'static,
    {
        self.with_exact_ctx(|ctx| ctx.bind(action, handler))
    }

    pub fn register<A, H>(&self, handler: H)
    where
        A: crate::Action,
        H: crate::registry::IntoHandler<S, A> + Send + Sync + 'static,
    {
        self.with_exact_ctx(|ctx| ctx.register::<A, H>(handler));
    }

    pub fn bind_local<T, A, H>(
        &self,
        action: A,
        field: crate::StateField<T>,
        handler: H,
    ) -> crate::ActionEnvelope
    where
        T: crate::GlobalState + Clone + 'static,
        A: crate::Action,
        H: crate::registry::IntoHandler<T, A> + Send + Sync + 'static,
    {
        let action_id = field.action_id::<A>();
        let field_key = field.key().clone();
        let reducer: crate::BoxedReducer = Box::new(
            move |app_states,
                  envelope: &crate::ActionEnvelope,
                  _target,
                  _effects,
                  _input|
                  -> anyhow::Result<()> {
                let action: A = serde_json::from_slice(&envelope.payload).map_err(|error| {
                    anyhow::anyhow!("Failed to deserialize local action: {error}")
                })?;
                let Some(store) = app_states
                    .get_mut(&TypeId::of::<crate::state::LocalStateStore>())
                    .and_then(|state| state.downcast_mut::<crate::state::LocalStateStore>())
                else {
                    anyhow::bail!("Fission local widget state store is not registered in Runtime");
                };
                let mut effects_builder = crate::Effects::<T>::new_headless(0);
                let mut reducer_ctx = crate::ReducerContext {
                    effects: &mut effects_builder,
                    input: _input,
                };
                store.update::<T>(&field_key, |value| {
                    handler.call(value, action, &mut reducer_ctx)
                })?;
                _effects.extend(effects_builder.out);
                Ok(())
            },
        );

        let (ctx, register_runtime_reducer) = BUILD_SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let Some(scope) = scopes.last() else {
                panic!(
                    "Fission build context for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            };
            (scope.ctx, scope.register_runtime_reducer)
        });
        unsafe {
            register_runtime_reducer(ctx, action_id, reducer);
        }

        crate::ActionEnvelope {
            id: action_id,
            payload: action.encode(),
        }
    }

    pub fn register_motion(&self, declaration: crate::motion::MotionDeclaration) {
        let motion_declarations = BUILD_SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let Some(scope) = scopes.last() else {
                panic!(
                    "Fission build context for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            };
            scope.motion_declarations
        });
        unsafe {
            (*motion_declarations).push(declaration);
        }
    }

    pub fn register_video(&self, registration: crate::registry::VideoRegistration) {
        let video_nodes = BUILD_SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let Some(scope) = scopes.last() else {
                panic!(
                    "Fission build context for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            };
            scope.video_nodes
        });
        unsafe {
            (*video_nodes).push(registration);
        }
    }

    pub fn register_web_view(&self, registration: crate::registry::WebRegistration) {
        let web_nodes = BUILD_SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let Some(scope) = scopes.last() else {
                panic!(
                    "Fission build context for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            };
            scope.web_nodes
        });
        unsafe {
            (*web_nodes).push(registration);
        }
    }

    pub fn with_resources<R>(
        &self,
        f: impl FnOnce(&mut crate::registry::ResourceRegistry) -> R,
    ) -> R {
        let resources = BUILD_SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let Some(scope) = scopes.last() else {
                panic!(
                    "Fission build context for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            };
            scope.resources
        });
        unsafe { f(&mut *resources) }
    }

    pub fn register_portal(&self, node: crate::Widget) {
        self.register_portal_with_layer(crate::PortalLayer::Default, None, node);
    }

    pub fn register_portal_with_id(&self, id: crate::WidgetId, node: crate::Widget) {
        self.register_portal_with_layer(crate::PortalLayer::Default, Some(id), node);
    }

    pub fn register_portal_with_layer(
        &self,
        layer: crate::PortalLayer,
        id: Option<crate::WidgetId>,
        node: crate::Widget,
    ) {
        let (ctx, portals, next_portal_seq) = BUILD_SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let Some(scope) = scopes.last() else {
                panic!(
                    "Fission build context for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            };
            (scope.ctx, scope.portals, scope.next_portal_seq)
        });
        unsafe {
            let seq = next_portal_seq(ctx);
            (*portals).push(crate::registry::PortalEntry {
                layer,
                seq,
                id,
                node,
            });
        }
    }

    pub fn video_controls(&self, target: crate::WidgetId) -> crate::registry::VideoControlCtx {
        self.with_exact_ctx(|ctx| ctx.video_controls(target))
    }
}

impl<S: GlobalState> ViewHandle<S> {
    fn with_common_scope<R>(&self, f: impl FnOnce(&BuildScope) -> R) -> R {
        BUILD_SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let Some(scope) = scopes.last() else {
                panic!(
                    "Fission view for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            };
            f(scope)
        })
    }

    pub fn state(&self) -> &S {
        BUILD_SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let Some(index) = exact_scope_index::<S>(&scopes) else {
                panic!(
                    "Fission view state for `{}` requested outside an active build pass",
                    type_name::<S>()
                );
            };
            unsafe { (*scopes[index].view.cast::<View<'_, S>>()).state }
        })
    }

    pub fn runtime(&self) -> &crate::RuntimeState {
        self.with_common_scope(|scope| unsafe { &*scope.runtime })
    }

    pub fn env(&self) -> &crate::Env {
        self.with_common_scope(|scope| unsafe { &*scope.env })
    }

    pub fn layout(&self) -> Option<&crate::LayoutSnapshot> {
        self.with_common_scope(|scope| unsafe { scope.layout.map(|layout| &*layout) })
    }

    pub fn theme(&self) -> &fission_theme::Theme {
        &self.env().theme
    }

    pub fn i18n(&self) -> &fission_i18n::I18nRegistry {
        &self.env().i18n
    }

    pub fn get_rect(&self, id: crate::WidgetId) -> Option<crate::LayoutRect> {
        let node_id: fission_ir::WidgetId = id.into();
        self.layout()
            .and_then(|layout| layout.get_node_rect(node_id))
    }

    pub fn get_constraints(&self, id: crate::WidgetId) -> Option<crate::BoxConstraints> {
        let node_id: fission_ir::WidgetId = id.into();
        self.layout()
            .and_then(|layout| layout.get_node_constraints(node_id))
    }

    pub fn viewport_size(&self) -> crate::LayoutSize {
        self.env().viewport_size
    }

    pub fn select<R>(&self, selector: impl FnOnce(&S) -> R) -> R {
        selector(self.state())
    }

    pub fn select_with<T: crate::view::Selector<S>>(&self) -> T::Output {
        T::select(*self)
    }

    pub fn global(&self) -> <S as crate::view::FissionViewField>::View<'_>
    where
        S: crate::view::FissionViewField,
    {
        <S as crate::view::FissionViewField>::view_field(self.state())
    }

    pub fn motion_value(
        &self,
        widget_id: crate::WidgetId,
        property: crate::MotionPropertyId,
    ) -> crate::MotionValue {
        self.runtime()
            .motion
            .values
            .get(&(widget_id, property.clone()))
            .cloned()
            .unwrap_or_else(|| property.default_value())
    }

    pub fn motion_scalar(
        &self,
        widget_id: crate::WidgetId,
        property: crate::MotionPropertyId,
    ) -> f32 {
        self.runtime().motion.scalar_value(widget_id, property)
    }

    pub fn video_state(&self, widget_id: crate::WidgetId) -> Option<&crate::env::VideoState> {
        self.runtime().video.states.get(&widget_id)
    }
}
