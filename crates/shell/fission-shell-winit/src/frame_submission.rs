use std::collections::{BTreeMap, HashMap};

use fission_core::env::{VideoStateMap, VideoStatus, WebStateMap};
use fission_ir::{CoreIR, EmbedKind, LayoutOp, Op, WidgetId};
use fission_layout::ParagraphResultStore;
use fission_render::capabilities::{ExternalSurfaceTransport, GraphicsCapabilities};
use fission_render::external_surface::{
    ExternalAlphaType, ExternalColorSpace, ExternalFrameId, ExternalFrameState, ExternalOwnership,
    ExternalProducerId, ExternalProducerKind, ExternalSurfaceBinding, ExternalSurfaceBindings,
    ExternalSurfaceSlotId, ExternalSynchronization,
};
use fission_render::frame::{
    DamageRegion, FrameId, FrameMetadata, FrameViewport, InteractiveFrame, ResourceEpoch,
    SemanticsEpoch,
};
use fission_render::paragraph::ParagraphFrameBindings;
use fission_render::resource::ResourceSnapshot;
use fission_render::surface::{PhysicalSize, ScaleFactor};
use fission_render::{embed_surface_id, LayoutRect, LayoutSize, RenderScene};
use fission_shell::{NativeSurfaceFrame, PlatformSurfaceCapabilities, VideoSurfaceFrame};

use crate::native_surface::NativeSurfaceRegistry;
use crate::web_backend::WebSurfaceFrame;

mod capabilities;
mod error;
#[cfg(test)]
mod ordering_tests;
mod placement;
mod resources;
mod software;

pub(super) use capabilities::{winit_software_capabilities, winit_vello_capabilities};
pub(crate) use error::FrameSubmissionError;
use error::{PlatformSurfaceSemantic, SurfaceOrderingIssue};

