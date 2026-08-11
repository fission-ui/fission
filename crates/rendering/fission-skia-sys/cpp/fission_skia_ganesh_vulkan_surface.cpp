#include "fission_skia_ganesh_vulkan_internal.h"

#if !defined(__linux__)
#error "The first Fission Ganesh Vulkan runtime is Linux-only"
#endif

#if !defined(FISSION_SKIA_ENABLE_GANESH_VULKAN)
#error "Compile this source only for the native-ganesh profile"
#endif

#include "include/core/SkColorSpace.h"
#include "include/core/SkImageInfo.h"
#include "include/gpu/ganesh/GrBackendSemaphore.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/vk/GrVkBackendSemaphore.h"
#include "include/gpu/ganesh/vk/GrVkBackendSurface.h"
#include "include/gpu/ganesh/vk/GrVkTypes.h"

#include <algorithm>
#include <array>
#include <climits>
#include <limits>
#include <new>
#include <utility>

// Vulkan's WSI headers need only opaque platform declarations. Keeping those
// declarations here avoids taking compile-time or link-time dependencies on
// Wayland, Xlib, or XCB while retaining the official Vulkan structure types.
struct wl_display;
struct wl_surface;
typedef struct _XDisplay Display;
typedef unsigned long Window;
typedef unsigned long VisualID;
typedef struct xcb_connection_t xcb_connection_t;
typedef uint32_t xcb_window_t;
typedef uint32_t xcb_visualid_t;

#if defined(SK_USE_INTERNAL_VULKAN_HEADERS) && !defined(SK_BUILD_FOR_GOOGLE3)
#include "include/third_party/vulkan/vulkan/vulkan_wayland.h"
#include "include/third_party/vulkan/vulkan/vulkan_xcb.h"
#include "include/third_party/vulkan/vulkan/vulkan_xlib.h"
#else
#include <vulkan/vulkan_wayland.h>
#include <vulkan/vulkan_xcb.h>
#include <vulkan/vulkan_xlib.h>
#endif

namespace fission::skia::ganesh {
namespace {

constexpr uint32_t kMaximumSwapchainImages = 64u;
constexpr uint32_t kMaximumSurfaceEntries = 256u;
constexpr const char* kWaylandSurfaceExtension = "VK_KHR_wayland_surface";
constexpr const char* kXlibSurfaceExtension = "VK_KHR_xlib_surface";
constexpr const char* kXcbSurfaceExtension = "VK_KHR_xcb_surface";

template <typename Function>
Function instance_proc(VkInstance instance, const char* name) {
    return reinterpret_cast<Function>(vkGetInstanceProcAddr(instance, name));
}

template <typename Value, typename Enumerate>
Result enumerate_surface_values(
    VulkanContext::Impl& context,
    Enumerate enumerate,
    VkSurfaceKHR surface,
    std::vector<Value>* out_values) {
    uint32_t count = 0;
    VkResult result = enumerate(
        context.physical_device, surface, &count, nullptr);
    if (result != VK_SUCCESS) return classify_vulkan_result(context, result);
    if (count == 0 || count > kMaximumSurfaceEntries) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "the Vulkan surface returned no usable bounded configuration list");
    }
    out_values->resize(count);
    result = enumerate(
        context.physical_device, surface, &count, out_values->data());
    if (result != VK_SUCCESS) return classify_vulkan_result(context, result);
    out_values->resize(count);
    return Result::success();
}

Result fail_attachment(VulkanSurface::Impl& surface, Result result) {
    destroy_swapchain_attachment(surface, false);
    surface.state = result.status == FISSION_SKIA_STATUS_DEVICE_LOST
        ? SurfaceState::kLost
        : SurfaceState::kSuspended;
    return result;
}

Result choose_surface_format(
    const std::vector<VkSurfaceFormatKHR>& formats,
    VkSurfaceFormatKHR* out_format) {
    if (formats.size() == 1 && formats[0].format == VK_FORMAT_UNDEFINED &&
        formats[0].colorSpace == VK_COLOR_SPACE_SRGB_NONLINEAR_KHR) {
        *out_format = VkSurfaceFormatKHR{
            VK_FORMAT_B8G8R8A8_SRGB,
            VK_COLOR_SPACE_SRGB_NONLINEAR_KHR,
        };
        return Result::success();
    }
    for (const auto& format : formats) {
        if (format.format == VK_FORMAT_B8G8R8A8_SRGB &&
            format.colorSpace == VK_COLOR_SPACE_SRGB_NONLINEAR_KHR) {
            *out_format = format;
            return Result::success();
        }
    }
    return Result::failure(
        FISSION_SKIA_STATUS_UNSUPPORTED,
        "the Vulkan surface does not support the required BGRA8 sRGB format");
}

