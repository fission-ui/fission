use std::fmt;
use std::sync::{Arc, Mutex};

use fission_render::backend::{BackendResult, GraphicsBackendSession};
use fission_render::frame::ResourceEpoch;
use fission_render::resource::{
    ResourceContentIdentity, ResourceEntry, ResourceId, ResourceKind, ResourcePayload,
    ResourceProvenance, ResourceSnapshot, ResourceSource,
};
use fission_skia_sys::web::WebParagraphFont;

use super::driver::{CanvasKitBackendPreference, CanvasKitDriver};
use super::host::{CanvasKitHost, CanvasKitParagraphHost, CanvasKitPixelRegion, CanvasKitReadback};
use super::resources::ResourceMap;
use crate::paragraph_engine::{
    new_canvaskit_paragraph_registry, CanvasKitFontState, CanvasKitParagraphBridge,
    CanvasKitParagraphDrawDataRegistry, CanvasKitParagraphEngine,
};

const FONT_RESOURCE_ID_BASE: u64 = 1 << 63;
const MAX_PROFILE_FONTS: usize = 4_096;
const HASH_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const HASH_PRIME: u64 = 0x0000_0100_0000_01b3;

/// One immutable Fission-owned font supplied to a CanvasKit profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasKitFont {
    pub family: String,
    pub bytes: Vec<u8>,
}

impl CanvasKitFont {
    pub fn new(family: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            family: family.into(),
            bytes: bytes.into(),
        }
    }
}

/// Invalid immutable font catalog supplied to a CanvasKit profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanvasKitProfileError {
    EmptyFontCatalog,
    TooManyFonts { count: usize, maximum: usize },
    EmptyFamily { index: usize },
    EmptyFontData { family: String },
    MissingDefaultFamily { family: String },
    InvalidContentIdentity,
}

impl fmt::Display for CanvasKitProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFontCatalog => {
                formatter.write_str("a paired CanvasKit profile requires at least one owned font")
            }
            Self::TooManyFonts { count, maximum } => write!(
                formatter,
                "CanvasKit profile has {count} fonts, exceeding the {maximum}-font wire limit"
            ),
            Self::EmptyFamily { index } => {
                write!(formatter, "CanvasKit font {index} has an empty family name")
            }
            Self::EmptyFontData { family } => {
                write!(formatter, "CanvasKit font family {family:?} has no bytes")
            }
            Self::MissingDefaultFamily { family } => write!(
                formatter,
                "CanvasKit default family {family:?} has no packaged font face"
            ),
            Self::InvalidContentIdentity => {
                formatter.write_str("CanvasKit could not construct a font content identity")
            }
        }
    }
}

impl std::error::Error for CanvasKitProfileError {}

#[derive(Clone)]
pub(crate) struct CanvasKitFontCatalog {
    resources: ResourceSnapshot,
    bindings: Box<[(ResourceId, String)]>,
    generation: u64,
}

