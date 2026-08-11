#ifndef FISSION_SKIA_GANESH_VULKAN_INTERNAL_H
#define FISSION_SKIA_GANESH_VULKAN_INTERNAL_H

#include "fission_skia_ganesh_vulkan.h"

#include "include/core/SkRefCnt.h"
#include "include/core/SkSurface.h"
#include "include/gpu/ganesh/GrBackendSurface.h"
#include "include/gpu/ganesh/GrDirectContext.h"
#include "include/gpu/vk/VulkanExtensions.h"
#include "include/gpu/vk/VulkanMemoryAllocator.h"
#include "include/private/gpu/vk/SkiaVulkan.h"

#include <atomic>
#include <limits>
#include <memory>
#include <vector>

namespace fission::skia::ganesh {

constexpr size_t kMaximumAcquireSemaphores = 8u;
constexpr uint32_t kNoImage = std::numeric_limits<uint32_t>::max();

struct InstanceDispatch {
    PFN_vkDestroyInstance destroy_instance = nullptr;
    PFN_vkEnumeratePhysicalDevices enumerate_physical_devices = nullptr;
    PFN_vkGetPhysicalDeviceProperties get_physical_device_properties = nullptr;
    PFN_vkGetPhysicalDeviceFeatures get_physical_device_features = nullptr;
    PFN_vkGetPhysicalDeviceQueueFamilyProperties get_queue_family_properties = nullptr;
    PFN_vkGetPhysicalDeviceSurfaceSupportKHR get_surface_support = nullptr;
    PFN_vkGetPhysicalDeviceSurfaceCapabilitiesKHR get_surface_capabilities = nullptr;
    PFN_vkGetPhysicalDeviceSurfaceFormatsKHR get_surface_formats = nullptr;
    PFN_vkGetPhysicalDeviceSurfacePresentModesKHR get_present_modes = nullptr;
    PFN_vkEnumerateDeviceExtensionProperties enumerate_device_extensions = nullptr;
    PFN_vkCreateDevice create_device = nullptr;
    PFN_vkDestroySurfaceKHR destroy_surface = nullptr;
};

struct DeviceDispatch {
    PFN_vkDestroyDevice destroy_device = nullptr;
    PFN_vkGetDeviceQueue get_device_queue = nullptr;
    PFN_vkDeviceWaitIdle device_wait_idle = nullptr;
    PFN_vkQueueWaitIdle queue_wait_idle = nullptr;
    PFN_vkCreateSwapchainKHR create_swapchain = nullptr;
    PFN_vkDestroySwapchainKHR destroy_swapchain = nullptr;
    PFN_vkGetSwapchainImagesKHR get_swapchain_images = nullptr;
    PFN_vkCreateSemaphore create_semaphore = nullptr;
    PFN_vkDestroySemaphore destroy_semaphore = nullptr;
    PFN_vkAcquireNextImageKHR acquire_next_image = nullptr;
    PFN_vkQueuePresentKHR queue_present = nullptr;
};

struct VulkanContext::Impl {
    uint32_t window_kind = 0;
    VkInstance instance = VK_NULL_HANDLE;
    VkPhysicalDevice physical_device = VK_NULL_HANDLE;
    VkDevice device = VK_NULL_HANDLE;
    VkQueue graphics_queue = VK_NULL_HANDLE;
    VkQueue initial_present_queue = VK_NULL_HANDLE;
    uint32_t graphics_queue_family = 0;
    uint32_t initial_present_queue_family = 0;
    uint32_t api_version = VK_API_VERSION_1_1;
    VkPhysicalDeviceFeatures enabled_features{};
    VkSurfaceKHR probe_surface = VK_NULL_HANDLE;
    InstanceDispatch instance_api;
    DeviceDispatch device_api;
    skgpu::VulkanExtensions extensions;
    sk_sp<skgpu::VulkanMemoryAllocator> allocator;
    sk_sp<GrDirectContext> ganesh;
    std::atomic<bool> device_lost{false};
};

struct SwapchainImage {
    VkImage image = VK_NULL_HANDLE;
    VkSemaphore rendering_complete = VK_NULL_HANDLE;
    GrBackendRenderTarget render_target;
    sk_sp<SkSurface> surface;
};

struct AcquireSemaphore {
    VkSemaphore handle = VK_NULL_HANDLE;
    std::atomic<bool> in_flight{false};
};

enum class SurfaceState {
    kIdle,
    kRecording,
    kReadyToPresent,
    kSuspended,
    kLost,
};

struct VulkanSurface::Impl {
    explicit Impl(VulkanContext& context) : context(&context) {}

    VulkanContext* context;
    fission_skia_native_window_t window{};
    uint32_t width = 0;
    uint32_t height = 0;
    VkExtent2D extent{};
    VkSurfaceKHR native_surface = VK_NULL_HANDLE;
    VkSwapchainKHR swapchain = VK_NULL_HANDLE;
    VkFormat format = VK_FORMAT_UNDEFINED;
    VkColorSpaceKHR color_space = VK_COLOR_SPACE_SRGB_NONLINEAR_KHR;
    VkImageUsageFlags usage = 0;
    VkSharingMode sharing_mode = VK_SHARING_MODE_EXCLUSIVE;
    VkQueue present_queue = VK_NULL_HANDLE;
    uint32_t present_queue_family = 0;
    bool readback_supported = false;
    bool recreate_after_present = false;
    SurfaceState state = SurfaceState::kSuspended;
    uint32_t active_image = kNoImage;
    AcquireSemaphore* active_acquire = nullptr;
    std::vector<SwapchainImage> images;
    std::vector<std::unique_ptr<AcquireSemaphore>> acquire_semaphores;
};

Result load_instance_dispatch(VulkanContext::Impl& context);
Result load_device_dispatch(VulkanContext::Impl& context);
Result create_native_surface(
    VulkanContext::Impl& context,
    const fission_skia_native_window_t& window,
    VkSurfaceKHR* out_surface);
Result create_swapchain_attachment(VulkanSurface::Impl& surface);
void destroy_swapchain_attachment(VulkanSurface::Impl& surface, bool wait_for_device);
Result classify_vulkan_result(VulkanContext::Impl& context, VkResult result);
const char* native_surface_extension(uint32_t window_kind);

}  // namespace fission::skia::ganesh

#endif  // FISSION_SKIA_GANESH_VULKAN_INTERNAL_H