VkPresentModeKHR choose_present_mode(
    const std::vector<VkPresentModeKHR>& modes) {
    if (std::find(modes.begin(), modes.end(), VK_PRESENT_MODE_MAILBOX_KHR) !=
        modes.end()) {
        return VK_PRESENT_MODE_MAILBOX_KHR;
    }
    return VK_PRESENT_MODE_FIFO_KHR;
}

VkCompositeAlphaFlagBitsKHR choose_composite_alpha(
    VkCompositeAlphaFlagsKHR supported) {
    constexpr std::array<VkCompositeAlphaFlagBitsKHR, 4> preferred = {
        VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
        VK_COMPOSITE_ALPHA_PRE_MULTIPLIED_BIT_KHR,
        VK_COMPOSITE_ALPHA_POST_MULTIPLIED_BIT_KHR,
        VK_COMPOSITE_ALPHA_INHERIT_BIT_KHR,
    };
    for (auto value : preferred) {
        if ((supported & value) != 0) return value;
    }
    return static_cast<VkCompositeAlphaFlagBitsKHR>(0);
}

SkColorType sk_color_type(VkFormat format) {
    switch (format) {
        case VK_FORMAT_B8G8R8A8_SRGB:
            return kBGRA_8888_SkColorType;
        default:
            return kUnknown_SkColorType;
    }
}

Result create_semaphore(
    VulkanContext::Impl& context,
    VkSemaphore* out_semaphore) {
    VkSemaphoreCreateInfo info{};
    info.sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO;
    VkResult result = context.device_api.create_semaphore(
        context.device, &info, nullptr, out_semaphore);
    return classify_vulkan_result(context, result);
}

void acquire_finished(void* raw_semaphore) {
    auto* semaphore = static_cast<AcquireSemaphore*>(raw_semaphore);
    if (semaphore != nullptr) {
        semaphore->in_flight.store(false, std::memory_order_release);
    }
}

Result acquire_semaphore(
    VulkanSurface::Impl& surface,
    AcquireSemaphore** out_semaphore) {
    auto& context = surface.context->internal_state();
    context.ganesh->checkAsyncWorkCompletion();
    for (const auto& candidate : surface.acquire_semaphores) {
        if (!candidate->in_flight.load(std::memory_order_acquire)) {
            candidate->in_flight.store(true, std::memory_order_release);
            *out_semaphore = candidate.get();
            return Result::success();
        }
    }
    if (surface.acquire_semaphores.size() >= kMaximumAcquireSemaphores) {
        if (!context.ganesh->submit(GrSyncCpu::kYes)) {
            if (context.ganesh->isDeviceLost()) {
                context.device_lost.store(true, std::memory_order_release);
                return Result::failure(
                    FISSION_SKIA_STATUS_DEVICE_LOST,
                    "the Vulkan device was lost while waiting for frame synchronization");
            }
            return Result::failure(
                FISSION_SKIA_STATUS_INTERNAL,
                "Ganesh could not drain frame synchronization work");
        }
        context.ganesh->checkAsyncWorkCompletion();
        for (const auto& candidate : surface.acquire_semaphores) {
            if (!candidate->in_flight.load(std::memory_order_acquire)) {
                candidate->in_flight.store(true, std::memory_order_release);
                *out_semaphore = candidate.get();
                return Result::success();
            }
        }
        return Result::failure(
            FISSION_SKIA_STATUS_INTERNAL,
            "completed Ganesh work did not release an acquisition semaphore");
    }
    auto candidate = std::unique_ptr<AcquireSemaphore>(
        new (std::nothrow) AcquireSemaphore{});
    if (!candidate) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "frame synchronization state could not be allocated");
    }
    Result status = create_semaphore(context, &candidate->handle);
    if (!status.ok()) return status;
    candidate->in_flight.store(true, std::memory_order_release);
    *out_semaphore = candidate.get();
    surface.acquire_semaphores.push_back(std::move(candidate));
    return Result::success();
}

