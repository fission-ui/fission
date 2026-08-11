use std::collections::BTreeSet;

use fission_ir::op::ImageSource;
use serde::{Deserialize, Serialize};

/// Stable identity reported by an interactive graphics backend.
///
/// Identity is diagnostic information. Core rendering decisions must be made
/// from [`GraphicsCapabilities`] instead of matching a backend name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BackendIdentity {
    pub name: String,
    pub version: String,
    pub profile: String,
}

impl BackendIdentity {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        profile: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            profile: profile.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RenderMode {
    Software,
    Gpu,
}

macro_rules! define_display_op_kinds {
    ($($kind:ident),+ $(,)?) => {
        /// The operation inventory accepted by a graphics backend.
        ///
        /// This mirrors `DisplayOp` deliberately. Adding a display operation
        /// requires an explicit backend disposition instead of relying on a
        /// wildcard match. The enum and `ALL` are emitted from one declaration
        /// so the production-baseline inventory cannot omit a declared kind.
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub enum DisplayOpKind {
            $($kind),+
        }

        impl DisplayOpKind {
            pub const ALL: [Self; [$(stringify!($kind)),+].len()] = [$(Self::$kind),+];
        }
    };
}

define_display_op_kinds!(
    Save,
    Restore,
    ClipRect,
    ClipRoundedRect,
    OpacityLayer,
    Translate,
    Transform,
    CachedScene,
    BackdropFilter,
    DrawRect,
    DrawText,
    DrawRichText,
    DrawImage,
    DrawPath,
    DrawSvg,
    DrawSurface,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ColorFormat {
    Rgba8Srgb,
    Bgra8Srgb,
    Rgba16Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExternalSurfaceTransport {
    CpuImage,
    NativeImage,
    GpuImage,
    NativeView,
    /// Transitional shell-owned encoding directly into the active target.
    ///
    /// This is not an interchangeable image and does not imply zero-copy
    /// composition. Backends must opt into it explicitly while legacy direct
    /// target producers are migrated to normal external-image bindings.
    DirectTarget,
}

/// Matrix semantics a backend can execute without discarding information.
///
/// The ordering is intentional: full 4x4 support includes finite 2D affine
/// matrices, while 2D affine support must reject perspective and 3D terms.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum TransformSupport {
    #[default]
    None,
    Affine2d,
    Full4x4,
}

impl TransformSupport {
    pub const fn satisfies(self, required: Self) -> bool {
        self as u8 >= required as u8
    }

    pub fn supports_matrix(self, matrix: &[f32; 16]) -> bool {
        match self {
            Self::None => false,
            Self::Affine2d => is_2d_affine_transform(matrix),
            Self::Full4x4 => matrix.iter().all(|value| value.is_finite()),
        }
    }
}

/// Returns whether a column-major 4x4 matrix is a finite 2D affine transform.
///
/// This is the canonical predicate for renderer adapters. Keeping it here
/// prevents each backend from accepting a subtly different subset.
pub fn is_2d_affine_transform(matrix: &[f32; 16]) -> bool {
    matrix.iter().all(|value| value.is_finite())
        && matrix[2] == 0.0
        && matrix[3] == 0.0
        && matrix[6] == 0.0
        && matrix[7] == 0.0
        && matrix[8] == 0.0
        && matrix[9] == 0.0
        && matrix[10] == 1.0
        && matrix[11] == 0.0
        && matrix[14] == 0.0
        && matrix[15] == 1.0
}

/// Optional text semantics that an operation-kind declaration alone cannot
/// prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TextFeature {
    CaretPainting,
    NonDefaultParagraphStyle,
    RichTextLocale,
    RichTextLineHeight,
    RichTextLetterSpacing,
}

impl TextFeature {
    pub const ALL: [Self; 5] = [
        Self::CaretPainting,
        Self::NonDefaultParagraphStyle,
        Self::RichTextLocale,
        Self::RichTextLineHeight,
        Self::RichTextLetterSpacing,
    ];
}

/// Target-independent classification of Fission image sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ImageSourceKind {
    Asset,
    File,
    Network,
    Memory,
    SvgText,
}

impl ImageSourceKind {
    pub const ALL: [Self; 5] = [
        Self::Asset,
        Self::File,
        Self::Network,
        Self::Memory,
        Self::SvgText,
    ];

    pub fn from_source(source: &ImageSource) -> Self {
        match source {
            ImageSource::Asset { .. } => Self::Asset,
            ImageSource::File { .. } => Self::File,
            ImageSource::Network { .. } => Self::Network,
            ImageSource::Memory { .. } => Self::Memory,
            ImageSource::SvgText { .. } => Self::SvgText,
        }
    }
}

/// SVG semantics a backend claims for `DrawSvg` operations.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum SvgProfile {
    #[default]
    None,
    /// Geometry is painted only from the Fission `fill`/`stroke` fields.
    GeometryWithFissionPaint,
    /// The SVG document's own paint and supported document semantics are
    /// honored when Fission does not supply an override.
    FullDocument,
}

