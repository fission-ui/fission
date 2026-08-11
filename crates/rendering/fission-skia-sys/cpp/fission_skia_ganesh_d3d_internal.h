#ifndef FISSION_SKIA_GANESH_D3D_INTERNAL_H
#define FISSION_SKIA_GANESH_D3D_INTERNAL_H

#if !defined(_WIN32)
#error "The Fission Ganesh Direct3D runtime is Windows-only"
#endif

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif

#include <windows.h>
#include <d3d12.h>
#include <dxgi1_6.h>

#include "fission_skia_ganesh_d3d.h"

#include "include/core/SkRefCnt.h"
#include "include/core/SkSurface.h"
#include "include/gpu/ganesh/GrBackendSurface.h"
#include "include/gpu/ganesh/GrDirectContext.h"
#include "include/gpu/ganesh/d3d/GrD3DBackendContext.h"

#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <memory>

namespace fission::skia::ganesh::d3d {

constexpr uint32_t kSwapchainBufferCount = 3u;
constexpr uint32_t kNoImage = std::numeric_limits<uint32_t>::max();

struct D3DContext::Impl {
    gr_cp<IDXGIFactory4> factory;
    gr_cp<IDXGIAdapter1> adapter;
    gr_cp<ID3D12Device> device;
    gr_cp<ID3D12CommandQueue> queue;
    gr_cp<ID3D12Fence> queue_fence;
    HANDLE queue_fence_event = nullptr;
    uint64_t next_queue_fence_value = 1;
    sk_sp<GrDirectContext> ganesh;
    std::atomic<bool> device_lost{false};
};

struct SwapchainImage {
    gr_cp<ID3D12Resource> resource;
    GrBackendRenderTarget render_target;
    sk_sp<SkSurface> surface;
    uint64_t completion_fence_value = 0;
};

enum class SurfaceState {
    kIdle,
    kRecording,
    kReadyToPresent,
    kSuspended,
    kLost,
};

struct D3DSurface::Impl {
    explicit Impl(D3DContext& context) : context(&context) {}

    D3DContext* context;
    WindowsWindow window{};
    uint32_t width = 0;
    uint32_t height = 0;
    gr_cp<IDXGISwapChain3> swapchain;
    std::array<SwapchainImage, kSwapchainBufferCount> images;
    uint32_t image_count = 0;
    uint32_t active_image = kNoImage;
    bool readback_supported = false;
    SurfaceState state = SurfaceState::kSuspended;
};

Result classify_d3d_result(
    D3DContext::Impl& context,
    HRESULT result,
    fission_skia_status_t fallback_status,
    const char* fallback_message);
Result current_context_health(D3DContext::Impl& context);
Result signal_command_queue(D3DContext::Impl& context, uint64_t* out_fence_value);
Result wait_for_queue_fence(D3DContext::Impl& context, uint64_t fence_value);
Result drain_command_queue(D3DContext::Impl& context);

Result create_swapchain_attachment(D3DSurface::Impl& surface);
Result destroy_swapchain_attachment(D3DSurface::Impl& surface, bool wait_for_device);

}  // namespace fission::skia::ganesh::d3d

#endif  // FISSION_SKIA_GANESH_D3D_INTERNAL_H