Result ganesh_submit_active(
    VulkanSurface::Impl& surface,
    bool for_present,
    bool sync_cpu) {
    auto& context = surface.context->internal_state();
    if (surface.active_image >= surface.images.size() ||
        surface.active_acquire == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the surface has no acquired swapchain image");
    }
    GrFlushInfo flush_info{};
    GrBackendSemaphore rendered;
    if (for_present) {
        rendered = GrBackendSemaphores::MakeVk(
            surface.images[surface.active_image].rendering_complete);
        flush_info.fNumSemaphores = 1;
        flush_info.fSignalSemaphores = &rendered;
    }
    flush_info.fFinishedProc = acquire_finished;
    flush_info.fFinishedContext = surface.active_acquire;
    const auto semaphore_result = context.ganesh->flush(
        surface.images[surface.active_image].surface.get(),
        for_present
            ? SkSurfaces::BackendSurfaceAccess::kPresent
            : SkSurfaces::BackendSurfaceAccess::kNoAccess,
        flush_info);
    const bool submitted = context.ganesh->submit(
        sync_cpu ? GrSyncCpu::kYes : GrSyncCpu::kNo);
    if (context.ganesh->isDeviceLost()) {
        context.device_lost.store(true, std::memory_order_release);
        return Result::failure(
            FISSION_SKIA_STATUS_DEVICE_LOST,
            "Ganesh could not submit work to the Vulkan device");
    }
    if (context.ganesh->oomed()) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Ganesh exhausted Vulkan memory while submitting the frame");
    }
    if (!submitted) {
        return Result::failure(
            FISSION_SKIA_STATUS_INTERNAL,
            "Ganesh rejected the Vulkan submission");
    }
    if (for_present && semaphore_result != GrSemaphoresSubmitted::kYes) {
        return Result::failure(
            FISSION_SKIA_STATUS_INTERNAL,
            "Ganesh did not submit the required presentation semaphore");
    }
    if (sync_cpu) context.ganesh->checkAsyncWorkCompletion();
    return Result::success();
}

Result rebuild_attachment(VulkanSurface::Impl& surface) {
    destroy_swapchain_attachment(surface, true);
    if (surface.width == 0 || surface.height == 0) {
        surface.state = SurfaceState::kSuspended;
        return Result::success();
    }
    return create_swapchain_attachment(surface);
}

void clear_active_frame(VulkanSurface::Impl& surface) {
    surface.active_image = kNoImage;
    surface.active_acquire = nullptr;
}

}  // namespace

const char* native_surface_extension(uint32_t window_kind) {
    switch (window_kind) {
        case FISSION_SKIA_NATIVE_WINDOW_WAYLAND:
            return kWaylandSurfaceExtension;
        case FISSION_SKIA_NATIVE_WINDOW_XLIB:
            return kXlibSurfaceExtension;
        case FISSION_SKIA_NATIVE_WINDOW_XCB:
            return kXcbSurfaceExtension;
        default:
            return nullptr;
    }
}

Result create_native_surface(
    VulkanContext::Impl& context,
    const fission_skia_native_window_t& window,
    VkSurfaceKHR* out_surface) {
    if (out_surface == nullptr || !valid_native_window(window) ||
        window.kind != context.window_kind) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the native window is invalid or uses a different window system");
    }
    *out_surface = VK_NULL_HANDLE;
    VkResult result = VK_ERROR_EXTENSION_NOT_PRESENT;
    switch (window.kind) {
        case FISSION_SKIA_NATIVE_WINDOW_WAYLAND: {
            auto create = instance_proc<PFN_vkCreateWaylandSurfaceKHR>(
                context.instance, "vkCreateWaylandSurfaceKHR");
            if (create == nullptr) break;
            VkWaylandSurfaceCreateInfoKHR info{};
            info.sType = VK_STRUCTURE_TYPE_WAYLAND_SURFACE_CREATE_INFO_KHR;
            info.display = reinterpret_cast<wl_display*>(
                static_cast<uintptr_t>(window.display));
            info.surface = reinterpret_cast<wl_surface*>(
                static_cast<uintptr_t>(window.window));
            result = create(context.instance, &info, nullptr, out_surface);
            break;
        }
        case FISSION_SKIA_NATIVE_WINDOW_XLIB: {
            auto create = instance_proc<PFN_vkCreateXlibSurfaceKHR>(
                context.instance, "vkCreateXlibSurfaceKHR");
            if (create == nullptr) break;
            VkXlibSurfaceCreateInfoKHR info{};
            info.sType = VK_STRUCTURE_TYPE_XLIB_SURFACE_CREATE_INFO_KHR;
            info.dpy = reinterpret_cast<Display*>(
                static_cast<uintptr_t>(window.display));
            info.window = static_cast<Window>(window.window);
            result = create(context.instance, &info, nullptr, out_surface);
            break;
        }
        case FISSION_SKIA_NATIVE_WINDOW_XCB: {
            auto create = instance_proc<PFN_vkCreateXcbSurfaceKHR>(
                context.instance, "vkCreateXcbSurfaceKHR");
            if (create == nullptr) break;
            VkXcbSurfaceCreateInfoKHR info{};
            info.sType = VK_STRUCTURE_TYPE_XCB_SURFACE_CREATE_INFO_KHR;
            info.connection = reinterpret_cast<xcb_connection_t*>(
                static_cast<uintptr_t>(window.display));
            info.window = static_cast<xcb_window_t>(window.window);
            result = create(context.instance, &info, nullptr, out_surface);
            break;
        }
        default:
            break;
    }
    return classify_vulkan_result(context, result);
}

