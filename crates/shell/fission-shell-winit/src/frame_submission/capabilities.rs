use fission_render::capabilities::{
    DisplayOpKind, ExternalSurfaceTransport, GraphicsCapabilities, RenderMode,
};

pub(super) fn winit_vello_capabilities(render_mode: RenderMode) -> GraphicsCapabilities {
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

pub(super) fn winit_software_capabilities() -> GraphicsCapabilities {
    let mut capabilities = fission_render_software::software_backend_capabilities();
    capabilities.identity.name = "winit-software".to_string();
    capabilities.identity.profile = if cfg!(target_arch = "wasm32") {
        "canvas2d-host-composited"
    } else {
        "native-upload-host-composited"
    }
    .to_string();
    // The standalone rasterizer remains truthful and rejects DrawSurface. The
    // Winit host validates bindings, makes Ready slots transparent, and maps
    // non-ready slots to explicit deterministic 2D dispositions first.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_backend_profiles_declare_only_proven_surface_transport() {
        let vello = winit_vello_capabilities(RenderMode::Gpu);
        let vello_cpu = winit_vello_capabilities(RenderMode::Software);
        let software = winit_software_capabilities();

        assert_eq!(vello.render_modes.len(), 1);
        assert!(vello.render_modes.contains(&RenderMode::Gpu));
        assert_eq!(vello_cpu.render_modes.len(), 1);
        assert!(vello_cpu.render_modes.contains(&RenderMode::Software));
        assert_ne!(
            vello.identity,
            fission_render_vello::vello_backend_capabilities().identity
        );
        assert_ne!(
            software.identity,
            fission_render_software::software_backend_capabilities().identity
        );

        assert!(vello.supports_external_surface_transport(ExternalSurfaceTransport::NativeView));
        assert!(software.supports_external_surface_transport(ExternalSurfaceTransport::NativeView));
        #[cfg(all(feature = "three-d", not(target_arch = "wasm32")))]
        assert!(vello.supports_external_surface_transport(ExternalSurfaceTransport::DirectTarget));
        #[cfg(all(feature = "three-d", not(target_arch = "wasm32")))]
        assert!(
            software.supports_external_surface_transport(ExternalSurfaceTransport::DirectTarget)
        );
        #[cfg(all(feature = "three-d", target_arch = "wasm32"))]
        {
            assert!(
                !vello.supports_external_surface_transport(ExternalSurfaceTransport::DirectTarget)
            );
            assert!(!software
                .supports_external_surface_transport(ExternalSurfaceTransport::DirectTarget));
        }
        assert!(!vello.supports_external_surface_transport(ExternalSurfaceTransport::GpuImage));
        assert!(!software.supports_external_surface_transport(ExternalSurfaceTransport::GpuImage));
        assert!(software.supports_display_op(DisplayOpKind::BackdropFilter));
        assert!(!vello.supports_display_op(DisplayOpKind::BackdropFilter));
        assert!(!fission_render_vello::vello_backend_capabilities()
            .supports_display_op(DisplayOpKind::BackdropFilter));
    }
}
