use crate::{Center, CircularProgress, CircularProgressMotion};
use fission_core::ui::{Text, Widget};
use fission_core::{RouteBuildOutcome, RouteDecision, WidgetId};

/// Lazily selects the component that represents one protected route.
///
/// Component values are retained without converting them to [`Widget`] until
/// their branch is selected. A pending, denied, or redirecting route therefore
/// cannot register resources or reducers from its protected component.
#[derive(Clone, Debug)]
pub struct ProtectedRoute<A, P = DefaultRoutePending, D = DefaultRouteDenied> {
    pub decision: RouteDecision,
    pub allowed: A,
    pub pending: P,
    pub denied: D,
}

impl<A> ProtectedRoute<A> {
    pub fn new(decision: RouteDecision, allowed: A) -> Self {
        Self {
            decision,
            allowed,
            pending: DefaultRoutePending,
            denied: DefaultRouteDenied,
        }
    }
}

impl<A, P, D> ProtectedRoute<A, P, D> {
    pub fn pending<P2>(self, pending: P2) -> ProtectedRoute<A, P2, D> {
        ProtectedRoute {
            decision: self.decision,
            allowed: self.allowed,
            pending,
            denied: self.denied,
        }
    }

    pub fn denied<D2>(self, denied: D2) -> ProtectedRoute<A, P, D2> {
        ProtectedRoute {
            decision: self.decision,
            allowed: self.allowed,
            pending: self.pending,
            denied,
        }
    }
}

impl<A, P, D> From<ProtectedRoute<A, P, D>> for Widget
where
    A: Into<Widget>,
    P: Into<Widget>,
    D: Into<Widget>,
{
    fn from(route: ProtectedRoute<A, P, D>) -> Self {
        let (ctx, _) = fission_core::build::current::<()>();
        match route.decision {
            RouteDecision::Allow => route.allowed.into(),
            RouteDecision::Pending => route.pending.into(),
            RouteDecision::Deny => {
                ctx.declare_route_outcome(RouteBuildOutcome::Denied);
                route.denied.into()
            }
            RouteDecision::Redirect(redirect) => {
                ctx.declare_route_outcome(RouteBuildOutcome::Redirect(redirect));
                route.pending.into()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultRoutePending;

impl From<DefaultRoutePending> for Widget {
    fn from(_: DefaultRoutePending) -> Self {
        Center {
            child: CircularProgress {
                id: WidgetId::explicit("fission.protected_route.pending"),
                motion: Some(CircularProgressMotion::Default),
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultRouteDenied;

impl From<DefaultRouteDenied> for Widget {
    fn from(_: DefaultRouteDenied) -> Self {
        Center {
            child: Text::new("Access denied").into(),
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::internal::BuildCtx;
    use fission_core::{Env, GlobalState, RouteRedirect, RuntimeState, View};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Debug, Default)]
    struct State;
    impl GlobalState for State {}

    #[derive(Clone)]
    struct Counted {
        count: Arc<AtomicUsize>,
        label: &'static str,
    }

    impl From<Counted> for Widget {
        fn from(component: Counted) -> Self {
            component.count.fetch_add(1, Ordering::SeqCst);
            Text::new(component.label).into()
        }
    }

    fn build(decision: RouteDecision) -> (Vec<usize>, Option<RouteBuildOutcome>) {
        let counts = [(); 3].map(|_| Arc::new(AtomicUsize::new(0)));
        let state = State;
        let runtime = RuntimeState::default();
        let env = Env::default();
        let view = View::new(&state, &runtime, &env, None);
        let mut ctx = BuildCtx::<State>::new();
        let route = ProtectedRoute::new(
            decision,
            Counted {
                count: counts[0].clone(),
                label: "allowed",
            },
        )
        .pending(Counted {
            count: counts[1].clone(),
            label: "pending",
        })
        .denied(Counted {
            count: counts[2].clone(),
            label: "denied",
        });
        let _ = fission_core::build::enter(&mut ctx, &view, || -> Widget { route.into() });
        let values = counts
            .iter()
            .map(|count| count.load(Ordering::SeqCst))
            .collect();
        (values, ctx.take_route_outcome())
    }

    #[test]
    fn only_the_allowed_component_is_converted() {
        let (counts, outcome) = build(RouteDecision::Allow);
        assert_eq!(counts, vec![1, 0, 0]);
        assert_eq!(outcome, None);
    }

    #[test]
    fn pending_and_denied_components_are_lazy() {
        let (pending, pending_outcome) = build(RouteDecision::Pending);
        assert_eq!(pending, vec![0, 1, 0]);
        assert_eq!(pending_outcome, None);

        let (denied, denied_outcome) = build(RouteDecision::Deny);
        assert_eq!(denied, vec![0, 0, 1]);
        assert_eq!(denied_outcome, Some(RouteBuildOutcome::Denied));
    }

    #[test]
    fn redirect_converts_only_pending_and_declares_navigation() {
        let redirect = RouteRedirect::replace("/sign-in");
        let (counts, outcome) = build(redirect.clone().into());
        assert_eq!(counts, vec![0, 1, 0]);
        assert_eq!(outcome, Some(RouteBuildOutcome::Redirect(redirect)));
    }
}
