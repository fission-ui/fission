use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::capabilities::{BackendIdentity, ExternalSurfaceTransport, GraphicsCapabilities};
use crate::conformance::{
    validate_scene_capabilities, DisplayOpProvenance, FrameConformanceError, FrameOperationSource,
};
use crate::external_surface::{
    ExternalFrameState, ExternalSurfaceBindings, ExternalSurfaceSlotId,
    InvalidExternalSurfaceBinding,
};
use crate::frame_geometry::validate_frame_geometry;
pub use crate::frame_geometry::{
    FrameGeometryElement, FrameGeometryError, FrameGeometryProblem, FrameGeometrySource,
};
use crate::resource::{ResourceSnapshot, ResourceSnapshotError};
use crate::surface::{PhysicalSize, ScaleFactor};
use crate::{Color, DisplayList, DisplayOp, LayoutRect, LayoutSize, RenderNode, RenderScene};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FrameId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceEpoch(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SemanticsEpoch(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameViewport {
    pub logical_size: LayoutSize,
    pub physical_size: PhysicalSize,
    pub scale_factor: ScaleFactor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DamageRegion {
    None,
    Full,
    Rects(Vec<LayoutRect>),
}

impl DamageRegion {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::None => true,
            Self::Full => false,
            Self::Rects(rects) => rects.is_empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameMetadata {
    pub frame_id: FrameId,
    pub viewport: FrameViewport,
    pub damage: DamageRegion,
    pub resource_epoch: ResourceEpoch,
    pub semantics_epoch: SemanticsEpoch,
}

/// Immutable submission passed to an interactive graphics session.
///
/// `RenderScene` remains the single paint and placement authority. This type
/// binds it to per-frame lifecycle metadata, immutable resource state, and
/// external producer state; it is not a second scene representation.
#[derive(Debug, Clone, Copy)]
pub struct InteractiveFrame<'a> {
    scene: &'a RenderScene,
    metadata: &'a FrameMetadata,
    resources: &'a ResourceSnapshot,
    external_surface_bindings: &'a ExternalSurfaceBindings,
    clear_color: Color,
}

impl<'a> InteractiveFrame<'a> {
    pub fn new(
        scene: &'a RenderScene,
        metadata: &'a FrameMetadata,
        resources: &'a ResourceSnapshot,
        external_surface_bindings: &'a ExternalSurfaceBindings,
    ) -> Self {
        Self {
            scene,
            metadata,
            resources,
            external_surface_bindings,
            clear_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
        }
    }

    /// Set the color used to initialize pixels not covered by scene paint.
    ///
    /// Headless callers keep the transparent default from [`Self::new`].
    /// Interactive shells should pass their resolved design-system background
    /// so every graphics backend receives the same complete-frame input.
    pub fn with_clear_color(mut self, clear_color: Color) -> Self {
        self.clear_color = clear_color;
        self
    }

    pub fn scene(self) -> &'a RenderScene {
        self.scene
    }

    pub fn metadata(self) -> &'a FrameMetadata {
        self.metadata
    }

    pub fn resources(self) -> &'a ResourceSnapshot {
        self.resources
    }

    pub fn external_surface_bindings(self) -> &'a ExternalSurfaceBindings {
        self.external_surface_bindings
    }

    pub fn clear_color(self) -> Color {
        self.clear_color
    }

    pub fn validate(self) -> Result<(), FrameValidationError> {
        validate_frame_geometry(self.metadata, self.scene)
            .map_err(FrameValidationError::InvalidGeometry)?;
        self.resources
            .validate()
            .map_err(FrameValidationError::InvalidResourceSnapshot)?;
        if self.metadata.resource_epoch != self.resources.epoch() {
            return Err(FrameValidationError::ResourceEpochMismatch {
                metadata: self.metadata.resource_epoch,
                snapshot: self.resources.epoch(),
            });
        }
        self.external_surface_bindings
            .validate()
            .map_err(FrameValidationError::InvalidSurfaceBinding)?;

        let placements = collect_scene_surface_placements(self.metadata.frame_id, self.scene);

        if let Some((&slot_id, _)) = placements
            .iter()
            .find(|(_, placements)| placements.len() > 1)
        {
            return Err(FrameValidationError::DuplicateSurfacePlacement(slot_id));
        }

        let placed: BTreeSet<_> = placements.keys().copied().collect();
        for slot_id in &placed {
            if !self.external_surface_bindings.contains(*slot_id) {
                return Err(FrameValidationError::MissingSurfaceBinding(*slot_id));
            }
        }
        for (slot_id, _) in self.external_surface_bindings {
            if !placed.contains(slot_id) {
                return Err(FrameValidationError::BindingWithoutPlacement(*slot_id));
            }
        }
        Ok(())
    }

    /// Check frame integrity and every operation against one backend
    /// capability profile without exposing the backend submission token.
    pub fn check_for(self, capabilities: &GraphicsCapabilities) -> Result<(), FrameGateError> {
        self.validate_for(capabilities).map(|_| ())
    }

    /// Construct the backend submission token after all frame gates pass.
    ///
    /// This remains crate-private so hosts cannot call implementation hooks
    /// directly or reuse a token with a different backend session.
    pub(crate) fn validate_for(
        self,
        capabilities: &GraphicsCapabilities,
    ) -> Result<ValidatedInteractiveFrame<'a>, FrameGateError> {
        self.validate().map_err(FrameGateError::InvalidFrame)?;
        validate_scene_capabilities(self.metadata.frame_id, self.scene, capabilities)
            .map_err(FrameGateError::UnsupportedOperations)?;
        validate_external_surface_capabilities(
            self.metadata.frame_id,
            self.scene,
            self.external_surface_bindings,
            capabilities,
        )
        .map_err(FrameGateError::UnsupportedExternalSurfaces)?;

        Ok(ValidatedInteractiveFrame {
            frame: self,
            validated_for: capabilities.identity.clone(),
        })
    }
}

