//! Shell-neutral navigation requests and hyperlink activation.

use crate::{Action, ActionId};
use fission_ir::Hyperlink;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

/// A navigation operation requested by application code or semantic activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigationCommand {
    /// Add a logical route to the active history stack.
    Push(String),
    /// Replace the active history entry with a logical route.
    Replace(String),
    /// Activate a complete hyperlink, including its browsing-context target.
    Open(Hyperlink),
    /// Move one entry backward in the active history.
    Back,
    /// Move one entry forward in the active history.
    Forward,
    /// Move by a signed number of entries in the active history.
    Go(i32),
    /// Reload the current browser document or rebuild the current native route.
    Reload,
}

/// Built-in action used by semantic links to enter the normal effect pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationRequested {
    /// Operation for the active shell to apply.
    pub command: NavigationCommand,
}

impl NavigationRequested {
    pub fn new(command: NavigationCommand) -> Self {
        Self { command }
    }
}

impl Action for NavigationRequested {
    fn static_id() -> ActionId {
        lazy_static! {
            static ref ID: ActionId = ActionId::from_name("fission_core::NavigationRequested");
        }
        *ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LinkTarget, Runtime, WidgetId};

    #[test]
    fn built_in_navigation_action_queues_without_an_application_reducer() {
        let mut runtime = Runtime::default();
        let command =
            NavigationCommand::Open(Hyperlink::new("/projects/42").target(LinkTarget::NewWindow));

        runtime
            .dispatch(
                NavigationRequested::new(command.clone()).into(),
                WidgetId::explicit("projects.link"),
            )
            .expect("built-in navigation action should dispatch");

        assert_eq!(runtime.take_pending_navigation(), vec![command]);
    }

    #[test]
    fn navigation_action_round_trips_complete_hyperlink_metadata() {
        let request = NavigationRequested::new(NavigationCommand::Open(
            Hyperlink::new("/report")
                .target(LinkTarget::Named("preview".into()))
                .rel("alternate")
                .download("report.pdf"),
        ));
        let envelope: crate::ActionEnvelope = request.clone().into();

        assert_eq!(
            serde_json::from_slice::<NavigationRequested>(&envelope.payload).unwrap(),
            request
        );
    }
}
