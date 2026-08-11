#ifndef FISSION_SKIA_GANESH_METAL_INTERNAL_H
#define FISSION_SKIA_GANESH_METAL_INTERNAL_H

#include "fission_skia_ganesh_metal.h"

#include "include/core/SkRefCnt.h"
#include "include/core/SkSurface.h"
#include "include/gpu/ganesh/GrDirectContext.h"
#include "include/gpu/ganesh/mtl/GrMtlTypes.h"
#include "include/ports/SkCFObject.h"

#include <atomic>
#include <memory>

namespace fission::skia::ganesh::metal {

// Shared with asynchronous MTLCommandBuffer completion handlers. The first
// failure is sticky until the owning context is rebuilt, which prevents later
// successful submissions from hiding a device or context loss.
struct DeviceHealth {
    std::atomic<uint32_t> status{FISSION_SKIA_STATUS_OK};

    void record(fission_skia_status_t failure) {
        uint32_t expected = FISSION_SKIA_STATUS_OK;
        status.compare_exchange_strong(
            expected,
            static_cast<uint32_t>(failure),
            std::memory_order_acq_rel,
            std::memory_order_acquire);
    }

    [[nodiscard]] fission_skia_status_t load() const {
        return static_cast<fission_skia_status_t>(
            status.load(std::memory_order_acquire));
    }
};

struct MetalContext::Impl {
    sk_cfp<GrMTLHandle> device;
    sk_cfp<GrMTLHandle> queue;
    sk_sp<GrDirectContext> ganesh;
    std::shared_ptr<DeviceHealth> health;
};

enum class SurfaceState {
    kIdle,
    kRecording,
    kReadyToPresent,
    kSuspended,
    kLost,
};

struct MetalSurface::Impl {
    explicit Impl(MetalContext& context) : context(&context) {}

    MetalContext* context;
    MacOSWindow window{};
    uint32_t width = 0;
    uint32_t height = 0;

    // window.ns_view is borrowed. layer is runtime-owned. previous_layer is a
    // temporary retain used only to restore host state deterministically.
    sk_cfp<GrMTLHandle> layer;
    sk_cfp<GrMTLHandle> previous_layer;
    bool previous_wants_layer = false;
    bool attachment_installed = false;

    sk_sp<SkSurface> surface;
    GrMTLHandle drawable = nullptr;
    SurfaceState state = SurfaceState::kSuspended;
};

Result drain_command_queue(MetalContext::Impl& context);
Result current_context_health(MetalContext::Impl& context);
void record_command_buffer_completion(
    const std::shared_ptr<DeviceHealth>& health,
    GrMTLHandle command_buffer);

Result attach_metal_layer(MetalSurface::Impl& surface);
void detach_metal_layer(MetalSurface::Impl& surface, bool drain_queue);
[[nodiscard]] bool attachment_is_current(const MetalSurface::Impl& surface);
void release_active_frame(MetalSurface::Impl& surface);

}  // namespace fission::skia::ganesh::metal

#endif  // FISSION_SKIA_GANESH_METAL_INTERNAL_H
