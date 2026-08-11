#ifndef FISSION_SKIA_GANESH_IOS_METAL_H
#define FISSION_SKIA_GANESH_IOS_METAL_H

#include "fission_skia.h"

#include <cstddef>
#include <cstdint>
#include <memory>

class SkCanvas;

namespace fission::skia::ganesh::ios_metal {

struct Result {
    fission_skia_status_t status = FISSION_SKIA_STATUS_OK;
    const char* message = "";

    [[nodiscard]] bool ok() const { return status == FISSION_SKIA_STATUS_OK; }

    static Result success() { return {}; }
    static Result failure(fission_skia_status_t status, const char* message) {
        return Result{status, message};
    }
};

// Borrowed iOS host descriptor. ui_view is a UIView*. Fission never owns or
// retains the view, UIWindow, or scene. The host must keep that hierarchy alive
// on the main thread for the complete lifetime of an attached surface. Context
// creation borrows the view only for a synchronous compatibility check.
struct IOSView {
    uint32_t struct_size = sizeof(IOSView);
    const void* ui_view = nullptr;
};

// Owns the iOS Metal device, command queue, and Ganesh context. The bridge
// destroys every surface first and enforces one main owner thread.
class IOSMetalContext final {
public:
    struct Impl;

    static Result create(
        const IOSView& compatible_view,
        std::unique_ptr<IOSMetalContext>* out_context);

    ~IOSMetalContext();

    IOSMetalContext(const IOSMetalContext&) = delete;
    IOSMetalContext& operator=(const IOSMetalContext&) = delete;
    IOSMetalContext(IOSMetalContext&&) = delete;
    IOSMetalContext& operator=(IOSMetalContext&&) = delete;

    Result trim_memory(uint32_t pressure);
    [[nodiscard]] Result health() const;
    [[nodiscard]] bool is_device_lost() const;

    [[nodiscard]] Impl& internal_state();
    [[nodiscard]] const Impl& internal_state() const;

private:
    IOSMetalContext();

    std::unique_ptr<Impl> impl_;

    friend class IOSMetalSurface;
};

// Owns one managed CAMetalLayer sublayer and its transient drawables. It never
// replaces UIView.layer. All lifecycle calls run on the main owner thread.
// Calls follow begin_frame -> finish_frame -> optional read_pixels -> present.
// A failed frame must be cancelled before reuse.
class IOSMetalSurface final {
public:
    struct Impl;

    struct Frame {
        Result result;
        SkCanvas* canvas = nullptr;
    };

    static Result create(
        IOSMetalContext& context,
        const IOSView& view,
        uint32_t width,
        uint32_t height,
        std::unique_ptr<IOSMetalSurface>* out_surface);

    ~IOSMetalSurface();

    IOSMetalSurface(const IOSMetalSurface&) = delete;
    IOSMetalSurface& operator=(const IOSMetalSurface&) = delete;
    IOSMetalSurface(IOSMetalSurface&&) = delete;
    IOSMetalSurface& operator=(IOSMetalSurface&&) = delete;

    Result resize(const IOSView& view, uint32_t width, uint32_t height);
    Result suspend();
    Result resume(const IOSView& view, uint32_t width, uint32_t height);

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
    explicit IOSMetalSurface(IOSMetalContext& context);

    std::unique_ptr<Impl> impl_;
};

[[nodiscard]] bool valid_ios_view(const IOSView& view);

}  // namespace fission::skia::ganesh::ios_metal

#endif  // FISSION_SKIA_GANESH_IOS_METAL_H