Result create_swapchain_attachment(VulkanSurface::Impl& surface) {
    auto& context = surface.context->internal_state();
    if (context.device_lost.load(std::memory_order_acquire)) {
        surface.state = SurfaceState::kLost;
        return Result::failure(
            FISSION_SKIA_STATUS_DEVICE_LOST,
            "the Vulkan device is lost");
    }
    if (surface.width == 0 || surface.height == 0) {
        surface.state = SurfaceState::kSuspended;
        return Result::success();
    }

    Result status = create_native_surface(
        context, surface.window, &surface.native_surface);
    if (!status.ok()) return fail_attachment(surface, status);

    VkBool32 graphics_supported = VK_FALSE;
    VkBool32 initial_present_supported = VK_FALSE;
    VkResult result = context.instance_api.get_surface_support(
        context.physical_device,
        context.graphics_queue_family,
        surface.native_surface,
        &graphics_supported);
    if (result != VK_SUCCESS) {
        return fail_attachment(surface, classify_vulkan_result(context, result));
    }
    result = context.instance_api.get_surface_support(
        context.physical_device,
        context.initial_present_queue_family,
        surface.native_surface,
        &initial_present_supported);
    if (result != VK_SUCCESS) {
        return fail_attachment(surface, classify_vulkan_result(context, result));
    }
    if (graphics_supported == VK_TRUE) {
        surface.present_queue_family = context.graphics_queue_family;
        surface.present_queue = context.graphics_queue;
    } else if (initial_present_supported == VK_TRUE) {
        surface.present_queue_family = context.initial_present_queue_family;
        surface.present_queue = context.initial_present_queue;
    } else {
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "the selected Vulkan device has no usable queue for this fresh surface"));
    }

    VkSurfaceCapabilitiesKHR capabilities{};
    result = context.instance_api.get_surface_capabilities(
        context.physical_device, surface.native_surface, &capabilities);
    if (result != VK_SUCCESS) {
        return fail_attachment(surface, classify_vulkan_result(context, result));
    }
    std::vector<VkSurfaceFormatKHR> formats;
    status = enumerate_surface_values(
        context,
        context.instance_api.get_surface_formats,
        surface.native_surface,
        &formats);
    if (!status.ok()) return fail_attachment(surface, status);
    std::vector<VkPresentModeKHR> present_modes;
    status = enumerate_surface_values(
        context,
        context.instance_api.get_present_modes,
        surface.native_surface,
        &present_modes);
    if (!status.ok()) return fail_attachment(surface, status);

    VkSurfaceFormatKHR selected_format{};
    status = choose_surface_format(formats, &selected_format);
    if (!status.ok()) return fail_attachment(surface, status);
    if ((capabilities.supportedUsageFlags & VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT) == 0) {
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "the Vulkan surface images cannot be used as color attachments"));
    }
    VkExtent2D extent{surface.width, surface.height};
    if (capabilities.currentExtent.width != UINT32_MAX) {
        extent = capabilities.currentExtent;
    }
    if (extent.width != surface.width || extent.height != surface.height ||
        extent.width < capabilities.minImageExtent.width ||
        extent.height < capabilities.minImageExtent.height ||
        extent.width > capabilities.maxImageExtent.width ||
        extent.height > capabilities.maxImageExtent.height) {
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_SURFACE_LOST,
                "the host size does not match the Vulkan surface extent"));
    }
    uint32_t image_count = capabilities.minImageCount + 1;
    if (capabilities.maxImageCount != 0) {
        image_count = std::min(image_count, capabilities.maxImageCount);
    }
    if (image_count == 0 || image_count > kMaximumSwapchainImages) {
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "the Vulkan surface requires an unsupported swapchain image count"));
    }
    const VkCompositeAlphaFlagBitsKHR composite_alpha =
        choose_composite_alpha(capabilities.supportedCompositeAlpha);
    if (composite_alpha == 0) {
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "the Vulkan surface exposes no supported composite-alpha mode"));
    }

    surface.extent = extent;
    surface.format = selected_format.format;
    surface.color_space = selected_format.colorSpace;
    surface.usage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
    surface.readback_supported =
        (capabilities.supportedUsageFlags & VK_IMAGE_USAGE_TRANSFER_SRC_BIT) != 0;
    if (surface.readback_supported) {
        surface.usage |= VK_IMAGE_USAGE_TRANSFER_SRC_BIT;
    }
    surface.sharing_mode = surface.present_queue_family == context.graphics_queue_family
        ? VK_SHARING_MODE_EXCLUSIVE
        : VK_SHARING_MODE_CONCURRENT;
    uint32_t queue_families[] = {
        context.graphics_queue_family,
        surface.present_queue_family,
    };
    VkSwapchainCreateInfoKHR swapchain_info{};
    swapchain_info.sType = VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR;
    swapchain_info.surface = surface.native_surface;
    swapchain_info.minImageCount = image_count;
    swapchain_info.imageFormat = surface.format;
    swapchain_info.imageColorSpace = surface.color_space;
    swapchain_info.imageExtent = surface.extent;
    swapchain_info.imageArrayLayers = 1;
    swapchain_info.imageUsage = surface.usage;
    swapchain_info.imageSharingMode = surface.sharing_mode;
    if (surface.sharing_mode == VK_SHARING_MODE_CONCURRENT) {
        swapchain_info.queueFamilyIndexCount = 2;
        swapchain_info.pQueueFamilyIndices = queue_families;
    }
    swapchain_info.preTransform = capabilities.currentTransform;
    swapchain_info.compositeAlpha = composite_alpha;
    swapchain_info.presentMode = choose_present_mode(present_modes);
    swapchain_info.clipped = VK_TRUE;
    result = context.device_api.create_swapchain(
        context.device, &swapchain_info, nullptr, &surface.swapchain);
    if (result != VK_SUCCESS) {
        return fail_attachment(surface, classify_vulkan_result(context, result));
    }

    uint32_t actual_count = 0;
    result = context.device_api.get_swapchain_images(
        context.device, surface.swapchain, &actual_count, nullptr);
    if (result != VK_SUCCESS) {
        return fail_attachment(surface, classify_vulkan_result(context, result));
    }
    if (actual_count == 0 || actual_count > kMaximumSwapchainImages) {
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "the Vulkan swapchain returned an unsupported image count"));
    }
    std::vector<VkImage> native_images(actual_count);
    result = context.device_api.get_swapchain_images(
        context.device, surface.swapchain, &actual_count, native_images.data());
    if (result != VK_SUCCESS) {
        return fail_attachment(surface, classify_vulkan_result(context, result));
    }
    native_images.resize(actual_count);
    surface.images.resize(actual_count);
    const SkColorType color_type = sk_color_type(surface.format);
    auto srgb = SkColorSpace::MakeSRGB();
    for (uint32_t index = 0; index < actual_count; ++index) {
        auto& image = surface.images[index];
        image.image = native_images[index];
        status = create_semaphore(context, &image.rendering_complete);
        if (!status.ok()) return fail_attachment(surface, status);

        GrVkImageInfo image_info;
        image_info.fImage = image.image;
        image_info.fImageTiling = VK_IMAGE_TILING_OPTIMAL;
        image_info.fImageLayout = VK_IMAGE_LAYOUT_UNDEFINED;
        image_info.fFormat = surface.format;
        image_info.fImageUsageFlags = surface.usage;
        image_info.fSampleCount = 1;
        image_info.fLevelCount = 1;
        image_info.fCurrentQueueFamily =
            surface.sharing_mode == VK_SHARING_MODE_CONCURRENT
                ? VK_QUEUE_FAMILY_IGNORED
                : context.graphics_queue_family;
        image_info.fProtected = skgpu::Protected::kNo;
        image_info.fSharingMode = surface.sharing_mode;
        image.render_target = GrBackendRenderTargets::MakeVk(
            static_cast<int>(surface.extent.width),
            static_cast<int>(surface.extent.height),
            image_info);
        if (!image.render_target.isValid()) {
            return fail_attachment(
                surface,
                Result::failure(
                    FISSION_SKIA_STATUS_UNSUPPORTED,
                    "Skia rejected a Vulkan swapchain render target"));
        }
        image.surface = SkSurfaces::WrapBackendRenderTarget(
            context.ganesh.get(),
            image.render_target,
            kTopLeft_GrSurfaceOrigin,
            color_type,
            srgb,
            nullptr);
        if (!image.surface) {
            return fail_attachment(
                surface,
                Result::failure(
                    FISSION_SKIA_STATUS_UNSUPPORTED,
                    "Skia could not wrap a Vulkan swapchain image"));
        }
    }
    surface.state = SurfaceState::kIdle;
    surface.recreate_after_present = false;
    return Result::success();
}

