use fission_render::capabilities::{
    BackendIdentity, ColorFormat, DisplayOpKind, GraphicsCapabilities, RenderMode, TransformSupport,
};

/// Semantics implemented by the direct-Skia raster paint profile.
///
/// Rectangle and path claims include every current Fission fill, stroke,
/// rounded-corner, dash, cap, join, and box-shadow variant. `CachedScene` is a
/// correctness-neutral cache hint and is recursively lowered. Opacity layers,
/// text, images, SVG, filters, and external surfaces remain unclaimed until
/// every variant represented by their operation can execute without loss.
pub fn skia_raster_capabilities() -> GraphicsCapabilities {
    let mut capabilities = GraphicsCapabilities::empty(BackendIdentity::new(
        "skia",
        env!("CARGO_PKG_VERSION"),
        "native-raster",
    ));
    capabilities.render_modes.insert(RenderMode::Software);
    capabilities.display_ops.extend([
        DisplayOpKind::Save,
        DisplayOpKind::Restore,
        DisplayOpKind::ClipRect,
        DisplayOpKind::ClipRoundedRect,
        DisplayOpKind::Translate,
        DisplayOpKind::Transform,
        DisplayOpKind::CachedScene,
        DisplayOpKind::DrawRect,
        DisplayOpKind::DrawPath,
    ]);
    capabilities.transform_support = TransformSupport::Affine2d;
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
    fn paint_profile_claims_only_complete_operation_kinds() {
        let capabilities = skia_raster_capabilities();

        assert_eq!(
            capabilities.render_modes,
            [RenderMode::Software].into_iter().collect()
        );
        assert!(capabilities.supports_display_op(DisplayOpKind::Save));
        assert!(capabilities.supports_display_op(DisplayOpKind::Restore));
        assert!(capabilities.supports_display_op(DisplayOpKind::ClipRect));
        assert!(capabilities.supports_display_op(DisplayOpKind::ClipRoundedRect));
        assert!(capabilities.supports_display_op(DisplayOpKind::Translate));
        assert!(capabilities.supports_display_op(DisplayOpKind::Transform));
        assert!(capabilities.supports_display_op(DisplayOpKind::CachedScene));
        assert!(capabilities.supports_display_op(DisplayOpKind::DrawRect));
        assert!(capabilities.supports_display_op(DisplayOpKind::DrawPath));
        assert!(!capabilities.supports_display_op(DisplayOpKind::DrawText));
        assert!(!capabilities.supports_display_op(DisplayOpKind::DrawSurface));
        assert_eq!(capabilities.transform_support, TransformSupport::Affine2d);
        assert!(capabilities.headless);
        assert!(capabilities.readback);
        assert!(capabilities.surface_loss_recovery);
        assert!(capabilities.device_loss_recovery);
    }
}
