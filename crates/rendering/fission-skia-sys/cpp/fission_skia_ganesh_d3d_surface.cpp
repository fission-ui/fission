#include "fission_skia_ganesh_d3d_internal.h"

#if !defined(FISSION_SKIA_ENABLE_GANESH_D3D)
#error "Compile this source only for the Direct3D native-Ganesh profile"
#endif

#include "include/core/SkColorSpace.h"
#include "include/core/SkImageInfo.h"
#include "include/gpu/ganesh/GrTypes.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/d3d/GrD3DBackendSurface.h"

#include <climits>
#include <limits>
#include <new>
#include <utility>

namespace fission::skia::ganesh::d3d {
namespace {

// The cross-backend native presentation contract is BGRA8 with an sRGB color
// space. DXGI flip-model swapchains expose the non-sRGB resource view here;
// Skia's surface color space supplies the sRGB transfer function.
constexpr DXGI_FORMAT kSwapchainFormat = DXGI_FORMAT_B8G8R8A8_UNORM;

HWND native_hwnd(const WindowsWindow& window) {
    return reinterpret_cast<HWND>(const_cast<void*>(window.hwnd));
}

bool valid_extent(uint32_t width, uint32_t height) {
    return width <= static_cast<uint32_t>(INT_MAX) &&
           height <= static_cast<uint32_t>(INT_MAX);
}

bool frame_is_active(SurfaceState state) {
    return state == SurfaceState::kRecording ||
           state == SurfaceState::kReadyToPresent;
}

void clear_active_frame(D3DSurface::Impl& surface) {
    surface.active_image = kNoImage;
}

void release_attachment_objects(D3DSurface::Impl& surface) {
    for (auto& image : surface.images) {
        image.surface.reset();
        image.render_target = GrBackendRenderTarget();
        image.resource.reset();
        image.completion_fence_value = 0;
    }
    surface.image_count = 0;
    surface.swapchain.reset();
    surface.readback_supported = false;
    clear_active_frame(surface);
}

Result ganesh_operation_failure(
    D3DContext::Impl& context,
    fission_skia_status_t fallback_status,
    const char* fallback_message) {
    Result health = current_context_health(context);
    if (!health.ok()) return health;
    if (context.factory->IsCurrent() == FALSE) {
        return Result::failure(
            FISSION_SKIA_STATUS_CONTEXT_LOST,
            "the DXGI adapter set changed and the Direct3D context must be recreated");
    }
    if (context.ganesh->oomed()) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Ganesh exhausted Direct3D memory");
    }
    return Result::failure(fallback_status, fallback_message);
}

Result flush_active_frame(D3DSurface::Impl& surface, bool wait_for_completion) {
    if (!frame_is_active(surface.state) ||
        surface.active_image >= surface.image_count) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D surface has no active frame to flush");
    }
    auto& context = surface.context->internal_state();
    Result health = current_context_health(context);
    if (!health.ok()) return health;

    auto& image = surface.images[surface.active_image];
    GrFlushInfo flush_info{};
    context.ganesh->flush(
        image.surface.get(),
        SkSurfaces::BackendSurfaceAccess::kPresent,
        flush_info);
    if (!context.ganesh->submit(GrSyncCpu::kNo)) {
        return ganesh_operation_failure(
            context,
            FISSION_SKIA_STATUS_INTERNAL,
            "Ganesh could not submit the Direct3D frame");
    }
    health = current_context_health(context);
    if (!health.ok()) return health;

    uint64_t fence_value = 0;
    Result status = signal_command_queue(context, &fence_value);
    if (!status.ok()) return status;
    image.completion_fence_value = fence_value;
    return wait_for_completion
        ? wait_for_queue_fence(context, fence_value)
        : Result::success();
}

