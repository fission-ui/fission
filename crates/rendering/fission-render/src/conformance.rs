use std::fmt;

use fission_ir::WidgetId;
use serde::{Deserialize, Serialize};

use crate::capabilities::{DisplayOpKind, GraphicsCapabilities};
use crate::frame::FrameId;
use crate::{DisplayList, DisplayOp, LayerClip, LayoutRect, RenderLayer, RenderNode, RenderScene};

/// Exact location of an operation in a retained frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameOperationSource {
    /// An operation stored in a display list.
    ///
    /// `node_path` contains child indices below the selected root.
    /// `operation_path` contains the operation index at each nested cached-list
    /// level, so `[4, 2]` means operation 2 inside cached operation 4.
    DisplayList {
        root_index: usize,
        node_path: Vec<usize>,
        operation_path: Vec<usize>,
    },
    /// An operation implied by `RenderLayer::style` and emitted by flattening.
    LayerStyle {
        root_index: usize,
        node_path: Vec<usize>,
        property: LayerOperationSource,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerOperationSource {
    Save,
    Clip,
    Opacity,
    Transform,
    Restore,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayOpProvenance {
    pub frame_id: FrameId,
    pub node_id: Option<WidgetId>,
    pub bounds: Option<LayoutRect>,
    pub source: FrameOperationSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnsupportedDisplayOperation {
    pub operation: DisplayOpKind,
    pub provenance: DisplayOpProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameConformanceError {
    pub backend: crate::capabilities::BackendIdentity,
    pub unsupported_operations: Vec<UnsupportedDisplayOperation>,
}

impl fmt::Display for FrameConformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "backend {} ({}) does not support {} operation(s) in this frame",
            self.backend.name,
            self.backend.profile,
            self.unsupported_operations.len()
        )
    }
}

impl std::error::Error for FrameConformanceError {}

pub(crate) fn validate_scene_capabilities(
    frame_id: FrameId,
    scene: &RenderScene,
    capabilities: &GraphicsCapabilities,
) -> Result<(), FrameConformanceError> {
    let mut visitor = ConformanceVisitor {
        frame_id,
        capabilities,
        unsupported_operations: Vec::new(),
    };

    for (root_index, root) in scene.roots.iter().enumerate() {
        visitor.visit_node(root, root_index, &mut Vec::new(), None);
    }

    if visitor.unsupported_operations.is_empty() {
        Ok(())
    } else {
        Err(FrameConformanceError {
            backend: capabilities.identity.clone(),
            unsupported_operations: visitor.unsupported_operations,
        })
    }
}

struct ConformanceVisitor<'a> {
    frame_id: FrameId,
    capabilities: &'a GraphicsCapabilities,
    unsupported_operations: Vec<UnsupportedDisplayOperation>,
}

