//! Backend-owned decoded image caching and frame-resource resolution.
//!
//! Fission's [`ResourceSnapshot`] is the source-data authority. This module
//! never opens a path or performs network I/O; it only decodes bytes already
//! captured for the submitted frame.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use fission_ir::op::{ImageAlignment, ImageRequest, ImageSource};
use fission_render::capabilities::ImageSourceKind;
use fission_render::diagnostics::{CacheDiagnostics, DiagnosticCategory};
use fission_render::image_cache_store::ImageCacheStore;
use fission_render::resource::{
    resolved_resource_content_identity, unresolved_resource_content_identity, ResourceEntry,
    ResourceId, ResourceKind, ResourcePayload, ResourceSnapshot, ResourceSource, ResourceStatus,
};
use fission_render::{ImageFit, LayoutRect};
use fission_skia_sys::DecodedImage;

const CACHE_NAME: &str = "fission-render-skia-images";
const DEFAULT_IMAGE_CACHE_BYTES: u64 = 50 * 1024 * 1024;

/// A frame-owned encoded image selected by exact logical identity and
/// requesting-node provenance.
pub(crate) struct ResolvedImageResource<'a> {
    pub cache_key: String,
    pub encoded: &'a [u8],
    /// Exact authoritative entry selected from this frame. Retained-picture
    /// caches use it to prevent a cache hint from bypassing resource changes.
    pub entry: &'a ResourceEntry,
}

/// Destination and clipping geometry in Fission logical coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImagePlacement {
    pub destination: LayoutRect,
    pub clip: LayoutRect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImageError {
    UnsupportedSource(ImageSourceKind),
    MissingResource {
        node_id: fission_ir::WidgetId,
        content_identity: String,
    },
    AmbiguousResource {
        node_id: fission_ir::WidgetId,
        content_identity: String,
        matches: usize,
    },
    ContentIdentityMismatch {
        resource_id: ResourceId,
        expected: String,
        actual: String,
    },
    UnexpectedKind {
        resource_id: ResourceId,
        actual: ResourceKind,
    },
    UnexpectedSource {
        resource_id: ResourceId,
        expected: ResourceSource,
        actual: ResourceSource,
    },
    NotReady {
        resource_id: ResourceId,
        status: ResourceStatus,
    },
    PayloadIsNotBytes {
        resource_id: ResourceId,
    },
    PayloadIdentityMismatch {
        resource_id: ResourceId,
    },
    DecodeFailed {
        content_identity: String,
        message: String,
    },
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSource(source) => {
                write!(formatter, "the Skia image path does not support {source:?} sources")
            }
            Self::MissingResource {
                node_id,
                content_identity,
            } => write!(
                formatter,
                "frame resources contain no image requested by node {node_id} with content identity {content_identity}"
            ),
            Self::AmbiguousResource {
                node_id,
                content_identity,
                matches,
            } => write!(
                formatter,
                "frame resources contain {matches} images requested by node {node_id} with content identity {content_identity}"
            ),
            Self::ContentIdentityMismatch {
                resource_id,
                expected,
                actual,
            } => write!(
                formatter,
                "frame resource {} has content identity {actual}, expected {expected}",
                resource_id.0
            ),
            Self::UnexpectedKind {
                resource_id,
                actual,
            } => write!(
                formatter,
                "frame resource {} has kind {actual:?}, expected Image",
                resource_id.0
            ),
            Self::UnexpectedSource {
                resource_id,
                expected,
                actual,
            } => write!(
                formatter,
                "frame resource {} has source {actual:?}, expected {expected:?}",
                resource_id.0
            ),
            Self::NotReady {
                resource_id,
                status,
            } => write!(
                formatter,
                "frame resource {} is {status:?}, expected Ready",
                resource_id.0
            ),
            Self::PayloadIsNotBytes { resource_id } => write!(
                formatter,
                "ready image resource {} does not contain encoded bytes",
                resource_id.0
            ),
            Self::PayloadIdentityMismatch { resource_id } => write!(
                formatter,
                "ready image resource {} bytes do not match its declared content identity",
                resource_id.0
            ),
            Self::DecodeFailed {
                content_identity,
                message,
            } => write!(
                formatter,
                "Skia could not decode image {content_identity}: {message}"
            ),
        }
    }
}

