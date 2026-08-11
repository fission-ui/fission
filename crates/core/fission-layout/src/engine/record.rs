use fission_diagnostics::prelude as diag;
use fission_ir::WidgetId;
use std::collections::HashMap;

use super::LayoutEngine;
use crate::{LayoutNodeGeometry, LayoutPoint, LayoutRect, LayoutSize};

impl LayoutEngine {
    pub(super) fn record_geometry(
        &self,
        node_id: WidgetId,
        origin: LayoutPoint,
        size: LayoutSize,
        content_size: LayoutSize,
        out: &mut HashMap<WidgetId, LayoutNodeGeometry>,
        record: bool,
    ) -> LayoutSize {
        let mut rect_origin = origin;
        let mut rect_size = size;
        let mut rect_content = content_size;
        let mut had_non_finite = false;

        if !rect_origin.x.is_finite() {
            rect_origin.x = 0.0;
            had_non_finite = true;
        }
        if !rect_origin.y.is_finite() {
            rect_origin.y = 0.0;
            had_non_finite = true;
        }
        if !rect_size.width.is_finite() {
            rect_size.width = 0.0;
            had_non_finite = true;
        }
        if !rect_size.height.is_finite() {
            rect_size.height = 0.0;
            had_non_finite = true;
        }
        if !rect_content.width.is_finite() {
            rect_content.width = 0.0;
            had_non_finite = true;
        }
        if !rect_content.height.is_finite() {
            rect_content.height = 0.0;
            had_non_finite = true;
        }

        if had_non_finite {
            diag::emit(
                diag::DiagCategory::Invariants,
                diag::DiagLevel::Error,
                diag::DiagEventKind::InvariantViolation {
                    kind: "non_finite_layout".into(),
                    node: Some(node_id.as_u128()),
                    details: format!(
                        "origin=({:.2},{:.2}) size=({:.2},{:.2}) content=({:.2},{:.2})",
                        origin.x,
                        origin.y,
                        size.width,
                        size.height,
                        content_size.width,
                        content_size.height
                    ),
                    dump_ref: None,
                },
            );
        }

        if record {
            let rect = LayoutRect::new(
                rect_origin.x,
                rect_origin.y,
                rect_size.width,
                rect_size.height,
            );
            out.insert(
                node_id,
                LayoutNodeGeometry {
                    rect,
                    content_size: rect_content,
                },
            );
        }
        rect_size
    }
}