void destroy_swapchain_attachment(
    VulkanSurface::Impl& surface,
    bool wait_for_device) {
    auto& context = surface.context->internal_state();
    if (wait_for_device && context.device != VK_NULL_HANDLE &&
        context.device_api.device_wait_idle != nullptr &&
        !context.device_lost.load(std::memory_order_acquire)) {
        const VkResult wait = context.device_api.device_wait_idle(context.device);
        if (wait == VK_ERROR_DEVICE_LOST) {
            context.device_lost.store(true, std::memory_order_release);
        }
    }
    if (context.ganesh &&
        !context.device_lost.load(std::memory_order_acquire)) {
        context.ganesh->checkAsyncWorkCompletion();
    }
    for (auto& image : surface.images) {
        image.surface.reset();
        image.render_target = GrBackendRenderTarget();
        if (image.rendering_complete != VK_NULL_HANDLE &&
            context.device_api.destroy_semaphore != nullptr) {
            context.device_api.destroy_semaphore(
                context.device, image.rendering_complete, nullptr);
            image.rendering_complete = VK_NULL_HANDLE;
        }
    }
    surface.images.clear();
    for (auto& semaphore : surface.acquire_semaphores) {
        if (semaphore->handle != VK_NULL_HANDLE &&
            context.device_api.destroy_semaphore != nullptr) {
            context.device_api.destroy_semaphore(
                context.device, semaphore->handle, nullptr);
            semaphore->handle = VK_NULL_HANDLE;
        }
    }
    surface.acquire_semaphores.clear();
    if (surface.swapchain != VK_NULL_HANDLE &&
        context.device_api.destroy_swapchain != nullptr) {
        context.device_api.destroy_swapchain(
            context.device, surface.swapchain, nullptr);
        surface.swapchain = VK_NULL_HANDLE;
    }
    if (surface.native_surface != VK_NULL_HANDLE &&
        context.instance_api.destroy_surface != nullptr) {
        context.instance_api.destroy_surface(
            context.instance, surface.native_surface, nullptr);
        surface.native_surface = VK_NULL_HANDLE;
    }
    surface.extent = {};
    surface.present_queue = VK_NULL_HANDLE;
    surface.readback_supported = false;
    surface.recreate_after_present = false;
    clear_active_frame(surface);
}

