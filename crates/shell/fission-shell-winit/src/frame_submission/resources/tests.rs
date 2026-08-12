use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use fission_ir::op::{HttpHeader, ImageAlignment, ImageCachePolicy, ImageFit, ImageRequest};
use fission_render::resource::{ResourceKind, ResourcePayload, ResourceStatus};

use super::loader::{LoadCompletion, LoadOutcome};
use super::*;

#[derive(Default)]
struct ManualLoader {
    pending: Mutex<VecDeque<(ImageSource, LoadCompletion)>>,
}

impl ManualLoader {
    fn pending_len(&self) -> usize {
        lock_unpoisoned(&self.pending).len()
    }

    fn complete_next(&self, outcome: LoadOutcome) -> ImageSource {
        let (source, completion) = lock_unpoisoned(&self.pending)
            .pop_front()
            .expect("manual loader has a pending request");
        completion(outcome);
        source
    }
}

impl SourceLoader for ManualLoader {
    fn start(&self, source: ImageSource, completion: LoadCompletion) -> Result<(), LoadFailure> {
        lock_unpoisoned(&self.pending).push_back((source, completion));
        Ok(())
    }
}

fn registry_with(loader: Arc<ManualLoader>) -> FrameResourceRegistry {
    FrameResourceRegistry::with_loader(loader)
}

fn add_image(ir: &mut CoreIR, node_id: WidgetId, source: ImageSource) {
    ir.add_node(
        node_id,
        Op::Paint(PaintOp::DrawImage {
            request: ImageRequest {
                source,
                ..ImageRequest::default()
            },
            fit: ImageFit::Contain,
            alignment: ImageAlignment::Center,
        }),
        Vec::new(),
    );
}

fn only_entry(snapshot: &ResourceSnapshot) -> &ResourceEntry {
    snapshot
        .iter()
        .next()
        .map(|(_, entry)| entry)
        .expect("snapshot contains one resource")
}

#[test]
fn memory_images_are_ready_without_backend_io() {
    let node_id = WidgetId::explicit("resource.memory");
    let source = ImageSource::Memory {
        bytes: vec![1, 2, 3],
        mime_type: Some("image/png".to_string()),
    };
    let mut ir = CoreIR::new();
    add_image(&mut ir, node_id, source.clone());
    let mut registry = FrameResourceRegistry::default();

    let snapshot = build_resource_snapshot(ResourceEpoch(7), &ir, &mut registry).unwrap();
    let entry = only_entry(&snapshot);

    assert_eq!(snapshot.epoch(), ResourceEpoch(7));
    assert_eq!(entry.kind(), &ResourceKind::Image);
    assert_eq!(entry.status(), ResourceStatus::Ready);
    assert_eq!(entry.provenance().source, ResourceSource::Memory);
    assert_eq!(entry.provenance().requested_by, Some(node_id));
    assert_eq!(entry.provenance().locator.as_deref(), Some("image/png"));
    assert_eq!(
        entry.content_identity(),
        &resolved_resource_content_identity(&ResourceKind::Image, &source, &[1, 2, 3])
    );
    assert_eq!(
        entry.payload(),
        Some(&ResourcePayload::Bytes(vec![1, 2, 3]))
    );
    assert!(!registry.has_pending());
}

#[test]
fn inline_svg_is_a_ready_typed_source_resource() {
    let node_id = WidgetId::explicit("resource.inline-svg");
    let content = "<svg viewBox='0 0 1 1'></svg>";
    let mut ir = CoreIR::new();
    ir.add_node(
        node_id,
        Op::Paint(PaintOp::DrawSvg {
            content: content.to_string(),
            fill: None,
            stroke: None,
        }),
        Vec::new(),
    );

    let snapshot =
        build_resource_snapshot(ResourceEpoch(1), &ir, &mut FrameResourceRegistry::default())
            .unwrap();
    let entry = only_entry(&snapshot);

    assert_eq!(entry.kind(), &ResourceKind::Svg);
    assert_eq!(entry.status(), ResourceStatus::Ready);
    assert_eq!(entry.provenance().source, ResourceSource::Embedded);
    assert_eq!(entry.provenance().locator.as_deref(), Some("inline-svg"));
    assert_eq!(
        entry.payload(),
        Some(&ResourcePayload::Text(content.to_string()))
    );
}

