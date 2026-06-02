use fission_ir::op::{Fill, LayoutOp, PaintOp};
use fission_ir::{CoreIR, Op, WidgetId};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSupportError {
    pub node_id: WidgetId,
    pub reason: String,
}

impl fmt::Display for TerminalSupportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "terminal shell cannot lower node {:?}: {}",
            self.node_id, self.reason
        )
    }
}

impl std::error::Error for TerminalSupportError {}

pub fn verify_terminal_ir(ir: &CoreIR) -> Result<(), TerminalSupportError> {
    for (node_id, node) in &ir.nodes {
        match &node.op {
            Op::Layout(layout) => verify_layout(*node_id, layout)?,
            Op::Paint(paint) => verify_paint(*node_id, paint)?,
            Op::Structural(_) | Op::Semantics(_) => {}
        }
    }
    Ok(())
}

fn verify_layout(node_id: WidgetId, layout: &LayoutOp) -> Result<(), TerminalSupportError> {
    match layout {
        LayoutOp::Box { .. }
        | LayoutOp::Flex { .. }
        | LayoutOp::Grid { .. }
        | LayoutOp::GridItem { .. }
        | LayoutOp::Scroll { .. }
        | LayoutOp::AbsoluteFill
        | LayoutOp::Positioned { .. }
        | LayoutOp::ZStack
        | LayoutOp::Align => Ok(()),
        LayoutOp::Clip { path: None } => Ok(()),
        LayoutOp::Clip { path: Some(_) } => Err(unsupported(
            node_id,
            "path clipping is not representable in a terminal",
        )),
        LayoutOp::Embed { kind, .. } => Err(unsupported(
            node_id,
            format!("embedded surface `{kind:?}` requires a non-terminal platform surface"),
        )),
        LayoutOp::Flyout { .. } => Err(unsupported(
            node_id,
            "flyout placement is not yet supported by the terminal shell",
        )),
        LayoutOp::Transform { .. } => Err(unsupported(
            node_id,
            "matrix transforms are not representable in a terminal cell grid",
        )),
    }
}

fn verify_paint(node_id: WidgetId, paint: &PaintOp) -> Result<(), TerminalSupportError> {
    match paint {
        PaintOp::DrawRect { fill, stroke, .. } => {
            if fill.as_ref().is_some_and(is_non_solid_fill) {
                return Err(unsupported(
                    node_id,
                    "gradient fills are not representable in the terminal shell",
                ));
            }
            if stroke
                .as_ref()
                .is_some_and(|stroke| is_non_solid_fill(&stroke.fill))
            {
                return Err(unsupported(
                    node_id,
                    "gradient strokes are not representable in the terminal shell",
                ));
            }
            Ok(())
        }
        PaintOp::DrawText { .. } | PaintOp::DrawRichText { .. } => Ok(()),
        PaintOp::DrawImage { .. } => Err(unsupported(
            node_id,
            "images require a graphical shell or an explicit terminal representation",
        )),
        PaintOp::DrawPath { .. } => Err(unsupported(
            node_id,
            "vector paths require a graphical shell or an explicit terminal representation",
        )),
        PaintOp::DrawSvg { .. } => Err(unsupported(
            node_id,
            "SVG content requires a graphical shell or an explicit terminal representation",
        )),
    }
}

fn is_non_solid_fill(fill: &Fill) -> bool {
    !matches!(fill, Fill::Solid(_))
}

fn unsupported(node_id: WidgetId, reason: impl Into<String>) -> TerminalSupportError {
    TerminalSupportError {
        node_id,
        reason: reason.into(),
    }
}
