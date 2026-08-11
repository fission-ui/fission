use std::collections::BTreeSet;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphicsCapabilities {
    pub identity: BackendIdentity,
    pub render_modes: BTreeSet<RenderMode>,
    pub display_ops: BTreeSet<DisplayOpKind>,
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
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityGap {
    RenderMode(RenderMode),
    DisplayOp(DisplayOpKind),
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
}