VulkanSurface::VulkanSurface(VulkanContext& context)
    : impl_(new (std::nothrow) Impl(context)) {}

VulkanSurface::~VulkanSurface() {
    if (impl_) destroy_swapchain_attachment(*impl_, true);
}

Result VulkanSurface::create(
    VulkanContext& context,
    const fission_skia_native_window_t& window,
    uint32_t width,
    uint32_t height,
    std::unique_ptr<VulkanSurface>* out_surface) {
    if (out_surface == nullptr || !valid_native_window(window) ||
        window.kind != context.window_kind() || width > INT_MAX || height > INT_MAX) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh surface output, window, or dimensions are invalid");
    }
    out_surface->reset();
    auto surface = std::unique_ptr<VulkanSurface>(
        new (std::nothrow) VulkanSurface(context));
    if (!surface || !surface->impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the Ganesh surface state could not be allocated");
    }
    surface->impl_->window = window;
    surface->impl_->width = width;
    surface->impl_->height = height;
    Result status = create_swapchain_attachment(*surface->impl_);
    if (!status.ok()) return status;
    *out_surface = std::move(surface);
    return Result::success();
}

Result VulkanSurface::resize(
    const fission_skia_native_window_t& window,
    uint32_t width,
    uint32_t height) {
    if (!impl_ || !valid_native_window(window) ||
        window.kind != impl_->context->window_kind() ||
        width > INT_MAX || height > INT_MAX) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh resize window or dimensions are invalid");
    }
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "a Ganesh surface cannot resize while a frame is active");
    }
    destroy_swapchain_attachment(*impl_, true);
    impl_->window = window;
    impl_->width = width;
    impl_->height = height;
    return create_swapchain_attachment(*impl_);
}

Result VulkanSurface::suspend() {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh surface is not initialized");
    }
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "a Ganesh surface cannot suspend while a frame is active");
    }
    destroy_swapchain_attachment(*impl_, true);
    impl_->width = 0;
    impl_->height = 0;
    impl_->state = SurfaceState::kSuspended;
    return Result::success();
}

Result VulkanSurface::resume(
    const fission_skia_native_window_t& window,
    uint32_t width,
    uint32_t height) {
    if (width == 0 || height == 0) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "a resumed Ganesh surface requires a non-zero extent");
    }
    return resize(window, width, height);
}

