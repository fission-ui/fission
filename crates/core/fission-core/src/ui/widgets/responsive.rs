use crate::authoring::Lower;
use crate::build;
use crate::lowering::{IrBuilder, LoweringContext};
use crate::ui::Widget;
use fission_ir::{
    op::{ResponsiveCondition, ResponsiveQuery},
    LayoutOp, Op, WidgetId,
};
use serde::{Deserialize, Serialize};

/// A declarative responsive branch selected for a width range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveCase {
    /// Inclusive minimum width for this branch.
    pub min_width: Option<f32>,
    /// Exclusive maximum width for this branch.
    pub max_width: Option<f32>,
    /// Child rendered when the width falls inside this branch.
    pub child: Widget,
}

impl ResponsiveCase {
    /// Matches widths greater than or equal to `min_width`.
    pub fn min_width(min_width: f32, child: impl Into<Widget>) -> Self {
        Self {
            min_width: Some(min_width),
            max_width: None,
            child: child.into(),
        }
    }

    /// Matches widths below `max_width`.
    pub fn max_width(max_width: f32, child: impl Into<Widget>) -> Self {
        Self {
            min_width: None,
            max_width: Some(max_width),
            child: child.into(),
        }
    }

    /// Matches an inclusive lower and exclusive upper width range.
    pub fn between(min_width: f32, max_width: f32, child: impl Into<Widget>) -> Self {
        Self {
            min_width: Some(min_width),
            max_width: Some(max_width),
            child: child.into(),
        }
    }

    fn condition(&self) -> ResponsiveCondition {
        ResponsiveCondition {
            min_width: self.min_width,
            max_width: self.max_width,
        }
    }
}

/// Selects a widget branch using declarative viewport or container breakpoints.
///
/// Prefer `Responsive` over manually reading `view.viewport_size()` when the
/// rule is only "render this child below or above this width". Keep viewport
/// reads for cases that also depend on app state, environment values, or
/// measured geometry.
///
/// # Example
///
/// ```rust,ignore
/// Responsive::new_with_id(WidgetId::explicit("mail.responsive"), || PhoneShell)
///     .case_between_with(900.0, 1200.0, || TabletShell)
///     .case_min_width_with(1200.0, || DesktopShell)
///     .into()
/// ```
///
/// Every branch is lowered, even though layout displays only the selected one.
/// The identity-aware constructors build each branch beneath an independent
/// retained-state scope. When using [`Responsive::new`] instead, build each
/// branch independently rather than cloning a `Widget` value, and keep explicit
/// widget IDs unique across the complete responsive tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Responsive {
    /// Optional stable identity for diagnostics and retained child identity.
    pub id: Option<WidgetId>,
    /// Width source used to evaluate the responsive cases.
    pub query: ResponsiveQuery,
    /// Ordered cases; the first matching case wins.
    pub cases: Vec<ResponsiveCase>,
    /// Child rendered when no case matches.
    pub fallback: Widget,
}

impl Responsive {
    /// Creates a responsive switch with a required fallback branch.
    pub fn new(fallback: impl Into<Widget>) -> Self {
        Self {
            id: None,
            query: ResponsiveQuery::Viewport,
            cases: Vec::new(),
            fallback: fallback.into(),
        }
    }

    /// Creates a responsive switch while constructing its fallback beneath a
    /// stable branch identity.
    ///
    /// Use this form when a branch contains components with retained local
    /// state. Supplying the responsive widget identity up front lets Fission
    /// give independently constructed branches distinct, deterministic scopes.
    /// The ordinary [`Responsive::new`] constructor remains suitable for
    /// branches that are already widgets or do not own retained state.
    pub fn new_with_id<W>(id: WidgetId, fallback: impl FnOnce() -> W) -> Self
    where
        W: Into<Widget>,
    {
        let branch_id = WidgetId::scoped(id.as_u128(), "responsive:fallback");
        let fallback = build::with_implicit_widget_id(branch_id, || fallback().into());
        Self {
            id: Some(id),
            query: ResponsiveQuery::Viewport,
            cases: Vec::new(),
            fallback,
        }
    }

    /// Sets an explicit identity for deterministic inspection and diffing.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Appends an ordered responsive case.
    pub fn case(mut self, case: ResponsiveCase) -> Self {
        self.cases.push(case);
        self
    }

    /// Adds a minimum-width branch constructed beneath an identity derived
    /// from this responsive widget and the breakpoint.
    pub fn case_min_width_with<W>(mut self, min_width: f32, child: impl FnOnce() -> W) -> Self
    where
        W: Into<Widget>,
    {
        let branch_id = self.branch_id(Some(min_width), None);
        let child = build::with_implicit_widget_id(branch_id, || child().into());
        self.cases.push(ResponsiveCase::min_width(min_width, child));
        self
    }

    /// Adds a maximum-width branch constructed beneath an identity derived
    /// from this responsive widget and the breakpoint.
    pub fn case_max_width_with<W>(mut self, max_width: f32, child: impl FnOnce() -> W) -> Self
    where
        W: Into<Widget>,
    {
        let branch_id = self.branch_id(None, Some(max_width));
        let child = build::with_implicit_widget_id(branch_id, || child().into());
        self.cases.push(ResponsiveCase::max_width(max_width, child));
        self
    }

    /// Adds a bounded-width branch constructed beneath an identity derived
    /// from this responsive widget and its breakpoints.
    pub fn case_between_with<W>(
        mut self,
        min_width: f32,
        max_width: f32,
        child: impl FnOnce() -> W,
    ) -> Self
    where
        W: Into<Widget>,
    {
        let branch_id = self.branch_id(Some(min_width), Some(max_width));
        let child = build::with_implicit_widget_id(branch_id, || child().into());
        self.cases
            .push(ResponsiveCase::between(min_width, max_width, child));
        self
    }

    /// Selects viewport or parent-container width evaluation.
    pub fn query(mut self, query: ResponsiveQuery) -> Self {
        self.query = query;
        self
    }

    /// Evaluates cases against the immediate parent constraints.
    pub fn container_query(self) -> Self {
        self.query(ResponsiveQuery::Container)
    }

    fn branch_id(&self, min_width: Option<f32>, max_width: Option<f32>) -> WidgetId {
        let id = self
            .id
            .expect("identity-aware responsive branches require a responsive widget id");
        let min = min_width
            .map(|width| width.to_bits().to_string())
            .unwrap_or_else(|| "none".into());
        let max = max_width
            .map(|width| width.to_bits().to_string())
            .unwrap_or_else(|| "none".into());
        WidgetId::scoped(
            id.as_u128(),
            &format!("responsive:case:min={min}:max={max}"),
        )
    }
}

impl Lower for Responsive {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        let children = cx.with_scope(id, |cx| {
            let mut children = Vec::with_capacity(self.cases.len() + 1);
            for case in &self.cases {
                children.push(case.child.lower(cx));
            }
            children.push(self.fallback.lower(cx));
            children
        });

        let mut builder = IrBuilder::new(
            id,
            Op::Layout(LayoutOp::Responsive {
                query: self.query,
                cases: self.cases.iter().map(ResponsiveCase::condition).collect(),
            }),
        );
        for child in children {
            builder.add_child(child);
        }
        builder.build(cx)
    }
}
