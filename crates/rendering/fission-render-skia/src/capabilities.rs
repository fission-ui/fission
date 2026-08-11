use fission_render::capabilities::{
    BackendIdentity, ColorFormat, DisplayOpKind, GraphicsCapabilities, ImageSourceKind, RenderMode,
    SvgProfile, TextFeature, TransformSupport,
};

/// Semantics implemented by a standalone direct-Skia raster session.
///
/// Rectangle and path claims include every current Fission fill, stroke,
/// rounded-corner, dash, cap, join, and box-shadow variant. `CachedScene` is a
/// correctness-neutral cache hint and is recursively lowered. Opacity uses a
/// native isolated save-layer so overlapping children receive group alpha
/// exactly once. Text is enabled only by [`crate::SkiaRasterProfile`], which
/// can prove that layout and paint share one draw-data registry. Memory images
/// are decoded only from the submitted frame resource snapshot. Backdrop blur
/// is an atomic native filter operation. SVG document paint is retained by
/// SkSVGDOM, while Fission fill/stroke overrides use the established geometry
/// subset and ordinary path paint machinery. Other filters and external
/// surfaces remain unclaimed.
pub fn skia_raster_capabilities() -> GraphicsCapabilities {
    raster_capabilities(false)
}

/// Semantics enabled only by an explicitly paired Skia raster profile.
pub(crate) fn skia_raster_profile_capabilities() -> GraphicsCapabilities {
    raster_capabilities(true)
}

fn raster_capabilities(paragraph_paint: bool) -> GraphicsCapabilities {
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
        DisplayOpKind::OpacityLayer,
        DisplayOpKind::Translate,
        DisplayOpKind::Transform,
        DisplayOpKind::CachedScene,
        DisplayOpKind::BackdropFilter,
        DisplayOpKind::DrawRect,
        DisplayOpKind::DrawImage,
        DisplayOpKind::DrawPath,
        DisplayOpKind::DrawSvg,
    ]);
    capabilities.image_sources.insert(ImageSourceKind::Memory);
    capabilities.svg_profile = SvgProfile::FullDocument;
    if paragraph_paint {
        capabilities
            .display_ops
            .extend([DisplayOpKind::DrawText, DisplayOpKind::DrawRichText]);
        capabilities.text_features.extend([
            TextFeature::CaretPainting,
            TextFeature::RichTextLocale,
            TextFeature::RichTextLineHeight,
            TextFeature::RichTextLetterSpacing,
        ]);
    }
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
        assert!(capabilities.supports_display_op(DisplayOpKind::OpacityLayer));
        assert!(capabilities.supports_display_op(DisplayOpKind::Translate));
        assert!(capabilities.supports_display_op(DisplayOpKind::Transform));
        assert!(capabilities.supports_display_op(DisplayOpKind::CachedScene));
        assert!(capabilities.supports_display_op(DisplayOpKind::BackdropFilter));
        assert!(capabilities.supports_display_op(DisplayOpKind::DrawRect));
        assert!(capabilities.supports_display_op(DisplayOpKind::DrawImage));
        assert!(capabilities.supports_display_op(DisplayOpKind::DrawPath));
        assert!(capabilities.supports_display_op(DisplayOpKind::DrawSvg));
        assert!(!capabilities.supports_display_op(DisplayOpKind::DrawText));
        assert!(!capabilities.supports_display_op(DisplayOpKind::DrawSurface));
        assert!(capabilities.supports_image_source(ImageSourceKind::Memory));
        assert!(!capabilities.supports_image_source(ImageSourceKind::Asset));
        assert!(!capabilities.supports_image_source(ImageSourceKind::File));
        assert!(!capabilities.supports_image_source(ImageSourceKind::Network));
        assert!(!capabilities.supports_image_source(ImageSourceKind::SvgText));
        assert_eq!(capabilities.svg_profile, SvgProfile::FullDocument);
        assert_eq!(capabilities.transform_support, TransformSupport::Affine2d);
        assert!(capabilities.headless);
        assert!(capabilities.readback);
        assert!(capabilities.surface_loss_recovery);
        assert!(capabilities.device_loss_recovery);
    }

    #[test]
    fn paired_profile_claims_only_supported_paragraph_variants() {
        let capabilities = skia_raster_profile_capabilities();

        assert!(capabilities.supports_display_op(DisplayOpKind::DrawText));
        assert!(capabilities.supports_display_op(DisplayOpKind::DrawRichText));
        assert!(capabilities.supports_text_feature(TextFeature::CaretPainting));
        assert!(!capabilities.supports_text_feature(TextFeature::NonDefaultParagraphStyle));
        assert!(capabilities.supports_text_feature(TextFeature::RichTextLocale));
        assert!(capabilities.supports_text_feature(TextFeature::RichTextLineHeight));
        assert!(capabilities.supports_text_feature(TextFeature::RichTextLetterSpacing));
    }
}
