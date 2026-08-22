use fission_render::external_surface::{ExternalFrameState, ExternalSurfaceBindings};
use fission_render::frame::InteractiveFrame;
use fission_render::{
    surface_placeholder_color, Color, DisplayList, DisplayOp, Fill, RenderNode, RenderScene,
};

pub(super) struct HostComposition {
    pub(super) scene: RenderScene,
    pub(super) external_surfaces: ExternalSurfaceBindings,
}

/// A frame transformed for host-owned native-view composition.
///
/// The transformed scene and its filtered bindings are owned together so a
/// caller cannot accidentally submit the original bindings after DrawSurface
/// operations have been removed or replaced.
pub(crate) struct HostCompositedFrame<'a> {
    submission: &'a super::FrameSubmission,
    composition: HostComposition,
}

impl<'a> HostCompositedFrame<'a> {
    pub(super) fn new(submission: &'a super::FrameSubmission, scene: &RenderScene) -> Self {
        Self {
            submission,
            composition: compose_host_frame(scene, &submission.external_surfaces),
        }
    }

    pub(crate) fn interactive_frame(&self) -> InteractiveFrame<'_> {
        InteractiveFrame::new(
            &self.composition.scene,
            &self.submission.metadata,
            &self.submission.resources,
            &self.composition.external_surfaces,
        )
        .with_paragraphs(&self.submission.paragraphs)
    }
}

impl super::FrameSubmission {
    pub(crate) fn host_composited_frame<'a>(
        &'a self,
        scene: &RenderScene,
    ) -> HostCompositedFrame<'a> {
        HostCompositedFrame::new(self, scene)
    }
}

pub(super) fn compose_host_frame(
    scene: &RenderScene,
    bindings: &ExternalSurfaceBindings,
) -> HostComposition {
    HostComposition {
        scene: compose_host_scene(scene, bindings),
        // Every DrawSurface is either removed for a Ready native view or
        // replaced with an explicit 2D disposition. Keeping its original
        // binding would make the transformed InteractiveFrame invalid because
        // the binding would no longer have a matching placement.
        external_surfaces: ExternalSurfaceBindings::new(),
    }
}

pub(super) fn compose_host_scene(
    scene: &RenderScene,
    bindings: &ExternalSurfaceBindings,
) -> RenderScene {
    RenderScene {
        bounds: scene.bounds,
        roots: scene
            .roots
            .iter()
            .map(|node| compose_node(node, bindings))
            .collect(),
    }
}

fn compose_node(node: &RenderNode, bindings: &ExternalSurfaceBindings) -> RenderNode {
    match node {
        RenderNode::Paint(list) => RenderNode::Paint(compose_list(list, bindings)),
        RenderNode::Layer(layer) => {
            let mut composed = layer.clone();
            if layer.children.iter().any(node_contains_surface) {
                // Surface disposition is frame state, not cached paint. An
                // ancestor cache key must not reuse a scene encoded for a
                // different Ready/Pending/Failed/Ended transition.
                composed.style.cache_key = None;
                composed.style.content_cache_key = None;
            }
            composed.children = layer
                .children
                .iter()
                .map(|child| compose_node(child, bindings))
                .collect();
            RenderNode::Layer(composed)
        }
    }
}

fn compose_list(list: &DisplayList, bindings: &ExternalSurfaceBindings) -> DisplayList {
    let mut composed = DisplayList::new(list.bounds);
    for operation in &list.ops {
        match operation {
            DisplayOp::CachedScene {
                cache_key,
                bounds,
                list,
            } => {
                if list_contains_surface(list) {
                    // Flatten only this cache boundary. Reusing its stable key
                    // would make producer-state transitions visually stale.
                    composed.ops.extend(compose_list(list, bindings).ops);
                } else {
                    composed.push(DisplayOp::CachedScene {
                        cache_key: *cache_key,
                        bounds: *bounds,
                        list: list.clone(),
                    });
                }
            }
            DisplayOp::DrawSurface {
                rect,
                surface_id,
                position,
                bounds,
                node_id,
            } => {
                let binding = bindings
                    .get(fission_render::external_surface::ExternalSurfaceSlotId(
                        *surface_id,
                    ))
                    .expect("validated host composition is missing a surface binding");
                if binding.state != ExternalFrameState::Ready {
                    composed.push(DisplayOp::DrawRect {
                        rect: *rect,
                        fill: Some(Fill::Solid(disposition_color(
                            binding.state,
                            *surface_id,
                            *position,
                        ))),
                        stroke: None,
                        corner_radius: 0.0,
                        shadow: None,
                        bounds: *bounds,
                        node_id: *node_id,
                    });
                }
            }
            operation => composed.push(operation.clone()),
        }
    }
    composed
}

fn node_contains_surface(node: &RenderNode) -> bool {
    match node {
        RenderNode::Layer(layer) => layer.children.iter().any(node_contains_surface),
        RenderNode::Paint(list) => list_contains_surface(list),
    }
}

fn list_contains_surface(list: &DisplayList) -> bool {
    list.ops.iter().any(|operation| match operation {
        DisplayOp::DrawSurface { .. } => true,
        DisplayOp::CachedScene { list, .. } => list_contains_surface(list),
        _ => false,
    })
}