impl CanvasKitFontCatalog {
    fn new(
        default_family: &str,
        mut fonts: Vec<CanvasKitFont>,
    ) -> Result<Self, CanvasKitProfileError> {
        if fonts.is_empty() {
            return Err(CanvasKitProfileError::EmptyFontCatalog);
        }
        if fonts.len() > MAX_PROFILE_FONTS {
            return Err(CanvasKitProfileError::TooManyFonts {
                count: fonts.len(),
                maximum: MAX_PROFILE_FONTS,
            });
        }
        for (index, font) in fonts.iter_mut().enumerate() {
            font.family = font.family.trim().to_string();
            if font.family.is_empty() {
                return Err(CanvasKitProfileError::EmptyFamily { index });
            }
            if font.bytes.is_empty() {
                return Err(CanvasKitProfileError::EmptyFontData {
                    family: font.family.clone(),
                });
            }
        }
        fonts.sort_by(|left, right| {
            left.family
                .to_ascii_lowercase()
                .cmp(&right.family.to_ascii_lowercase())
                .then_with(|| left.family.cmp(&right.family))
                .then_with(|| left.bytes.cmp(&right.bytes))
        });
        fonts.dedup();
        if !fonts
            .iter()
            .any(|font| font.family.eq_ignore_ascii_case(default_family))
        {
            return Err(CanvasKitProfileError::MissingDefaultFamily {
                family: default_family.to_string(),
            });
        }

        let mut generation_hash = StableHash::new();
        generation_hash.bytes(b"fission-canvaskit-font-catalog-v1\0");
        let mut entries = Vec::with_capacity(fonts.len());
        let mut bindings = Vec::with_capacity(fonts.len());
        for (index, font) in fonts.into_iter().enumerate() {
            generation_hash.sized(font.family.as_bytes());
            generation_hash.sized(&font.bytes);
            let resource_id = ResourceId(
                FONT_RESOURCE_ID_BASE
                    .checked_add(index as u64)
                    .expect("the bounded font count fits the reserved identifier range"),
            );
            let mut content_hash = StableHash::new();
            content_hash.bytes(b"fission-canvaskit-font-v1\0");
            content_hash.sized(font.family.as_bytes());
            content_hash.sized(&font.bytes);
            let identity = ResourceContentIdentity::try_new(format!(
                "fission-canvaskit-font-v1:{:016x}",
                content_hash.finish_nonzero()
            ))
            .map_err(|_| CanvasKitProfileError::InvalidContentIdentity)?;
            entries.push(ResourceEntry::ready(
                resource_id,
                identity,
                ResourceKind::Font,
                ResourceProvenance {
                    source: ResourceSource::Embedded,
                    locator: Some(font.family.clone()),
                    requested_by: None,
                },
                ResourcePayload::Bytes(font.bytes),
            ));
            bindings.push((resource_id, font.family.clone()));
        }
        let generation = generation_hash.finish_nonzero();
        let resources = ResourceSnapshot::try_new(ResourceEpoch(1), entries)
            .expect("validated profile font identifiers are unique");
        Ok(Self {
            resources,
            bindings: bindings.into_boxed_slice(),
            generation,
        })
    }