VulkanSurface::Frame VulkanSurface::begin_frame() {
    if (!impl_ || impl_->state != SurfaceState::kIdle ||
        impl_->swapchain == VK_NULL_HANDLE) {
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_INVALID_STATE,
                "the Ganesh surface is not ready to begin a frame"),
            nullptr,
        };
    }
    auto& context = impl_->context->internal_state();
    if (context.device_lost.load(std::memory_order_acquire)) {
        impl_->state = SurfaceState::kLost;
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_DEVICE_LOST,
                "the Vulkan device is lost"),
            nullptr,
        };
    }

    AcquireSemaphore* acquired = nullptr;
    Result status = acquire_semaphore(*impl_, &acquired);
    if (!status.ok()) return Frame{status, nullptr};
    uint32_t image_index = kNoImage;
    VkResult result = context.device_api.acquire_next_image(
        context.device,
        impl_->swapchain,
        UINT64_MAX,
        acquired->handle,
        VK_NULL_HANDLE,
        &image_index);
    if (result == VK_ERROR_OUT_OF_DATE_KHR) {
        acquired->in_flight.store(false, std::memory_order_release);
        status = rebuild_attachment(*impl_);
        if (!status.ok()) return Frame{status, nullptr};
        status = acquire_semaphore(*impl_, &acquired);
        if (!status.ok()) return Frame{status, nullptr};
        result = context.device_api.acquire_next_image(
            context.device,
            impl_->swapchain,
            UINT64_MAX,
            acquired->handle,
            VK_NULL_HANDLE,
            &image_index);
    }
    if (result != VK_SUCCESS && result != VK_SUBOPTIMAL_KHR) {
        acquired->in_flight.store(false, std::memory_order_release);
        status = classify_vulkan_result(context, result);
        if (status.status == FISSION_SKIA_STATUS_SURFACE_LOST ||
            status.status == FISSION_SKIA_STATUS_DEVICE_LOST) {
            destroy_swapchain_attachment(
                *impl_, status.status != FISSION_SKIA_STATUS_DEVICE_LOST);
            impl_->state = SurfaceState::kLost;
        }
        return Frame{status, nullptr};
    }
    if (image_index >= impl_->images.size()) {
        acquired->in_flight.store(false, std::memory_order_release);
        destroy_swapchain_attachment(*impl_, true);
        impl_->state = SurfaceState::kLost;
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_INTERNAL,
                "Vulkan acquired an out-of-range swapchain image"),
            nullptr,
        };
    }
    impl_->recreate_after_present = result == VK_SUBOPTIMAL_KHR;
    auto wait = GrBackendSemaphores::MakeVk(acquired->handle);
    if (!context.ganesh->wait(1, &wait, false)) {
        destroy_swapchain_attachment(*impl_, true);
        impl_->state = SurfaceState::kLost;
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_INTERNAL,
                "Ganesh could not wait for the acquired swapchain image"),
            nullptr,
        };
    }
    impl_->active_image = image_index;
    impl_->active_acquire = acquired;
    impl_->state = SurfaceState::kRecording;
    SkCanvas* canvas = impl_->images[image_index].surface->getCanvas();
    if (canvas == nullptr) {
        const Result cancelled = cancel_frame();
        return Frame{
            cancelled.ok()
                ? Result::failure(
                    FISSION_SKIA_STATUS_SURFACE_LOST,
                    "the Ganesh swapchain surface has no canvas")
                : cancelled,
            nullptr,
        };
    }
    return Frame{Result::success(), canvas};
}

Result VulkanSurface::finish_frame() {
    if (!impl_ || impl_->state != SurfaceState::kRecording) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh surface has no recording frame to finish");
    }
    impl_->state = SurfaceState::kReadyToPresent;
    return Result::success();
}

Result VulkanSurface::cancel_frame() {
    if (!impl_ || (impl_->state != SurfaceState::kRecording &&
        impl_->state != SurfaceState::kReadyToPresent)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh surface has no active frame to cancel");
    }
    Result status = ganesh_submit_active(*impl_, false, true);
    clear_active_frame(*impl_);
    const Result rebuilt = rebuild_attachment(*impl_);
    if (!status.ok()) return status;
    return rebuilt;
}

