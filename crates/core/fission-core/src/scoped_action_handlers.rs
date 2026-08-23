//! Scoped external action handlers.
//!
//! This module lets embedders register action handlers for a scoped widget
//! subtree. Fission owns dispatch routing; embedders own handler lifecycle,
//! state, and cleanup.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};

use crate::{ActionEnvelope, ActionId, ActionInput, ActionScopeId, WidgetId};

/// Result of handling an action emitted inside an [`ActionScope`](crate::ui::ActionScope).
///
/// Handlers can consume the action directly or forward it as a new envelope for
/// normal reducer and effect processing. The original target and input are
/// preserved when forwarding.
#[derive(Clone, Debug)]
pub enum ScopedActionResolution {
    Handled,
    Forward(ActionEnvelope),
}

pub type ScopedActionHandler = Box<
    dyn FnMut(&ActionEnvelope, WidgetId, &ActionInput) -> Result<ScopedActionResolution>
        + Send
        + 'static,
>;

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
) -> Result<Option<ScopedActionResolution>> {
    let Some(scope_id) = input.action_scope_id() else {
        return Ok(None);
    };
    let mut handlers = handlers()
        .lock()
        .map_err(|_| anyhow!("scoped action handler registry is poisoned"))?;
    let Some(scoped_handlers) = handlers.get_mut(&(scope_id, action.id)) else {
        return Ok(None);
    };
    let mut forwarded = None;
    for handler in scoped_handlers {
        if let ScopedActionResolution::Forward(envelope) = handler(action, target, input)? {
            if forwarded.replace(envelope).is_some() {
                return Err(anyhow!(
                    "multiple scoped action handlers attempted to forward one action"
                ));
            }
        }
    }
    Ok(Some(
        forwarded
            .map(ScopedActionResolution::Forward)
            .unwrap_or(ScopedActionResolution::Handled),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ActionInput, GlobalState, Runtime};
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
                Ok(ScopedActionResolution::Handled)
            }),
        )
        .unwrap();

        let envelope = ActionEnvelope {
            id: action,
            payload: Vec::new(),
        };
        let target = WidgetId::from_u128(7);
        assert!(
            dispatch_scoped_action_handler(&envelope, target, &ActionInput::None)
                .unwrap()
                .is_none(),
            "unscoped input must not invoke scoped action handlers"
        );
        assert_eq!(*calls.lock().unwrap(), 0);

        let scoped = ActionInput::scoped_raw(scope.as_u128(), target, ActionInput::None);
        assert!(matches!(
            dispatch_scoped_action_handler(&envelope, target, &scoped).unwrap(),
            Some(ScopedActionResolution::Handled)
        ));
        assert_eq!(*calls.lock().unwrap(), 1);

        clear_scoped_action_handlers(scope).unwrap();
    }

    #[test]
    fn scoped_handler_can_forward_a_rewritten_envelope() {
        let scope = ActionScopeId::from_name("test.forwarding.scope");
        let emitted_action = ActionId::from_name("test.emitted.action");
        let forwarded_action = ActionId::from_name("test.forwarded.action");
        clear_scoped_action_handlers(scope).unwrap();
        register_scoped_action_handler(
            scope,
            emitted_action,
            Box::new(move |_, _, _| {
                Ok(ScopedActionResolution::Forward(ActionEnvelope {
                    id: forwarded_action,
                    payload: b"forwarded".to_vec(),
                }))
            }),
        )
        .unwrap();

        let emitted = ActionEnvelope {
            id: emitted_action,
            payload: b"emitted".to_vec(),
        };
        let target = WidgetId::from_u128(11);
        let input = ActionInput::scoped_raw(scope.as_u128(), target, ActionInput::None);
        let resolution = dispatch_scoped_action_handler(&emitted, target, &input).unwrap();

        let Some(ScopedActionResolution::Forward(forwarded)) = resolution else {
            panic!("scoped handler should forward a rewritten envelope");
        };
        assert_eq!(forwarded.id, forwarded_action);
        assert_eq!(forwarded.payload, b"forwarded");

        clear_scoped_action_handlers(scope).unwrap();
    }

    #[derive(Debug, Default)]
    struct ForwardState {
        reducer_calls: usize,
    }

    impl GlobalState for ForwardState {}

    fn count_forwarded_action(
        state: &mut ForwardState,
        _action: &ActionEnvelope,
        _target: WidgetId,
    ) -> Result<()> {
        state.reducer_calls += 1;
        Ok(())
    }

    #[test]
    fn forwarding_the_same_action_bypasses_the_scoped_handler() {
        let scope = ActionScopeId::from_name("test.forwarding.same-action.scope");
        let action_id = ActionId::from_name("test.forwarding.same-action");
        clear_scoped_action_handlers(scope).unwrap();
        let handler_calls = Arc::new(Mutex::new(0usize));
        let calls_for_handler = handler_calls.clone();
        register_scoped_action_handler(
            scope,
            action_id,
            Box::new(move |action, _, _| {
                *calls_for_handler.lock().unwrap() += 1;
                Ok(ScopedActionResolution::Forward(action.clone()))
            }),
        )
        .unwrap();

        let mut runtime = Runtime::default();
        runtime
            .add_app_state(Box::new(ForwardState::default()))
            .unwrap();
        runtime
            .register_reducer::<ForwardState>(action_id, count_forwarded_action)
            .unwrap();
        let target = WidgetId::from_u128(13);
        let input = ActionInput::scoped_raw(scope.as_u128(), target, ActionInput::None);
        runtime
            .dispatch_with_input(
                ActionEnvelope {
                    id: action_id,
                    payload: Vec::new(),
                },
                target,
                &input,
            )
            .unwrap();

        assert_eq!(*handler_calls.lock().unwrap(), 1);
        assert_eq!(
            runtime
                .get_app_state::<ForwardState>()
                .expect("forward state")
                .reducer_calls,
            1
        );
        clear_scoped_action_handlers(scope).unwrap();
    }

    #[test]
    fn multiple_forwarding_handlers_are_rejected() {
        let scope = ActionScopeId::from_name("test.forwarding.ambiguous.scope");
        let action_id = ActionId::from_name("test.forwarding.ambiguous");
        clear_scoped_action_handlers(scope).unwrap();
        for payload in [b"first".as_slice(), b"second".as_slice()] {
            let payload = payload.to_vec();
            register_scoped_action_handler(
                scope,
                action_id,
                Box::new(move |_, _, _| {
                    Ok(ScopedActionResolution::Forward(ActionEnvelope {
                        id: action_id,
                        payload: payload.clone(),
                    }))
                }),
            )
            .unwrap();
        }

        let target = WidgetId::from_u128(17);
        let input = ActionInput::scoped_raw(scope.as_u128(), target, ActionInput::None);
        let error = dispatch_scoped_action_handler(
            &ActionEnvelope {
                id: action_id,
                payload: Vec::new(),
            },
            target,
            &input,
        )
        .expect_err("ambiguous forwarding must fail");

        assert!(error
            .to_string()
            .contains("multiple scoped action handlers"));
        clear_scoped_action_handlers(scope).unwrap();
    }
}