impl SvgProfile {
    pub const fn satisfies(self, required: Self) -> bool {
        self as u8 >= required as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphicsCapabilities {
    pub identity: BackendIdentity,
    pub render_modes: BTreeSet<RenderMode>,
    pub display_ops: BTreeSet<DisplayOpKind>,
    #[serde(default)]
    pub transform_support: TransformSupport,
    #[serde(default)]
    pub text_features: BTreeSet<TextFeature>,
    #[serde(default)]
    pub image_sources: BTreeSet<ImageSourceKind>,
    #[serde(default)]
    pub svg_profile: SvgProfile,
    pub color_formats: BTreeSet<ColorFormat>,
    pub external_surface_transports: BTreeSet<ExternalSurfaceTransport>,
    pub headless: bool,
    pub readback: bool,
    pub surface_loss_recovery: bool,
    pub device_loss_recovery: bool,
}

impl GraphicsCapabilities {
    pub fn empty(identity: BackendIdentity) -> Self {
        Self {
            identity,
            render_modes: BTreeSet::new(),
            display_ops: BTreeSet::new(),
            transform_support: TransformSupport::None,
            text_features: BTreeSet::new(),
            image_sources: BTreeSet::new(),
            svg_profile: SvgProfile::None,
            color_formats: BTreeSet::new(),
            external_surface_transports: BTreeSet::new(),
            headless: false,
            readback: false,
            surface_loss_recovery: false,
            device_loss_recovery: false,
        }
    }

    pub fn supports_display_op(&self, operation: DisplayOpKind) -> bool {
        self.display_ops.contains(&operation)
    }

    pub fn supports_text_feature(&self, feature: TextFeature) -> bool {
        self.text_features.contains(&feature)
    }

    pub fn supports_image_source(&self, source: ImageSourceKind) -> bool {
        self.image_sources.contains(&source)
    }

    pub fn supports_external_surface_transport(&self, transport: ExternalSurfaceTransport) -> bool {
        self.external_surface_transports.contains(&transport)
    }

    pub fn validate(&self, requirements: &GraphicsRequirements) -> CapabilityReport {
        let mut gaps = Vec::new();

        for mode in requirements
            .required_render_modes
            .difference(&self.render_modes)
        {
            gaps.push(CapabilityGap::RenderMode(*mode));
        }
        for operation in requirements
            .required_display_ops
            .difference(&self.display_ops)
        {
            gaps.push(CapabilityGap::DisplayOp(*operation));
        }
        if !self
            .transform_support
            .satisfies(requirements.required_transform_support)
        {
            gaps.push(CapabilityGap::TransformSupport(
                requirements.required_transform_support,
            ));
        }
        for feature in requirements
            .required_text_features
            .difference(&self.text_features)
        {
            gaps.push(CapabilityGap::TextFeature(*feature));
        }
        for source in requirements
            .required_image_sources
            .difference(&self.image_sources)
        {
            gaps.push(CapabilityGap::ImageSource(*source));
        }
        if !self
            .svg_profile
            .satisfies(requirements.required_svg_profile)
        {
            gaps.push(CapabilityGap::SvgProfile(requirements.required_svg_profile));
        }
        for format in requirements
            .required_color_formats
            .difference(&self.color_formats)
        {
            gaps.push(CapabilityGap::ColorFormat(*format));
        }
        for transport in requirements
            .required_external_surface_transports
            .difference(&self.external_surface_transports)
        {
            gaps.push(CapabilityGap::ExternalSurfaceTransport(*transport));
        }
        if requirements.headless && !self.headless {
            gaps.push(CapabilityGap::Headless);
        }
        if requirements.readback && !self.readback {
            gaps.push(CapabilityGap::Readback);
        }
        if requirements.surface_loss_recovery && !self.surface_loss_recovery {
            gaps.push(CapabilityGap::SurfaceLossRecovery);
        }
        if requirements.device_loss_recovery && !self.device_loss_recovery {
            gaps.push(CapabilityGap::DeviceLossRecovery);
        }

        CapabilityReport { gaps }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GraphicsRequirements {
    pub required_render_modes: BTreeSet<RenderMode>,
    pub required_display_ops: BTreeSet<DisplayOpKind>,
    #[serde(default)]
    pub required_transform_support: TransformSupport,
    #[serde(default)]
    pub required_text_features: BTreeSet<TextFeature>,
    #[serde(default)]
    pub required_image_sources: BTreeSet<ImageSourceKind>,
    #[serde(default)]
    pub required_svg_profile: SvgProfile,
    pub required_color_formats: BTreeSet<ColorFormat>,
    pub required_external_surface_transports: BTreeSet<ExternalSurfaceTransport>,
    pub headless: bool,
    pub readback: bool,
    pub surface_loss_recovery: bool,
    pub device_loss_recovery: bool,
}

impl GraphicsRequirements {
    /// The display-operation baseline used by Fission's current built-in
    /// widgets. Surface transport and lifecycle requirements remain profile
    /// specific.
    pub fn current_display_list_baseline() -> Self {
        Self {
            required_display_ops: DisplayOpKind::ALL.into_iter().collect(),
            required_transform_support: TransformSupport::Full4x4,
            required_text_features: TextFeature::ALL.into_iter().collect(),
            required_image_sources: ImageSourceKind::ALL.into_iter().collect(),
            required_svg_profile: SvgProfile::FullDocument,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityGap {
    RenderMode(RenderMode),
    DisplayOp(DisplayOpKind),
    TransformSupport(TransformSupport),
    TextFeature(TextFeature),
    ImageSource(ImageSourceKind),
    SvgProfile(SvgProfile),
    ColorFormat(ColorFormat),
    ExternalSurfaceTransport(ExternalSurfaceTransport),
    Headless,
    Readback,
    SurfaceLossRecovery,
    DeviceLossRecovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityReport {
    pub gaps: Vec<CapabilityGap>,
}

impl CapabilityReport {
    pub fn is_compatible(&self) -> bool {
        self.gaps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_requires_every_known_display_operation() {
        let requirements = GraphicsRequirements::current_display_list_baseline();
        let unique: BTreeSet<_> = DisplayOpKind::ALL.into_iter().collect();

        assert_eq!(unique.len(), DisplayOpKind::ALL.len());
        assert_eq!(requirements.required_display_ops, unique);
        assert_eq!(
            requirements.required_transform_support,
            TransformSupport::Full4x4
        );
        assert_eq!(
            requirements.required_text_features,
            TextFeature::ALL.into_iter().collect()
        );
        assert_eq!(
            requirements.required_image_sources,
            ImageSourceKind::ALL.into_iter().collect()
        );
        assert_eq!(requirements.required_svg_profile, SvgProfile::FullDocument);
    }

    #[test]
    fn validation_reports_each_missing_requirement() {
        let capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("test-backend", "1", "software"));
        let mut requirements = GraphicsRequirements::current_display_list_baseline();
        requirements.readback = true;

        let report = capabilities.validate(&requirements);

        assert!(!report.is_compatible());
        assert!(report.gaps.contains(&CapabilityGap::Readback));
        assert!(report
            .gaps
            .contains(&CapabilityGap::DisplayOp(DisplayOpKind::DrawRect)));
        assert!(report
            .gaps
            .contains(&CapabilityGap::TransformSupport(TransformSupport::Full4x4)));
        assert!(report
            .gaps
            .contains(&CapabilityGap::TextFeature(TextFeature::CaretPainting)));
        assert!(report
            .gaps
            .contains(&CapabilityGap::ImageSource(ImageSourceKind::Network)));
        assert!(report
            .gaps
            .contains(&CapabilityGap::SvgProfile(SvgProfile::FullDocument)));
    }

    #[test]
    fn direct_target_composition_is_an_explicit_transitional_capability() {
        let mut requirements = GraphicsRequirements::default();
        requirements
            .required_external_surface_transports
            .insert(ExternalSurfaceTransport::DirectTarget);
        let mut capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("test-backend", "1", "gpu"));
        capabilities.render_modes.insert(RenderMode::Gpu);

        let missing = capabilities.validate(&requirements);
        assert!(missing
            .gaps
            .contains(&CapabilityGap::ExternalSurfaceTransport(
                ExternalSurfaceTransport::DirectTarget
            )));

        capabilities
            .external_surface_transports
            .insert(ExternalSurfaceTransport::DirectTarget);
        assert!(capabilities.validate(&requirements).is_compatible());
    }

    #[test]
    fn affine_predicate_rejects_perspective_3d_and_non_finite_matrices() {
        let affine = [
            1.0, 0.25, 0.0, 0.0, -0.5, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 12.0, 8.0, 0.0, 1.0,
        ];
        assert!(is_2d_affine_transform(&affine));
        assert!(TransformSupport::Affine2d.supports_matrix(&affine));
        assert!(TransformSupport::Full4x4.supports_matrix(&affine));

        let mut perspective = affine;
        perspective[3] = 0.1;
        assert!(!is_2d_affine_transform(&perspective));
        assert!(!TransformSupport::Affine2d.supports_matrix(&perspective));
        assert!(TransformSupport::Full4x4.supports_matrix(&perspective));

        let mut non_finite = affine;
        non_finite[0] = f32::NAN;
        assert!(!is_2d_affine_transform(&non_finite));
        assert!(!TransformSupport::Full4x4.supports_matrix(&non_finite));
    }

    #[test]
    fn newly_added_structured_fields_default_when_deserializing_older_profiles() {
        let profile = serde_json::json!({
            "identity": { "name": "legacy", "version": "1", "profile": "gpu" },
            "render_modes": [],
            "display_ops": ["Transform", "DrawText"],
            "color_formats": [],
            "external_surface_transports": [],
            "headless": false,
            "readback": false,
            "surface_loss_recovery": false,
            "device_loss_recovery": false
        });

        let capabilities: GraphicsCapabilities = serde_json::from_value(profile).unwrap();

        assert_eq!(capabilities.transform_support, TransformSupport::None);
        assert!(capabilities.text_features.is_empty());
        assert!(capabilities.image_sources.is_empty());
        assert_eq!(capabilities.svg_profile, SvgProfile::None);
    }
}