Result rebuild_attachment(D3DSurface::Impl& surface) {
    Result status = destroy_swapchain_attachment(surface, true);
    if (!status.ok()) {
        surface.state = SurfaceState::kLost;
        return status;
    }
    status = create_swapchain_attachment(surface);
    if (!status.ok()) surface.state = SurfaceState::kLost;
    return status;
}

Result present_result(D3DContext::Impl& context, HRESULT result) {
    if (SUCCEEDED(result) || result == DXGI_STATUS_OCCLUDED) {
        return Result::success();
    }
    return classify_d3d_result(
        context,
        result,
        FISSION_SKIA_STATUS_SURFACE_LOST,
        "the DXGI swapchain can no longer present to its host window");
}

}  // namespace

Result create_swapchain_attachment(D3DSurface::Impl& surface) {
    auto& context = surface.context->internal_state();
    if (!valid_windows_window(surface.window) ||
        !valid_extent(surface.width, surface.height)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Direct3D host window or surface dimensions are invalid");
    }
    if (surface.width == 0 || surface.height == 0) {
        release_attachment_objects(surface);
        // A zero-sized surface has no attachment, so it must not retain even a
        // non-owning dependency on an HWND that the host may now destroy.
        surface.window = WindowsWindow{};
        surface.state = SurfaceState::kSuspended;
        return Result::success();
    }
    Result health = current_context_health(context);
    if (!health.ok()) return health;

    DXGI_SWAP_CHAIN_DESC1 description{};
    description.Width = surface.width;
    description.Height = surface.height;
    description.Format = kSwapchainFormat;
    description.Stereo = FALSE;
    description.SampleDesc.Count = 1;
    description.SampleDesc.Quality = 0;
    description.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    description.BufferCount = kSwapchainBufferCount;
    description.Scaling = DXGI_SCALING_STRETCH;
    description.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
    description.AlphaMode = DXGI_ALPHA_MODE_IGNORE;
    description.Flags = 0;

    gr_cp<IDXGISwapChain1> swapchain1;
    HRESULT result = context.factory->CreateSwapChainForHwnd(
        context.queue.get(),
        native_hwnd(surface.window),
        &description,
        nullptr,
        nullptr,
        &swapchain1);
    if (FAILED(result)) {
        const fission_skia_status_t fallback = result == DXGI_ERROR_INVALID_CALL
            ? FISSION_SKIA_STATUS_INVALID_STATE
            : FISSION_SKIA_STATUS_SURFACE_LOST;
        return classify_d3d_result(
            context,
            result,
            fallback,
            result == DXGI_ERROR_INVALID_CALL
                ? "the HWND is invalid or already has a live DXGI swapchain"
                : "DXGI could not create a swapchain for the host window");
    }

    result = context.factory->MakeWindowAssociation(
        native_hwnd(surface.window), DXGI_MWA_NO_ALT_ENTER);
    if (FAILED(result)) {
        return classify_d3d_result(
            context,
            result,
            FISSION_SKIA_STATUS_SURFACE_LOST,
            "DXGI could not disable implicit fullscreen transitions");
    }
    result = swapchain1->QueryInterface(IID_PPV_ARGS(&surface.swapchain));
    if (FAILED(result)) {
        return classify_d3d_result(
            context,
            result,
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "DXGI does not expose the required flip-model swapchain interface");
    }

    auto srgb = SkColorSpace::MakeSRGB();
    for (uint32_t index = 0; index < kSwapchainBufferCount; ++index) {
        auto& image = surface.images[index];
        result = surface.swapchain->GetBuffer(
            index, IID_PPV_ARGS(&image.resource));
        if (FAILED(result)) {
            release_attachment_objects(surface);
            return classify_d3d_result(
                context,
                result,
                FISSION_SKIA_STATUS_SURFACE_LOST,
                "DXGI did not expose a required swapchain buffer");
        }
        const D3D12_RESOURCE_DESC resource_description =
            image.resource->GetDesc();
        if (resource_description.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D ||
            resource_description.Width != surface.width ||
            resource_description.Height != surface.height ||
            resource_description.Format != kSwapchainFormat ||
            resource_description.DepthOrArraySize != 1 ||
            resource_description.MipLevels != 1 ||
            resource_description.SampleDesc.Count != 1 ||
            (resource_description.Flags & D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET) == 0) {
            release_attachment_objects(surface);
            return Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "DXGI returned an incompatible Direct3D swapchain buffer");
        }

        GrD3DTextureResourceInfo resource_info;
        resource_info.fResource = image.resource;
        resource_info.fResourceState = D3D12_RESOURCE_STATE_PRESENT;
        resource_info.fFormat = kSwapchainFormat;
        resource_info.fSampleCount = 1;
        resource_info.fLevelCount = 1;
        resource_info.fSampleQualityPattern =
            resource_description.SampleDesc.Quality;
        resource_info.fProtected = skgpu::Protected::kNo;
        image.render_target = GrBackendRenderTargets::MakeD3D(
            static_cast<int>(surface.width),
            static_cast<int>(surface.height),
            resource_info);
        if (!image.render_target.isValid()) {
            release_attachment_objects(surface);
            return Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "Skia rejected a Direct3D swapchain render target");
        }
        image.surface = SkSurfaces::WrapBackendRenderTarget(
            context.ganesh.get(),
            image.render_target,
            kTopLeft_GrSurfaceOrigin,
            kBGRA_8888_SkColorType,
            srgb,
            nullptr);
        if (!image.surface) {
            release_attachment_objects(surface);
            return ganesh_operation_failure(
                context,
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "Skia could not wrap a Direct3D swapchain buffer");
        }
    }

    surface.image_count = kSwapchainBufferCount;
    surface.readback_supported = true;
    surface.active_image = kNoImage;
    surface.state = SurfaceState::kIdle;
    return Result::success();
}

