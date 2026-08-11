use std::collections::BTreeMap;

use fission_ir::op::ImageSource;
use fission_ir::{CoreIR, Op, PaintOp, WidgetId};
use fission_render::frame::ResourceEpoch;
use fission_render::resource::{
    ResourceContentIdentity, ResourceEntry, ResourceId, ResourceKind, ResourcePayload,
    ResourceProvenance, ResourceSnapshot, ResourceSource,
};

use super::{take_monotonic, FrameSubmissionError};

/// Assigns backend-neutral resource identities to stable image nodes.
///
/// The mapping lives for the shell runtime lifetime so a node keeps the same
/// logical resource id when its immutable content version changes. Backends
/// use the snapshot's content identity to invalidate derived decode caches.
#[derive(Debug, Clone)]
pub(super) struct FrameResourceRegistry {
    next_id: u64,
    image_ids: BTreeMap<WidgetId, ResourceId>,
}

impl Default for FrameResourceRegistry {
    fn default() -> Self {
        Self {
            next_id: 1,
            image_ids: BTreeMap::new(),
        }
    }
}

impl FrameResourceRegistry {
    fn image_id(&mut self, node_id: WidgetId) -> Result<ResourceId, FrameSubmissionError> {
        if let Some(id) = self.image_ids.get(&node_id) {
            return Ok(*id);
        }

        let id = ResourceId(take_monotonic(&mut self.next_id, "resource id")?);
        self.image_ids.insert(node_id, id);
        Ok(id)
    }
}

pub(super) fn build_resource_snapshot(
    epoch: ResourceEpoch,
    ir: &CoreIR,
    registry: &mut FrameResourceRegistry,
) -> Result<ResourceSnapshot, FrameSubmissionError> {
    let mut memory_images = ir
        .nodes
        .iter()
        .filter_map(|(&node_id, node)| match &node.op {
            Op::Paint(PaintOp::DrawImage {
                request,
                fit: _,
                alignment: _,
            }) => match &request.source {
                ImageSource::Memory { bytes, mime_type } => {
                    Some((node_id, &request.source, bytes, mime_type.as_deref()))
                }
                ImageSource::Asset { .. }
                | ImageSource::File { .. }
                | ImageSource::Network { .. }
                | ImageSource::SvgText { .. } => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    memory_images.sort_unstable_by_key(|(node_id, _, _, _)| *node_id);

    let mut entries = Vec::with_capacity(memory_images.len());
    for (node_id, source, bytes, mime_type) in memory_images {
        let id = registry.image_id(node_id)?;
        let mut provenance = ResourceProvenance::new(ResourceSource::Memory);
        provenance.locator = mime_type.map(ToOwned::to_owned);
        provenance.requested_by = Some(node_id);
        entries.push(ResourceEntry::ready(
            id,
            ResourceContentIdentity::try_new(source.stable_identity())
                .expect("an image source stable identity is never empty"),
            ResourceKind::Image,
            provenance,
            ResourcePayload::Bytes(bytes.clone()),
        ));
    }

    Ok(ResourceSnapshot::try_new(epoch, entries)
        .expect("the frame resource registry assigns unique, valid resource entries"))
}

#[cfg(test)]
mod tests {
    use fission_ir::op::{ImageAlignment, ImageFit, ImageRequest};
    use fission_render::resource::{ResourceKind, ResourcePayload, ResourceStatus};

    use super::*;

    fn add_memory_image(ir: &mut CoreIR, id: WidgetId, bytes: &[u8]) {
        ir.add_node(
            id,
            Op::Paint(PaintOp::DrawImage {
                request: ImageRequest {
                    source: ImageSource::Memory {
                        bytes: bytes.to_vec(),
                        mime_type: Some("image/png".to_string()),
                    },
                    ..Default::default()
                },
                fit: ImageFit::Contain,
                alignment: ImageAlignment::Center,
            }),
            Vec::new(),
        );
    }

    #[test]
    fn memory_images_become_ready_frame_resources() {
        let node_id = WidgetId::explicit("resource.image");
        let mut ir = CoreIR::new();
        add_memory_image(&mut ir, node_id, &[1, 2, 3]);

        let snapshot =
            build_resource_snapshot(ResourceEpoch(7), &ir, &mut FrameResourceRegistry::default())
                .unwrap();
        let (_, entry) = snapshot.iter().next().unwrap();

        assert_eq!(snapshot.epoch(), ResourceEpoch(7));
        assert_eq!(snapshot.len(), 1);
        assert_eq!(entry.kind(), &ResourceKind::Image);
        assert_eq!(entry.status(), ResourceStatus::Ready);
        assert_eq!(entry.provenance().source, ResourceSource::Memory);
        assert_eq!(entry.provenance().requested_by, Some(node_id));
        assert_eq!(entry.provenance().locator.as_deref(), Some("image/png"));
        assert_eq!(
            entry.payload(),
            Some(&ResourcePayload::Bytes(vec![1, 2, 3]))
        );
    }

    #[test]
    fn logical_id_survives_content_version_changes() {
        let node_id = WidgetId::explicit("resource.versioned-image");
        let mut ir = CoreIR::new();
        let mut registry = FrameResourceRegistry::default();
        add_memory_image(&mut ir, node_id, &[1]);
        let first = build_resource_snapshot(ResourceEpoch(1), &ir, &mut registry).unwrap();

        add_memory_image(&mut ir, node_id, &[2]);
        let second = build_resource_snapshot(ResourceEpoch(2), &ir, &mut registry).unwrap();
        let first_entry = first.iter().next().unwrap().1;
        let second_entry = second.iter().next().unwrap().1;

        assert_eq!(first_entry.id(), second_entry.id());
        assert_ne!(
            first_entry.content_identity(),
            second_entry.content_identity()
        );
    }

    #[test]
    fn non_memory_images_are_not_claimed_as_ready_resources() {
        let node_id = WidgetId::explicit("resource.file-image");
        let mut ir = CoreIR::new();
        ir.add_node(
            node_id,
            Op::Paint(PaintOp::DrawImage {
                request: ImageRequest {
                    source: ImageSource::File {
                        path: "/tmp/not-opened-by-the-renderer.png".to_string(),
                    },
                    ..Default::default()
                },
                fit: ImageFit::Fill,
                alignment: ImageAlignment::Center,
            }),
            Vec::new(),
        );

        let snapshot =
            build_resource_snapshot(ResourceEpoch(3), &ir, &mut FrameResourceRegistry::default())
                .unwrap();

        assert!(snapshot.is_empty());
    }
}
