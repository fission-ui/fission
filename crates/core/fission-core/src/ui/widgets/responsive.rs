use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use fission_ir::{
    op::{ResponsiveCondition, ResponsiveQuery},
    LayoutOp, Op, WidgetId,
};
use serde::{Deserialize, Serialize};

/// A declarative responsive branch selected for a width range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveCase {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Responsive {
    pub id: Option<WidgetId>,
    pub query: ResponsiveQuery,
    pub cases: Vec<ResponsiveCase>,
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

    /// Selects viewport or parent-container width evaluation.
    pub fn query(mut self, query: ResponsiveQuery) -> Self {
        self.query = query;
        self
    }

    /// Evaluates cases against the immediate parent constraints.
    pub fn container_query(self) -> Self {
        self.query(ResponsiveQuery::Container)
    }
}

impl InternalLower for Responsive {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);
        let mut children = Vec::with_capacity(self.cases.len() + 1);
        for case in &self.cases {
            children.push(case.child.lower(cx));
        }
        children.push(self.fallback.lower(cx));
        cx.pop_scope();

        let mut builder = InternalIrBuilder::new(
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
