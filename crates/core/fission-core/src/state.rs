use crate::action::{Action, ActionId, GlobalState};
use anyhow::{anyhow, Result};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct StateMap {
    pub states: HashMap<TypeId, Box<dyn GlobalState>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalStateKey {
    component: &'static str,
    field: &'static str,
    key_path: Vec<String>,
    ordinal: usize,
}

impl LocalStateKey {
    pub(crate) fn new_scoped(
        component: &'static str,
        field: &'static str,
        key_path: Vec<String>,
        ordinal: usize,
    ) -> Self {
        Self {
            component,
            field,
            key_path,
            ordinal,
        }
    }

    pub fn component(&self) -> &'static str {
        self.component
    }

    pub fn field(&self) -> &'static str {
        self.field
    }

    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn explicit_key(&self) -> Option<&str> {
        self.key_path.last().map(String::as_str)
    }

    pub fn key_path(&self) -> &[String] {
        &self.key_path
    }

    pub fn action_id<A: Action>(&self) -> ActionId {
        ActionId::from_name(&format!(
            "fission.local_state.v1:{}:{}:{}:{}:{}",
            self.component,
            self.field,
            self.key_path.join("/"),
            self.ordinal,
            A::static_id().as_u128()
        ))
    }
}

type BoxedLocalValue = Box<dyn Any + Send + Sync>;

#[derive(Clone, Default)]
pub struct LocalStateStore {
    values: Arc<Mutex<HashMap<LocalStateKey, BoxedLocalValue>>>,
}

impl fmt::Debug for LocalStateStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.values.lock().map(|values| values.len()).unwrap_or(0);
        f.debug_struct("LocalStateStore")
            .field("len", &len)
            .finish()
    }
}

impl GlobalState for LocalStateStore {}

impl LocalStateStore {
    pub fn get_or_insert_with<T>(&self, key: LocalStateKey, make_default: impl FnOnce() -> T) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        let mut values = self
            .values
            .lock()
            .expect("Fission local widget state store mutex poisoned");
        let entry = values
            .entry(key)
            .or_insert_with(|| Box::new(make_default()) as BoxedLocalValue);
        entry
            .downcast_ref::<T>()
            .unwrap_or_else(|| {
                panic!(
                    "Fission local widget state type mismatch: requested `{}` for an existing field with a different type",
                    std::any::type_name::<T>()
                )
            })
            .clone()
    }

    pub fn update<T>(&self, key: &LocalStateKey, update: impl FnOnce(&mut T)) -> Result<()>
    where
        T: Send + Sync + 'static,
    {
        let mut values = self
            .values
            .lock()
            .map_err(|_| anyhow!("Fission local widget state store mutex poisoned"))?;
        let value = values.get_mut(key).ok_or_else(|| {
            anyhow!(
                "Fission local widget state field `{}` on `{}` was not found",
                key.field,
                key.component
            )
        })?;
        let value = value.downcast_mut::<T>().ok_or_else(|| {
            anyhow!(
                "Fission local widget state type mismatch while updating `{}` on `{}`",
                key.field,
                key.component
            )
        })?;
        update(value);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.values
            .lock()
            .map(|values| values.len())
            .unwrap_or_default()
    }

    pub(crate) fn retain_active(&self, active: &std::collections::HashSet<LocalStateKey>) {
        let mut values = self
            .values
            .lock()
            .expect("Fission local widget state store mutex poisoned");
        values.retain(|key, _| active.contains(key));
    }
}

#[derive(Clone, Debug)]
pub struct StateField<T> {
    key: LocalStateKey,
    value: T,
}

impl<T> StateField<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(component: &'static str, field: &'static str, value: T) -> Self {
        Self::new_with(component, field, || value)
    }

    pub fn new_with(
        component: &'static str,
        field: &'static str,
        make_default: impl FnOnce() -> T,
    ) -> Self {
        crate::build::resolve_local_state(component, field, make_default)
    }

    pub(crate) fn resolved(key: LocalStateKey, value: T) -> Self {
        Self { key, value }
    }

    pub fn get(&self) -> T {
        self.value.clone()
    }

    pub fn component(&self) -> &'static str {
        self.key.component()
    }

    pub fn field(&self) -> &'static str {
        self.key.field()
    }

    pub fn key(&self) -> &LocalStateKey {
        &self.key
    }

    pub fn action_id<A: Action>(&self) -> ActionId {
        self.key.action_id::<A>()
    }
}
