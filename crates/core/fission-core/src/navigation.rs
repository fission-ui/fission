//! Shell-neutral navigation requests and hyperlink activation.

use crate::{Action, ActionId, RouteLocation};
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

/// History behavior for a declarative protected-route redirect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteRedirectHistory {
    /// Replace the attempted route so browser back does not immediately retry it.
    #[default]
    Replace,
    /// Retain the attempted route and push the redirect destination.
    Push,
}

/// A shell-neutral redirect declared while building a protected route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteRedirect {
    pub destination: String,
    pub history: RouteRedirectHistory,
}

impl RouteRedirect {
    /// Creates a replace-history redirect, the safe default for access checks.
    pub fn replace(destination: impl Into<String>) -> Self {
        Self {
            destination: destination.into(),
            history: RouteRedirectHistory::Replace,
        }
    }

    /// Creates a push-history redirect.
    pub fn push(destination: impl Into<String>) -> Self {
        Self {
            destination: destination.into(),
            history: RouteRedirectHistory::Push,
        }
    }

    /// Adds the current logical route as an encoded `return_to` query value.
    pub fn return_to(mut self, location: &RouteLocation) -> Self {
        let route = location.logical_route();
        self.destination = append_query_parameter(
            &self.destination,
            "return_to",
            &percent_encode_query_value(&route),
        );
        self
    }

    pub fn navigation_command(&self) -> NavigationCommand {
        match self.history {
            RouteRedirectHistory::Replace => NavigationCommand::Replace(self.destination.clone()),
            RouteRedirectHistory::Push => NavigationCommand::Push(self.destination.clone()),
        }
    }
}

/// Declarative access result consumed by `ProtectedRoute`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteDecision {
    /// Authentication or another prerequisite is still being resolved.
    #[default]
    Pending,
    /// Build the protected route.
    Allow,
    /// Build the denied branch and mark the route forbidden where supported.
    Deny,
    /// Build the pending branch while the shell redirects.
    Redirect(RouteRedirect),
}

impl From<RouteRedirect> for RouteDecision {
    fn from(redirect: RouteRedirect) -> Self {
        Self::Redirect(redirect)
    }
}

/// Route result declared by the active authoring tree for its host shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteBuildOutcome {
    Denied,
    Redirect(RouteRedirect),
}

fn append_query_parameter(destination: &str, name: &str, encoded_value: &str) -> String {
    let (base, fragment) = destination
        .split_once('#')
        .map_or((destination, None), |(base, fragment)| {
            (base, Some(fragment))
        });
    let separator = if base.contains('?') { '&' } else { '?' };
    let mut result = format!("{base}{separator}{name}={encoded_value}");
    if let Some(fragment) = fragment {
        result.push('#');
        result.push_str(fragment);
    }
    result
}

fn percent_encode_query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
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

    #[test]
    fn protected_route_redirect_defaults_to_replace_and_encodes_return_location() {
        let location = RouteLocation::from_route("/account/billing?tab=invoices#overdue");
        let redirect = RouteRedirect::replace("/sign-in?source=guard").return_to(&location);

        assert_eq!(
            redirect.destination,
            "/sign-in?source=guard&return_to=%2Faccount%2Fbilling%3Ftab%3Dinvoices%23overdue"
        );
        assert_eq!(
            redirect.navigation_command(),
            NavigationCommand::Replace(redirect.destination.clone())
        );
    }

    #[test]
    fn return_location_is_inserted_before_a_destination_fragment() {
        let redirect = RouteRedirect::replace("/sign-in#form")
            .return_to(&RouteLocation::from_route("/account"));
        assert_eq!(redirect.destination, "/sign-in?return_to=%2Faccount#form");
    }
}
