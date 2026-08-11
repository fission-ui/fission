use std::fmt;

use fission_ir::WidgetId;
use fission_render::capabilities::{DisplayOpKind, ExternalSurfaceTransport};
use fission_render::external_surface::{DuplicateExternalSurfaceBinding, ExternalSurfaceSlotId};
use fission_render::frame::{FrameGateError, FrameId};
use fission_render::surface::InvalidScaleFactor;

use super::placement::SurfacePlacementIssue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlatformSurfaceSemantic {
    RectangularClip,
    AffineTransform,
    Opacity,
    PaintOrder,
}

impl fmt::Display for PlatformSurfaceSemantic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RectangularClip => formatter.write_str("rectangular clipping"),
            Self::AffineTransform => formatter.write_str("rotation, shear, or reflection"),
            Self::Opacity => formatter.write_str("retained opacity"),
            Self::PaintOrder => formatter.write_str("overlapping platform-view paint order"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceOrderingIssue {
    TwoDPaintAfterDeferredSurface {
        transport: ExternalSurfaceTransport,
        operation: DisplayOpKind,
    },
    NativeViewBeforeDirectTarget {
        native_slot_id: ExternalSurfaceSlotId,
    },
}

impl fmt::Display for SurfaceOrderingIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TwoDPaintAfterDeferredSurface {
                transport,
                operation,
            } => write!(
                formatter,
                "later overlapping {operation:?} paint; the {transport:?} surface is presented after 2D encoding"
            ),
            Self::NativeViewBeforeDirectTarget { native_slot_id } => write!(
                formatter,
                "native-view slot {} before it; native views are committed after the direct-target pass",
                native_slot_id.0
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) enum FrameSubmissionError {
    CounterExhausted(&'static str),
    NonMonotonicCommit {
        previous: Option<FrameId>,
        attempted: FrameId,
    },
    InvalidScaleFactor(InvalidScaleFactor),
    MissingProducerDescriptor(ExternalSurfaceSlotId),
    SurfaceSlotCollision {
        slot_id: ExternalSurfaceSlotId,
        first: WidgetId,
        second: WidgetId,
    },
    DuplicateProducerFrame {
        widget_id: WidgetId,
        kind: &'static str,
    },
    UnclaimedCustomSurface {
        widget_id: WidgetId,
        slot_id: ExternalSurfaceSlotId,
    },
    UnavailablePlatformPresenter {
        widget_id: WidgetId,
        slot_id: ExternalSurfaceSlotId,
        kind: &'static str,
    },
    #[cfg(feature = "three-d")]
    InvalidThreeDSubmission {
        widget_id: WidgetId,
        error: fission_3d_model::Scene3DSubmissionError,
    },
    UnsupportedDirectTargetPlacement {
        widget_id: WidgetId,
        slot_id: ExternalSurfaceSlotId,
        issue: SurfacePlacementIssue,
    },
    UnsupportedNativeViewPlacement {
        widget_id: WidgetId,
        slot_id: ExternalSurfaceSlotId,
        issue: SurfacePlacementIssue,
    },
    SurfacePaintOrderExhausted {
        widget_id: WidgetId,
        slot_id: ExternalSurfaceSlotId,
        paint_order: u64,
    },
    UnsupportedPlatformSurfaceSemantics {
        widget_id: WidgetId,
        slot_id: ExternalSurfaceSlotId,
        kind: &'static str,
        semantic: PlatformSurfaceSemantic,
    },
    UnsupportedSurfaceOrdering {
        slot_id: ExternalSurfaceSlotId,
        issue: SurfaceOrderingIssue,
    },
    DuplicateBinding(DuplicateExternalSurfaceBinding),
    FrameGate(FrameGateError),
}

impl fmt::Display for FrameSubmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CounterExhausted(name) => {
                write!(formatter, "interactive frame {name} counter is exhausted")
            }
            Self::NonMonotonicCommit {
                previous,
                attempted,
            } => write!(
                formatter,
                "interactive frame {} cannot commit after frame {:?}",
                attempted.0,
                previous.map(|frame_id| frame_id.0)
            ),
            Self::InvalidScaleFactor(error) => error.fmt(formatter),
            Self::MissingProducerDescriptor(slot_id) => write!(
                formatter,
                "render scene surface slot {} has no matching Video, Web, or Custom embed",
                slot_id.0
            ),
            Self::SurfaceSlotCollision {
                slot_id,
                first,
                second,
            } => write!(
                formatter,
                "surface slot {} collides between widgets {} and {}",
                slot_id.0,
                first.as_u128(),
                second.as_u128()
            ),
            Self::DuplicateProducerFrame { widget_id, kind } => write!(
                formatter,
                "{kind} producer emitted more than one frame for widget {}",
                widget_id.as_u128()
            ),
            Self::UnclaimedCustomSurface { widget_id, slot_id } => write!(
                formatter,
                "custom surface slot {} for widget {} has no registered native-view handler or built-in 3D producer",
                slot_id.0,
                widget_id.as_u128()
            ),
            Self::UnavailablePlatformPresenter {
                widget_id,
                slot_id,
                kind,
            } => write!(
                formatter,
                "{kind} surface slot {} for widget {} has producer data but no active platform presenter",
                slot_id.0,
                widget_id.as_u128()
            ),
            #[cfg(feature = "three-d")]
            Self::InvalidThreeDSubmission { widget_id, error } => write!(
                formatter,
                "Fission 3D surface for widget {} has a recognized but invalid submission: {error}",
                widget_id.as_u128()
            ),
            Self::UnsupportedDirectTargetPlacement {
                widget_id,
                slot_id,
                issue,
            } => write!(
                formatter,
                "direct-target 3D surface slot {} for widget {} uses {issue}",
                slot_id.0,
                widget_id.as_u128()
            ),
            Self::UnsupportedNativeViewPlacement {
                widget_id,
                slot_id,
                issue,
            } => write!(
                formatter,
                "native-view surface slot {} for widget {} uses {issue}",
                slot_id.0,
                widget_id.as_u128()
            ),
            Self::SurfacePaintOrderExhausted {
                widget_id,
                slot_id,
                paint_order,
            } => write!(
                formatter,
                "surface slot {} for widget {} has paint order {paint_order}, which exceeds the platform-view contract",
                slot_id.0,
                widget_id.as_u128()
            ),
            Self::UnsupportedPlatformSurfaceSemantics {
                widget_id,
                slot_id,
                kind,
                semantic,
            } => write!(
                formatter,
                "{kind} surface slot {} for widget {} requires {semantic}, which its active platform presenter does not support",
                slot_id.0,
                widget_id.as_u128()
            ),
            Self::UnsupportedSurfaceOrdering { slot_id, issue } => write!(
                formatter,
                "surface slot {} cannot preserve retained scene ordering because it has {issue}",
                slot_id.0
            ),
            Self::DuplicateBinding(error) => error.fmt(formatter),
            Self::FrameGate(FrameGateError::InvalidFrame(error)) => write!(
                formatter,
                "interactive frame integrity failed before encoding: {error}"
            ),
            Self::FrameGate(FrameGateError::UnsupportedOperations(error)) => {
                write!(formatter, "{error}")?;
                if let Some(first) = error.unsupported_operations.first() {
                    write!(
                        formatter,
                        "; first unsupported operation is {:?} at {:?}",
                        first.operation, first.provenance.source
                    )?;
                }
                Ok(())
            }
            Self::FrameGate(FrameGateError::UnsupportedExternalSurfaces(error)) => {
                write!(formatter, "{error}")?;
                if let Some(first) = error.unsupported_bindings.first() {
                    write!(
                        formatter,
                        "; first unsupported binding is slot {} using {:?}",
                        first.slot_id.0, first.transport
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for FrameSubmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidScaleFactor(error) => Some(error),
            Self::DuplicateBinding(error) => Some(error),
            Self::FrameGate(error) => Some(error),
            Self::CounterExhausted(_)
            | Self::NonMonotonicCommit { .. }
            | Self::MissingProducerDescriptor(_)
            | Self::SurfaceSlotCollision { .. }
            | Self::DuplicateProducerFrame { .. }
            | Self::UnclaimedCustomSurface { .. }
            | Self::UnavailablePlatformPresenter { .. }
            | Self::UnsupportedDirectTargetPlacement { .. }
            | Self::UnsupportedNativeViewPlacement { .. }
            | Self::SurfacePaintOrderExhausted { .. }
            | Self::UnsupportedPlatformSurfaceSemantics { .. }
            | Self::UnsupportedSurfaceOrdering { .. } => None,
            #[cfg(feature = "three-d")]
            Self::InvalidThreeDSubmission { error, .. } => Some(error),
        }
    }
}