impl ConformanceVisitor<'_> {
    fn visit_node(
        &mut self,
        node: &RenderNode,
        root_index: usize,
        node_path: &mut Vec<usize>,
        inherited_node_id: Option<WidgetId>,
    ) {
        match node {
            RenderNode::Paint(list) => self.visit_display_list(
                list,
                root_index,
                node_path,
                &mut Vec::new(),
                inherited_node_id,
            ),
            RenderNode::Layer(layer) => {
                self.visit_layer(layer, root_index, node_path, inherited_node_id)
            }
        }
    }

    fn visit_layer(
        &mut self,
        layer: &RenderLayer,
        root_index: usize,
        node_path: &mut Vec<usize>,
        inherited_node_id: Option<WidgetId>,
    ) {
        let node_id = layer.node_id.or(inherited_node_id);
        let needs_save = layer.style.clip.is_some()
            || layer.style.transform.is_some()
            || (layer.style.opacity - 1.0).abs() > 0.001;

        if needs_save {
            self.check_layer_operation(
                DisplayOpKind::Save,
                root_index,
                node_path,
                node_id,
                LayerOperationSource::Save,
                Some(layer.bounds),
            );
        }
        if let Some(clip) = &layer.style.clip {
            let (operation, bounds) = match clip {
                LayerClip::Rect(rect) => (DisplayOpKind::ClipRect, *rect),
                LayerClip::RoundedRect { rect, .. } => (DisplayOpKind::ClipRoundedRect, *rect),
            };
            self.check_layer_operation(
                operation,
                root_index,
                node_path,
                node_id,
                LayerOperationSource::Clip,
                Some(bounds),
            );
        }
        if (layer.style.opacity - 1.0).abs() > 0.001 {
            self.check_layer_operation(
                DisplayOpKind::OpacityLayer,
                root_index,
                node_path,
                node_id,
                LayerOperationSource::Opacity,
                Some(layer.bounds),
            );
        }
        if layer.style.transform.is_some() {
            self.check_layer_operation(
                DisplayOpKind::Transform,
                root_index,
                node_path,
                node_id,
                LayerOperationSource::Transform,
                Some(layer.bounds),
            );
        }

        for (child_index, child) in layer.children.iter().enumerate() {
            node_path.push(child_index);
            self.visit_node(child, root_index, node_path, node_id);
            node_path.pop();
        }

        if needs_save {
            self.check_layer_operation(
                DisplayOpKind::Restore,
                root_index,
                node_path,
                node_id,
                LayerOperationSource::Restore,
                Some(layer.bounds),
            );
        }
    }

    fn check_layer_operation(
        &mut self,
        operation: DisplayOpKind,
        root_index: usize,
        node_path: &[usize],
        node_id: Option<WidgetId>,
        property: LayerOperationSource,
        bounds: Option<LayoutRect>,
    ) {
        if self.capabilities.supports_display_op(operation) {
            return;
        }
        self.unsupported_operations
            .push(UnsupportedDisplayOperation {
                operation,
                provenance: DisplayOpProvenance {
                    frame_id: self.frame_id,
                    node_id,
                    bounds,
                    source: FrameOperationSource::LayerStyle {
                        root_index,
                        node_path: node_path.to_vec(),
                        property,
                    },
                },
            });
    }

    fn visit_display_list(
        &mut self,
        list: &DisplayList,
        root_index: usize,
        node_path: &[usize],
        operation_path: &mut Vec<usize>,
        inherited_node_id: Option<WidgetId>,
    ) {
        for (operation_index, operation) in list.ops.iter().enumerate() {
            operation_path.push(operation_index);
            let kind = operation.kind();
            if !self.capabilities.supports_display_op(kind) {
                self.unsupported_operations
                    .push(UnsupportedDisplayOperation {
                        operation: kind,
                        provenance: DisplayOpProvenance {
                            frame_id: self.frame_id,
                            node_id: operation_node_id(operation).or(inherited_node_id),
                            bounds: operation_bounds(operation, list.bounds),
                            source: FrameOperationSource::DisplayList {
                                root_index,
                                node_path: node_path.to_vec(),
                                operation_path: operation_path.clone(),
                            },
                        },
                    });
            }

            if let DisplayOp::CachedScene { list, .. } = operation {
                self.visit_display_list(
                    list,
                    root_index,
                    node_path,
                    operation_path,
                    inherited_node_id,
                );
            }
            operation_path.pop();
        }
    }
}

fn operation_node_id(operation: &DisplayOp) -> Option<WidgetId> {
    match operation {
        DisplayOp::BackdropFilter { node_id, .. }
        | DisplayOp::DrawRect { node_id, .. }
        | DisplayOp::DrawText { node_id, .. }
        | DisplayOp::DrawRichText { node_id, .. }
        | DisplayOp::DrawImage { node_id, .. }
        | DisplayOp::DrawPath { node_id, .. }
        | DisplayOp::DrawSvg { node_id, .. }
        | DisplayOp::DrawSurface { node_id, .. } => *node_id,
        DisplayOp::Save
        | DisplayOp::Restore
        | DisplayOp::ClipRect(_)
        | DisplayOp::ClipRoundedRect { .. }
        | DisplayOp::OpacityLayer { .. }
        | DisplayOp::Translate(_)
        | DisplayOp::Transform(_)
        | DisplayOp::CachedScene { .. } => None,
    }
}

