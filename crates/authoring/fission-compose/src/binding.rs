use crate::{
    core,
    events::{EditEvent, Event, FieldAction, HandlerFn},
    identity::{self, Mount},
    scope::Scope,
};
use core::{BuildCtxHandle, GlobalState, ReducerContext, StateField};

/// One typed value/update contract accepted by built-in and custom controls.
/// No widget-tree interpretation happens in this type.
pub struct Control<T, E: EditEvent<T>> {
    pub value: T,
    pub edit: E::Edit,
    pub on_change: Option<Event<E>>,
}
impl<T: Clone, E: EditEvent<T>> Clone for Control<T, E> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            edit: self.edit.clone(),
            on_change: self.on_change.clone(),
        }
    }
}

pub trait Source<T: Clone + Send + Sync + 'static>: Sized {
    fn read(self) -> T;
}
pub trait ControlSource<T: Clone + Send + Sync + 'static>: Source<T> {
    fn control<E: EditEvent<T>>(self) -> Control<T, E>;
}

/// Explicit one-way source. In Component!, `read(expression)` also disables
/// automatic field detection. A control with no update event is read-only.
pub struct Read<T>(pub T);
pub struct ReadOnly<T>(pub T);
pub fn read<T>(value: T) -> ReadOnly<T> {
    ReadOnly(value)
}
impl<U, T> Source<T> for Read<U>
where
    T: Clone + Send + Sync + 'static,
    U: Into<T>,
{
    fn read(self) -> T {
        self.0.into()
    }
}
impl<U, T> Source<T> for ReadOnly<U>
where
    T: Clone + Send + Sync + 'static,
    U: Into<T>,
{
    fn read(self) -> T {
        self.0.into()
    }
}
impl<U, T> ControlSource<T> for ReadOnly<U>
where
    T: Clone + Send + Sync + 'static,
    U: Into<T>,
{
    fn control<E: EditEvent<T>>(self) -> Control<T, E> {
        let value: T = self.0.into();
        Control {
            edit: E::initial(&value),
            value,
            on_change: None,
        }
    }
}

pub struct StateSource<P: Scope, T> {
    cx: BuildCtxHandle<P::Root>,
    mount: Mount,
    key: P::Key,
    symbol: &'static str,
    value: T,
    get: for<'a> fn(&'a P::Target) -> &'a T,
    set: fn(&mut P::Target, T),
}
pub fn state_source<P: Scope, T: Clone>(
    cx: BuildCtxHandle<P::Root>,
    mount: &Mount,
    key: P::Key,
    symbol: &'static str,
    value: &T,
    get: for<'a> fn(&'a P::Target) -> &'a T,
    set: fn(&mut P::Target, T),
) -> StateSource<P, T> {
    StateSource {
        cx,
        mount: mount.clone(),
        key,
        symbol,
        value: value.clone(),
        get,
        set,
    }
}

fn edit_field<T, E: EditEvent<T>>(value: &T) -> Option<StateField<E::Edit>> {
    E::RETAIN_EDIT.then(|| {
        core::build::with_widget_id(property_id(), || {
            StateField::new_with("fission.component.control", "editing", || E::initial(value))
        })
    })
}

impl<P, T> Source<T> for StateSource<P, T>
where
    P: Scope,
    T: Clone + Send + Sync + 'static,
{
    fn read(self) -> T {
        self.value
    }
}
impl<P, T> ControlSource<T> for StateSource<P, T>
where
    P: Scope,
    T: Clone + Send + Sync + 'static,
{
    fn control<E: EditEvent<T>>(self) -> Control<T, E> {
        let Self {
            cx,
            mount,
            key,
            symbol,
            value,
            get,
            set,
        } = self;
        let node = identity::node().id;
        let property = property_id();
        let control_mount = mount.control(property, symbol);
        let retained = edit_field::<T, E>(&value);
        let edit = retained
            .as_ref()
            .map(StateField::get)
            .unwrap_or_else(|| E::initial(&value));
        let edit_key = retained.as_ref().map(|f| f.key().clone());
        let payload = FieldAction {
            key: key.clone(),
            incarnation: control_mount.incarnation,
        };
        let live = control_mount.clone();
        let handler = HandlerFn(
            move |root: &mut P::Root,
                  action: FieldAction<P::Key>,
                  ctx: &mut ReducerContext<P::Root>| {
                if action.key != key || !live.alive(action.incarnation) {
                    return;
                }
                let Some(target) = P::get_mut(root, &action.key) else {
                    return;
                };
                let current = get(target).clone();
                let mut editing = match &edit_key {
                    Some(k) => match live.snapshot::<E::Edit>(k) {
                        Some(v) => v,
                        None => return,
                    },
                    None => E::initial(&current),
                };
                let next = E::update(&current, &mut editing, ctx.input);
                if let Some(k) = &edit_key {
                    if !live.replace(k, editing) {
                        return;
                    }
                }
                if let Some(next) = next {
                    set(target, next);
                }
            },
        );
        let envelope = cx.bind_scoped(property, symbol, payload, handler);
        identity::record(&control_mount, node, symbol, &envelope);
        Control {
            value,
            edit,
            on_change: Some(Event::new(envelope)),
        }
    }
}

