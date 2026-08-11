use fission_render::capabilities::{
    BackendIdentity, ColorFormat, DisplayOpKind, GraphicsCapabilities, ImageSourceKind, RenderMode,
    SvgProfile, TransformSupport,
};

/// Capabilities proven by the standalone software renderer.
///
/// Headless rendering and readback are inherent in [`crate::SoftwareRenderer`]:
/// each render call returns the complete premultiplied RGBA pixel buffer. The
/// crate does not own a presentation surface or device lifecycle, so it does
/// not claim surface or device recovery and does not expose a synthetic
/// graphics-session driver.
pub fn software_backend_capabilities() -> GraphicsCapabilities {
    let mut capabilities = GraphicsCapabilities::empty(BackendIdentity::new(
        "fission-software",
        env!("CARGO_PKG_VERSION"),
        "tiny-skia-fontdue",
    ));
    capabilities.render_modes.insert(RenderMode::Software);
    // Keep this list narrower than the renderer's transitional match arms.
    // Placeholder output, partial semantic handling, and an asynchronous
    // resource miss are not proof that an operation is conformantly rendered.
    capabilities.display_ops.extend([
        DisplayOpKind::Save,
        DisplayOpKind::Restore,
        DisplayOpKind::ClipRect,
        DisplayOpKind::ClipRoundedRect,
        DisplayOpKind::OpacityLayer,
        DisplayOpKind::Translate,
        DisplayOpKind::Transform,
        DisplayOpKind::CachedScene,
        DisplayOpKind::BackdropFilter,
        DisplayOpKind::DrawRect,
        DisplayOpKind::DrawText,
        DisplayOpKind::DrawRichText,
        DisplayOpKind::DrawImage,
        DisplayOpKind::DrawPath,
        DisplayOpKind::DrawSvg,
    ]);
    capabilities.transform_support = TransformSupport::Affine2d;
    capabilities.image_sources.extend([
        ImageSourceKind::Asset,
        ImageSourceKind::Memory,
        ImageSourceKind::Network,
    ]);
    #[cfg(not(target_arch = "wasm32"))]
    capabilities.image_sources.insert(ImageSourceKind::File);
    capabilities.svg_profile = SvgProfile::GeometryWithFissionPaint;
    capabilities.color_formats.insert(ColorFormat::Rgba8Srgb);
    capabilities.headless = true;
    capabilities.readback = true;
    capabilities
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn capabilities_match_the_exposed_software_renderer_contract() {
        let capabilities = software_backend_capabilities();

        for operation in [
            DisplayOpKind::Save,
            DisplayOpKind::Restore,
            DisplayOpKind::ClipRect,
            DisplayOpKind::ClipRoundedRect,
            DisplayOpKind::OpacityLayer,
            DisplayOpKind::Translate,
            DisplayOpKind::Transform,
            DisplayOpKind::CachedScene,
            DisplayOpKind::BackdropFilter,
            DisplayOpKind::DrawRect,
            DisplayOpKind::DrawText,
            DisplayOpKind::DrawRichText,
            DisplayOpKind::DrawImage,
            DisplayOpKind::DrawPath,
            DisplayOpKind::DrawSvg,
        ] {
            assert!(capabilities.supports_display_op(operation));
        }
        assert!(!capabilities.supports_display_op(DisplayOpKind::DrawSurface));
        assert_eq!(
            capabilities.render_modes,
            BTreeSet::from([RenderMode::Software])
        );
        assert_eq!(
            capabilities.color_formats,
            BTreeSet::from([ColorFormat::Rgba8Srgb])
        );
        assert!(capabilities.headless);
        assert!(capabilities.readback);
        assert!(capabilities.external_surface_transports.is_empty());
        assert_eq!(capabilities.transform_support, TransformSupport::Affine2d);
        assert!(capabilities.text_features.is_empty());
        assert!(capabilities.supports_image_source(ImageSourceKind::Memory));
        assert!(!capabilities.supports_image_source(ImageSourceKind::SvgText));
        assert_eq!(
            capabilities.svg_profile,
            SvgProfile::GeometryWithFissionPaint
        );
        assert!(!capabilities.surface_loss_recovery);
        assert!(!capabilities.device_loss_recovery);
    }
}