Result destroy_swapchain_attachment(
    D3DSurface::Impl& surface,
    bool wait_for_device) {
    auto& context = surface.context->internal_state();
    Result status = Result::success();
    if (frame_is_active(surface.state) &&
        surface.active_image < surface.image_count && context.ganesh) {
        status = flush_active_frame(surface, false);
    }
    clear_active_frame(surface);

    if (wait_for_device && context.ganesh &&
        !context.device_lost.load(std::memory_order_acquire)) {
        Result drained = drain_command_queue(context);
        if (status.ok()) status = drained;
        if (drained.ok()) context.ganesh->checkAsyncWorkCompletion();
    }
    release_attachment_objects(surface);
    surface.state = SurfaceState::kSuspended;
    return status;
}

D3DSurface::D3DSurface(D3DContext& context)
    : impl_(new (std::nothrow) Impl(context)) {}

D3DSurface::~D3DSurface() {
    if (impl_) {
        const bool wait_for_device =
            !impl_->context->is_device_lost();
        (void)destroy_swapchain_attachment(*impl_, wait_for_device);
    }
}

Result D3DSurface::create(
    D3DContext& context,
    const WindowsWindow& window,
    uint32_t width,
    uint32_t height,
    std::unique_ptr<D3DSurface>* out_surface) {
    if (out_surface == nullptr || !valid_windows_window(window) ||
        !valid_extent(width, height)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Direct3D surface output, host window, or dimensions are invalid");
    }
    Result health = context.health();
    if (!health.ok()) return health;
    out_surface->reset();

    auto surface = std::unique_ptr<D3DSurface>(
        new (std::nothrow) D3DSurface(context));
    if (!surface || !surface->impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the Direct3D Ganesh surface state could not be allocated");
    }
    surface->impl_->window = window;
    surface->impl_->width = width;
    surface->impl_->height = height;
    Result status = create_swapchain_attachment(*surface->impl_);
    if (!status.ok()) return status;
    *out_surface = std::move(surface);
    return Result::success();
}