fn validate_external_surface_capabilities(
    frame_id: FrameId,
    scene: &RenderScene,
    bindings: &ExternalSurfaceBindings,
    capabilities: &GraphicsCapabilities,
) -> Result<(), ExternalSurfaceConformanceError> {
    let placements = collect_scene_surface_placements(frame_id, scene);
    let unsupported_bindings = bindings
        .iter()
        .filter_map(|(slot_id, binding)| {
            (binding.state == ExternalFrameState::Ready
                && !capabilities.supports_external_surface_transport(binding.transport))
            .then_some(UnsupportedExternalSurfaceBinding {
                slot_id: *slot_id,
                transport: binding.transport,
                provenance: placements
                    .get(slot_id)
                    .and_then(|placements| placements.first())
                    .cloned()
                    .expect("frame integrity gate requires one placement per binding"),
            })
        })
        .collect::<Vec<_>>();

    if unsupported_bindings.is_empty() {
        Ok(())
    } else {
        Err(ExternalSurfaceConformanceError {
            backend: capabilities.identity.clone(),
            unsupported_bindings,
        })
    }
}

/// Capability-checked frame accepted by a backend implementation hook.
///
/// Its fields are private by design; construction is only possible through the
/// Fission-owned graphics-session gate.
#[derive(Debug, Clone)]
pub struct ValidatedInteractiveFrame<'a> {
    frame: InteractiveFrame<'a>,
    validated_for: BackendIdentity,
}

impl<'a> ValidatedInteractiveFrame<'a> {
    pub fn frame(&self) -> InteractiveFrame<'a> {
        self.frame
    }

    pub fn validated_for(&self) -> &BackendIdentity {
        &self.validated_for
    }
}

fn collect_scene_surface_placements(
    frame_id: FrameId,
    scene: &RenderScene,
) -> BTreeMap<ExternalSurfaceSlotId, Vec<DisplayOpProvenance>> {
    let mut placements = BTreeMap::new();
    for (root_index, root) in scene.roots.iter().enumerate() {
        collect_node_surface_placements(
            frame_id,
            root,
            root_index,
            &mut Vec::new(),
            None,
            &mut placements,
        );
    }
    placements
}

