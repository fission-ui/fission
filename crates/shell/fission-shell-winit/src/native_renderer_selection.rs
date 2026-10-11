//! Automatic native renderer selection; explicit requests keep their existing behavior.

use crate::renderer_diagnostics::RendererRequest;

pub(super) fn should_auto_select_native_software(
    request: RendererRequest,
    windows: bool,
    device_type: wgpu::DeviceType,
    adapter_name: &str,
) -> bool {
    if request != RendererRequest::Auto {
        return false;
    }
    let adapter_name = adapter_name.trim().to_ascii_lowercase();
    // Running GPU compute shaders through a CPU adapter is unnecessarily
    // expensive for ordinary 2D rendering; use the native CPU rasterizer.
    device_type == wgpu::DeviceType::Cpu
        || (windows
            && (adapter_name.contains("warp")
                || adapter_name.contains("microsoft basic render driver")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_auto_uses_software_for_cpu_and_warp_adapters() {
        use wgpu::DeviceType::{Cpu, IntegratedGpu};

        assert!(should_auto_select_native_software(
            RendererRequest::Auto,
            true,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
        assert!(should_auto_select_native_software(
            RendererRequest::Auto,
            true,
            IntegratedGpu,
            "Microsoft Direct3D12 (WARP)"
        ));
        assert!(should_auto_select_native_software(
            RendererRequest::Auto,
            true,
            IntegratedGpu,
            "Microsoft Basic Render Driver"
        ));
    }

    #[test]
    fn linux_auto_uses_software_for_cpu_adapters() {
        assert!(should_auto_select_native_software(
            RendererRequest::Auto,
            false,
            wgpu::DeviceType::Cpu,
            "llvmpipe (LLVM 15.0.6, 128 bits)"
        ));
    }

    #[test]
    fn native_software_auto_selection_preserves_hardware_and_explicit_choices() {
        use wgpu::DeviceType::{Cpu, DiscreteGpu, IntegratedGpu};

        assert!(!should_auto_select_native_software(
            RendererRequest::Auto,
            false,
            IntegratedGpu,
            "Microsoft Basic Render Driver"
        ));
        for device_type in [IntegratedGpu, DiscreteGpu] {
            assert!(!should_auto_select_native_software(
                RendererRequest::Auto,
                false,
                device_type,
                "Linux hardware adapter"
            ));
        }
        for request in [
            RendererRequest::NativeVelloGpu,
            RendererRequest::NativeVelloCpu,
            RendererRequest::NativeSoftware,
        ] {
            assert!(!should_auto_select_native_software(
                request, false, Cpu, "llvmpipe"
            ));
        }
        assert!(!should_auto_select_native_software(
            RendererRequest::Auto,
            true,
            IntegratedGpu,
            "Qualcomm Adreno X1"
        ));
        assert!(!should_auto_select_native_software(
            RendererRequest::NativeVelloGpu,
            true,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
        assert!(!should_auto_select_native_software(
            RendererRequest::NativeVelloCpu,
            true,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
        assert!(!should_auto_select_native_software(
            RendererRequest::NativeSoftware,
            true,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
    }
}
