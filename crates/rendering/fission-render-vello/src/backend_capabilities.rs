use fission_render::capabilities::{
    BackendIdentity, ColorFormat, DisplayOpKind, GraphicsCapabilities, ImageSourceKind, RenderMode,
    SvgProfile, TextFeature, TransformSupport,
};

/// Capabilities of the existing Vello scene encoder.
///
/// Surface transport, recovery, and readback remain owned by the current shell
/// until presentation is moved behind `GraphicsBackendSession`; they are not
/// overstated here merely because the Winit compositor can perform them.
pub fn vello_backend_capabilities() -> GraphicsCapabilities {
    let mut capabilities = GraphicsCapabilities::empty(BackendIdentity::new(
        "vello",
        env!("CARGO_PKG_VERSION"),
        "vello-wgpu",
    ));
    capabilities.render_modes.insert(RenderMode::Gpu);
    capabilities
        .display_ops
        .extend(DisplayOpKind::ALL.iter().copied().filter(|operation| {
            !matches!(
                operation,
                DisplayOpKind::BackdropFilter | DisplayOpKind::DrawSurface
            )
        }));
    capabilities.transform_support = TransformSupport::Affine2d;
    capabilities
        .text_features
        .extend(TextFeature::ALL.iter().copied());
    capabilities.image_sources.extend([
        ImageSourceKind::Asset,
        ImageSourceKind::Memory,
        ImageSourceKind::Network,
    ]);
    #[cfg(not(target_arch = "wasm32"))]
    capabilities.image_sources.insert(ImageSourceKind::File);
    capabilities.svg_profile = SvgProfile::GeometryWithFissionPaint;
    capabilities.color_formats.extend([
        ColorFormat::Rgba8Srgb,
        ColorFormat::Bgra8Srgb,
        ColorFormat::Rgba16Float,
    ]);
    capabilities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vello_reports_only_semantics_implemented_by_its_scene_encoder() {
        let capabilities = vello_backend_capabilities();

        assert!(!capabilities.supports_display_op(DisplayOpKind::BackdropFilter));
        assert!(!capabilities.supports_display_op(DisplayOpKind::DrawSurface));
        assert_eq!(capabilities.transform_support, TransformSupport::Affine2d);
        assert_eq!(
            capabilities.text_features,
            TextFeature::ALL.into_iter().collect()
        );
        assert!(capabilities.supports_image_source(ImageSourceKind::Memory));
        assert!(!capabilities.supports_image_source(ImageSourceKind::SvgText));
        assert_eq!(
            capabilities.svg_profile,
            SvgProfile::GeometryWithFissionPaint
        );
    }
}