Result VulkanSurface::read_pixels_rgba8888(
    int32_t x,
    int32_t y,
    uint32_t width,
    uint32_t height,
    uint8_t* destination,
    size_t destination_length,
    size_t destination_row_bytes) {
    if (!impl_ || impl_->state != SurfaceState::kReadyToPresent ||
        impl_->active_image >= impl_->images.size()) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "Ganesh readback is only valid after frame execution and before present");
    }
    if (!impl_->readback_supported) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "the Vulkan swapchain does not support transfer-source readback");
    }
    if (x < 0 || y < 0 || width == 0 || height == 0 || destination == nullptr ||
        width > static_cast<uint32_t>(INT_MAX) ||
        height > static_cast<uint32_t>(INT_MAX) ||
        width > std::numeric_limits<size_t>::max() / 4) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh readback request is invalid");
    }
    const size_t minimum_row_bytes = static_cast<size_t>(width) * 4;
    if (destination_row_bytes < minimum_row_bytes ||
        height > std::numeric_limits<size_t>::max() / destination_row_bytes ||
        destination_length < destination_row_bytes * height ||
        static_cast<uint64_t>(x) + width > impl_->width ||
        static_cast<uint64_t>(y) + height > impl_->height) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh readback bounds or destination stride are invalid");
    }
    auto srgb = SkColorSpace::MakeSRGB();
    const SkImageInfo info = SkImageInfo::Make(
        static_cast<int>(width),
        static_cast<int>(height),
        kRGBA_8888_SkColorType,
        kPremul_SkAlphaType,
        srgb);
    if (!impl_->images[impl_->active_image].surface->readPixels(
            info, destination, destination_row_bytes, x, y)) {
        auto& context = impl_->context->internal_state();
        if (context.ganesh->isDeviceLost()) {
            context.device_lost.store(true, std::memory_order_release);
            return Result::failure(
                FISSION_SKIA_STATUS_DEVICE_LOST,
                "the Vulkan device was lost during readback");
        }
        if (context.ganesh->oomed()) {
            return Result::failure(
                FISSION_SKIA_STATUS_OUT_OF_MEMORY,
                "Ganesh exhausted memory during readback");
        }
        return Result::failure(
            FISSION_SKIA_STATUS_SURFACE_LOST,
            "Ganesh could not read the swapchain surface");
    }
    return Result::success();
}

Result VulkanSurface::present() {
    if (!impl_ || impl_->state != SurfaceState::kReadyToPresent ||
        impl_->active_image >= impl_->images.size()) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh surface has no completed frame to present");
    }
    auto& context = impl_->context->internal_state();
    const uint32_t image_index = impl_->active_image;
    Result status = ganesh_submit_active(*impl_, true, false);
    if (!status.ok()) {
        clear_active_frame(*impl_);
        destroy_swapchain_attachment(
            *impl_, status.status != FISSION_SKIA_STATUS_DEVICE_LOST);
        impl_->state = SurfaceState::kLost;
        return status;
    }

    VkSemaphore wait_semaphore = impl_->images[image_index].rendering_complete;
    VkPresentInfoKHR present_info{};
    present_info.sType = VK_STRUCTURE_TYPE_PRESENT_INFO_KHR;
    present_info.waitSemaphoreCount = 1;
    present_info.pWaitSemaphores = &wait_semaphore;
    present_info.swapchainCount = 1;
    present_info.pSwapchains = &impl_->swapchain;
    present_info.pImageIndices = &image_index;
    const VkResult result = context.device_api.queue_present(
        impl_->present_queue, &present_info);
    clear_active_frame(*impl_);
    impl_->state = SurfaceState::kIdle;

    if (result == VK_SUCCESS && !impl_->recreate_after_present) {
        return Result::success();
    }
    if (result == VK_SUBOPTIMAL_KHR ||
        (result == VK_SUCCESS && impl_->recreate_after_present)) {
        status = rebuild_attachment(*impl_);
        return status.ok() ? Result::success() : status;
    }
    if (result == VK_ERROR_OUT_OF_DATE_KHR) {
        status = rebuild_attachment(*impl_);
        if (!status.ok() && status.status == FISSION_SKIA_STATUS_DEVICE_LOST) {
            return status;
        }
        return Result::failure(
            FISSION_SKIA_STATUS_SURFACE_LOST,
            status.ok()
                ? "the out-of-date swapchain was recreated; the frame must be redrawn"
                : "the out-of-date swapchain could not be recreated");
    }
    if (result == VK_ERROR_SURFACE_LOST_KHR) {
        destroy_swapchain_attachment(*impl_, true);
        impl_->state = SurfaceState::kLost;
        return Result::failure(
            FISSION_SKIA_STATUS_SURFACE_LOST,
            "the Vulkan native surface was lost");
    }
    status = classify_vulkan_result(context, result);
    destroy_swapchain_attachment(
        *impl_, status.status != FISSION_SKIA_STATUS_DEVICE_LOST);
    impl_->state = SurfaceState::kLost;
    return status;
}

uint32_t VulkanSurface::width() const {
    return impl_ ? impl_->width : 0;
}

uint32_t VulkanSurface::height() const {
    return impl_ ? impl_->height : 0;
}

bool VulkanSurface::is_zero_sized() const {
    return !impl_ || impl_->width == 0 || impl_->height == 0;
}

bool VulkanSurface::supports_readback() const {
    return impl_ && impl_->readback_supported;
}

}  // namespace fission::skia::ganesh
