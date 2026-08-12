use fission_render::capabilities::{
    DisplayOpKind, ExternalSurfaceTransport, GraphicsCapabilities, RenderMode,
};

pub(crate) fn winit_vello_capabilities(render_mode: RenderMode) -> GraphicsCapabilities {
    let mut capabilities = fission_render_vello::vello_backend_capabilities();
    capabilities.identity.name = "winit-vello".to_string();
    capabilities.identity.profile = match render_mode {
        RenderMode::Gpu => "winit-vello-gpu-composited",
        RenderMode::Software => "winit-vello-cpu-composited",
    }
    .to_string();
    capabilities.render_modes.clear();
    capabilities.render_modes.insert(render_mode);
    // The standalone encoder deliberately refuses to claim external surfaces.
    // This host validates their binding and placement, then presents them via
    // the declared transport alongside the encoded Vello frame.
    capabilities.display_ops.insert(DisplayOpKind::DrawSurface);
    capabilities
        .external_surface_transports
        .insert(ExternalSurfaceTransport::NativeView);
    #[cfg(all(feature = "three-d", not(target_arch = "wasm32")))]
    capabilities
        .external_surface_transports
        .insert(ExternalSurfaceTransport::DirectTarget);
    capabilities
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn winit_skia_raster_capabilities(
    capabilities: &GraphicsCapabilities,
) -> GraphicsCapabilities {
    let mut capabilities = capabilities.clone();
    capabilities.identity.name = "winit-skia".to_string();
    capabilities.identity.profile = "native-raster-upload-host-composited".to_string();
    // Skia sees the host-composited frame, where every DrawSurface is removed
    // or replaced and the matching binding set is filtered. These claims
    // describe Winit's composition step, not Skia's standalone raster driver.
    capabilities.display_ops.insert(DisplayOpKind::DrawSurface);
    capabilities
        .external_surface_transports
        .insert(ExternalSurfaceTransport::NativeView);
    #[cfg(feature = "three-d")]
    capabilities
        .external_surface_transports
        .insert(ExternalSurfaceTransport::DirectTarget);
    capabilities
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn winit_canvaskit_capabilities(
    capabilities: &GraphicsCapabilities,
) -> GraphicsCapabilities {
    let mut capabilities = capabilities.clone();
    capabilities.identity.name = "winit-canvaskit".to_string();
    capabilities.identity.profile = "web-software-host-composited".to_string();
    // CanvasKit sees the host-composited frame, where every DrawSurface is
    // removed or replaced and the matching binding set is filtered. These
    // claims describe Winit's DOM composition step, not the standalone driver.
    capabilities.display_ops.insert(DisplayOpKind::DrawSurface);
    capabilities
        .external_surface_transports
        .insert(ExternalSurfaceTransport::NativeView);
    capabilities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_backend_profiles_declare_only_proven_surface_transport() {
        let vello = winit_vello_capabilities(RenderMode::Gpu);
        let vello_cpu = winit_vello_capabilities(RenderMode::Software);

        assert_eq!(vello.render_modes.len(), 1);
        assert!(vello.render_modes.contains(&RenderMode::Gpu));
        assert_eq!(vello_cpu.render_modes.len(), 1);
        assert!(vello_cpu.render_modes.contains(&RenderMode::Software));
        assert_ne!(
            vello.identity,
            fission_render_vello::vello_backend_capabilities().identity
        );
        assert!(vello.supports_external_surface_transport(ExternalSurfaceTransport::NativeView));
        #[cfg(all(feature = "three-d", not(target_arch = "wasm32")))]
        assert!(vello.supports_external_surface_transport(ExternalSurfaceTransport::DirectTarget));
        #[cfg(all(feature = "three-d", target_arch = "wasm32"))]
        {
            assert!(
                !vello.supports_external_surface_transport(ExternalSurfaceTransport::DirectTarget)
            );
        }
        assert!(!vello.supports_external_surface_transport(ExternalSurfaceTransport::GpuImage));
        assert!(!vello.supports_display_op(DisplayOpKind::BackdropFilter));
        assert!(!fission_render_vello::vello_backend_capabilities()
            .supports_display_op(DisplayOpKind::BackdropFilter));
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn canvaskit_host_profile_adds_only_host_owned_surface_semantics() {
        let mut backend =
            GraphicsCapabilities::empty(fission_render::capabilities::BackendIdentity::new(
                "skia",
                "test",
                "web-canvaskit-software",
            ));
        backend.display_ops.insert(DisplayOpKind::DrawRect);
        let hosted = winit_canvaskit_capabilities(&backend);

        assert!(hosted.supports_display_op(DisplayOpKind::DrawRect));
        assert!(hosted.supports_display_op(DisplayOpKind::DrawSurface));
        assert!(hosted.supports_external_surface_transport(ExternalSurfaceTransport::NativeView));
        assert!(!hosted.supports_external_surface_transport(ExternalSurfaceTransport::DirectTarget));
        assert!(!hosted.supports_external_surface_transport(ExternalSurfaceTransport::GpuImage));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn skia_host_profile_adds_only_host_owned_surface_semantics() {
        let mut backend = GraphicsCapabilities::empty(
            fission_render::capabilities::BackendIdentity::new("skia", "test", "raster"),
        );
        backend.display_ops.insert(DisplayOpKind::DrawRect);
        backend
            .image_sources
            .insert(fission_render::capabilities::ImageSourceKind::Memory);

        let host = winit_skia_raster_capabilities(&backend);

        assert!(host.supports_display_op(DisplayOpKind::DrawRect));
        assert!(host.supports_display_op(DisplayOpKind::DrawSurface));
        assert!(host.supports_external_surface_transport(ExternalSurfaceTransport::NativeView));
        assert!(host.supports_image_source(fission_render::capabilities::ImageSourceKind::Memory));
        assert!(!host.supports_image_source(fission_render::capabilities::ImageSourceKind::File));
    }
}
