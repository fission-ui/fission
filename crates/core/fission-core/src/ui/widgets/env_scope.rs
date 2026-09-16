use crate::authoring::{custom_widget, LowerWidget};
use crate::env::Env;
use crate::lowering::LoweringContext;
use crate::ui::Widget;
use fission_ir::WidgetId;
use std::fmt;
use std::sync::Arc;

/// Lowers a subtree against its own environment.
///
/// A widget tree is built against a [`View`](crate::View) and then lowered
/// against the host's [`Env`]. When part of the tree should use another theme,
/// density or locale (a live preview of a design system inside a tool, say),
/// build that part with the scoped environment and wrap it in `EnvScope`. The
/// recipe values widgets read while lowering then come from the same
/// environment the subtree was built with, so its controls, spacing and text
/// agree with each other.
///
/// The scope adds no layout or paint of its own.
#[derive(Clone)]
pub struct EnvScope {
    pub env: Arc<Env>,
    pub child: Widget,
}

impl EnvScope {
    pub fn new(env: impl Into<Arc<Env>>, child: impl Into<Widget>) -> Self {
        Self {
            env: env.into(),
            child: child.into(),
        }
    }
}

impl fmt::Debug for EnvScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnvScope")
            .field("child", &self.child)
            .finish_non_exhaustive()
    }
}

impl LowerWidget for EnvScope {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        cx.with_env(&self.env, |cx| {
            crate::internal::lower_widget(&self.child, cx)
        })
    }

    fn children(&self) -> Vec<&Widget> {
        vec![&self.child]
    }
}

impl From<EnvScope> for Widget {
    fn from(scope: EnvScope) -> Self {
        custom_widget("EnvScope", scope)
    }
}
