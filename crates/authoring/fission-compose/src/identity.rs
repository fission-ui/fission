use crate::core::state::{LocalStateKey, LocalStateStore};
use crate::core::{self, ActionId, GlobalState, StateField, WidgetId};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

/// A stable logical key used to retain component and control identity.
///
/// Implementations must return the same value for the same domain identity
/// across rebuilds and process launches. Presentation text, list positions,
/// pointer addresses, and floating-point values are deliberately unsuitable.
pub trait UiKey {
    fn encode_ui_key(&self) -> String;
}

impl<T: UiKey + ?Sized> UiKey for &T {
    fn encode_ui_key(&self) -> String {
        (*self).encode_ui_key()
    }
}
impl UiKey for str {
    fn encode_ui_key(&self) -> String {
        format!("str:{}:{self}", self.len())
    }
}
impl UiKey for String {
    fn encode_ui_key(&self) -> String {
        self.as_str().encode_ui_key()
    }
}
impl UiKey for bool {
    fn encode_ui_key(&self) -> String {
        format!("bool:{self}")
    }
}
impl UiKey for char {
    fn encode_ui_key(&self) -> String {
        format!("char:{}", u32::from(*self))
    }
}
impl UiKey for () {
    fn encode_ui_key(&self) -> String {
        "unit".into()
    }
}
impl UiKey for WidgetId {
    fn encode_ui_key(&self) -> String {
        format!("widget:{}", self.as_u128())
    }
}

macro_rules! integer_keys {
    ($($ty:ty),* $(,)?) => {$(
        impl UiKey for $ty {
            fn encode_ui_key(&self) -> String {
                format!(concat!(stringify!($ty), ":{}"), self)
            }
        }
    )*};
}
integer_keys!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

impl<T: UiKey> UiKey for Option<T> {
    fn encode_ui_key(&self) -> String {
        match self {
            Some(value) => format!("some:{}", value.encode_ui_key()),
            None => "none".into(),
        }
    }
}
impl<A: UiKey, B: UiKey> UiKey for (A, B) {
    fn encode_ui_key(&self) -> String {
        let a = self.0.encode_ui_key();
        let b = self.1.encode_ui_key();
        format!("tuple2:{}:{a}:{}:{b}", a.len(), b.len())
    }
}

/// Only a per-build duplicate-key checker. Not a retained tree or a dispatch registry.
#[derive(Clone, Default)]
pub struct BuildSession(Arc<Mutex<HashSet<WidgetId>>>);
impl BuildSession {
    pub fn run<R>(f: impl FnOnce() -> R) -> R {
        if core::build::try_read::<Self>().is_some() {
            f()
        } else {
            core::build::provide(Self::default(), f)
        }
    }
    pub fn claim(id: WidgetId, name: &str) {
        if let Some(session) = core::build::try_read::<Self>() {
            assert!(
                session
                    .0
                    .lock()
                    .expect("duplicate-key checker poisoned")
                    .insert(id),
                "duplicate component/control identity for {name}; supply a distinct semantic key"
            );
        }
    }
}

/// Hashes a canonical, length-delimited tuple rather than display text or a
/// declaration ordinal.
pub fn child_id<K: UiKey + ?Sized>(parent: WidgetId, namespace: &str, key: &K) -> WidgetId {
    let key = key.encode_ui_key();
    let encoded = format!(
        "fission.compose.child.v1:{}:{}:{}:{}:{}",
        parent.as_u128(),
        namespace.len(),
        namespace,
        key.len(),
        key,
    );
    WidgetId::from_u128(ActionId::from_name(&encoded).as_u128())
}
pub fn current_id() -> WidgetId {
    core::build::current_identity().expect("Component! must run in an active Fission build")
}
pub fn in_slot<R>(name: &str, f: impl FnOnce() -> R) -> R {
    let id = child_id(current_id(), "slot", name);
    core::build::with_widget_id(id, f)
}