#[test]
fn asynchronous_sources_publish_loading_then_ready_and_wake_host() {
    let loader = Arc::new(ManualLoader::default());
    let mut registry = registry_with(Arc::clone(&loader));
    let wakes = Arc::new(AtomicU64::new(0));
    let wake_counter = Arc::clone(&wakes);
    registry.install_wake(Arc::new(move || {
        wake_counter.fetch_add(1, Ordering::AcqRel);
    }));
    let node_id = WidgetId::explicit("resource.network");
    let source = ImageSource::Network {
        url: "https://user:password@example.test/icon.png?token=url-secret".into(),
        headers: vec![HttpHeader {
            name: "Authorization".into(),
            value: "Bearer header-secret".into(),
        }],
        cache_policy: ImageCachePolicy::Default,
    };
    let mut ir = CoreIR::new();
    add_image(&mut ir, node_id, source.clone());

    let acquisition_key = RequestedResource::new(node_id, source.clone(), ResourceKind::Image).key;
    assert!(!acquisition_key
        .request_fingerprint
        .contains("header-secret"));

    let loading = build_resource_snapshot(ResourceEpoch(1), &ir, &mut registry).unwrap();
    let loading_entry = only_entry(&loading);
    assert_eq!(loading_entry.status(), ResourceStatus::Loading);
    assert_eq!(loading_entry.provenance().source, ResourceSource::Network);
    assert_eq!(
        loading_entry.provenance().locator.as_deref(),
        Some("https://<redacted>@example.test/icon.png?<redacted>")
    );
    assert!(!loading_entry
        .content_identity()
        .as_str()
        .contains("header-secret"));
    assert!(registry.has_pending());
    assert_eq!(loader.pending_len(), 1);

    assert_eq!(
        loader.complete_next(LoadOutcome::Ready(vec![7, 8, 9])),
        source
    );
    assert_eq!(registry.generation(), 1);
    assert_eq!(wakes.load(Ordering::Acquire), 1);

    let ready = build_resource_snapshot(ResourceEpoch(2), &ir, &mut registry).unwrap();
    let ready_entry = only_entry(&ready);
    assert_eq!(ready_entry.status(), ResourceStatus::Ready);
    assert_eq!(
        ready_entry.payload(),
        Some(&ResourcePayload::Bytes(vec![7, 8, 9]))
    );
    assert_ne!(
        ready_entry.content_identity(),
        loading_entry.content_identity()
    );
    assert!(!ready_entry
        .content_identity()
        .as_str()
        .contains("header-secret"));
    assert!(!registry.has_pending());
}

#[test]
fn duplicate_source_requests_share_one_acquisition() {
    let loader = Arc::new(ManualLoader::default());
    let mut registry = registry_with(Arc::clone(&loader));
    let source = ImageSource::Asset {
        path: "assets/shared.png".into(),
    };
    let mut ir = CoreIR::new();
    add_image(
        &mut ir,
        WidgetId::explicit("resource.shared.first"),
        source.clone(),
    );
    add_image(
        &mut ir,
        WidgetId::explicit("resource.shared.second"),
        source,
    );

    let snapshot = build_resource_snapshot(ResourceEpoch(1), &ir, &mut registry).unwrap();

    assert_eq!(snapshot.len(), 2);
    assert!(snapshot
        .iter()
        .all(|(_, entry)| entry.status() == ResourceStatus::Loading));
    assert_eq!(loader.pending_len(), 1);
}

