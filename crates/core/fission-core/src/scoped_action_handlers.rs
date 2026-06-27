//! Scoped external action handlers.
//!
//! This module lets embedders register action handlers for a scoped widget
//! subtree. Fission owns dispatch routing; embedders own handler lifecycle,
//! state, and cleanup.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};

use crate::{ActionEnvelope, ActionId, ActionInput, ActionScopeId, WidgetId};

pub type ScopedActionHandler =
    Box<dyn FnMut(&ActionEnvelope, WidgetId, &ActionInput) -> Result<()> + Send + 'static>;

type ScopedActionHandlerMap = BTreeMap<(u128, ActionId), Vec<ScopedActionHandler>>;

static SCOPED_ACTION_HANDLERS: OnceLock<Mutex<ScopedActionHandlerMap>> = OnceLock::new();

fn handlers() -> &'static Mutex<ScopedActionHandlerMap> {
    SCOPED_ACTION_HANDLERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn register_scoped_action_handler(
    scope: ActionScopeId,
    action_id: ActionId,
    handler: ScopedActionHandler,
) -> Result<()> {
    let mut handlers = handlers()
        .lock()
        .map_err(|_| anyhow!("scoped action handler registry is poisoned"))?;
    handlers
        .entry((scope.as_u128(), action_id))
        .or_default()
        .push(handler);
    Ok(())
}

pub fn clear_scoped_action_handlers(scope: ActionScopeId) -> Result<()> {
    let mut handlers = handlers()
        .lock()
        .map_err(|_| anyhow!("scoped action handler registry is poisoned"))?;
    handlers.retain(|(registered_scope, _), _| *registered_scope != scope.as_u128());
    Ok(())
}

pub(crate) fn dispatch_scoped_action_handler(
    action: &ActionEnvelope,
    target: WidgetId,
    input: &ActionInput,
) -> Result<bool> {
    let Some(scope_id) = input.action_scope_id() else {
        return Ok(false);
    };
    let mut handlers = handlers()
        .lock()
        .map_err(|_| anyhow!("scoped action handler registry is poisoned"))?;
    let Some(scoped_handlers) = handlers.get_mut(&(scope_id, action.id)) else {
        return Ok(false);
    };
    for handler in scoped_handlers {
        handler(action, target, input)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActionInput;
    use std::sync::{Arc, Mutex};

    #[test]
    fn scoped_handler_runs_only_for_matching_scope_and_action() {
        let scope = ActionScopeId::from_name("test.scope");
        let action = ActionId::from_name("test.action");
        clear_scoped_action_handlers(scope).unwrap();
        let calls = Arc::new(Mutex::new(0usize));
        let calls_for_handler = calls.clone();
        register_scoped_action_handler(
            scope,
            action,
            Box::new(move |_, _, _| {
                *calls_for_handler.lock().unwrap() += 1;
                Ok(())
            }),
        )
        .unwrap();

        let envelope = ActionEnvelope {
            id: action,
            payload: Vec::new(),
        };
        let target = WidgetId::from_u128(7);
        assert!(
            !dispatch_scoped_action_handler(&envelope, target, &ActionInput::None).unwrap(),
            "unscoped input must not invoke scoped action handlers"
        );
        assert_eq!(*calls.lock().unwrap(), 0);

        let scoped = ActionInput::scoped_raw(scope.as_u128(), target, ActionInput::None);
        assert!(dispatch_scoped_action_handler(&envelope, target, &scoped).unwrap());
        assert_eq!(*calls.lock().unwrap(), 1);

        clear_scoped_action_handlers(scope).unwrap();
    }
}