impl std::error::Error for ImageError {}

impl ImageError {
    pub(crate) fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::UnsupportedSource(_) => "skia-image-source-unsupported",
            Self::MissingResource { .. } => "skia-image-resource-missing",
            Self::AmbiguousResource { .. } => "skia-image-resource-ambiguous",
            Self::ContentIdentityMismatch { .. } => "skia-image-resource-identity-mismatch",
            Self::UnexpectedKind { .. } => "skia-image-resource-kind-invalid",
            Self::UnexpectedSource { .. } => "skia-image-resource-source-invalid",
            Self::NotReady { .. } => "skia-image-resource-not-ready",
            Self::PayloadIsNotBytes { .. } => "skia-image-resource-payload-invalid",
            Self::PayloadIdentityMismatch { .. } => "skia-image-resource-payload-mismatch",
            Self::DecodeFailed { .. } => "skia-image-decode-failed",
        }
    }

    pub(crate) fn diagnostic_category(&self) -> DiagnosticCategory {
        match self {
            Self::UnsupportedSource(_) => DiagnosticCategory::Capability,
            _ => DiagnosticCategory::Resource,
        }
    }
}

/// One renderer session's disposable cache of Skia-derived image objects.
pub(crate) struct SkiaImageCache {
    store: ImageCacheStore<DecodedImage>,
    budget_bytes: u64,
    evictions: Arc<AtomicU64>,
}

impl Default for SkiaImageCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SkiaImageCache {
    pub fn new() -> Self {
        Self::with_budget_bytes(configured_image_cache_bytes())
    }

    pub(crate) fn with_budget_bytes(budget_bytes: u64) -> Self {
        let budget_bytes = budget_bytes.clamp(1, u64::from(u32::MAX));
        let evictions = Arc::new(AtomicU64::new(0));
        Self {
            store: image_store(budget_bytes, Arc::clone(&evictions)),
            budget_bytes,
            evictions,
        }
    }

    pub fn get_or_decode(
        &self,
        content_identity: &str,
        encoded: &[u8],
    ) -> Result<DecodedImage, ImageError> {
        if let Some(image) = self.store.get(content_identity) {
            return Ok(image);
        }

        let max_decoded_bytes = usize::try_from(self.budget_bytes).unwrap_or(usize::MAX);
        let image = DecodedImage::decode_encoded(encoded, max_decoded_bytes).map_err(|error| {
            ImageError::DecodeFailed {
                content_identity: content_identity.to_owned(),
                message: error.to_string(),
            }
        })?;
        self.store
            .insert(content_identity.to_owned(), image.clone());
        Ok(image)
    }

    pub fn clear(&self) {
        self.store.invalidate_all();
        self.store.run_pending_tasks();
    }

    pub fn diagnostics(&self) -> CacheDiagnostics {
        self.store.run_pending_tasks();
        CacheDiagnostics {
            name: CACHE_NAME.into(),
            entries: self.store.entry_count(),
            used_bytes: self.store.weighted_size(),
            budget_bytes: Some(self.budget_bytes),
            evictions: self.evictions.load(Ordering::Acquire),
        }
    }
}