Result D3DSurface::resize(
    const WindowsWindow& window,
    uint32_t width,
    uint32_t height) {
    if (!impl_ || !valid_windows_window(window) ||
        !valid_extent(width, height)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Direct3D resize window or dimensions are invalid");
    }
    if (frame_is_active(impl_->state)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "a Direct3D surface cannot resize while a frame is active");
    }

    Result status = destroy_swapchain_attachment(*impl_, true);
    impl_->window = window;
    impl_->width = width;
    impl_->height = height;
    if (!status.ok()) {
        impl_->state = SurfaceState::kLost;
        return status;
    }
    status = create_swapchain_attachment(*impl_);
    if (!status.ok()) impl_->state = SurfaceState::kLost;
    return status;
}

Result D3DSurface::suspend() {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D Ganesh surface is not initialized");
    }
    if (frame_is_active(impl_->state)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "a Direct3D surface cannot suspend while a frame is active");
    }
    Result status = destroy_swapchain_attachment(*impl_, true);
    impl_->window = WindowsWindow{};
    impl_->width = 0;
    impl_->height = 0;
    impl_->state = status.ok()
        ? SurfaceState::kSuspended
        : SurfaceState::kLost;
    return status;
}

Result D3DSurface::resume(
    const WindowsWindow& window,
    uint32_t width,
    uint32_t height) {
    if (width == 0 || height == 0) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "a resumed Direct3D surface requires a non-zero extent");
    }
    return resize(window, width, height);
}

D3DSurface::Frame D3DSurface::begin_frame() {
    if (!impl_ || impl_->state != SurfaceState::kIdle || !impl_->swapchain ||
        impl_->image_count != kSwapchainBufferCount) {
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_INVALID_STATE,
                "the Direct3D surface is not ready to begin a frame"),
            nullptr,
        };
    }
    if (!valid_windows_window(impl_->window)) {
        impl_->state = SurfaceState::kLost;
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_SURFACE_LOST,
                "the Direct3D host window is no longer valid"),
            nullptr,
        };
    }
    auto& context = impl_->context->internal_state();
    Result health = current_context_health(context);
    if (!health.ok()) {
        impl_->state = SurfaceState::kLost;
        return Frame{health, nullptr};
    }

    const uint32_t image_index = impl_->swapchain->GetCurrentBackBufferIndex();
    if (image_index >= impl_->image_count) {
        impl_->state = SurfaceState::kLost;
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_INTERNAL,
                "DXGI returned an out-of-range swapchain buffer index"),
            nullptr,
        };
    }
    Result status = wait_for_queue_fence(
        context, impl_->images[image_index].completion_fence_value);
    if (!status.ok()) {
        impl_->state = SurfaceState::kLost;
        return Frame{status, nullptr};
    }
    context.ganesh->checkAsyncWorkCompletion();

    impl_->active_image = image_index;
    impl_->state = SurfaceState::kRecording;
    SkCanvas* canvas = impl_->images[image_index].surface->getCanvas();
    if (canvas == nullptr) {
        clear_active_frame(*impl_);
        impl_->state = SurfaceState::kLost;
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_SURFACE_LOST,
                "the Direct3D swapchain surface has no canvas"),
            nullptr,
        };
    }
    return Frame{Result::success(), canvas};
}

Result D3DSurface::finish_frame() {
    if (!impl_ || impl_->state != SurfaceState::kRecording) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D surface has no recording frame to finish");
    }
    impl_->state = SurfaceState::kReadyToPresent;
    return Result::success();
}

Result D3DSurface::cancel_frame() {
    if (!impl_ || !frame_is_active(impl_->state)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D surface has no active frame to cancel");
    }

    Result status = flush_active_frame(*impl_, true);
    clear_active_frame(*impl_);
    Result rebuilt = rebuild_attachment(*impl_);
    if (!status.ok()) return status;
    return rebuilt;
}