/// Lexically scoped node metadata, used by typed property adapters and diagnostics.
#[derive(Clone)]
pub struct NodeContext {
    pub id: WidgetId,
    pub label: String,
}
pub fn with_node<R, K: UiKey>(name: &str, key: &K, f: impl FnOnce() -> R) -> R {
    let id = child_id(current_id(), name, key);
    BuildSession::claim(id, name);
    let node = NodeContext {
        id,
        label: key.encode_ui_key(),
    };
    // Generated components add their own type/owner scope during their normal conversion.
    core::build::provide(node, || core::build::with_widget_id(id, f))
}
pub fn node() -> NodeContext {
    core::build::try_read().unwrap_or_else(|| NodeContext {
        id: current_id(),
        label: String::new(),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Incarnation(u64);
impl GlobalState for Incarnation {}
static NEXT_MOUNT: AtomicU64 = AtomicU64::new(1);

/// An ordinary Fission retained field is also the event-liveness witness. Rebuilds
/// reuse it; an actual unmount/remount gets a new witness. No build references escape.
#[derive(Clone)]
pub struct Mount {
    pub owner: WidgetId,
    pub store: LocalStateStore,
    witness: LocalStateKey,
    pub incarnation: u64,
}
impl Mount {
    pub fn new<R: GlobalState>(component: &'static str, view: core::ViewHandle<R>) -> Self {
        let token = StateField::new_with(component, "__component_mount", || {
            let n = NEXT_MOUNT.fetch_add(1, Ordering::Relaxed);
            assert!(n != u64::MAX, "component mount counter exhausted");
            Incarnation(n)
        });
        Self {
            owner: current_id(),
            store: view.runtime().local_widget_state.clone(),
            witness: token.key().clone(),
            incarnation: token.get().0,
        }
    }
    /// Creates an independently retained liveness witness for one control.
    /// A conditionally removed control therefore rejects late events even when
    /// its containing component remains mounted.
    pub fn control(&self, node: WidgetId, symbol: &'static str) -> Self {
        let token = core::build::with_widget_id(node, || {
            StateField::new_with("fission.compose.control", symbol, || {
                let n = NEXT_MOUNT.fetch_add(1, Ordering::Relaxed);
                assert!(n != u64::MAX, "control mount counter exhausted");
                Incarnation(n)
            })
        });
        Self {
            owner: self.owner,
            store: self.store.clone(),
            witness: token.key().clone(),
            incarnation: token.get().0,
        }
    }
    pub fn alive(&self, incarnation: u64) -> bool {
        if incarnation != self.incarnation {
            return false;
        }
        let mut alive = false;
        self.store
            .update::<Incarnation>(&self.witness, |v| alive = v.0 == incarnation)
            .is_ok()
            && alive
    }
    pub fn snapshot<T: Clone + Send + Sync + 'static>(&self, key: &LocalStateKey) -> Option<T> {
        let mut out = None;
        self.store
            .update::<T>(key, |value| out = Some(value.clone()))
            .ok()?;
        out
    }
    pub fn replace<T: Send + Sync + 'static>(&self, key: &LocalStateKey, value: T) -> bool {
        self.store.update::<T>(key, |slot| *slot = value).is_ok()
    }
}

/// Probe for tests/devtools only. Applications need not install it.
#[derive(Clone, Default)]
pub struct BindingProbe(pub Arc<Mutex<Vec<BindingRecord>>>);
impl BindingProbe {
    pub fn records(&self) -> Vec<BindingRecord> {
        self.0.lock().expect("binding probe poisoned").clone()
    }
}
#[derive(Clone, Debug)]
pub struct BindingRecord {
    pub owner: WidgetId,
    pub node: WidgetId,
    pub symbol: String,
    pub envelope: core::ActionEnvelope,
}
pub fn record(mount: &Mount, node: WidgetId, symbol: &str, envelope: &core::ActionEnvelope) {
    if let Some(probe) = core::build::try_read::<BindingProbe>() {
        probe
            .0
            .lock()
            .expect("binding probe poisoned")
            .push(BindingRecord {
                owner: mount.owner,
                node,
                symbol: symbol.into(),
                envelope: envelope.clone(),
            });
    }
}