#[test]
fn failed_acquisition_is_explicit_and_has_no_payload() {
    let loader = Arc::new(ManualLoader::default());
    let mut registry = registry_with(Arc::clone(&loader));
    let mut ir = CoreIR::new();
    add_image(
        &mut ir,
        WidgetId::explicit("resource.failed"),
        ImageSource::File {
            path: "/missing/image.png".into(),
        },
    );

    let first = build_resource_snapshot(ResourceEpoch(1), &ir, &mut registry).unwrap();
    assert_eq!(only_entry(&first).status(), ResourceStatus::Loading);
    loader.complete_next(LoadOutcome::Failed(LoadFailure::too_large()));

    let failed = build_resource_snapshot(ResourceEpoch(2), &ir, &mut registry).unwrap();
    let entry = only_entry(&failed);
    assert_eq!(entry.status(), ResourceStatus::Failed);
    assert!(entry.payload().is_none());
    let failure = entry.failure().expect("failed entry owns diagnostics");
    assert_eq!(failure.code(), "resource-source-too-large");
    assert!(!failure.retryable());
}

#[test]
fn stale_completion_cannot_overwrite_a_new_source_generation() {
    let loader = Arc::new(ManualLoader::default());
    let mut registry = registry_with(Arc::clone(&loader));
    let node_id = WidgetId::explicit("resource.replaced");
    let first_source = ImageSource::Asset {
        path: "assets/old.png".into(),
    };
    let second_source = ImageSource::Asset {
        path: "assets/new.png".into(),
    };
    let mut ir = CoreIR::new();
    add_image(&mut ir, node_id, first_source.clone());
    let first = build_resource_snapshot(ResourceEpoch(1), &ir, &mut registry).unwrap();
    let logical_id = only_entry(&first).id();

    add_image(&mut ir, node_id, second_source.clone());
    let second = build_resource_snapshot(ResourceEpoch(2), &ir, &mut registry).unwrap();
    assert_eq!(only_entry(&second).id(), logical_id);
    assert_eq!(loader.pending_len(), 2);

    assert_eq!(
        loader.complete_next(LoadOutcome::Ready(vec![1])),
        first_source
    );
    assert_eq!(registry.generation(), 0);
    let still_loading = build_resource_snapshot(ResourceEpoch(3), &ir, &mut registry).unwrap();
    assert_eq!(only_entry(&still_loading).status(), ResourceStatus::Loading);

    assert_eq!(
        loader.complete_next(LoadOutcome::Ready(vec![2])),
        second_source
    );
    assert_eq!(registry.generation(), 1);
    let ready = build_resource_snapshot(ResourceEpoch(4), &ir, &mut registry).unwrap();
    assert_eq!(
        only_entry(&ready).payload(),
        Some(&ResourcePayload::Bytes(vec![2]))
    );
}

#[test]
fn logical_id_survives_an_immutable_content_version_change() {
    let node_id = WidgetId::explicit("resource.versioned-memory");
    let mut ir = CoreIR::new();
    let mut registry = FrameResourceRegistry::default();
    add_image(
        &mut ir,
        node_id,
        ImageSource::Memory {
            bytes: vec![1],
            mime_type: None,
        },
    );
    let first = build_resource_snapshot(ResourceEpoch(1), &ir, &mut registry).unwrap();

    add_image(
        &mut ir,
        node_id,
        ImageSource::Memory {
            bytes: vec![2],
            mime_type: None,
        },
    );
    let second = build_resource_snapshot(ResourceEpoch(2), &ir, &mut registry).unwrap();

    assert_eq!(only_entry(&first).id(), only_entry(&second).id());
    assert_ne!(
        only_entry(&first).content_identity(),
        only_entry(&second).content_identity()
    );
}

#[test]
fn resolved_path_identity_changes_with_bytes() {
    let source = ImageSource::File {
        path: "/same/path.png".into(),
    };

    assert_ne!(
        resolved_resource_content_identity(&ResourceKind::Image, &source, &[1, 2, 3]),
        resolved_resource_content_identity(&ResourceKind::Image, &source, &[1, 2, 4])
    );
}
