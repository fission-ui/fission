#ifndef FISSION_SKIA_GANESH_IOS_METAL_INTERNAL_H
#define FISSION_SKIA_GANESH_IOS_METAL_INTERNAL_H

#include "fission_skia_ganesh_ios_metal.h"

#include "include/core/SkRefCnt.h"
#include "include/core/SkSurface.h"
#include "include/gpu/ganesh/GrDirectContext.h"
#include "include/gpu/ganesh/mtl/GrMtlTypes.h"
#include "include/ports/SkCFObject.h"

#include <atomic>
#include <memory>

namespace fission::skia::ganesh::ios_metal {

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

struct IOSMetalContext::Impl {
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

struct IOSMetalSurface::Impl {
    explicit Impl(IOSMetalContext& context) : context(&context) {}

    IOSMetalContext* context;
    IOSView view{};
    uint32_t width = 0;
    uint32_t height = 0;

    // view.ui_view is borrowed; layer and drawable are runtime-owned.
    sk_cfp<GrMTLHandle> layer;
    bool attachment_installed = false;
    sk_sp<SkSurface> surface;
    GrMTLHandle drawable = nullptr;
    SurfaceState state = SurfaceState::kSuspended;
};

Result drain_command_queue(IOSMetalContext::Impl& context);
Result current_context_health(IOSMetalContext::Impl& context);
void record_command_buffer_completion(
    const std::shared_ptr<DeviceHealth>& health,
    GrMTLHandle command_buffer);

Result attach_metal_layer(IOSMetalSurface::Impl& surface);
void detach_metal_layer(IOSMetalSurface::Impl& surface, bool drain_queue);
[[nodiscard]] bool attachment_is_current(const IOSMetalSurface::Impl& surface);
void release_active_frame(IOSMetalSurface::Impl& surface);

}  // namespace fission::skia::ganesh::ios_metal

#endif  // FISSION_SKIA_GANESH_IOS_METAL_INTERNAL_H