fn operation_bounds(operation: &DisplayOp, list_bounds: LayoutRect) -> Option<LayoutRect> {
    match operation {
        DisplayOp::ClipRect(rect) => Some(*rect),
        DisplayOp::ClipRoundedRect { rect, .. }
        | DisplayOp::OpacityLayer { bounds: rect, .. }
        | DisplayOp::CachedScene { bounds: rect, .. }
        | DisplayOp::BackdropFilter { bounds: rect, .. }
        | DisplayOp::DrawRect { bounds: rect, .. }
        | DisplayOp::DrawText { bounds: rect, .. }
        | DisplayOp::DrawRichText { bounds: rect, .. }
        | DisplayOp::DrawImage { bounds: rect, .. }
        | DisplayOp::DrawPath { bounds: rect, .. }
        | DisplayOp::DrawSvg { bounds: rect, .. }
        | DisplayOp::DrawSurface { bounds: rect, .. } => Some(*rect),
        DisplayOp::Save
        | DisplayOp::Restore
        | DisplayOp::Translate(_)
        | DisplayOp::Transform(_) => Some(list_bounds),
    }
}

#[cfg(test)]
mod tests {
    use fission_ir::op::{BackdropFilter, ImageAlignment, ImageRequest};

    use super::*;
    use crate::capabilities::{BackendIdentity, ExternalSurfaceTransport, GraphicsCapabilities};
    use crate::external_surface::{
        ExternalAlphaType, ExternalColorSpace, ExternalFrameId, ExternalFrameState,
        ExternalFrameToken, ExternalOwnership, ExternalProducerId, ExternalProducerKind,
        ExternalSurfaceBinding, ExternalSurfaceBindings, ExternalSurfaceSlotId,
        ExternalSynchronization,
    };
    use crate::frame::{
        DamageRegion, FrameMetadata, FrameViewport, InteractiveFrame, ResourceEpoch, SemanticsEpoch,
    };
    use crate::resource::ResourceSnapshot;
    use crate::surface::{PhysicalSize, ScaleFactor};
    use crate::{Color, ImageFit, LayerStyle, LayoutPoint, LayoutSize, RenderLayer, TextRun};

    fn bounds() -> LayoutRect {
        LayoutRect::new(0.0, 0.0, 100.0, 80.0)
    }

    fn metadata() -> FrameMetadata {
        FrameMetadata {
            frame_id: FrameId(91),
            viewport: FrameViewport {
                logical_size: LayoutSize {
                    width: 100.0,
                    height: 80.0,
                },
                physical_size: PhysicalSize::new(100, 80),
                scale_factor: ScaleFactor::ONE,
            },
            damage: DamageRegion::Full,
            resource_epoch: ResourceEpoch(1),
            semantics_epoch: SemanticsEpoch(2),
        }
    }

