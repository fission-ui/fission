#ifndef FISSION_SKIA_GANESH_ANDROID_VULKAN_H
#define FISSION_SKIA_GANESH_ANDROID_VULKAN_H

#include "fission_skia.h"

#include <cstddef>
#include <cstdint>
#include <memory>

struct ANativeWindow;
class SkCanvas;

namespace fission::skia::ganesh::android_vulkan {

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

// Borrowed Android host descriptor. The caller owns native_window and keeps it
// valid for the synchronous call that receives this descriptor. A live surface
// takes its own ANativeWindow reference for the complete Vulkan attachment
// lifetime, then releases that reference on resize, suspend, or destruction.
struct AndroidWindow {
    uint32_t struct_size = sizeof(AndroidWindow);
    ANativeWindow* native_window = nullptr;
};

// Owns the Vulkan instance, device, queues, allocator, and Ganesh context. The
// bridge registry remains responsible for enforcing its owner-thread contract
// and for destroying every AndroidVulkanSurface before this context.
class AndroidVulkanContext final {
public:
    struct Impl;

    static Result create(
        const AndroidWindow& compatible_window,
        std::unique_ptr<AndroidVulkanContext>* out_context);

    ~AndroidVulkanContext();

    AndroidVulkanContext(const AndroidVulkanContext&) = delete;
    AndroidVulkanContext& operator=(const AndroidVulkanContext&) = delete;
    AndroidVulkanContext(AndroidVulkanContext&&) = delete;
    AndroidVulkanContext& operator=(AndroidVulkanContext&&) = delete;

    Result set_resource_cache_limit(uint64_t limit_bytes);
    Result resource_cache_usage(
        uint64_t* out_resource_count,
        uint64_t* out_resource_bytes) const;
    Result trim_memory(uint32_t pressure);
    [[nodiscard]] bool is_device_lost() const;

    // Bridge-private access for the split context and surface units.
    [[nodiscard]] Impl& internal_state();
    [[nodiscard]] const Impl& internal_state() const;

private:
    AndroidVulkanContext();

    std::unique_ptr<Impl> impl_;

    friend class AndroidVulkanSurface;
};

// Owns a VkSurfaceKHR for one retained ANativeWindow, its swapchain,
// synchronization primitives, and the SkSurfaces wrapping its images. Calls
// follow: begin_frame -> finish_frame -> optional read_pixels -> present. A
// failed frame must be cancelled before another frame begins.
class AndroidVulkanSurface final {
public:
    struct Impl;

    struct Frame {
        Result result;
        SkCanvas* canvas = nullptr;
    };

    static Result create(
        AndroidVulkanContext& context,
        const AndroidWindow& window,
        uint32_t width,
        uint32_t height,
        std::unique_ptr<AndroidVulkanSurface>* out_surface);

    ~AndroidVulkanSurface();

    AndroidVulkanSurface(const AndroidVulkanSurface&) = delete;
    AndroidVulkanSurface& operator=(const AndroidVulkanSurface&) = delete;
    AndroidVulkanSurface(AndroidVulkanSurface&&) = delete;
    AndroidVulkanSurface& operator=(AndroidVulkanSurface&&) = delete;

    Result resize(const AndroidWindow& window, uint32_t width, uint32_t height);
    Result suspend();
    Result resume(const AndroidWindow& window, uint32_t width, uint32_t height);

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
    explicit AndroidVulkanSurface(AndroidVulkanContext& context);

    std::unique_ptr<Impl> impl_;
};

[[nodiscard]] bool valid_android_window(const AndroidWindow& window);

}  // namespace fission::skia::ganesh::android_vulkan

#endif  // FISSION_SKIA_GANESH_ANDROID_VULKAN_H
