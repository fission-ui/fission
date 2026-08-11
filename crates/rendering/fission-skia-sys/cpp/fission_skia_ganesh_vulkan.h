#ifndef FISSION_SKIA_GANESH_VULKAN_H
#define FISSION_SKIA_GANESH_VULKAN_H

#include "fission_skia.h"

#include <cstddef>
#include <cstdint>
#include <memory>

class SkCanvas;

namespace fission::skia::ganesh {

// This is an internal C++ result. The bridge copies message into its structured
// error object, so all messages intentionally have static storage duration.
struct Result {
    fission_skia_status_t status = FISSION_SKIA_STATUS_OK;
    const char* message = "";

    [[nodiscard]] bool ok() const { return status == FISSION_SKIA_STATUS_OK; }

    static Result success() { return {}; }
    static Result failure(fission_skia_status_t status, const char* message) {
        return Result{status, message};
    }
};

// Owns the Vulkan instance, device, queues, allocator, and Ganesh context. The
// C ABI registry remains responsible for enforcing its owner-thread contract
// and for ensuring that every VulkanSurface is destroyed first.
class VulkanContext final {
public:
    struct Impl;

    static Result create(
        const fission_skia_native_window_t& compatible_window,
        std::unique_ptr<VulkanContext>* out_context);

    ~VulkanContext();

    VulkanContext(const VulkanContext&) = delete;
    VulkanContext& operator=(const VulkanContext&) = delete;
    VulkanContext(VulkanContext&&) = delete;
    VulkanContext& operator=(VulkanContext&&) = delete;

    Result set_resource_cache_limit(uint64_t limit_bytes);
    Result resource_cache_usage(
        uint64_t* out_resource_count,
        uint64_t* out_resource_bytes) const;
    Result trim_memory(uint32_t pressure);
    [[nodiscard]] bool is_device_lost() const;
    [[nodiscard]] uint32_t window_kind() const;

    // Internal implementation access for the split context/surface units.
    // This header is bridge-private and is not part of the C ABI.
    [[nodiscard]] Impl& internal_state();
    [[nodiscard]] const Impl& internal_state() const;

private:
    VulkanContext();

    std::unique_ptr<Impl> impl_;

    friend class VulkanSurface;
};

// Owns one native VkSurfaceKHR, its swapchain, synchronization primitives,
// and the SkSurfaces wrapping its images. Calls follow this state machine:
// begin_frame -> finish_frame -> optional read_pixels -> present. A failed
// frame must call cancel_frame before another operation.
class VulkanSurface final {
public:
    struct Impl;

    struct Frame {
        Result result;
        SkCanvas* canvas = nullptr;
    };

    static Result create(
        VulkanContext& context,
        const fission_skia_native_window_t& window,
        uint32_t width,
        uint32_t height,
        std::unique_ptr<VulkanSurface>* out_surface);

    ~VulkanSurface();

    VulkanSurface(const VulkanSurface&) = delete;
    VulkanSurface& operator=(const VulkanSurface&) = delete;
    VulkanSurface(VulkanSurface&&) = delete;
    VulkanSurface& operator=(VulkanSurface&&) = delete;

    Result resize(
        const fission_skia_native_window_t& window,
        uint32_t width,
        uint32_t height);
    Result suspend();
    Result resume(
        const fission_skia_native_window_t& window,
        uint32_t width,
        uint32_t height);

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
    explicit VulkanSurface(VulkanContext& context);

    std::unique_ptr<Impl> impl_;
};

[[nodiscard]] bool valid_native_window(
    const fission_skia_native_window_t& window);

}  // namespace fission::skia::ganesh

#endif  // FISSION_SKIA_GANESH_VULKAN_H
