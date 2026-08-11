#ifndef FISSION_SKIA_GANESH_D3D_H
#define FISSION_SKIA_GANESH_D3D_H

#include "fission_skia.h"

#include <cstddef>
#include <cstdint>
#include <memory>

class SkCanvas;

namespace fission::skia::ganesh::d3d {

// This result remains inside the C++ bridge. Messages have static storage
// duration so the public C ABI can copy them without retaining platform data.
struct Result {
    fission_skia_status_t status = FISSION_SKIA_STATUS_OK;
    const char* message = "";

    [[nodiscard]] bool ok() const { return status == FISSION_SKIA_STATUS_OK; }

    static Result success() { return {}; }
    static Result failure(fission_skia_status_t status, const char* message) {
        return Result{status, message};
    }
};

// Borrowed Windows host descriptor. hwnd is an HWND encoded as an opaque
// pointer so this bridge-private interface does not expose Windows headers to
// platform-neutral translation units. Fission never owns or destroys the
// window. The host must keep it alive on its creating thread for a surface's
// complete attachment lifetime. Context creation borrows it only for the
// synchronous compatibility check.
struct WindowsWindow {
    uint32_t struct_size = sizeof(WindowsWindow);
    const void* hwnd = nullptr;
};

// Owns the DXGI factory and adapter, D3D12 device and direct command queue,
// synchronization fence, and Ganesh context. The bridge registry must destroy
// every D3DSurface before destroying its D3DContext.
class D3DContext final {
public:
    struct Impl;

    static Result create(
        const WindowsWindow& compatible_window,
        std::unique_ptr<D3DContext>* out_context);

    ~D3DContext();

    D3DContext(const D3DContext&) = delete;
    D3DContext& operator=(const D3DContext&) = delete;
    D3DContext(D3DContext&&) = delete;
    D3DContext& operator=(D3DContext&&) = delete;

    Result set_resource_cache_limit(uint64_t limit_bytes);
    Result resource_cache_usage(
        uint64_t* out_resource_count,
        uint64_t* out_resource_bytes) const;
    Result trim_memory(uint32_t pressure);
    [[nodiscard]] Result health() const;
    [[nodiscard]] bool is_device_lost() const;

    // Bridge-private access for the split context and surface implementation.
    [[nodiscard]] Impl& internal_state();
    [[nodiscard]] const Impl& internal_state() const;

private:
    D3DContext();

    std::unique_ptr<Impl> impl_;

    friend class D3DSurface;
};

// Owns a flip-model DXGI swapchain for one borrowed HWND and SkSurfaces that
// wrap its buffers. Calls follow this state machine:
// begin_frame -> finish_frame -> optional read_pixels -> present. A failed
// frame must be cancelled before another frame begins.
class D3DSurface final {
public:
    struct Impl;

    struct Frame {
        Result result;
        SkCanvas* canvas = nullptr;
    };

    static Result create(
        D3DContext& context,
        const WindowsWindow& window,
        uint32_t width,
        uint32_t height,
        std::unique_ptr<D3DSurface>* out_surface);

    ~D3DSurface();

    D3DSurface(const D3DSurface&) = delete;
    D3DSurface& operator=(const D3DSurface&) = delete;
    D3DSurface(D3DSurface&&) = delete;
    D3DSurface& operator=(D3DSurface&&) = delete;

    Result resize(const WindowsWindow& window, uint32_t width, uint32_t height);
    Result suspend();
    Result resume(const WindowsWindow& window, uint32_t width, uint32_t height);

    Frame begin_frame();
    Result finish_frame();
    Result cancel_frame();
    Result read_pixels_rgba8888(
        int32_t x,
        int32_t y,
        uint32_t width,
        uint32_t height,
        uint8_t* destination,
        size_t destination_length,
        size_t destination_row_bytes);
    Result present();

    [[nodiscard]] uint32_t width() const;
    [[nodiscard]] uint32_t height() const;
    [[nodiscard]] bool is_zero_sized() const;
    [[nodiscard]] bool supports_readback() const;

private:
    explicit D3DSurface(D3DContext& context);

    std::unique_ptr<Impl> impl_;
};

[[nodiscard]] bool valid_windows_window(const WindowsWindow& window);

}  // namespace fission::skia::ganesh::d3d

#endif  // FISSION_SKIA_GANESH_D3D_H