pub struct LocalSource<R: GlobalState, T: Clone + Send + Sync + 'static> {
    cx: BuildCtxHandle<R>,
    mount: Mount,
    field: StateField<T>,
    symbol: &'static str,
}
pub fn local_source<R: GlobalState, T: Clone + Send + Sync + 'static>(
    cx: BuildCtxHandle<R>,
    mount: &Mount,
    field: StateField<T>,
    symbol: &'static str,
) -> LocalSource<R, T> {
    LocalSource {
        cx,
        mount: mount.clone(),
        field,
        symbol,
    }
}
impl<R, T> Source<T> for LocalSource<R, T>
where
    R: GlobalState,
    T: Clone + Send + Sync + 'static,
{
    fn read(self) -> T {
        self.field.get()
    }
}
impl<R, T> ControlSource<T> for LocalSource<R, T>
where
    R: GlobalState,
    T: Clone + Send + Sync + 'static,
{
    fn control<E: EditEvent<T>>(self) -> Control<T, E> {
        let Self {
            cx,
            mount,
            field,
            symbol,
        } = self;
        let value = field.get();
        let field_key = field.key().clone();
        let node = identity::node().id;
        let property = property_id();
        let control_mount = mount.control(property, symbol);
        let retained = edit_field::<T, E>(&value);
        let edit = retained
            .as_ref()
            .map(StateField::get)
            .unwrap_or_else(|| E::initial(&value));
        let edit_key = retained.as_ref().map(|f| f.key().clone());
        let payload = FieldAction {
            key: (),
            incarnation: control_mount.incarnation,
        };
        let live = control_mount.clone();
        let handler = HandlerFn(
            move |_: &mut R, action: FieldAction<()>, ctx: &mut ReducerContext<R>| {
                if !live.alive(action.incarnation) {
                    return;
                }
                let Some(current) = live.snapshot::<T>(&field_key) else {
                    return;
                };
                let mut editing = match &edit_key {
                    Some(k) => match live.snapshot::<E::Edit>(k) {
                        Some(v) => v,
                        None => return,
                    },
                    None => E::initial(&current),
                };
                let next = E::update(&current, &mut editing, ctx.input);
                if let Some(k) = &edit_key {
                    if !live.replace(k, editing) {
                        return;
                    }
                }
                if let Some(next) = next {
                    live.replace(&field_key, next);
                }
            },
        );
        let envelope = cx.bind_scoped(property, symbol, payload, handler);
        identity::record(&control_mount, node, symbol, &envelope);
        Control {
            value,
            edit,
            on_change: Some(Event::new(envelope)),
        }
    }
}

/// Escape hatch for a computed/optional value: supply ordinary static accessor
/// functions through a StateSource or implement Source<T> for an application adapter.
/// This helper intentionally does NOT invent an inverse for a computed expression.
pub fn one_way<T>(value: T) -> ReadOnly<T> {
    ReadOnly(value)
}

#[derive(Clone)]
struct PropertyName(&'static str);
pub fn control_for<T, E, S>(name: &'static str, source: S) -> Control<T, E>
where
    T: Clone + Send + Sync + 'static,
    E: EditEvent<T>,
    S: ControlSource<T>,
{
    core::build::provide(PropertyName(name), || source.control::<E>())
}
fn property_id() -> core::WidgetId {
    let property = core::build::try_read::<PropertyName>().map_or("value", |p| p.0);
    identity::child_id(identity::node().id, "property", property)
}