fn collect_node_surface_placements(
    frame_id: FrameId,
    node: &RenderNode,
    root_index: usize,
    node_path: &mut Vec<usize>,
    inherited_node_id: Option<fission_ir::WidgetId>,
    placements: &mut BTreeMap<ExternalSurfaceSlotId, Vec<DisplayOpProvenance>>,
) {
    match node {
        RenderNode::Layer(layer) => {
            let node_id = layer.node_id.or(inherited_node_id);
            for (child_index, child) in layer.children.iter().enumerate() {
                node_path.push(child_index);
                collect_node_surface_placements(
                    frame_id, child, root_index, node_path, node_id, placements,
                );
                node_path.pop();
            }
        }
        RenderNode::Paint(list) => collect_display_list_surface_placements(
            frame_id,
            list,
            root_index,
            node_path,
            &mut Vec::new(),
            inherited_node_id,
            placements,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_display_list_surface_placements(
    frame_id: FrameId,
    list: &DisplayList,
    root_index: usize,
    node_path: &[usize],
    operation_path: &mut Vec<usize>,
    inherited_node_id: Option<fission_ir::WidgetId>,
    placements: &mut BTreeMap<ExternalSurfaceSlotId, Vec<DisplayOpProvenance>>,
) {
    for (operation_index, operation) in list.ops.iter().enumerate() {
        operation_path.push(operation_index);
        match operation {
            DisplayOp::DrawSurface {
                surface_id,
                bounds,
                node_id,
                ..
            } => {
                placements
                    .entry(ExternalSurfaceSlotId(*surface_id))
                    .or_default()
                    .push(DisplayOpProvenance {
                        frame_id,
                        node_id: node_id.or(inherited_node_id),
                        bounds: Some(*bounds),
                        source: FrameOperationSource::DisplayList {
                            root_index,
                            node_path: node_path.to_vec(),
                            operation_path: operation_path.clone(),
                        },
                    });
            }
            DisplayOp::CachedScene { list, .. } => collect_display_list_surface_placements(
                frame_id,
                list,
                root_index,
                node_path,
                operation_path,
                inherited_node_id,
                placements,
            ),
            _ => {}
        }
        operation_path.pop();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameValidationError {
    InvalidGeometry(FrameGeometryError),
    InvalidResourceSnapshot(ResourceSnapshotError),
    InvalidSurfaceBinding(InvalidExternalSurfaceBinding),
    ResourceEpochMismatch {
        metadata: ResourceEpoch,
        snapshot: ResourceEpoch,
    },
    DuplicateSurfacePlacement(ExternalSurfaceSlotId),
    MissingSurfaceBinding(ExternalSurfaceSlotId),
    BindingWithoutPlacement(ExternalSurfaceSlotId),
}

impl fmt::Display for FrameValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGeometry(error) => error.fmt(formatter),
            Self::InvalidResourceSnapshot(error) => error.fmt(formatter),
            Self::InvalidSurfaceBinding(error) => error.fmt(formatter),
            Self::ResourceEpochMismatch { metadata, snapshot } => write!(
                formatter,
                "frame resource epoch {} does not match snapshot epoch {}",
                metadata.0, snapshot.0
            ),
            Self::DuplicateSurfacePlacement(slot_id) => write!(
                formatter,
                "external surface slot {} has more than one placement",
                slot_id.0
            ),
            Self::MissingSurfaceBinding(slot_id) => write!(
                formatter,
                "external surface slot {} has no producer binding",
                slot_id.0
            ),
            Self::BindingWithoutPlacement(slot_id) => write!(
                formatter,
                "external surface binding {} has no frame placement",
                slot_id.0
            ),
        }
    }
}

impl std::error::Error for FrameValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidGeometry(error) => Some(error),
            Self::InvalidResourceSnapshot(error) => Some(error),
            Self::InvalidSurfaceBinding(error) => Some(error),
            Self::ResourceEpochMismatch { .. }
            | Self::DuplicateSurfacePlacement(_)
            | Self::MissingSurfaceBinding(_)
            | Self::BindingWithoutPlacement(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnsupportedExternalSurfaceBinding {
    pub slot_id: ExternalSurfaceSlotId,
    pub transport: ExternalSurfaceTransport,
    pub provenance: DisplayOpProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalSurfaceConformanceError {
    pub backend: BackendIdentity,
    pub unsupported_bindings: Vec<UnsupportedExternalSurfaceBinding>,
}

impl fmt::Display for ExternalSurfaceConformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "backend {} ({}) does not support {} external surface binding(s) in this frame",
            self.backend.name,
            self.backend.profile,
            self.unsupported_bindings.len()
        )
    }
}

impl std::error::Error for ExternalSurfaceConformanceError {}

#[derive(Debug, Clone, PartialEq)]
pub enum FrameGateError {
    InvalidFrame(FrameValidationError),
    UnsupportedOperations(FrameConformanceError),
    UnsupportedExternalSurfaces(ExternalSurfaceConformanceError),
}

impl fmt::Display for FrameGateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrame(error) => error.fmt(formatter),
            Self::UnsupportedOperations(error) => error.fmt(formatter),
            Self::UnsupportedExternalSurfaces(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for FrameGateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidFrame(error) => Some(error),
            Self::UnsupportedOperations(error) => Some(error),
            Self::UnsupportedExternalSurfaces(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::ExternalSurfaceTransport;
    use crate::external_surface::{
        ExternalAlphaType, ExternalColorSpace, ExternalFrameId, ExternalFrameState,
        ExternalFrameToken, ExternalOwnership, ExternalProducerId, ExternalProducerKind,
        ExternalSurfaceBinding, ExternalSynchronization,
    };
    use crate::resource::ResourceSnapshot;
    use crate::{LayoutPoint, LayoutRect};

    fn rect() -> LayoutRect {
        LayoutRect {
            origin: LayoutPoint { x: 0.0, y: 0.0 },
            size: LayoutSize {
                width: 100.0,
                height: 50.0,
            },
        }
    }

    fn metadata() -> FrameMetadata {
        FrameMetadata {
            frame_id: FrameId(1),
            viewport: FrameViewport {
                logical_size: rect().size,
                physical_size: PhysicalSize::new(100, 50),
                scale_factor: ScaleFactor::ONE,
            },
            damage: DamageRegion::Full,
            resource_epoch: ResourceEpoch(2),
            semantics_epoch: SemanticsEpoch(3),
        }
    }

    fn binding(slot_id: u64) -> ExternalSurfaceBinding {
        ExternalSurfaceBinding {
            slot_id: ExternalSurfaceSlotId(slot_id),
            producer_id: ExternalProducerId(1),
            producer_kind: ExternalProducerKind::Video,
            frame_id: ExternalFrameId(2),
            frame_token: Some(ExternalFrameToken(3)),
            state: ExternalFrameState::Ready,
            transport: ExternalSurfaceTransport::GpuImage,
            color_space: ExternalColorSpace::Srgb,
            alpha_type: ExternalAlphaType::Opaque,
            ownership: ExternalOwnership::BorrowedForFrame,
            synchronization: ExternalSynchronization::None,
            zero_copy: true,
            damaged: true,
        }
    }

    #[test]
    fn interactive_frame_carries_the_resolved_clear_color() {
        let scene = RenderScene::new(rect());
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let bindings = ExternalSurfaceBindings::new();
        let background = Color {
            r: 12,
            g: 34,
            b: 56,
            a: 255,
        };

        let transparent = InteractiveFrame::new(&scene, &metadata, &resources, &bindings);
        assert_eq!(
            transparent.clear_color(),
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            }
        );
        assert_eq!(
            transparent.with_clear_color(background).clear_color(),
            background
        );
    }

    #[test]
    fn frame_rejects_surface_placement_without_binding() {
        let bounds = rect();
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: 42,
            position: 0,
            bounds,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(list);
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let bindings = ExternalSurfaceBindings::new();

        let error = InteractiveFrame::new(&scene, &metadata, &resources, &bindings)
            .validate()
            .unwrap_err();

        assert_eq!(
            error,
            FrameValidationError::MissingSurfaceBinding(ExternalSurfaceSlotId(42))
        );
    }

    #[test]
    fn frame_accepts_one_placement_and_one_binding() {
        let bounds = rect();
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: 42,
            position: 0,
            bounds,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(list);
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let mut bindings = ExternalSurfaceBindings::new();
        bindings.insert(binding(42)).unwrap();

        InteractiveFrame::new(&scene, &metadata, &resources, &bindings)
            .validate()
            .unwrap();
    }

    #[test]
    fn frame_rejects_duplicate_slot_placement_across_roots_and_cached_scenes() {
        let bounds = rect();
        let mut nested = DisplayList::new(bounds);
        nested.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: 42,
            position: 0,
            bounds,
            node_id: None,
        });
        let mut cached = DisplayList::new(bounds);
        cached.push(DisplayOp::CachedScene {
            cache_key: 1,
            bounds,
            list: Box::new(nested),
        });
        let mut direct = DisplayList::new(bounds);
        direct.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: 42,
            position: 1,
            bounds,
            node_id: None,
        });
        let mut scene = RenderScene::new(bounds);
        scene.roots.push(RenderNode::Paint(cached));
        scene.roots.push(RenderNode::Paint(direct));
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let mut bindings = ExternalSurfaceBindings::new();
        bindings.insert(binding(42)).unwrap();

        let error = InteractiveFrame::new(&scene, &metadata, &resources, &bindings)
            .validate()
            .unwrap_err();

        assert_eq!(
            error,
            FrameValidationError::DuplicateSurfacePlacement(ExternalSurfaceSlotId(42))
        );
    }

    #[test]
    fn frame_rejects_a_resource_snapshot_from_another_epoch() {
        let scene = RenderScene::new(rect());
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(ResourceEpoch(metadata.resource_epoch.0 + 1));
        let bindings = ExternalSurfaceBindings::new();

        let error = InteractiveFrame::new(&scene, &metadata, &resources, &bindings)
            .validate()
            .unwrap_err();

        assert_eq!(
            error,
            FrameValidationError::ResourceEpochMismatch {
                metadata: metadata.resource_epoch,
                snapshot: resources.epoch(),
            }
        );
    }

    #[test]
    fn capability_gate_reports_every_unsupported_surface_transport() {
        let bounds = rect();
        let mut list = DisplayList::new(bounds);
        for surface_id in [7, 3] {
            list.push(DisplayOp::DrawSurface {
                rect: bounds,
                surface_id,
                position: 0,
                bounds,
                node_id: None,
            });
        }
        let scene = RenderScene::from_display_list(list);
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let mut bindings = ExternalSurfaceBindings::new();
        let mut first = binding(7);
        first.transport = ExternalSurfaceTransport::NativeView;
        bindings.insert(first).unwrap();
        bindings.insert(binding(3)).unwrap();
        let mut capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("test", "1", "native"));
        capabilities.display_ops = crate::capabilities::DisplayOpKind::ALL
            .into_iter()
            .collect();

        let error = InteractiveFrame::new(&scene, &metadata, &resources, &bindings)
            .validate_for(&capabilities)
            .unwrap_err();

        let FrameGateError::UnsupportedExternalSurfaces(error) = error else {
            panic!("expected external surface conformance error");
        };
        assert_eq!(error.unsupported_bindings.len(), 2);
        assert_eq!(
            (
                error.unsupported_bindings[0].slot_id,
                error.unsupported_bindings[0].transport,
            ),
            (ExternalSurfaceSlotId(3), ExternalSurfaceTransport::GpuImage,)
        );
        assert_eq!(
            error.unsupported_bindings[0].provenance.source,
            FrameOperationSource::DisplayList {
                root_index: 0,
                node_path: vec![],
                operation_path: vec![1],
            }
        );
        assert_eq!(
            (
                error.unsupported_bindings[1].slot_id,
                error.unsupported_bindings[1].transport,
            ),
            (
                ExternalSurfaceSlotId(7),
                ExternalSurfaceTransport::NativeView,
            )
        );
        assert_eq!(
            error.unsupported_bindings[1].provenance.source,
            FrameOperationSource::DisplayList {
                root_index: 0,
                node_path: vec![],
                operation_path: vec![0],
            }
        );
    }

    #[test]
    fn non_ready_surface_uses_host_disposition_without_transport_support() {
        let bounds = rect();
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: 7,
            position: 0,
            bounds,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(list);
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let mut pending = binding(7);
        pending.state = ExternalFrameState::Pending;
        pending.frame_token = None;
        let mut bindings = ExternalSurfaceBindings::new();
        bindings.insert(pending).unwrap();
        let mut capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("test", "1", "host-disposition"));
        capabilities.display_ops = crate::capabilities::DisplayOpKind::ALL
            .into_iter()
            .collect();

        InteractiveFrame::new(&scene, &metadata, &resources, &bindings)
            .validate_for(&capabilities)
            .unwrap();
    }

    #[test]
    fn frame_rejects_invalid_external_binding_state_before_capability_checks() {
        let bounds = rect();
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: 42,
            position: 0,
            bounds,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(list);
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let mut invalid = binding(42);
        invalid.frame_token = None;
        let mut bindings = ExternalSurfaceBindings::new();
        bindings.insert(invalid).unwrap();

        let error = InteractiveFrame::new(&scene, &metadata, &resources, &bindings)
            .validate()
            .unwrap_err();

        assert!(matches!(
            error,
            FrameValidationError::InvalidSurfaceBinding(InvalidExternalSurfaceBinding {
                slot_id: ExternalSurfaceSlotId(42),
                ..
            })
        ));
    }

    #[test]
    fn nested_surface_provenance_tracks_root_layer_and_cached_operation_paths() {
        let bounds = rect();
        let slot_id = ExternalSurfaceSlotId(17);
        let layer_id = fission_ir::WidgetId::explicit("surface.layer");
        let mut innermost = DisplayList::new(bounds);
        innermost.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: slot_id.0,
            position: 0,
            bounds,
            node_id: None,
        });
        let mut middle = DisplayList::new(bounds);
        middle.push(DisplayOp::CachedScene {
            cache_key: 2,
            bounds,
            list: Box::new(innermost),
        });
        let mut outer = DisplayList::new(bounds);
        outer.push(DisplayOp::CachedScene {
            cache_key: 1,
            bounds,
            list: Box::new(middle),
        });
        let mut layer = crate::RenderLayer::new(bounds);
        layer.node_id = Some(layer_id);
        layer.children.push(RenderNode::Paint(outer));
        let mut scene = RenderScene::new(bounds);
        scene
            .roots
            .push(RenderNode::Paint(DisplayList::new(bounds)));
        scene.roots.push(RenderNode::Layer(layer));
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let mut bindings = ExternalSurfaceBindings::new();
        bindings.insert(binding(slot_id.0)).unwrap();
        let mut capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("test", "1", "gpu"));
        capabilities.display_ops = crate::capabilities::DisplayOpKind::ALL
            .into_iter()
            .collect();

        let error = InteractiveFrame::new(&scene, &metadata, &resources, &bindings)
            .validate_for(&capabilities)
            .unwrap_err();
        let FrameGateError::UnsupportedExternalSurfaces(error) = error else {
            panic!("expected external surface capability failure");
        };

        let unsupported = &error.unsupported_bindings[0];
        assert_eq!(unsupported.slot_id, slot_id);
        assert_eq!(unsupported.provenance.node_id, Some(layer_id));
        assert_eq!(
            unsupported.provenance.source,
            FrameOperationSource::DisplayList {
                root_index: 1,
                node_path: vec![0],
                operation_path: vec![0, 0, 0],
            }
        );
    }
}
