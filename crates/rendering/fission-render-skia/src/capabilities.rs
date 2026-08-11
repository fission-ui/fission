use fission_render::capabilities::{
    BackendIdentity, ColorFormat, DisplayOpKind, GraphicsCapabilities, RenderMode,
};

/// Semantics implemented by the initial direct-Skia raster foundation.
///
/// Save/restore are faithfully lowered for the currently admitted operation
/// set. `CachedScene` is a correctness-neutral cache hint and is recursively
/// lowered. Shape, text, image, SVG, filter, and external-surface operation
/// kinds remain unclaimed until every variant represented by the Fission
/// operation can be executed without degradation.
pub fn skia_raster_capabilities() -> GraphicsCapabilities {
    let mut capabilities = GraphicsCapabilities::empty(BackendIdentity::new(
        "skia",
        env!("CARGO_PKG_VERSION"),
        "raster-foundation",
    ));
    capabilities.render_modes.insert(RenderMode::Software);
    capabilities.display_ops.extend([
        DisplayOpKind::Save,
        DisplayOpKind::Restore,
        DisplayOpKind::CachedScene,
    ]);
    capabilities.color_formats.insert(ColorFormat::Rgba8Srgb);
    capabilities.headless = true;
    capabilities.readback = true;
    capabilities.surface_loss_recovery = true;
    capabilities.device_loss_recovery = true;
    capabilities
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_render::capabilities::{RenderMode, TransformSupport};

    #[test]
    fn foundation_profile_does_not_claim_unimplemented_paint_semantics() {
        let capabilities = skia_raster_capabilities();

        assert_eq!(
            capabilities.render_modes,
            [RenderMode::Software].into_iter().collect()
        );
        assert!(capabilities.supports_display_op(DisplayOpKind::Save));
        assert!(capabilities.supports_display_op(DisplayOpKind::Restore));
        assert!(capabilities.supports_display_op(DisplayOpKind::CachedScene));
        assert!(!capabilities.supports_display_op(DisplayOpKind::DrawRect));
        assert!(!capabilities.supports_display_op(DisplayOpKind::DrawText));
        assert!(!capabilities.supports_display_op(DisplayOpKind::DrawSurface));
        assert_eq!(capabilities.transform_support, TransformSupport::None);
        assert!(capabilities.headless);
        assert!(capabilities.readback);
        assert!(capabilities.surface_loss_recovery);
        assert!(capabilities.device_loss_recovery);
    }
}