fn disposition_color(state: ExternalFrameState, surface_id: u64, position: u64) -> Color {
    match state {
        ExternalFrameState::Pending => surface_placeholder_color(surface_id, position),
        ExternalFrameState::Failed => Color {
            r: 176,
            g: 35,
            b: 49,
            a: 255,
        },
        ExternalFrameState::Ended => Color {
            r: 96,
            g: 96,
            b: 96,
            a: 255,
        },
        ExternalFrameState::Ready => Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_render::capabilities::ExternalSurfaceTransport;
    use fission_render::external_surface::{
        ExternalAlphaType, ExternalColorSpace, ExternalFrameId, ExternalOwnership,
        ExternalProducerId, ExternalProducerKind, ExternalSurfaceBinding, ExternalSurfaceSlotId,
        ExternalSynchronization,
    };
    use fission_render::frame::{
        DamageRegion, FrameId, FrameMetadata, FrameViewport, ResourceEpoch, SemanticsEpoch,
    };
    use fission_render::resource::ResourceSnapshot;
    use fission_render::surface::{PhysicalSize, ScaleFactor};
    use fission_render::LayoutRect;
    use fission_render::LayoutSize;

    fn binding(
        slot_id: ExternalSurfaceSlotId,
        state: ExternalFrameState,
    ) -> ExternalSurfaceBinding {
        ExternalSurfaceBinding {
            slot_id,
            producer_id: ExternalProducerId(1),
            producer_kind: ExternalProducerKind::Video,
            frame_id: ExternalFrameId(1),
            frame_token: None,
            state,
            transport: ExternalSurfaceTransport::NativeView,
            color_space: ExternalColorSpace::Srgb,
            alpha_type: ExternalAlphaType::Opaque,
            ownership: ExternalOwnership::Shared,
            synchronization: ExternalSynchronization::None,
            zero_copy: false,
            damaged: true,
        }
    }

    fn scene(slot_id: ExternalSurfaceSlotId) -> RenderScene {
        let bounds = LayoutRect::new(1.0, 2.0, 30.0, 40.0);
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: slot_id.0,
            position: 7,
            bounds,
            node_id: None,
        });
        RenderScene::from_display_list(list)
    }

    #[test]
    fn ready_surface_is_transparent_and_pending_surface_has_placeholder() {
        let slot_id = ExternalSurfaceSlotId(9);
        let mut ready = ExternalSurfaceBindings::new();
        ready
            .insert(binding(slot_id, ExternalFrameState::Ready))
            .unwrap();
        assert!(compose_host_scene(&scene(slot_id), &ready)
            .flatten()
            .ops
            .is_empty());

        let mut pending = ExternalSurfaceBindings::new();
        pending
            .insert(binding(slot_id, ExternalFrameState::Pending))
            .unwrap();
        assert!(matches!(
            compose_host_scene(&scene(slot_id), &pending)
                .flatten()
                .ops
                .as_slice(),
            [DisplayOp::DrawRect { .. }]
        ));
    }

    #[test]
    fn host_composition_removes_bindings_with_their_surface_placements() {
        let slot_id = ExternalSurfaceSlotId(9);
        let mut bindings = ExternalSurfaceBindings::new();
        bindings
            .insert(binding(slot_id, ExternalFrameState::Ready))
            .unwrap();

        let composition = compose_host_frame(&scene(slot_id), &bindings);

        assert!(composition.scene.flatten().ops.is_empty());
        assert!(composition.external_surfaces.is_empty());

        let metadata = FrameMetadata {
            frame_id: FrameId(1),
            viewport: FrameViewport {
                logical_size: LayoutSize::new(30.0, 40.0),
                physical_size: PhysicalSize::new(30, 40),
                scale_factor: ScaleFactor::ONE,
            },
            damage: DamageRegion::Full,
            resource_epoch: ResourceEpoch(1),
            semantics_epoch: SemanticsEpoch(1),
        };
        let resources = ResourceSnapshot::empty(ResourceEpoch(1));
        InteractiveFrame::new(
            &composition.scene,
            &metadata,
            &resources,
            &composition.external_surfaces,
        )
        .validate()
        .unwrap();
    }

    #[test]
    fn stateful_surface_rewrite_bypasses_display_and_layer_caches() {
        let slot_id = ExternalSurfaceSlotId(12);
        let bounds = LayoutRect::new(0.0, 0.0, 20.0, 20.0);
        let mut nested = DisplayList::new(bounds);
        nested.push(DisplayOp::DrawSurface {
            rect: bounds,
            surface_id: slot_id.0,
            position: 0,
            bounds,
            node_id: None,
        });
        let mut cached = DisplayList::new(bounds);
        cached.push(DisplayOp::CachedScene {
            cache_key: 42,
            bounds,
            list: Box::new(nested),
        });
        let mut layer = fission_render::RenderLayer::new(bounds);
        layer.style.cache_key = Some(7);
        layer.style.content_cache_key = Some(8);
        layer.children.push(RenderNode::Paint(cached));
        let mut source = RenderScene::new(bounds);
        source.roots.push(RenderNode::Layer(layer));

        for (state, expects_placeholder) in [
            (ExternalFrameState::Ready, false),
            (ExternalFrameState::Pending, true),
        ] {
            let mut bindings = ExternalSurfaceBindings::new();
            bindings.insert(binding(slot_id, state)).unwrap();
            let composed = compose_host_scene(&source, &bindings);
            let RenderNode::Layer(layer) = &composed.roots[0] else {
                panic!("expected composed layer");
            };
            assert_eq!(layer.style.cache_key, None);
            assert_eq!(layer.style.content_cache_key, None);
            assert_eq!(
                composed
                    .flatten()
                    .ops
                    .iter()
                    .any(|operation| matches!(operation, DisplayOp::DrawRect { .. })),
                expects_placeholder
            );
        }
    }
}