use placement::{
    collect_surface_placements, CollectedSurfacePlacements, NativeViewGeometry,
    ResolvedSurfacePlacement, ScenePaintBounds, ScenePaintItem, SurfacePlacementIssue,
};
use resources::{build_resource_snapshot, FrameResourceRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProducerDescriptorKind {
    Video,
    Web,
    #[cfg(feature = "three-d")]
    ThreeD,
    CustomNativeView,
    CustomUnclaimed,
}

impl ProducerDescriptorKind {
    fn from_embed(
        kind: &EmbedKind,
        widget_id: WidgetId,
        native_views: &NativeSurfaceRegistry,
    ) -> Result<Self, FrameSubmissionError> {
        let descriptor = match kind {
            EmbedKind::Video => Self::Video,
            EmbedKind::Web => Self::Web,
            EmbedKind::Custom(payload) => {
                #[cfg(feature = "three-d")]
                match classify_builtin_three_d_payload(payload) {
                    ThreeDPayloadClassification::Ready(_) => return Ok(Self::ThreeD),
                    ThreeDPayloadClassification::Malformed(error) => {
                        return Err(FrameSubmissionError::InvalidThreeDSubmission {
                            widget_id,
                            error,
                        });
                    }
                    ThreeDPayloadClassification::Unrelated => {}
                }

                if native_views.claims_native_view_payload(payload) {
                    Self::CustomNativeView
                } else {
                    Self::CustomUnclaimed
                }
            }
        };
        Ok(descriptor)
    }

    fn embed_kind(self) -> EmbedKind {
        match self {
            Self::Video => EmbedKind::Video,
            Self::Web => EmbedKind::Web,
            #[cfg(feature = "three-d")]
            Self::ThreeD => EmbedKind::Custom(Vec::new()),
            Self::CustomNativeView | Self::CustomUnclaimed => EmbedKind::Custom(Vec::new()),
        }
    }

    fn external_kind(self) -> Option<ExternalProducerKind> {
        match self {
            Self::Video => Some(ExternalProducerKind::Video),
            Self::Web => Some(ExternalProducerKind::WebView),
            #[cfg(feature = "three-d")]
            Self::ThreeD => Some(ExternalProducerKind::ThreeD),
            Self::CustomNativeView => Some(ExternalProducerKind::Custom("native-view".to_string())),
            Self::CustomUnclaimed => None,
        }
    }

    fn transport(self) -> Option<ExternalSurfaceTransport> {
        match self {
            Self::Video | Self::Web | Self::CustomNativeView => {
                Some(ExternalSurfaceTransport::NativeView)
            }
            #[cfg(feature = "three-d")]
            Self::ThreeD => Some(ExternalSurfaceTransport::DirectTarget),
            Self::CustomUnclaimed => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Web => "web",
            #[cfg(feature = "three-d")]
            Self::ThreeD => "three-d",
            Self::CustomNativeView | Self::CustomUnclaimed => "custom",
        }
    }
}

#[cfg(feature = "three-d")]
fn decode_builtin_three_d_payload(payload: &[u8]) -> Option<fission_3d_model::Scene3DModel> {
    match classify_builtin_three_d_payload(payload) {
        ThreeDPayloadClassification::Ready(model) => Some(model),
        ThreeDPayloadClassification::Unrelated | ThreeDPayloadClassification::Malformed(_) => None,
    }
}

#[cfg(feature = "three-d")]
fn is_builtin_three_d_payload(payload: &[u8]) -> bool {
    decode_builtin_three_d_payload(payload).is_some()
}

#[cfg(feature = "three-d")]
enum ThreeDPayloadClassification {
    Unrelated,
    Ready(fission_3d_model::Scene3DModel),
    Malformed(fission_3d_model::Scene3DSubmissionError),
}

#[cfg(feature = "three-d")]
fn classify_builtin_three_d_payload(payload: &[u8]) -> ThreeDPayloadClassification {
    use fission_3d_model::Scene3DSubmissionError;

    match fission_3d_model::decode_scene3d_submission(payload) {
        Ok(model) => ThreeDPayloadClassification::Ready(model),
        Err(Scene3DSubmissionError::TruncatedHeader | Scene3DSubmissionError::InvalidMagic) => {
            ThreeDPayloadClassification::Unrelated
        }
        Err(error) => ThreeDPayloadClassification::Malformed(error),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProducerDescriptor {
    widget_id: WidgetId,
    kind: ProducerDescriptorKind,
}

#[derive(Debug, Clone)]
pub(super) struct FrameSubmission {
    metadata: FrameMetadata,
    resources: ResourceSnapshot,
    paragraphs: ParagraphFrameBindings,
    external_surfaces: ExternalSurfaceBindings,
    staged_surfaces: StagedSurfaceFrames,
    #[cfg(feature = "three-d")]
    direct_target_three_d: Vec<DirectTargetThreeDFrame>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct StagedSurfaceFrames {
    video: Vec<VideoSurfaceFrame>,
    web: Vec<WebSurfaceFrame>,
    native: Vec<NativeSurfaceFrame>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SurfacePresenterCapabilities {
    video: PlatformSurfaceCapabilities,
    web: PlatformSurfaceCapabilities,
    direct_target_available: bool,
}

impl SurfacePresenterCapabilities {
    pub(super) fn new(
        video: PlatformSurfaceCapabilities,
        web: PlatformSurfaceCapabilities,
        direct_target_available: bool,
    ) -> Self {
        Self {
            video,
            web,
            direct_target_available,
        }
    }

    #[cfg(test)]
    fn fully_capable() -> Self {
        Self::new(
            PlatformSurfaceCapabilities::FULL,
            PlatformSurfaceCapabilities::FULL,
            true,
        )
    }
}

impl StagedSurfaceFrames {
    pub(super) fn video(&self) -> &[VideoSurfaceFrame] {
        &self.video
    }

    pub(super) fn web(&self) -> &[WebSurfaceFrame] {
        &self.web
    }

    pub(super) fn native(&self) -> &[NativeSurfaceFrame] {
        &self.native
    }
}

fn paragraph_bindings(ir: &CoreIR, store: Option<&ParagraphResultStore>) -> ParagraphFrameBindings {
    let Some(store) = store else {
        return ParagraphFrameBindings::new();
    };
    let mut bindings = ParagraphFrameBindings::new();
    for (node_id, node) in &ir.nodes {
        if !matches!(
            &node.op,
            Op::Paint(fission_ir::PaintOp::DrawText { .. })
                | Op::Paint(fission_ir::PaintOp::DrawRichText { .. })
        ) {
            continue;
        }
        if let Some(result) = store.get(*node_id) {
            bindings.insert(*node_id, result);
        }
    }
    bindings
}

/// Legacy 3D work derived for the transitional `DirectTarget` adapter.
///
/// Placement is resolved from the retained scene's matching `DrawSurface` and
/// its ancestor layer semantics, never from `NativeSurfaceFrame`; the producer
/// frame contributes only payload and identity. This execution plan is
/// disposable and is not a second authority.
#[cfg(feature = "three-d")]
#[derive(Debug, Clone)]
pub(super) struct DirectTargetThreeDFrame {
    pub(super) model: fission_3d_model::Scene3DModel,
    pub(super) viewport: LayoutRect,
    pub(super) clip: LayoutRect,
    pub(super) opacity: f32,
    paint_order: u64,
}

impl FrameSubmission {
    pub(super) fn metadata(&self) -> &FrameMetadata {
        &self.metadata
    }

    #[cfg(test)]
    fn external_surfaces(&self) -> &ExternalSurfaceBindings {
        &self.external_surfaces
    }

    #[cfg(test)]
    fn resources(&self) -> &ResourceSnapshot {
        &self.resources
    }

    #[cfg(test)]
    fn staged_surfaces(&self) -> &StagedSurfaceFrames {
        &self.staged_surfaces
    }

    #[cfg(feature = "three-d")]
    pub(super) fn direct_target_three_d(&self) -> &[DirectTargetThreeDFrame] {
        &self.direct_target_three_d
    }

    pub(super) fn interactive_frame<'a>(&'a self, scene: &'a RenderScene) -> InteractiveFrame<'a> {
        InteractiveFrame::new(
            scene,
            &self.metadata,
            &self.resources,
            &self.external_surfaces,
        )
        .with_paragraphs(&self.paragraphs)
    }

    pub(super) fn has_external_surfaces(&self) -> bool {
        !self.external_surfaces.is_empty()
    }

    pub(super) fn compose_host_scene(&self, scene: &RenderScene) -> RenderScene {
        software::compose_host_scene(scene, &self.external_surfaces)
    }

    pub(super) fn validate_for(
        &self,
        scene: &RenderScene,
        capabilities: &GraphicsCapabilities,
    ) -> Result<(), FrameSubmissionError> {
        self.interactive_frame(scene)
            .check_for(capabilities)
            .map_err(FrameSubmissionError::FrameGate)
    }
}

#[derive(Debug, Clone)]
pub(super) struct FrameSubmissionState {
    next_frame_id: u64,
    next_resource_epoch: u64,
    next_semantics_epoch: u64,
    last_committed_frame_id: Option<FrameId>,
    resources: FrameResourceRegistry,
}

impl Default for FrameSubmissionState {
    fn default() -> Self {
        Self {
            next_frame_id: 1,
            next_resource_epoch: 1,
            next_semantics_epoch: 1,
            last_committed_frame_id: None,
            resources: FrameResourceRegistry::default(),
        }
    }
}

impl FrameSubmissionState {
    pub(super) fn commit(
        &mut self,
        submission: &FrameSubmission,
        publish: impl FnOnce(&StagedSurfaceFrames),
    ) -> Result<(), FrameSubmissionError> {
        if self
            .last_committed_frame_id
            .is_some_and(|last| submission.metadata.frame_id <= last)
        {
            return Err(FrameSubmissionError::NonMonotonicCommit {
                previous: self.last_committed_frame_id,
                attempted: submission.metadata.frame_id,
            });
        }
        publish(&submission.staged_surfaces);
        self.last_committed_frame_id = Some(submission.metadata.frame_id);
        Ok(())
    }

    #[cfg(test)]
    fn last_committed_frame_id(&self) -> Option<FrameId> {
        self.last_committed_frame_id
    }

    pub(super) fn prepare_with_states(
        &mut self,
        scene: &RenderScene,
        ir: &CoreIR,
        paragraph_store: Option<&ParagraphResultStore>,
        video_frames: &[VideoSurfaceFrame],
        web_frames: &[WebSurfaceFrame],
        custom_frames: &[NativeSurfaceFrame],
        video_states: &VideoStateMap,
        web_states: &WebStateMap,
        native_views: &NativeSurfaceRegistry,
        presenters: SurfacePresenterCapabilities,
        logical_size: LayoutSize,
        physical_size: PhysicalSize,
        scale_factor: f64,
    ) -> Result<FrameSubmission, FrameSubmissionError> {
        let frame_id = FrameId(take_monotonic(&mut self.next_frame_id, "frame id")?);
        let resource_epoch = ResourceEpoch(take_monotonic(
            &mut self.next_resource_epoch,
            "resource epoch",
        )?);
        let semantics_epoch = SemanticsEpoch(take_monotonic(
            &mut self.next_semantics_epoch,
            "semantics epoch",
        )?);
        let metadata = FrameMetadata {
            frame_id,
            viewport: FrameViewport {
                logical_size,
                physical_size,
                scale_factor: ScaleFactor::new(scale_factor)
                    .map_err(FrameSubmissionError::InvalidScaleFactor)?,
            },
            // The current encoders redraw the complete target. Reporting full
            // damage is conservative and does not invent retained damage data.
            damage: DamageRegion::Full,
            resource_epoch,
            semantics_epoch,
        };
        let paragraphs = paragraph_bindings(ir, paragraph_store);
        let resources = build_resource_snapshot(resource_epoch, ir, &mut self.resources)?;
        let descriptors = collect_producer_descriptors(ir, native_views)?;
        let CollectedSurfacePlacements {
            placements: surface_placements,
            paint_sequence,
        } = collect_surface_placements(scene);
        let producer_frames = ProducerFrames::new(
            video_frames,
            video_states,
            web_frames,
            web_states,
            custom_frames,
        )?;
        let mut external_surfaces = ExternalSurfaceBindings::new();
        let mut staged_surfaces = StagedSurfaceFrames::default();
        let mut native_view_plans = Vec::new();
        #[cfg(feature = "three-d")]
        let mut direct_target_three_d = Vec::new();

        for (&slot_id, &placement) in &surface_placements {
            let descriptor = descriptors
                .get(&slot_id)
                .copied()
                .ok_or(FrameSubmissionError::MissingProducerDescriptor(slot_id))?;
            let producer_kind = descriptor.kind.external_kind().ok_or(
                FrameSubmissionError::UnclaimedCustomSurface {
                    widget_id: descriptor.widget_id,
                    slot_id,
                },
            )?;
            let transport = descriptor.kind.transport().ok_or(
                FrameSubmissionError::UnclaimedCustomSurface {
                    widget_id: descriptor.widget_id,
                    slot_id,
                },
            )?;
            let presenter_capabilities =
                producer_frames.presenter_capabilities(descriptor, native_views, presenters);
            let presenter_absence_is_terminal = match descriptor.kind {
                // Built-in widgets retain a deterministic placeholder when a
                // target has no presenter. One unsupported embed must not
                // suppress the surrounding frame.
                ProducerDescriptorKind::Video | ProducerDescriptorKind::Web => false,
                ProducerDescriptorKind::CustomNativeView => native_views.has_attached_host(),
                #[cfg(feature = "three-d")]
                ProducerDescriptorKind::ThreeD => false,
                ProducerDescriptorKind::CustomUnclaimed => false,
            };
            if transport == ExternalSurfaceTransport::NativeView
                && producer_frames.has_frame(descriptor)
                && presenter_absence_is_terminal
                && !presenter_capabilities.available
            {
                return Err(FrameSubmissionError::UnavailablePlatformPresenter {
                    widget_id: descriptor.widget_id,
                    slot_id,
                    kind: descriptor.kind.label(),
                });
            }
            let state = producer_frames.state_for(descriptor, presenter_capabilities);
            let should_stage_native_view = transport == ExternalSurfaceTransport::NativeView
                && (state == ExternalFrameState::Ready
                    || (descriptor.kind == ProducerDescriptorKind::Web
                        && producer_frames.has_frame(descriptor)
                        && presenter_capabilities.available));
            if should_stage_native_view {
                if let Some(geometry) =
                    placement.native_view_geometry(descriptor.widget_id, slot_id)?
                {
                    validate_platform_surface_geometry(
                        descriptor,
                        slot_id,
                        placement,
                        geometry,
                        presenter_capabilities,
                    )?;
                    let presenter_domain = producer_frames
                        .presenter_domain(descriptor, native_views)
                        .ok_or(FrameSubmissionError::UnavailablePlatformPresenter {
                            widget_id: descriptor.widget_id,
                            slot_id,
                            kind: descriptor.kind.label(),
                        })?;
                    native_view_plans.push(NativeViewPlan {
                        descriptor,
                        slot_id,
                        geometry,
                        capabilities: presenter_capabilities,
                        presenter_domain,
                    });
                    match descriptor.kind {
                        ProducerDescriptorKind::Video => {
                            let producer = producer_frames.video[&descriptor.widget_id];
                            staged_surfaces.video.push(VideoSurfaceFrame {
                                widget_id: descriptor.widget_id,
                                surface_id: producer.surface_id,
                                rect: geometry.rect,
                                visible_rect: geometry.visible_rect,
                                transform: geometry.transform,
                                opacity: geometry.opacity,
                                paint_order: geometry.paint_order,
                            });
                        }
                        ProducerDescriptorKind::Web => {
                            let producer = producer_frames.web[&descriptor.widget_id];
                            staged_surfaces.web.push(WebSurfaceFrame {
                                widget_id: descriptor.widget_id,
                                url: producer.url.clone(),
                                user_agent: producer.user_agent.clone(),
                                rect: geometry.rect,
                                visible_rect: geometry.visible_rect,
                                transform: geometry.transform,
                                opacity: geometry.opacity,
                                paint_order: geometry.paint_order,
                            });
                        }
                        ProducerDescriptorKind::CustomNativeView => {
                            let producer = producer_frames.custom[&descriptor.widget_id];
                            staged_surfaces.native.push(NativeSurfaceFrame {
                                widget_id: descriptor.widget_id,
                                rect: geometry.rect,
                                payload: producer.payload.clone(),
                                visible_rect: geometry.visible_rect,
                                transform: geometry.transform,
                                opacity: geometry.opacity,
                                paint_order: geometry.paint_order,
                            });
                        }
                        #[cfg(feature = "three-d")]
                        ProducerDescriptorKind::ThreeD => {}
                        ProducerDescriptorKind::CustomUnclaimed => unreachable!(),
                    }
                }
            }
            #[cfg(feature = "three-d")]
            if descriptor.kind == ProducerDescriptorKind::ThreeD
                && state == ExternalFrameState::Ready
            {
                if let Some(model) = producer_frames
                    .custom
                    .get(&descriptor.widget_id)
                    .and_then(|frame| decode_builtin_three_d_payload(&frame.payload))
                {
                    if let Some((viewport, clip, opacity, paint_order)) =
                        placement.direct_target_geometry(descriptor.widget_id, slot_id)?
                    {
                        direct_target_three_d.push(DirectTargetThreeDFrame {
                            model,
                            viewport,
                            clip,
                            opacity,
                            paint_order,
                        });
                    }
                }
            }
            external_surfaces
                .insert(ExternalSurfaceBinding {
                    slot_id,
                    producer_id: producer_id(descriptor),
                    producer_kind,
                    frame_id: ExternalFrameId(frame_id.0),
                    // Current host adapters resolve native views and the
                    // transitional direct-target producer by stable identity;
                    // no portable image token or synchronization primitive
                    // exists yet.
                    frame_token: None,
                    state,
                    transport,
                    color_space: ExternalColorSpace::Srgb,
                    alpha_type: ExternalAlphaType::Opaque,
                    ownership: ExternalOwnership::Shared,
                    synchronization: ExternalSynchronization::None,
                    zero_copy: false,
                    damaged: true,
                })
                .map_err(FrameSubmissionError::DuplicateBinding)?;
        }

        #[cfg(feature = "three-d")]
        direct_target_three_d.sort_by_key(|frame| frame.paint_order);
        staged_surfaces.video.sort_by_key(|frame| frame.paint_order);
        staged_surfaces.web.sort_by_key(|frame| frame.paint_order);
        staged_surfaces
            .native
            .sort_by_key(|frame| frame.paint_order);
        validate_native_view_overlap_order(&native_view_plans)?;
        validate_surface_execution_order(
            &paint_sequence,
            &surface_placements,
            &external_surfaces,
            &native_view_plans,
        )?;

        Ok(FrameSubmission {
            resources,
            paragraphs,
            metadata,
            external_surfaces,
            staged_surfaces,
            #[cfg(feature = "three-d")]
            direct_target_three_d,
        })
    }

    #[cfg(test)]
    fn prepare(
        &mut self,
        scene: &RenderScene,
        ir: &CoreIR,
        video_frames: &[VideoSurfaceFrame],
        web_frames: &[WebSurfaceFrame],
        custom_frames: &[NativeSurfaceFrame],
        native_views: &NativeSurfaceRegistry,
        presenters: SurfacePresenterCapabilities,
        logical_size: LayoutSize,
        physical_size: PhysicalSize,
        scale_factor: f64,
    ) -> Result<FrameSubmission, FrameSubmissionError> {
        self.prepare_with_states(
            scene,
            ir,
            None,
            video_frames,
            web_frames,
            custom_frames,
            &VideoStateMap::default(),
            &WebStateMap::default(),
            native_views,
            presenters,
            logical_size,
            physical_size,
            scale_factor,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct NativeViewPlan {
    descriptor: ProducerDescriptor,
    slot_id: ExternalSurfaceSlotId,
    geometry: NativeViewGeometry,
    capabilities: PlatformSurfaceCapabilities,
    presenter_domain: NativeViewPresenterDomain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeViewPresenterDomain {
    Video,
    Web,
    Custom(usize),
}

fn validate_platform_surface_geometry(
    descriptor: ProducerDescriptor,
    slot_id: ExternalSurfaceSlotId,
    placement: ResolvedSurfacePlacement,
    geometry: NativeViewGeometry,
    capabilities: PlatformSurfaceCapabilities,
) -> Result<(), FrameSubmissionError> {
    let unsupported = if geometry.visible_rect != geometry.rect && !capabilities.rectangular_clip {
        Some(PlatformSurfaceSemantic::RectangularClip)
    // NativeSurfaceFrame carries a transformed AABB and an informational
    // accumulated matrix, not an unambiguous local rect + transform pair.
    // No current presenter can therefore prove rotation/shear/reflection
    // semantics without risking a double transform.
    } else if placement.requires_affine_transform() {
        Some(PlatformSurfaceSemantic::AffineTransform)
    } else if (geometry.opacity - 1.0).abs() > 0.001 && !capabilities.opacity {
        Some(PlatformSurfaceSemantic::Opacity)
    } else {
        None
    };
    if let Some(semantic) = unsupported {
        return Err(FrameSubmissionError::UnsupportedPlatformSurfaceSemantics {
            widget_id: descriptor.widget_id,
            slot_id,
            kind: descriptor.kind.label(),
            semantic,
        });
    }
    Ok(())
}

fn validate_native_view_overlap_order(
    plans: &[NativeViewPlan],
) -> Result<(), FrameSubmissionError> {
    for (index, first) in plans.iter().enumerate() {
        for second in &plans[index + 1..] {
            if rects_overlap(first.geometry.visible_rect, second.geometry.visible_rect)
                && (first.presenter_domain != second.presenter_domain
                    || !first.capabilities.paint_order
                    || !second.capabilities.paint_order)
            {
                return Err(FrameSubmissionError::UnsupportedPlatformSurfaceSemantics {
                    widget_id: second.descriptor.widget_id,
                    slot_id: second.slot_id,
                    kind: second.descriptor.kind.label(),
                    semantic: PlatformSurfaceSemantic::PaintOrder,
                });
            }
        }
    }
    Ok(())
}

fn validate_surface_execution_order(
    paint_sequence: &[ScenePaintItem],
    placements: &BTreeMap<ExternalSurfaceSlotId, ResolvedSurfacePlacement>,
    bindings: &ExternalSurfaceBindings,
    native_view_plans: &[NativeViewPlan],
) -> Result<(), FrameSubmissionError> {
    let mut deferred_surfaces = Vec::new();

    for item in paint_sequence {
        match *item {
            ScenePaintItem::Paint { operation, bounds } => {
                if bounds == ScenePaintBounds::Invisible {
                    continue;
                }
                for &(slot_id, transport) in &deferred_surfaces {
                    let overlaps = match bounds {
                        ScenePaintBounds::Visible(paint_bounds) => placements
                            .get(&slot_id)
                            .and_then(|placement| placement.visible_rect())
                            .map_or(true, |surface_bounds| {
                                rects_overlap(paint_bounds, surface_bounds)
                            }),
                        ScenePaintBounds::Invisible => false,
                        ScenePaintBounds::Unknown => true,
                    };
                    if overlaps {
                        return Err(FrameSubmissionError::UnsupportedSurfaceOrdering {
                            slot_id,
                            issue: SurfaceOrderingIssue::TwoDPaintAfterDeferredSurface {
                                transport,
                                operation,
                            },
                        });
                    }
                }
            }
            ScenePaintItem::Surface(slot_id) => {
                let Some(binding) = bindings.get(slot_id) else {
                    continue;
                };
                let will_present = match binding.transport {
                    ExternalSurfaceTransport::NativeView => {
                        native_view_plans.iter().any(|plan| plan.slot_id == slot_id)
                    }
                    ExternalSurfaceTransport::DirectTarget => {
                        binding.state == ExternalFrameState::Ready
                    }
                    ExternalSurfaceTransport::CpuImage
                    | ExternalSurfaceTransport::NativeImage
                    | ExternalSurfaceTransport::GpuImage => false,
                };
                if !will_present {
                    continue;
                }
                if !placements
                    .get(&slot_id)
                    .is_some_and(|placement| placement.is_visible())
                {
                    continue;
                }
                match binding.transport {
                    ExternalSurfaceTransport::NativeView => {
                        deferred_surfaces.push((slot_id, binding.transport));
                    }
                    ExternalSurfaceTransport::DirectTarget => {
                        let direct_bounds = placements
                            .get(&slot_id)
                            .and_then(|placement| placement.visible_rect());
                        for &(native_slot_id, transport) in &deferred_surfaces {
                            let overlaps = direct_bounds.map_or(true, |direct_bounds| {
                                placements
                                    .get(&native_slot_id)
                                    .and_then(|placement| placement.visible_rect())
                                    .map_or(true, |native_bounds| {
                                        rects_overlap(direct_bounds, native_bounds)
                                    })
                            });
                            if transport == ExternalSurfaceTransport::NativeView && overlaps {
                                return Err(FrameSubmissionError::UnsupportedSurfaceOrdering {
                                    slot_id,
                                    issue: SurfaceOrderingIssue::NativeViewBeforeDirectTarget {
                                        native_slot_id,
                                    },
                                });
                            }
                        }
                        deferred_surfaces.push((slot_id, binding.transport));
                    }
                    ExternalSurfaceTransport::CpuImage
                    | ExternalSurfaceTransport::NativeImage
                    | ExternalSurfaceTransport::GpuImage => {}
                }
            }
        }
    }
    Ok(())
}

fn rects_overlap(first: LayoutRect, second: LayoutRect) -> bool {
    first.x() < second.right()
        && first.right() > second.x()
        && first.y() < second.bottom()
        && first.bottom() > second.y()
}

fn take_monotonic(counter: &mut u64, name: &'static str) -> Result<u64, FrameSubmissionError> {
    let current = *counter;
    *counter = counter
        .checked_add(1)
        .ok_or(FrameSubmissionError::CounterExhausted(name))?;
    Ok(current)
}

fn collect_producer_descriptors(
    ir: &CoreIR,
    native_views: &NativeSurfaceRegistry,
) -> Result<BTreeMap<ExternalSurfaceSlotId, ProducerDescriptor>, FrameSubmissionError> {
    let mut descriptors = BTreeMap::new();
    for node in ir.nodes.values() {
        let Op::Layout(LayoutOp::Embed {
            kind, widget_id, ..
        }) = &node.op
        else {
            continue;
        };
        let kind = ProducerDescriptorKind::from_embed(kind, *widget_id, native_views)?;
        let descriptor = ProducerDescriptor {
            widget_id: *widget_id,
            kind,
        };
        let slot_id = ExternalSurfaceSlotId(embed_surface_id(&kind.embed_kind(), *widget_id));
        if let Some(existing) = descriptors.insert(slot_id, descriptor) {
            if existing != descriptor {
                return Err(FrameSubmissionError::SurfaceSlotCollision {
                    slot_id,
                    first: existing.widget_id,
                    second: descriptor.widget_id,
                });
            }
        }
    }
    Ok(descriptors)
}

struct ProducerFrames<'a> {
    video: HashMap<WidgetId, &'a VideoSurfaceFrame>,
    video_states: &'a VideoStateMap,
    web: HashMap<WidgetId, &'a WebSurfaceFrame>,
    web_states: &'a WebStateMap,
    custom: HashMap<WidgetId, &'a NativeSurfaceFrame>,
}

impl<'a> ProducerFrames<'a> {
    fn new(
        video_frames: &'a [VideoSurfaceFrame],
        video_states: &'a VideoStateMap,
        web_frames: &'a [WebSurfaceFrame],
        web_states: &'a WebStateMap,
        custom_frames: &'a [NativeSurfaceFrame],
    ) -> Result<Self, FrameSubmissionError> {
        Ok(Self {
            video: index_frames(
                video_frames,
                |frame| frame.widget_id,
                ProducerDescriptorKind::Video,
            )?,
            video_states,
            web: index_frames(
                web_frames,
                |frame| frame.widget_id,
                ProducerDescriptorKind::Web,
            )?,
            web_states,
            custom: index_frames(
                custom_frames,
                |frame| frame.widget_id,
                ProducerDescriptorKind::CustomNativeView,
            )?,
        })
    }

    fn presenter_capabilities(
        &self,
        descriptor: ProducerDescriptor,
        native_views: &NativeSurfaceRegistry,
        presenters: SurfacePresenterCapabilities,
    ) -> PlatformSurfaceCapabilities {
        match descriptor.kind {
            ProducerDescriptorKind::Video => presenters.video,
            ProducerDescriptorKind::Web => presenters.web,
            #[cfg(feature = "three-d")]
            ProducerDescriptorKind::ThreeD => {
                if presenters.direct_target_available {
                    PlatformSurfaceCapabilities::FULL
                } else {
                    PlatformSurfaceCapabilities::UNAVAILABLE
                }
            }
            ProducerDescriptorKind::CustomNativeView => self
                .custom
                .get(&descriptor.widget_id)
                .map(|frame| native_views.native_view_capabilities(&frame.payload))
                .unwrap_or(PlatformSurfaceCapabilities::UNAVAILABLE),
            ProducerDescriptorKind::CustomUnclaimed => PlatformSurfaceCapabilities::UNAVAILABLE,
        }
    }

    fn presenter_domain(
        &self,
        descriptor: ProducerDescriptor,
        native_views: &NativeSurfaceRegistry,
    ) -> Option<NativeViewPresenterDomain> {
        match descriptor.kind {
            ProducerDescriptorKind::Video => Some(NativeViewPresenterDomain::Video),
            ProducerDescriptorKind::Web => Some(NativeViewPresenterDomain::Web),
            ProducerDescriptorKind::CustomNativeView => self
                .custom
                .get(&descriptor.widget_id)
                .and_then(|frame| native_views.presenter_index(&frame.payload))
                .map(NativeViewPresenterDomain::Custom),
            #[cfg(feature = "three-d")]
            ProducerDescriptorKind::ThreeD => None,
            ProducerDescriptorKind::CustomUnclaimed => None,
        }
    }

    fn has_frame(&self, descriptor: ProducerDescriptor) -> bool {
        match descriptor.kind {
            ProducerDescriptorKind::Video => self.video.contains_key(&descriptor.widget_id),
            ProducerDescriptorKind::Web => self.web.contains_key(&descriptor.widget_id),
            #[cfg(feature = "three-d")]
            ProducerDescriptorKind::ThreeD => self.custom.contains_key(&descriptor.widget_id),
            ProducerDescriptorKind::CustomNativeView => {
                self.custom.contains_key(&descriptor.widget_id)
            }
            ProducerDescriptorKind::CustomUnclaimed => false,
        }
    }

    fn state_for(
        &self,
        descriptor: ProducerDescriptor,
        presenter_capabilities: PlatformSurfaceCapabilities,
    ) -> ExternalFrameState {
        match descriptor.kind {
            ProducerDescriptorKind::Video => {
                let Some(frame) = self.video.get(&descriptor.widget_id) else {
                    return ExternalFrameState::Pending;
                };
                if frame.surface_id == 0 || !presenter_capabilities.available {
                    return ExternalFrameState::Pending;
                }
                match self
                    .video_states
                    .states
                    .get(&descriptor.widget_id)
                    .map(|state| (state.status, state.duration_ms.is_some()))
                {
                    Some((VideoStatus::Error, _)) => ExternalFrameState::Failed,
                    Some((_, false)) => ExternalFrameState::Pending,
                    // Playback lifecycle is separate from frame availability:
                    // buffering and ended players retain their last decoded
                    // frame after the Ready event.
                    Some((_, true)) | None => ExternalFrameState::Ready,
                }
            }
            ProducerDescriptorKind::Web => match self.web.get(&descriptor.widget_id) {
                Some(frame) if !frame.url.trim().is_empty() && presenter_capabilities.available => {
                    if self
                        .web_states
                        .states
                        .get(&descriptor.widget_id)
                        .is_some_and(|state| state.loading)
                    {
                        ExternalFrameState::Pending
                    } else {
                        ExternalFrameState::Ready
                    }
                }
                _ => ExternalFrameState::Pending,
            },
            #[cfg(feature = "three-d")]
            ProducerDescriptorKind::ThreeD => {
                if presenter_capabilities.available
                    && self
                        .custom
                        .get(&descriptor.widget_id)
                        .is_some_and(|frame| is_builtin_three_d_payload(&frame.payload))
                {
                    ExternalFrameState::Ready
                } else {
                    ExternalFrameState::Pending
                }
            }
            ProducerDescriptorKind::CustomNativeView => {
                if self.custom.contains_key(&descriptor.widget_id)
                    && presenter_capabilities.available
                {
                    ExternalFrameState::Ready
                } else {
                    ExternalFrameState::Pending
                }
            }
            ProducerDescriptorKind::CustomUnclaimed => ExternalFrameState::Pending,
        }
    }
}

fn index_frames<'a, T>(
    frames: &'a [T],
    widget_id: impl Fn(&T) -> WidgetId,
    kind: ProducerDescriptorKind,
) -> Result<HashMap<WidgetId, &'a T>, FrameSubmissionError> {
    let mut indexed = HashMap::new();
    for frame in frames {
        let widget_id = widget_id(frame);
        if indexed.insert(widget_id, frame).is_some() {
            return Err(FrameSubmissionError::DuplicateProducerFrame {
                widget_id,
                kind: kind.label(),
            });
        }
    }
    Ok(indexed)
}

fn producer_id(descriptor: ProducerDescriptor) -> ExternalProducerId {
    const PRODUCER_DOMAIN: u64 = 0x5052_4F44_5543_4552;
    let kind_domain = match descriptor.kind {
        ProducerDescriptorKind::Video => 0x01,
        ProducerDescriptorKind::Web => 0x02,
        #[cfg(feature = "three-d")]
        ProducerDescriptorKind::ThreeD => 0x03,
        ProducerDescriptorKind::CustomNativeView | ProducerDescriptorKind::CustomUnclaimed => 0x04,
    };
    ExternalProducerId(
        embed_surface_id(&descriptor.kind.embed_kind(), descriptor.widget_id).rotate_left(17)
            ^ PRODUCER_DOMAIN
            ^ kind_domain,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_render::capabilities::{BackendIdentity, RenderMode};
    use fission_render::frame::{FrameGateError, FrameValidationError};
    use fission_render::{DisplayList, DisplayOp, LayerClip, LayoutRect, RenderNode};
    use fission_shell::{NativeSurfaceHandler, NativeSurfaceHost, PlatformSurfaceCapabilities};
    use placement::{Affine2d, ClipRegion};
    use raw_window_handle::{RawWindowHandle, WindowHandle};

    struct ClaimAllNativeViews;

    impl NativeSurfaceHandler for ClaimAllNativeViews {
        fn handles_payload(&self, _payload: &[u8]) -> bool {
            true
        }

        fn surface_capabilities(&self, _payload: &[u8]) -> PlatformSurfaceCapabilities {
            PlatformSurfaceCapabilities::FULL
        }

        fn attach_host(&mut self, _host: NativeSurfaceHost<'_>) {}

        fn detach_host(&mut self) {}

        fn present_surfaces(&mut self, _frames: &[NativeSurfaceFrame]) {}
    }

    fn native_view_registry() -> NativeSurfaceRegistry {
        let mut registry = NativeSurfaceRegistry::default();
        registry.register(ClaimAllNativeViews);
        let raw = RawWindowHandle::Web(raw_window_handle::WebWindowHandle::new(0));
        let handle = unsafe { WindowHandle::borrow_raw(raw) };
        registry.attach_host(NativeSurfaceHost::from_window_handle(handle));
        registry
    }

    fn add_embed(
        ir: &mut CoreIR,
        node_id: WidgetId,
        widget_id: WidgetId,
        kind: EmbedKind,
    ) -> ExternalSurfaceSlotId {
        ir.add_node(
            node_id,
            Op::Layout(LayoutOp::Embed {
                kind: kind.clone(),
                widget_id,
                width: Some(100.0),
                height: Some(50.0),
            }),
            Vec::new(),
        );
        ExternalSurfaceSlotId(embed_surface_id(&kind, widget_id))
    }

    fn scene_with_slots(slots: &[ExternalSurfaceSlotId]) -> RenderScene {
        let bounds = LayoutRect::new(0.0, 0.0, 300.0, 200.0);
        let mut list = DisplayList::new(bounds);
        for (position, slot_id) in slots.iter().enumerate() {
            list.push(DisplayOp::DrawSurface {
                rect: bounds,
                surface_id: slot_id.0,
                position: position as u64,
                bounds,
                node_id: None,
            });
        }
        RenderScene::from_display_list(list)
    }

    fn scene_with_nonoverlapping_slots(slots: &[ExternalSurfaceSlotId]) -> RenderScene {
        let bounds = LayoutRect::new(0.0, 0.0, 300.0, 200.0);
        let mut list = DisplayList::new(bounds);
        for (position, slot_id) in slots.iter().enumerate() {
            let rect = LayoutRect::new(position as f32 * 100.0, 0.0, 100.0, 50.0);
            list.push(DisplayOp::DrawSurface {
                rect,
                surface_id: slot_id.0,
                position: position as u64,
                bounds: rect,
                node_id: None,
            });
        }
        RenderScene::from_display_list(list)
    }

    fn translation_matrix(x: f32, y: f32) -> [f32; 16] {
        [
            1.0, 0.0, 0.0, 0.0, // x basis
            0.0, 1.0, 0.0, 0.0, // y basis
            0.0, 0.0, 1.0, 0.0, // z basis
            x, y, 0.0, 1.0,
        ]
    }

    fn prepare(
        state: &mut FrameSubmissionState,
        scene: &RenderScene,
        ir: &CoreIR,
        video: &[VideoSurfaceFrame],
        web: &[WebSurfaceFrame],
        custom: &[NativeSurfaceFrame],
    ) -> FrameSubmission {
        let native_views = native_view_registry();
        state
            .prepare(
                scene,
                ir,
                video,
                web,
                custom,
                &native_views,
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(600, 400),
                2.0,
            )
            .unwrap()
    }

    fn video_frame(widget_id: WidgetId, surface_id: u64) -> VideoSurfaceFrame {
        VideoSurfaceFrame {
            widget_id,
            surface_id,
            rect: LayoutRect::new(1.0, 2.0, 100.0, 50.0),
            visible_rect: LayoutRect::new(1.0, 2.0, 90.0, 40.0),
            transform: None,
            opacity: 0.5,
            paint_order: 1,
        }
    }

    fn web_frame(widget_id: WidgetId) -> WebSurfaceFrame {
        WebSurfaceFrame {
            widget_id,
            url: "https://example.invalid".to_string(),
            user_agent: None,
            rect: LayoutRect::new(3.0, 4.0, 100.0, 50.0),
            visible_rect: LayoutRect::new(3.0, 4.0, 80.0, 30.0),
            transform: None,
            opacity: 0.75,
            paint_order: 2,
        }
    }

    fn custom_frame(widget_id: WidgetId) -> NativeSurfaceFrame {
        custom_frame_with_payload(widget_id, vec![1, 2, 3])
    }

    fn custom_frame_with_payload(widget_id: WidgetId, payload: Vec<u8>) -> NativeSurfaceFrame {
        NativeSurfaceFrame {
            widget_id,
            rect: LayoutRect::new(5.0, 6.0, 100.0, 50.0),
            payload,
            visible_rect: LayoutRect::new(5.0, 6.0, 70.0, 20.0),
            transform: None,
            opacity: 0.25,
            paint_order: 3,
        }
    }

    #[test]
    fn video_web_and_custom_frames_map_to_native_view_bindings() {
        let video_id = WidgetId::explicit("frame.video");
        let web_id = WidgetId::explicit("frame.web");
        let custom_id = WidgetId::explicit("frame.custom");
        let mut ir = CoreIR::new();
        let video_slot = add_embed(
            &mut ir,
            WidgetId::derived(1, &[0]),
            video_id,
            EmbedKind::Video,
        );
        let web_slot = add_embed(&mut ir, WidgetId::derived(1, &[1]), web_id, EmbedKind::Web);
        let custom_slot = add_embed(
            &mut ir,
            WidgetId::derived(1, &[2]),
            custom_id,
            EmbedKind::Custom(vec![9]),
        );
        let scene = scene_with_nonoverlapping_slots(&[video_slot, web_slot, custom_slot]);
        let submission = prepare(
            &mut FrameSubmissionState::default(),
            &scene,
            &ir,
            &[video_frame(video_id, 77)],
            &[web_frame(web_id)],
            &[custom_frame(custom_id)],
        );

        let video = submission.external_surfaces().get(video_slot).unwrap();
        let web = submission.external_surfaces().get(web_slot).unwrap();
        let custom = submission.external_surfaces().get(custom_slot).unwrap();
        assert_eq!(video.producer_kind, ExternalProducerKind::Video);
        assert_eq!(web.producer_kind, ExternalProducerKind::WebView);
        assert_eq!(
            custom.producer_kind,
            ExternalProducerKind::Custom("native-view".to_string())
        );
        for binding in [video, web, custom] {
            assert_eq!(binding.state, ExternalFrameState::Ready);
            assert_eq!(binding.transport, ExternalSurfaceTransport::NativeView);
            assert_eq!(binding.frame_token, None);
            assert_eq!(binding.synchronization, ExternalSynchronization::None);
            assert!(!binding.zero_copy);
        }
        assert_ne!(video.producer_id, web.producer_id);
        assert_ne!(web.producer_id, custom.producer_id);
        let staged = submission.staged_surfaces();
        assert_eq!(staged.video().len(), 1);
        assert_eq!(staged.web().len(), 1);
        assert_eq!(staged.native().len(), 1);
        let retained_bounds = LayoutRect::new(0.0, 0.0, 300.0, 200.0);
        assert_eq!(staged.video()[0].rect, retained_bounds);
        assert_eq!(staged.web()[0].rect, retained_bounds);
        assert_eq!(staged.native()[0].rect, retained_bounds);
        assert_eq!(staged.video()[0].opacity, 1.0);
        assert_eq!(staged.web()[0].opacity, 1.0);
        assert_eq!(staged.native()[0].opacity, 1.0);
    }

    #[test]
    fn missing_producer_data_is_bound_as_pending() {
        let video_id = WidgetId::explicit("pending.video");
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(2, &[0]),
            video_id,
            EmbedKind::Video,
        );
        let scene = scene_with_slots(&[slot]);

        let submission = prepare(
            &mut FrameSubmissionState::default(),
            &scene,
            &ir,
            &[],
            &[],
            &[],
        );

        let binding = submission.external_surfaces().get(slot).unwrap();
        assert_eq!(binding.state, ExternalFrameState::Pending);
        assert_eq!(binding.frame_token, None);
    }

    #[test]
    fn video_without_a_platform_surface_remains_pending() {
        let video_id = WidgetId::explicit("pending.video.surface");
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(2, &[1]),
            video_id,
            EmbedKind::Video,
        );
        let scene = scene_with_slots(&[slot]);

        let submission = prepare(
            &mut FrameSubmissionState::default(),
            &scene,
            &ir,
            &[video_frame(video_id, 0)],
            &[],
            &[],
        );

        assert_eq!(
            submission.external_surfaces().get(slot).unwrap().state,
            ExternalFrameState::Pending
        );

        let ready = prepare(
            &mut FrameSubmissionState::default(),
            &scene,
            &ir,
            &[video_frame(video_id, 99)],
            &[],
            &[],
        );
        assert_eq!(
            ready.external_surfaces().get(slot).unwrap().state,
            ExternalFrameState::Ready
        );
        assert_eq!(ready.staged_surfaces().video()[0].surface_id, 99);

        let unavailable = FrameSubmissionState::default()
            .prepare(
                &scene,
                &ir,
                &[video_frame(video_id, 99)],
                &[],
                &[],
                &NativeSurfaceRegistry::default(),
                SurfacePresenterCapabilities::new(
                    PlatformSurfaceCapabilities::UNAVAILABLE,
                    PlatformSurfaceCapabilities::UNAVAILABLE,
                    false,
                ),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap_err();
        assert!(matches!(
            unavailable,
            FrameSubmissionError::UnavailablePlatformPresenter {
                widget_id,
                slot_id,
                kind: "video",
            } if widget_id == video_id && slot_id == slot
        ));
    }

    #[test]
    fn video_runtime_status_drives_external_frame_state() {
        let video_id = WidgetId::explicit("status.video.surface");
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(2, &[3]),
            video_id,
            EmbedKind::Video,
        );
        let scene = scene_with_slots(&[slot]);

        for (status, ready, expected, staged) in [
            (
                VideoStatus::Buffering,
                false,
                ExternalFrameState::Pending,
                0,
            ),
            (VideoStatus::Buffering, true, ExternalFrameState::Ready, 1),
            (VideoStatus::Ended, true, ExternalFrameState::Ready, 1),
            (VideoStatus::Error, true, ExternalFrameState::Failed, 0),
        ] {
            let mut video_states = VideoStateMap::default();
            let state = video_states.states.entry(video_id).or_default();
            state.status = status;
            state.duration_ms = ready.then_some(1_000);
            let submission = FrameSubmissionState::default()
                .prepare_with_states(
                    &scene,
                    &ir,
                    None,
                    &[video_frame(video_id, 99)],
                    &[],
                    &[],
                    &video_states,
                    &WebStateMap::default(),
                    &NativeSurfaceRegistry::default(),
                    SurfacePresenterCapabilities::fully_capable(),
                    LayoutSize::new(300.0, 200.0),
                    PhysicalSize::new(300, 200),
                    1.0,
                )
                .unwrap();

            assert_eq!(
                submission.external_surfaces().get(slot).unwrap().state,
                expected
            );
            assert_eq!(submission.staged_surfaces().video().len(), staged);
        }
    }

    #[test]
    fn pending_web_navigation_is_staged_without_claiming_ready_content() {
        let web_id = WidgetId::explicit("loading.web.surface");
        let mut ir = CoreIR::new();
        let slot = add_embed(&mut ir, WidgetId::derived(2, &[4]), web_id, EmbedKind::Web);
        let scene = scene_with_slots(&[slot]);
        let mut web_states = WebStateMap::default();
        web_states.states.entry(web_id).or_default().loading = true;

        let submission = FrameSubmissionState::default()
            .prepare_with_states(
                &scene,
                &ir,
                None,
                &[],
                &[web_frame(web_id)],
                &[],
                &VideoStateMap::default(),
                &web_states,
                &NativeSurfaceRegistry::default(),
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap();

        assert_eq!(
            submission.external_surfaces().get(slot).unwrap().state,
            ExternalFrameState::Pending
        );
        assert_eq!(submission.staged_surfaces().web().len(), 1);
    }

    #[test]
    fn custom_native_view_waits_for_an_attached_host() {
        let custom_id = WidgetId::explicit("pending.custom.host");
        let payload = vec![7, 8, 9];
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(2, &[2]),
            custom_id,
            EmbedKind::Custom(payload.clone()),
        );
        let scene = scene_with_slots(&[slot]);
        let mut native_views = NativeSurfaceRegistry::default();
        native_views.register(ClaimAllNativeViews);

        let submission = FrameSubmissionState::default()
            .prepare(
                &scene,
                &ir,
                &[],
                &[],
                &[custom_frame_with_payload(custom_id, payload)],
                &native_views,
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap();

        assert_eq!(
            submission.external_surfaces().get(slot).unwrap().state,
            ExternalFrameState::Pending
        );
    }

    #[test]
    fn producer_ids_and_frame_epochs_are_stable_and_monotonic() {
        let web_id = WidgetId::explicit("monotonic.web");
        let mut ir = CoreIR::new();
        let slot = add_embed(&mut ir, WidgetId::derived(3, &[0]), web_id, EmbedKind::Web);
        let scene = scene_with_slots(&[slot]);
        let mut state = FrameSubmissionState::default();

        let first = prepare(&mut state, &scene, &ir, &[], &[web_frame(web_id)], &[]);
        let second = prepare(&mut state, &scene, &ir, &[], &[web_frame(web_id)], &[]);

        assert_eq!(
            first.external_surfaces().get(slot).unwrap().producer_id,
            second.external_surfaces().get(slot).unwrap().producer_id
        );
        assert!(second.metadata().frame_id.0 > first.metadata().frame_id.0);
        assert!(second.metadata().resource_epoch.0 > first.metadata().resource_epoch.0);
        assert!(second.metadata().semantics_epoch.0 > first.metadata().semantics_epoch.0);
        assert_eq!(first.resources().epoch(), first.metadata().resource_epoch);
        assert_eq!(second.resources().epoch(), second.metadata().resource_epoch);
    }

    #[test]
    fn failed_frame_keeps_last_commit_and_next_commit_uses_retained_scroll_geometry_once() {
        use std::cell::RefCell;

        let video_id = WidgetId::explicit("commit.video");
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(3, &[1]),
            video_id,
            EmbedKind::Video,
        );
        let initial_scene = scene_with_slots(&[slot]);
        let producer = video_frame(video_id, 41);
        let native_views = native_view_registry();
        let mut state = FrameSubmissionState::default();
        let initial = state
            .prepare(
                &initial_scene,
                &ir,
                &[producer],
                &[],
                &[],
                &native_views,
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap();
        let published = RefCell::new(Vec::new());
        state
            .commit(&initial, |frames| {
                published.borrow_mut().push(frames.video()[0].rect);
            })
            .unwrap();

        let draw_rect = LayoutRect::new(50.0, 60.0, 80.0, 70.0);
        let mut list = DisplayList::new(draw_rect);
        list.push(DisplayOp::DrawSurface {
            rect: draw_rect,
            surface_id: slot.0,
            position: 0,
            bounds: draw_rect,
            node_id: None,
        });
        let mut translated = fission_render::RenderLayer::new(draw_rect);
        translated.style.transform = Some(translation_matrix(-25.0, -40.0));
        translated.children.push(RenderNode::Paint(list));
        let mut clipped = fission_render::RenderLayer::new(LayoutRect::new(0.0, 0.0, 100.0, 100.0));
        clipped.style.clip = Some(LayerClip::Rect(clipped.bounds));
        clipped.children.push(RenderNode::Layer(translated));
        let mut scrolled_scene = RenderScene::new(LayoutRect::new(0.0, 0.0, 300.0, 200.0));
        scrolled_scene.roots.push(RenderNode::Layer(clipped));
        let next = state
            .prepare(
                &scrolled_scene,
                &ir,
                &[producer],
                &[],
                &[],
                &native_views,
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap();

        let rejecting = GraphicsCapabilities::empty(BackendIdentity::new("rejecting", "1", "test"));
        assert!(next.validate_for(&scrolled_scene, &rejecting).is_err());
        assert_eq!(
            state.last_committed_frame_id(),
            Some(initial.metadata().frame_id)
        );
        assert_eq!(published.borrow().len(), 1);

        state
            .commit(&next, |frames| {
                let frame = frames.video()[0];
                assert_eq!(frame.rect, LayoutRect::new(25.0, 20.0, 80.0, 70.0));
                assert_eq!(frame.visible_rect, LayoutRect::new(25.0, 20.0, 75.0, 70.0));
                published.borrow_mut().push(frame.rect);
            })
            .unwrap();
        let publish_count = published.borrow().len();
        assert!(matches!(
            state.commit(&next, |_| panic!("duplicate commit published twice")),
            Err(FrameSubmissionError::NonMonotonicCommit { .. })
        ));
        assert_eq!(published.borrow().len(), publish_count);
    }

    #[test]
    fn producer_identity_is_stable_and_kind_specific() {
        let widget_id = WidgetId::explicit("producer.identity");
        let video = ProducerDescriptor {
            widget_id,
            kind: ProducerDescriptorKind::Video,
        };
        let web = ProducerDescriptor {
            widget_id,
            kind: ProducerDescriptorKind::Web,
        };

        assert_eq!(producer_id(video), producer_id(video));
        assert_ne!(producer_id(video), producer_id(web));
    }

    #[test]
    fn duplicate_producer_frames_are_rejected() {
        let video_id = WidgetId::explicit("duplicate.video");
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(4, &[0]),
            video_id,
            EmbedKind::Video,
        );
        let scene = scene_with_slots(&[slot]);
        let duplicate = video_frame(video_id, 1);

        let error = FrameSubmissionState::default()
            .prepare(
                &scene,
                &ir,
                &[duplicate, duplicate],
                &[],
                &[],
                &NativeSurfaceRegistry::default(),
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            FrameSubmissionError::DuplicateProducerFrame {
                widget_id,
                kind: "video"
            } if widget_id == video_id
        ));
    }

    #[test]
    fn validation_stops_duplicate_placement_before_encoding() {
        let custom_id = WidgetId::explicit("duplicate.placement");
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(5, &[0]),
            custom_id,
            EmbedKind::Custom(vec![1]),
        );
        let scene = scene_with_slots(&[slot, slot]);
        let submission = prepare(
            &mut FrameSubmissionState::default(),
            &scene,
            &ir,
            &[],
            &[],
            &[custom_frame(custom_id)],
        );

        let error = submission
            .validate_for(&scene, &winit_software_capabilities())
            .unwrap_err();

        assert!(matches!(
            error,
            FrameSubmissionError::FrameGate(FrameGateError::InvalidFrame(
                FrameValidationError::DuplicateSurfacePlacement(duplicate)
            )) if duplicate == slot
        ));
    }

    #[test]
    fn native_view_rejects_overlapping_later_two_d_paint() {
        let video_id = WidgetId::explicit("ordering.video");
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(5, &[3]),
            video_id,
            EmbedKind::Video,
        );
        let bounds = LayoutRect::new(0.0, 0.0, 100.0, 50.0);
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: slot.0,
            position: 0,
            bounds,
            node_id: None,
        });
        list.push(DisplayOp::DrawRect {
            rect: bounds,
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(list);

        let error = FrameSubmissionState::default()
            .prepare(
                &scene,
                &ir,
                &[video_frame(video_id, 1)],
                &[],
                &[],
                &NativeSurfaceRegistry::default(),
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            FrameSubmissionError::UnsupportedSurfaceOrdering {
                slot_id,
                issue: SurfaceOrderingIssue::TwoDPaintAfterDeferredSurface {
                    transport: ExternalSurfaceTransport::NativeView,
                    operation: DisplayOpKind::DrawRect,
                },
            } if slot_id == slot
        ));
    }

    #[test]
    fn unclaimed_custom_surface_fails_instead_of_claiming_native_view() {
        let custom_id = WidgetId::explicit("unclaimed.custom");
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(5, &[1]),
            custom_id,
            EmbedKind::Custom(vec![1, 2, 3]),
        );
        let scene = scene_with_slots(&[slot]);

        let error = FrameSubmissionState::default()
            .prepare(
                &scene,
                &ir,
                &[],
                &[],
                &[custom_frame(custom_id)],
                &NativeSurfaceRegistry::default(),
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            FrameSubmissionError::UnclaimedCustomSurface {
                widget_id,
                slot_id
            } if widget_id == custom_id && slot_id == slot
        ));
    }

    #[cfg(feature = "three-d")]
    #[test]
    fn untagged_bincode_payload_is_not_misclassified_as_three_d() {
        let raw = bincode::serialize(&Vec::<fission_3d_model::Primitive3D>::new()).unwrap();

        assert!(!is_builtin_three_d_payload(&raw));
        assert_eq!(
            ProducerDescriptorKind::from_embed(
                &EmbedKind::Custom(raw),
                WidgetId::explicit("unrelated.3d"),
                &NativeSurfaceRegistry::default(),
            )
            .unwrap(),
            ProducerDescriptorKind::CustomUnclaimed,
        );

        let widget_id = WidgetId::explicit("malformed.3d");
        let mut malformed = fission_3d_model::encode_scene3d_submission(&[]);
        malformed[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert!(matches!(
            ProducerDescriptorKind::from_embed(
                &EmbedKind::Custom(malformed),
                widget_id,
                &NativeSurfaceRegistry::default(),
            ),
            Err(FrameSubmissionError::InvalidThreeDSubmission {
                widget_id: rejected,
                error: fission_3d_model::Scene3DSubmissionError::UnsupportedVersion(2),
            }) if rejected == widget_id
        ));
    }

    #[cfg(feature = "three-d")]
    #[test]
    fn direct_target_rejects_overlapping_later_two_d_paint() {
        let widget_id = WidgetId::explicit("ordering.three-d");
        let payload = fission_3d_model::encode_scene3d_submission(&[]);
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(5, &[4]),
            widget_id,
            EmbedKind::Custom(payload.clone()),
        );
        let bounds = LayoutRect::new(0.0, 0.0, 100.0, 50.0);
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: slot.0,
            position: 0,
            bounds,
            node_id: None,
        });
        list.push(DisplayOp::DrawRect {
            rect: bounds,
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(list);

        let error = FrameSubmissionState::default()
            .prepare(
                &scene,
                &ir,
                &[],
                &[],
                &[custom_frame_with_payload(widget_id, payload)],
                &NativeSurfaceRegistry::default(),
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            FrameSubmissionError::UnsupportedSurfaceOrdering {
                slot_id,
                issue: SurfaceOrderingIssue::TwoDPaintAfterDeferredSurface {
                    transport: ExternalSurfaceTransport::DirectTarget,
                    operation: DisplayOpKind::DrawRect,
                },
            } if slot_id == slot
        ));
    }

    #[cfg(feature = "three-d")]
    #[test]
    fn three_d_uses_transformed_and_clipped_scene_placement() {
        let custom_id = WidgetId::explicit("direct-target.three-d");
        let payload = fission_3d_model::encode_scene3d_submission(&[]);
        let mut ir = CoreIR::new();
        let slot = add_embed(
            &mut ir,
            WidgetId::derived(5, &[2]),
            custom_id,
            EmbedKind::Custom(payload.clone()),
        );
        let draw_rect = LayoutRect::new(50.0, 60.0, 80.0, 70.0);
        let mut list = DisplayList::new(draw_rect);
        list.push(DisplayOp::DrawSurface {
            rect: draw_rect,
            surface_id: slot.0,
            position: 0,
            bounds: draw_rect,
            node_id: None,
        });
        let mut translated = fission_render::RenderLayer::new(draw_rect);
        translated.style.transform = Some(translation_matrix(-25.0, -40.0));
        translated.children.push(RenderNode::Paint(list));
        let mut clipped = fission_render::RenderLayer::new(LayoutRect::new(0.0, 0.0, 100.0, 100.0));
        clipped.style.clip = Some(LayerClip::Rect(clipped.bounds));
        clipped.children.push(RenderNode::Layer(translated));
        let mut scene = RenderScene::new(LayoutRect::new(0.0, 0.0, 300.0, 200.0));
        scene.roots.push(RenderNode::Layer(clipped));
        let producer_frame = custom_frame_with_payload(custom_id, payload);
        assert_ne!(producer_frame.rect, draw_rect);

        let submission = FrameSubmissionState::default()
            .prepare(
                &scene,
                &ir,
                &[],
                &[],
                &[producer_frame],
                &NativeSurfaceRegistry::default(),
                SurfacePresenterCapabilities::fully_capable(),
                LayoutSize::new(300.0, 200.0),
                PhysicalSize::new(300, 200),
                1.0,
            )
            .unwrap();

        let binding = submission.external_surfaces().get(slot).unwrap();
        assert_eq!(binding.producer_kind, ExternalProducerKind::ThreeD);
        assert_eq!(binding.transport, ExternalSurfaceTransport::DirectTarget);
        assert_eq!(binding.state, ExternalFrameState::Ready);
        assert_eq!(submission.direct_target_three_d().len(), 1);
        assert_eq!(
            submission.direct_target_three_d()[0].viewport,
            LayoutRect::new(25.0, 20.0, 80.0, 70.0),
            "ancestor scroll-style transforms must move the retained DrawSurface"
        );
        assert_eq!(
            submission.direct_target_three_d()[0].clip,
            LayoutRect::new(25.0, 20.0, 75.0, 70.0),
            "ancestor clips must remain separate from the full transformed viewport"
        );
        assert_eq!(submission.direct_target_three_d()[0].opacity, 1.0);
        submission
            .validate_for(&scene, &winit_vello_capabilities(RenderMode::Gpu))
            .unwrap();
        let mut without_direct_target = winit_vello_capabilities(RenderMode::Gpu);
        without_direct_target
            .external_surface_transports
            .remove(&ExternalSurfaceTransport::DirectTarget);
        assert!(matches!(
            submission
                .validate_for(&scene, &without_direct_target)
                .unwrap_err(),
            FrameSubmissionError::FrameGate(FrameGateError::UnsupportedExternalSurfaces(_))
        ));
    }

    #[test]
    fn validation_rejects_unsupported_operations_before_encoding() {
        let bounds = LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::DrawRect {
            rect: bounds,
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(list);
        let submission = prepare(
            &mut FrameSubmissionState::default(),
            &scene,
            &CoreIR::new(),
            &[],
            &[],
            &[],
        );
        let capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("rejecting", "1", "test"));

        let error = submission.validate_for(&scene, &capabilities).unwrap_err();

        assert!(matches!(
            &error,
            FrameSubmissionError::FrameGate(FrameGateError::UnsupportedOperations(_))
        ));
        assert!(error.to_string().contains("DrawRect"));
    }
}