Result D3DSurface::read_pixels_rgba8888(
    int32_t x,
    int32_t y,
    uint32_t width,
    uint32_t height,
    uint8_t* destination,
    size_t destination_length,
    size_t destination_row_bytes) {
    if (!impl_ || impl_->state != SurfaceState::kReadyToPresent ||
        impl_->active_image >= impl_->image_count) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "Direct3D readback is only valid after frame execution and before present");
    }
    if (!impl_->readback_supported) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "the Direct3D swapchain does not support readback");
    }
    if (x < 0 || y < 0 || width == 0 || height == 0 ||
        destination == nullptr || width > static_cast<uint32_t>(INT_MAX) ||
        height > static_cast<uint32_t>(INT_MAX) ||
        width > std::numeric_limits<size_t>::max() / 4) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Direct3D readback request is invalid");
    }
    const size_t minimum_row_bytes = static_cast<size_t>(width) * 4;
    if (destination_row_bytes < minimum_row_bytes ||
        height > std::numeric_limits<size_t>::max() / destination_row_bytes ||
        destination_length < destination_row_bytes * height ||
        static_cast<uint64_t>(x) + width > impl_->width ||
        static_cast<uint64_t>(y) + height > impl_->height) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Direct3D readback bounds or destination stride are invalid");
    }

    auto srgb = SkColorSpace::MakeSRGB();
    const SkImageInfo info = SkImageInfo::Make(
        static_cast<int>(width),
        static_cast<int>(height),
        kRGBA_8888_SkColorType,
        kPremul_SkAlphaType,
        srgb);
    auto& context = impl_->context->internal_state();
    if (!impl_->images[impl_->active_image].surface->readPixels(
            info, destination, destination_row_bytes, x, y)) {
        return ganesh_operation_failure(
            context,
            FISSION_SKIA_STATUS_SURFACE_LOST,
            "Ganesh could not read the Direct3D swapchain surface");
    }
    return current_context_health(context);
}

Result D3DSurface::present() {
    if (!impl_ || impl_->state != SurfaceState::kReadyToPresent ||
        impl_->active_image >= impl_->image_count || !impl_->swapchain) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D surface has no completed frame to present");
    }
    auto& context = impl_->context->internal_state();
    const uint32_t image_index = impl_->active_image;
    auto& image = impl_->images[image_index];

    GrFlushInfo flush_info{};
    context.ganesh->flush(
        image.surface.get(),
        SkSurfaces::BackendSurfaceAccess::kPresent,
        flush_info);
    if (!context.ganesh->submit(GrSyncCpu::kNo)) {
        Result status = ganesh_operation_failure(
            context,
            FISSION_SKIA_STATUS_INTERNAL,
            "Ganesh could not submit the Direct3D frame for presentation");
        clear_active_frame(*impl_);
        impl_->state = SurfaceState::kLost;
        return status;
    }

    const HRESULT dxgi_result = impl_->swapchain->Present(1, 0);
    Result status = present_result(context, dxgi_result);
    uint64_t fence_value = 0;
    Result signal = signal_command_queue(context, &fence_value);
    if (signal.ok()) image.completion_fence_value = fence_value;
    clear_active_frame(*impl_);

    if (!status.ok()) {
        impl_->state = SurfaceState::kLost;
        return status;
    }
    if (!signal.ok()) {
        impl_->state = SurfaceState::kLost;
        return signal;
    }
    status = current_context_health(context);
    if (!status.ok()) {
        impl_->state = SurfaceState::kLost;
        return status;
    }
    impl_->state = SurfaceState::kIdle;
    return Result::success();
}

uint32_t D3DSurface::width() const {
    return impl_ ? impl_->width : 0;
}

uint32_t D3DSurface::height() const {
    return impl_ ? impl_->height : 0;
}

bool D3DSurface::is_zero_sized() const {
    return !impl_ || impl_->width == 0 || impl_->height == 0;
}

bool D3DSurface::supports_readback() const {
    return impl_ && impl_->readback_supported;
}

}  // namespace fission::skia::ganesh::d3d