    pub(crate) fn resources(&self) -> &ResourceSnapshot {
        &self.resources
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn families(&self) -> Vec<String> {
        let mut families = Vec::new();
        for (_, family) in &self.bindings {
            if !families
                .iter()
                .any(|current: &String| current.eq_ignore_ascii_case(family))
            {
                families.push(family.clone());
            }
        }
        families
    }

    pub(super) fn wire_fonts(
        &self,
        resources: &ResourceMap,
    ) -> Result<Vec<WebParagraphFont>, CanvasKitFontCatalogError> {
        self.bindings
            .iter()
            .map(|(resource_id, family)| {
                let handle = resources.handle(*resource_id).ok_or(
                    CanvasKitFontCatalogError::MissingInstalledHandle {
                        resource_id: *resource_id,
                    },
                )?;
                Ok(WebParagraphFont {
                    handle,
                    family: family.clone(),
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CanvasKitFontCatalogError {
    MissingInstalledHandle { resource_id: ResourceId },
    EpochExhausted { frame_epoch: u64 },
}

impl fmt::Display for CanvasKitFontCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInstalledHandle { resource_id } => write!(
                formatter,
                "CanvasKit font resource {} has no installed generational handle",
                resource_id.0
            ),
            Self::EpochExhausted { frame_epoch } => write!(
                formatter,
                "frame resource epoch {frame_epoch} cannot be mapped after the CanvasKit font epoch"
            ),
        }
    }
}

struct SharedCanvasKitHost<H> {
    host: Arc<Mutex<H>>,
}

impl<H> Clone for SharedCanvasKitHost<H> {
    fn clone(&self) -> Self {
        Self {
            host: Arc::clone(&self.host),
        }
    }
}

impl<H> SharedCanvasKitHost<H> {
    fn new(host: H) -> Self {
        Self {
            host: Arc::new(Mutex::new(host)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SharedHostError(String);

impl fmt::Display for SharedHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<H> CanvasKitHost for SharedCanvasKitHost<H>
where
    H: CanvasKitParagraphHost,
{
    type Error = SharedHostError;

    fn exchange(&mut self, request: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        self.host
            .lock()
            .map_err(|_| SharedHostError("CanvasKit host lock is poisoned".into()))?
            .exchange(request)
            .map_err(|error| SharedHostError(error.to_string()))
    }

    fn supports_readback(&self) -> bool {
        self.host
            .lock()
            .map(|host| host.supports_readback())
            .unwrap_or(false)
    }

    fn read_pixels_rgba8888(
        &mut self,
        region: CanvasKitPixelRegion,
    ) -> Result<Option<CanvasKitReadback>, Self::Error> {
        self.host
            .lock()
            .map_err(|_| SharedHostError("CanvasKit host lock is poisoned".into()))?
            .read_pixels_rgba8888(region)
            .map_err(|error| SharedHostError(error.to_string()))
    }

    fn trim_memory(
        &mut self,
        pressure: fission_render::surface::MemoryPressure,
    ) -> Result<bool, Self::Error> {
        self.host
            .lock()
            .map_err(|_| SharedHostError("CanvasKit host lock is poisoned".into()))?
            .trim_memory(pressure)
            .map_err(|error| SharedHostError(error.to_string()))
    }

    fn poll_lifecycle_event(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        self.host
            .lock()
            .map_err(|_| SharedHostError("CanvasKit host lock is poisoned".into()))?
            .poll_lifecycle_event()
            .map_err(|error| SharedHostError(error.to_string()))
    }
}

impl<H> CanvasKitParagraphBridge for SharedCanvasKitHost<H>
where
    H: CanvasKitParagraphHost,
{
    fn layout_paragraph(&self, request: Vec<u8>) -> Result<Vec<u8>, String> {
        self.host
            .lock()
            .map_err(|_| "CanvasKit host lock is poisoned".to_string())?
            .layout_paragraph(request)
            .map_err(|error| error.to_string())
    }

    fn destroy_paragraph(
        &self,
        handle: fission_skia_sys::web::ResourceHandle,
    ) -> Result<(), String> {
        self.host
            .lock()
            .map_err(|_| "CanvasKit host lock is poisoned".to_string())?
            .destroy_paragraph(handle)
            .map_err(|error| error.to_string())
    }
}

/// Paired CanvasKit renderer and SkParagraph geometry profile.
///
/// Clones share one browser executor, immutable font catalog, and paragraph
/// draw-data registry. A paragraph measured by this profile is therefore the
/// exact object painted by its graphics session.
pub struct CanvasKitProfile<H: CanvasKitParagraphHost> {
    host: SharedCanvasKitHost<H>,
    catalog: Arc<CanvasKitFontCatalog>,
    font_state: Arc<CanvasKitFontState>,
    draw_data: Arc<CanvasKitParagraphDrawDataRegistry>,
}

impl<H: CanvasKitParagraphHost> CanvasKitProfile<H> {
    pub fn new(
        host: H,
        default_family: impl Into<String>,
        fonts: Vec<CanvasKitFont>,
    ) -> Result<Self, CanvasKitProfileError> {
        let default_family = default_family.into();
        let catalog = Arc::new(CanvasKitFontCatalog::new(&default_family, fonts)?);
        let font_state = Arc::new(CanvasKitFontState::new(
            catalog.generation(),
            default_family,
            catalog.families(),
        ));
        Ok(Self {
            host: SharedCanvasKitHost::new(host),
            catalog,
            font_state,
            draw_data: new_canvaskit_paragraph_registry(),
        })
    }

    pub fn paragraph_engine(&self) -> CanvasKitParagraphEngine {
        CanvasKitParagraphEngine::new(
            Arc::new(self.host.clone()),
            Arc::clone(&self.font_state),
            Arc::clone(&self.draw_data),
        )
    }

    pub fn create_session(
        &self,
        backend_preference: CanvasKitBackendPreference,
    ) -> BackendResult<GraphicsBackendSession<'static>> {
        GraphicsBackendSession::new(CanvasKitDriver::with_paragraph_profile(
            self.host.clone(),
            backend_preference,
            Arc::clone(&self.catalog),
            Arc::clone(&self.font_state),
            Arc::clone(&self.draw_data),
        ))
    }
}

struct StableHash(u64);

impl StableHash {
    const fn new() -> Self {
        Self(HASH_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(HASH_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.byte(byte);
        }
    }

    fn sized(&mut self, bytes: &[u8]) {
        self.bytes(&(bytes.len() as u64).to_le_bytes());
        self.bytes(bytes);
    }

    const fn finish_nonzero(self) -> u64 {
        if self.0 == 0 {
            1
        } else {
            self.0
        }
    }
}