pub(crate) fn resolve_image_resource<'a>(
    resources: &'a ResourceSnapshot,
    request: &ImageRequest,
    node_id: fission_ir::WidgetId,
) -> Result<ResolvedImageResource<'a>, ImageError> {
    let (expected_source, requested_bytes) = match &request.source {
        ImageSource::Asset { .. } => (ResourceSource::Asset, None),
        ImageSource::File { .. } => (ResourceSource::File, None),
        ImageSource::Network { .. } => (ResourceSource::Network, None),
        ImageSource::Memory { bytes, .. } => (ResourceSource::Memory, Some(bytes.as_slice())),
        ImageSource::SvgText { .. } => {
            return Err(ImageError::UnsupportedSource(ImageSourceKind::SvgText))
        }
    };

    let matches = resources
        .iter()
        .map(|(_, entry)| entry)
        .filter(|entry| entry.provenance().requested_by == Some(node_id))
        .collect::<Vec<_>>();
    let entry = match matches.as_slice() {
        [] => {
            return Err(ImageError::MissingResource {
                node_id,
                content_identity: request.source.stable_identity(),
            })
        }
        [entry] => *entry,
        entries => {
            return Err(ImageError::AmbiguousResource {
                node_id,
                content_identity: request.source.stable_identity(),
                matches: entries.len(),
            })
        }
    };

    if entry.kind() != &ResourceKind::Image {
        return Err(ImageError::UnexpectedKind {
            resource_id: entry.id(),
            actual: entry.kind().clone(),
        });
    }
    if entry.provenance().source != expected_source {
        return Err(ImageError::UnexpectedSource {
            resource_id: entry.id(),
            expected: expected_source,
            actual: entry.provenance().source.clone(),
        });
    }

    if entry.status() != ResourceStatus::Ready {
        let expected = unresolved_resource_content_identity(&ResourceKind::Image, &request.source);
        if entry.content_identity() != &expected {
            return Err(ImageError::ContentIdentityMismatch {
                resource_id: entry.id(),
                expected: expected.as_str().to_owned(),
                actual: entry.content_identity().as_str().to_owned(),
            });
        }
        return Err(ImageError::NotReady {
            resource_id: entry.id(),
            status: entry.status(),
        });
    }
    let Some(ResourcePayload::Bytes(encoded)) = entry.payload() else {
        return Err(ImageError::PayloadIsNotBytes {
            resource_id: entry.id(),
        });
    };
    let expected =
        resolved_resource_content_identity(&ResourceKind::Image, &request.source, encoded);
    if entry.content_identity() != &expected {
        return Err(ImageError::ContentIdentityMismatch {
            resource_id: entry.id(),
            expected: expected.as_str().to_owned(),
            actual: entry.content_identity().as_str().to_owned(),
        });
    }
    if requested_bytes.is_some_and(|requested| encoded.as_slice() != requested) {
        return Err(ImageError::PayloadIdentityMismatch {
            resource_id: entry.id(),
        });
    }

    Ok(ResolvedImageResource {
        cache_key: entry.content_identity().as_str().to_owned(),
        encoded,
        entry,
    })
}

pub(crate) fn place_image(
    rect: LayoutRect,
    image_width: u32,
    image_height: u32,
    fit: ImageFit,
    alignment: ImageAlignment,
) -> Option<ImagePlacement> {
    let rect_width = rect.width();
    let rect_height = rect.height();
    let image_width = image_width as f32;
    let image_height = image_height as f32;
    if rect_width <= 0.0 || rect_height <= 0.0 || image_width <= 0.0 || image_height <= 0.0 {
        return None;
    }

    let (width, height) = match fit {
        ImageFit::Fill => (rect_width, rect_height),
        ImageFit::Contain => {
            let scale = (rect_width / image_width).min(rect_height / image_height);
            (image_width * scale, image_height * scale)
        }
        ImageFit::Cover => {
            let scale = (rect_width / image_width).max(rect_height / image_height);
            (image_width * scale, image_height * scale)
        }
        ImageFit::None => (image_width, image_height),
    };
    let (offset_x, offset_y) = if matches!(fit, ImageFit::Fill | ImageFit::None) {
        (0.0, 0.0)
    } else {
        aligned_offset(rect_width - width, rect_height - height, alignment)
    };

    Some(ImagePlacement {
        destination: LayoutRect::new(rect.x() + offset_x, rect.y() + offset_y, width, height),
        clip: rect,
    })
}

fn aligned_offset(extra_width: f32, extra_height: f32, alignment: ImageAlignment) -> (f32, f32) {
    let x = match alignment {
        ImageAlignment::TopStart | ImageAlignment::CenterStart | ImageAlignment::BottomStart => 0.0,
        ImageAlignment::TopCenter | ImageAlignment::Center | ImageAlignment::BottomCenter => {
            extra_width / 2.0
        }
        ImageAlignment::TopEnd | ImageAlignment::CenterEnd | ImageAlignment::BottomEnd => {
            extra_width
        }
    };
    let y = match alignment {
        ImageAlignment::TopStart | ImageAlignment::TopCenter | ImageAlignment::TopEnd => 0.0,
        ImageAlignment::CenterStart | ImageAlignment::Center | ImageAlignment::CenterEnd => {
            extra_height / 2.0
        }
        ImageAlignment::BottomStart | ImageAlignment::BottomCenter | ImageAlignment::BottomEnd => {
            extra_height
        }
    };
    (x, y)
}