    fn capabilities() -> GraphicsCapabilities {
        let mut capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("test-backend", "1", "test-profile"));
        capabilities
            .display_ops
            .extend(DisplayOpKind::ALL.iter().copied());
        capabilities
            .external_surface_transports
            .insert(ExternalSurfaceTransport::GpuImage);
        capabilities
    }

    fn surface_binding(slot_id: u64) -> ExternalSurfaceBinding {
        ExternalSurfaceBinding {
            slot_id: ExternalSurfaceSlotId(slot_id),
            producer_id: ExternalProducerId(10),
            producer_kind: ExternalProducerKind::Video,
            frame_id: ExternalFrameId(11),
            frame_token: Some(ExternalFrameToken(12)),
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
    fn nested_and_layer_operations_report_every_capability_gap_with_provenance() {
        let bounds = bounds();
        let layer_id = WidgetId::explicit("conformance.layer");
        let svg_id = WidgetId::explicit("conformance.svg");
        let mut cached = DisplayList::new(bounds);
        cached.push(DisplayOp::DrawSvg {
            content: "<svg/>".into(),
            fill: None,
            stroke: None,
            bounds,
            node_id: Some(svg_id),
        });
        cached.push(DisplayOp::DrawPath {
            path: "M 0 0 L 10 10".into(),
            fill: None,
            stroke: None,
            bounds,
            node_id: None,
        });
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::CachedScene {
            cache_key: 7,
            bounds,
            list: Box::new(cached),
        });
        let mut layer = RenderLayer::new(bounds);
        layer.node_id = Some(layer_id);
        layer.style = LayerStyle {
            clip: Some(LayerClip::RoundedRect {
                rect: bounds,
                radius: 8.0,
            }),
            ..LayerStyle::default()
        };
        layer.children.push(RenderNode::Paint(list));
        let mut scene = RenderScene::new(bounds);
        scene.roots.push(RenderNode::Layer(layer));
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let bindings = ExternalSurfaceBindings::new();
        let frame = InteractiveFrame::new(&scene, &metadata, &resources, &bindings);
        let mut capabilities = capabilities();
        capabilities
            .display_ops
            .remove(&DisplayOpKind::ClipRoundedRect);
        capabilities.display_ops.remove(&DisplayOpKind::DrawSvg);
        capabilities.display_ops.remove(&DisplayOpKind::DrawPath);

        let error = frame.validate_for(&capabilities).unwrap_err();
        let crate::frame::FrameGateError::UnsupportedOperations(error) = error else {
            panic!("expected capability failure");
        };

        assert_eq!(error.unsupported_operations.len(), 3);
        let layer_gap = &error.unsupported_operations[0];
        assert_eq!(layer_gap.operation, DisplayOpKind::ClipRoundedRect);
        assert_eq!(layer_gap.provenance.node_id, Some(layer_id));
        assert_eq!(
            layer_gap.provenance.source,
            FrameOperationSource::LayerStyle {
                root_index: 0,
                node_path: vec![],
                property: LayerOperationSource::Clip,
            }
        );

        let svg_gap = &error.unsupported_operations[1];
        assert_eq!(svg_gap.operation, DisplayOpKind::DrawSvg);
        assert_eq!(svg_gap.provenance.node_id, Some(svg_id));
        assert_eq!(
            svg_gap.provenance.source,
            FrameOperationSource::DisplayList {
                root_index: 0,
                node_path: vec![0],
                operation_path: vec![0, 0],
            }
        );

        let path_gap = &error.unsupported_operations[2];
        assert_eq!(path_gap.operation, DisplayOpKind::DrawPath);
        assert_eq!(path_gap.provenance.node_id, Some(layer_id));
        assert_eq!(
            path_gap.provenance.source,
            FrameOperationSource::DisplayList {
                root_index: 0,
                node_path: vec![0],
                operation_path: vec![0, 1],
            }
        );
    }

    #[test]
    fn complete_supported_operation_set_produces_a_validated_frame() {
        let bounds = bounds();
        let mut nested = DisplayList::new(bounds);
        nested.push(DisplayOp::DrawRect {
            rect: bounds,
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds,
            node_id: None,
        });
        let mut list = DisplayList::new(bounds);
        list.ops = vec![
            DisplayOp::Save,
            DisplayOp::ClipRect(bounds),
            DisplayOp::ClipRoundedRect {
                rect: bounds,
                radius: 4.0,
            },
            DisplayOp::OpacityLayer { alpha: 0.5, bounds },
            DisplayOp::Translate(LayoutPoint::new(1.0, 2.0)),
            DisplayOp::Transform([0.0; 16]),
            DisplayOp::CachedScene {
                cache_key: 1,
                bounds,
                list: Box::new(nested),
            },
            DisplayOp::BackdropFilter {
                rect: bounds,
                filter: BackdropFilter::Blur(2.0),
                corner_radius: 4.0,
                bounds,
                node_id: None,
            },
            DisplayOp::DrawRect {
                rect: bounds,
                fill: None,
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
                bounds,
                node_id: None,
            },
            DisplayOp::DrawText {
                text: "text".into(),
                position: LayoutPoint::new(0.0, 0.0),
                size: 14.0,
                color: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                bounds,
                node_id: None,
                underline: false,
                wrap: false,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style: None,
            },
            DisplayOp::DrawRichText {
                runs: Vec::<TextRun>::new(),
                position: LayoutPoint::new(0.0, 0.0),
                bounds,
                node_id: None,
                wrap: false,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style: None,
                annotations: Vec::new(),
            },
            DisplayOp::DrawImage {
                rect: bounds,
                request: ImageRequest::default(),
                fit: ImageFit::Contain,
                alignment: ImageAlignment::Center,
                bounds,
                node_id: None,
            },
            DisplayOp::DrawPath {
                path: "M 0 0".into(),
                fill: None,
                stroke: None,
                bounds,
                node_id: None,
            },
            DisplayOp::DrawSvg {
                content: "<svg/>".into(),
                fill: None,
                stroke: None,
                bounds,
                node_id: None,
            },
            DisplayOp::DrawSurface {
                rect: bounds,
                surface_id: 42,
                position: 0,
                bounds,
                node_id: None,
            },
            DisplayOp::Restore,
        ];
        let scene = RenderScene::from_display_list(list);
        let metadata = metadata();
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let mut bindings = ExternalSurfaceBindings::new();
        bindings.insert(surface_binding(42)).unwrap();
        let frame = InteractiveFrame::new(&scene, &metadata, &resources, &bindings);
        let capabilities = capabilities();

        let validated = frame.validate_for(&capabilities).unwrap();

        assert_eq!(validated.validated_for(), &capabilities.identity);
        assert_eq!(validated.frame().metadata().frame_id, FrameId(91));
    }

    #[test]
    fn traversal_is_depth_first_across_every_root_layer_and_cached_scene_level() {
        let bounds = bounds();
        let root_layer_id = WidgetId::explicit("conformance.deep-root");

        let mut root_zero_leaf = DisplayList::new(bounds);
        root_zero_leaf.push(DisplayOp::DrawPath {
            path: "M 0 0 L 4 4".into(),
            fill: None,
            stroke: None,
            bounds,
            node_id: None,
        });
        let mut root_zero_middle = DisplayList::new(bounds);
        root_zero_middle.push(DisplayOp::CachedScene {
            cache_key: 2,
            bounds,
            list: Box::new(root_zero_leaf),
        });
        let mut root_zero = DisplayList::new(bounds);
        root_zero.push(DisplayOp::CachedScene {
            cache_key: 1,
            bounds,
            list: Box::new(root_zero_middle),
        });

        let mut root_one_leaf = DisplayList::new(bounds);
        root_one_leaf.push(DisplayOp::DrawSvg {
            content: "<svg/>".into(),
            fill: None,
            stroke: None,
            bounds,
            node_id: None,
        });
        let mut root_one_middle = DisplayList::new(bounds);
        root_one_middle.push(DisplayOp::CachedScene {
            cache_key: 4,
            bounds,
            list: Box::new(root_one_leaf),
        });
        let mut root_one_list = DisplayList::new(bounds);
        root_one_list.push(DisplayOp::CachedScene {
            cache_key: 3,
            bounds,
            list: Box::new(root_one_middle),
        });
        let mut nested_layer = RenderLayer::new(bounds);
        nested_layer.style.transform = Some([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]);
        nested_layer.children.push(RenderNode::Paint(root_one_list));
        let mut root_layer = RenderLayer::new(bounds);
        root_layer.node_id = Some(root_layer_id);
        root_layer.style.opacity = 0.5;
        root_layer.children.push(RenderNode::Layer(nested_layer));

        let mut scene = RenderScene::new(bounds);
        scene.roots.push(RenderNode::Paint(root_zero));
        scene.roots.push(RenderNode::Layer(root_layer));
        let mut capabilities = capabilities();
        for unsupported in [
            DisplayOpKind::DrawPath,
            DisplayOpKind::OpacityLayer,
            DisplayOpKind::Transform,
            DisplayOpKind::DrawSvg,
        ] {
            capabilities.display_ops.remove(&unsupported);
        }

        let error = validate_scene_capabilities(FrameId(92), &scene, &capabilities).unwrap_err();

        assert_eq!(
            error
                .unsupported_operations
                .iter()
                .map(|operation| operation.operation)
                .collect::<Vec<_>>(),
            vec![
                DisplayOpKind::DrawPath,
                DisplayOpKind::OpacityLayer,
                DisplayOpKind::Transform,
                DisplayOpKind::DrawSvg,
            ]
        );
        assert_eq!(
            error.unsupported_operations[0].provenance.source,
            FrameOperationSource::DisplayList {
                root_index: 0,
                node_path: vec![],
                operation_path: vec![0, 0, 0],
            }
        );
        assert_eq!(
            error.unsupported_operations[1].provenance.source,
            FrameOperationSource::LayerStyle {
                root_index: 1,
                node_path: vec![],
                property: LayerOperationSource::Opacity,
            }
        );
        assert_eq!(
            error.unsupported_operations[2].provenance.source,
            FrameOperationSource::LayerStyle {
                root_index: 1,
                node_path: vec![0],
                property: LayerOperationSource::Transform,
            }
        );
        assert_eq!(
            error.unsupported_operations[3].provenance.source,
            FrameOperationSource::DisplayList {
                root_index: 1,
                node_path: vec![0, 0],
                operation_path: vec![0, 0, 0],
            }
        );
        assert!(error
            .unsupported_operations
            .iter()
            .skip(1)
            .all(|operation| operation.provenance.node_id == Some(root_layer_id)));
    }
}
