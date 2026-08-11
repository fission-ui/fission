#ifndef FISSION_SKIA_GANESH_METAL_H
#define FISSION_SKIA_GANESH_METAL_H

#include "fission_skia.h"

#include <cstddef>
#include <cstdint>
#include <memory>

class SkCanvas;

namespace fission::skia::ganesh::metal {

// This result remains inside the bridge. Messages have static storage duration
// so the C ABI can copy them into its structured error value without retaining
// platform-owned objects.
struct Result {
    fission_skia_status_t status = FISSION_SKIA_STATUS_OK;
    const char* message = "";

    [[nodiscard]] bool ok() const { return status == FISSION_SKIA_STATUS_OK; }

    static Result success() { return {}; }
    static Result failure(fission_skia_status_t status, const char* message) {
        return Result{status, message};
    }
};

// Borrowed macOS host descriptor. ns_view is an NSView*. Fission never takes
// ownership of the view or its NSWindow hierarchy. The host must keep both
// alive, on the main thread, for the complete lifetime of a live attachment.
// Context creation borrows the view only for synchronous compatibility checks.
struct MacOSWindow {
    uint32_t struct_size = sizeof(MacOSWindow);
    const void* ns_view = nullptr;
};

// Owns the Metal device, command queue, and Ganesh context. The bridge registry
// must destroy every MetalSurface before destroying its MetalContext. All calls
// are owner-thread calls and context/surface attachment calls must be made on
// the macOS main thread because they access AppKit state.
class MetalContext final {
public:
    struct Impl;

    static Result create(
        const MacOSWindow& compatible_window,
        std::unique_ptr<MetalContext>* out_context);

    ~MetalContext();

    MetalContext(const MetalContext&) = delete;
    MetalContext& operator=(const MetalContext&) = delete;
    MetalContext(MetalContext&&) = delete;
    MetalContext& operator=(MetalContext&&) = delete;

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
    MetalContext();

    std::unique_ptr<Impl> impl_;

    friend class MetalSurface;
};

// Owns a CAMetalLayer attached to a borrowed host NSView and the transient
// drawables acquired from that layer. Calls follow this state machine:
// begin_frame -> finish_frame -> optional read_pixels -> present. A failed
// frame must be cancelled before another frame begins.
class MetalSurface final {
public:
    struct Impl;

    struct Frame {
        Result result;
        SkCanvas* canvas = nullptr;
    };

    static Result create(
        MetalContext& context,
        const MacOSWindow& window,
        uint32_t width,
        uint32_t height,
        std::unique_ptr<MetalSurface>* out_surface);

    ~MetalSurface();

    MetalSurface(const MetalSurface&) = delete;
    MetalSurface& operator=(const MetalSurface&) = delete;
    MetalSurface(MetalSurface&&) = delete;
    MetalSurface& operator=(MetalSurface&&) = delete;

    Result resize(const MacOSWindow& window, uint32_t width, uint32_t height);
    Result suspend();
    Result resume(const MacOSWindow& window, uint32_t width, uint32_t height);

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
    explicit MetalSurface(MetalContext& context);

    std::unique_ptr<Impl> impl_;
};

[[nodiscard]] bool valid_macos_window(const MacOSWindow& window);

}  // namespace fission::skia::ganesh::metal

#endif  // FISSION_SKIA_GANESH_METAL_H