fn image_store(budget_bytes: u64, evictions: Arc<AtomicU64>) -> ImageCacheStore<DecodedImage> {
    ImageCacheStore::new(
        CACHE_NAME,
        budget_bytes,
        |image: &DecodedImage| {
            image
                .approximate_decoded_bytes()
                .clamp(1, u32::MAX as usize) as u32
        },
        move || {
            evictions.fetch_add(1, Ordering::AcqRel);
        },
    )
}

fn configured_image_cache_bytes() -> u64 {
    configured_image_cache_bytes_from(std::env::var("FISSION_IMAGE_CACHE_BYTES").ok().as_deref())
}

fn configured_image_cache_bytes_from(value: Option<&str>) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_IMAGE_CACHE_BYTES)
        .min(u64::from(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_render::frame::ResourceEpoch;
    use fission_render::resource::{ResourceEntry, ResourceProvenance};

    fn memory_request(bytes: &[u8]) -> ImageRequest {
        ImageRequest {
            source: ImageSource::Memory {
                bytes: bytes.to_vec(),
                mime_type: Some("image/png".into()),
            },
            ..ImageRequest::default()
        }
    }

    fn entry(
        id: u64,
        request: &ImageRequest,
        requested_by: fission_ir::WidgetId,
        status: ResourceStatus,
        payload: Option<ResourcePayload>,
    ) -> ResourceEntry {
        let source = match &request.source {
            ImageSource::Asset { .. } => ResourceSource::Asset,
            ImageSource::File { .. } => ResourceSource::File,
            ImageSource::Network { .. } => ResourceSource::Network,
            ImageSource::Memory { .. } => ResourceSource::Memory,
            ImageSource::SvgText { .. } => ResourceSource::Embedded,
        };
        let content_identity = match payload.as_ref() {
            Some(ResourcePayload::Bytes(bytes)) if status == ResourceStatus::Ready => {
                resolved_resource_content_identity(&ResourceKind::Image, &request.source, bytes)
            }
            _ => unresolved_resource_content_identity(&ResourceKind::Image, &request.source),
        };
        ResourceEntry::try_new(
            ResourceId(id),
            content_identity,
            ResourceKind::Image,
            ResourceProvenance {
                source,
                locator: Some("image/png".into()),
                requested_by: Some(requested_by),
            },
            status,
            payload,
            None,
        )
        .unwrap()
    }

    #[test]
    fn resolver_requires_exact_content_and_requesting_node() {
        let request = memory_request(&[1, 2, 3]);
        let requested_by = fission_ir::WidgetId::explicit("image.expected");
        let other = fission_ir::WidgetId::explicit("image.other");
        let resources = ResourceSnapshot::try_new(
            ResourceEpoch(1),
            [
                entry(
                    1,
                    &request,
                    other,
                    ResourceStatus::Ready,
                    Some(ResourcePayload::Bytes(vec![9])),
                ),
                entry(
                    2,
                    &request,
                    requested_by,
                    ResourceStatus::Ready,
                    Some(ResourcePayload::Bytes(vec![1, 2, 3])),
                ),
            ],
        )
        .unwrap();

        let resolved = resolve_image_resource(&resources, &request, requested_by).unwrap();

        assert_eq!(
            resolved.cache_key,
            resolved.entry.content_identity().as_str()
        );
        assert_eq!(resolved.encoded, &[1, 2, 3]);
        assert!(matches!(
            resolve_image_resource(
                &resources,
                &request,
                fission_ir::WidgetId::explicit("missing")
            ),
            Err(ImageError::MissingResource { .. })
        ));
    }

    #[test]
    fn resolver_reports_non_ready_and_ambiguous_resources_distinctly() {
        let request = memory_request(&[4, 5, 6]);
        let node_id = fission_ir::WidgetId::explicit("image.node");
        let loading = ResourceSnapshot::try_new(
            ResourceEpoch(1),
            [entry(1, &request, node_id, ResourceStatus::Loading, None)],
        )
        .unwrap();

        assert!(matches!(
            resolve_image_resource(&loading, &request, node_id),
            Err(ImageError::NotReady {
                status: ResourceStatus::Loading,
                ..
            })
        ));

        let duplicate = ResourceSnapshot::try_new(
            ResourceEpoch(1),
            [
                entry(
                    1,
                    &request,
                    node_id,
                    ResourceStatus::Ready,
                    Some(ResourcePayload::Bytes(vec![4, 5, 6])),
                ),
                entry(
                    2,
                    &request,
                    node_id,
                    ResourceStatus::Ready,
                    Some(ResourcePayload::Bytes(vec![4, 5, 6])),
                ),
            ],
        )
        .unwrap();
        assert!(matches!(
            resolve_image_resource(&duplicate, &request, node_id),
            Err(ImageError::AmbiguousResource { matches: 2, .. })
        ));

        let mismatched = ResourceSnapshot::try_new(
            ResourceEpoch(2),
            [entry(
                3,
                &request,
                node_id,
                ResourceStatus::Ready,
                Some(ResourcePayload::Bytes(vec![9, 9, 9])),
            )],
        )
        .unwrap();
        assert!(matches!(
            resolve_image_resource(&mismatched, &request, node_id),
            Err(ImageError::PayloadIdentityMismatch {
                resource_id: ResourceId(3)
            })
        ));
    }

    #[test]
    fn resolver_accepts_authority_owned_asset_file_and_network_bytes() {
        let sources = [
            ImageSource::Asset {
                path: "assets/logo.png".into(),
            },
            ImageSource::File {
                path: "/tmp/logo.png".into(),
            },
            ImageSource::Network {
                url: "https://example.test/logo.png".into(),
                headers: Vec::new(),
                cache_policy: Default::default(),
            },
        ];

        for (index, source) in sources.into_iter().enumerate() {
            let request = ImageRequest {
                source,
                ..ImageRequest::default()
            };
            let node_id = fission_ir::WidgetId::explicit(format!("image.source.{index}"));
            let resources = ResourceSnapshot::try_new(
                ResourceEpoch(index as u64 + 1),
                [entry(
                    index as u64 + 1,
                    &request,
                    node_id,
                    ResourceStatus::Ready,
                    Some(ResourcePayload::Bytes(vec![index as u8, 7, 9])),
                )],
            )
            .unwrap();

            let resolved = resolve_image_resource(&resources, &request, node_id).unwrap();
            assert_eq!(resolved.encoded, &[index as u8, 7, 9]);
            assert_eq!(
                resolved.cache_key,
                resolved.entry.content_identity().as_str()
            );
        }
    }

    #[test]
    fn all_image_fits_have_explicit_geometry_and_match_existing_none_semantics() {
        let rect = LayoutRect::new(10.0, 20.0, 100.0, 100.0);

        let contain =
            place_image(rect, 200, 100, ImageFit::Contain, ImageAlignment::BottomEnd).unwrap();
        assert_eq!(
            contain.destination,
            LayoutRect::new(10.0, 70.0, 100.0, 50.0)
        );

        let cover = place_image(rect, 200, 100, ImageFit::Cover, ImageAlignment::TopEnd).unwrap();
        assert_eq!(
            cover.destination,
            LayoutRect::new(-90.0, 20.0, 200.0, 100.0)
        );

        let fill = place_image(rect, 200, 100, ImageFit::Fill, ImageAlignment::TopStart).unwrap();
        assert_eq!(fill.destination, rect);

        let natural = place_image(rect, 20, 40, ImageFit::None, ImageAlignment::Center).unwrap();
        assert_eq!(natural.destination, LayoutRect::new(10.0, 20.0, 20.0, 40.0));
        assert_eq!(natural.clip, rect);
    }

    #[test]
    fn cache_budget_has_a_safe_default_and_rejects_zero_or_invalid_overrides() {
        assert_eq!(configured_image_cache_bytes_from(None), 50 * 1024 * 1024);
        assert_eq!(
            configured_image_cache_bytes_from(Some("0")),
            50 * 1024 * 1024
        );
        assert_eq!(
            configured_image_cache_bytes_from(Some("invalid")),
            50 * 1024 * 1024
        );
        assert_eq!(configured_image_cache_bytes_from(Some("1024")), 1024);
        assert_eq!(
            configured_image_cache_bytes_from(Some("18446744073709551615")),
            u64::from(u32::MAX)
        );
    }
}
