#include "fission_skia_ganesh_android_vulkan_internal.h"

#if !defined(FISSION_SKIA_ENABLE_GANESH_ANDROID_VULKAN)
#error "Compile this source only for the Android native-Ganesh profile"
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

namespace fission::skia::ganesh::android_vulkan {
namespace {

constexpr uint32_t kMaximumSwapchainImages = 64u;
constexpr uint32_t kMaximumSurfaceEntries = 256u;

template <typename Function>
Function instance_proc(VkInstance instance, const char* name) {
    return reinterpret_cast<Function>(vkGetInstanceProcAddr(instance, name));
}

template <typename Value, typename Enumerate>
Result enumerate_surface_values(
    AndroidVulkanContext::Impl& context,
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
            "the Android Vulkan surface returned no usable bounded configuration list");
    }
    out_values->resize(count);
    result = enumerate(
        context.physical_device, surface, &count, out_values->data());
    if (result != VK_SUCCESS) return classify_vulkan_result(context, result);
    out_values->resize(count);
    return Result::success();
}

bool is_loss_status(fission_skia_status_t status) {
    return status == FISSION_SKIA_STATUS_SURFACE_LOST ||
           status == FISSION_SKIA_STATUS_CONTEXT_LOST ||
           status == FISSION_SKIA_STATUS_DEVICE_LOST;
}

Result fail_attachment(AndroidVulkanSurface::Impl& surface, Result result) {
    destroy_swapchain_attachment(surface, false);
    surface.state = is_loss_status(result.status)
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
            VK_FORMAT_R8G8B8A8_UNORM,
            VK_COLOR_SPACE_SRGB_NONLINEAR_KHR,
        };
        return Result::success();
    }

    constexpr std::array<VkFormat, 4> preferred = {
        VK_FORMAT_R8G8B8A8_SRGB,
        VK_FORMAT_B8G8R8A8_SRGB,
        VK_FORMAT_R8G8B8A8_UNORM,
        VK_FORMAT_B8G8R8A8_UNORM,
    };
    for (VkFormat wanted : preferred) {
        for (const auto& format : formats) {
            if (format.format == wanted &&
                format.colorSpace == VK_COLOR_SPACE_SRGB_NONLINEAR_KHR) {
                *out_format = format;
                return Result::success();
            }
        }
    }
    return Result::failure(
        FISSION_SKIA_STATUS_UNSUPPORTED,
        "the Android Vulkan surface has no supported RGBA8 or BGRA8 sRGB presentation format");
}

Result choose_present_mode(
    const std::vector<VkPresentModeKHR>& modes,
    VkPresentModeKHR* out_mode) {
    if (std::find(modes.begin(), modes.end(), VK_PRESENT_MODE_FIFO_KHR) ==
        modes.end()) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "the Android Vulkan surface does not expose FIFO presentation");
    }
    // FIFO is universally available on conformant Android Vulkan devices and
    // integrates with the platform compositor without unbounded frame churn.
    *out_mode = VK_PRESENT_MODE_FIFO_KHR;
    return Result::success();
}

VkCompositeAlphaFlagBitsKHR choose_composite_alpha(
    VkCompositeAlphaFlagsKHR supported) {
    constexpr std::array<VkCompositeAlphaFlagBitsKHR, 4> preferred = {
        VK_COMPOSITE_ALPHA_INHERIT_BIT_KHR,
        VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
        VK_COMPOSITE_ALPHA_PRE_MULTIPLIED_BIT_KHR,
        VK_COMPOSITE_ALPHA_POST_MULTIPLIED_BIT_KHR,
    };
    for (auto value : preferred) {
        if ((supported & value) != 0) return value;
    }
    return static_cast<VkCompositeAlphaFlagBitsKHR>(0);
}

SkColorType sk_color_type(VkFormat format) {
    switch (format) {
        case VK_FORMAT_R8G8B8A8_SRGB:
        case VK_FORMAT_R8G8B8A8_UNORM:
            return kRGBA_8888_SkColorType;
        case VK_FORMAT_B8G8R8A8_SRGB:
        case VK_FORMAT_B8G8R8A8_UNORM:
            return kBGRA_8888_SkColorType;
        default:
            return kUnknown_SkColorType;
    }
}

Result current_context_health(AndroidVulkanContext::Impl& context) {
    if (context.device_lost.load(std::memory_order_acquire)) {
        return Result::failure(
            FISSION_SKIA_STATUS_DEVICE_LOST,
            "the Android Vulkan device is lost");
    }
    if (context.device == VK_NULL_HANDLE ||
        context.graphics_queue == VK_NULL_HANDLE || !context.ganesh) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Android Ganesh Vulkan context is not initialized");
    }
    if (context.ganesh->isDeviceLost()) {
        context.device_lost.store(true, std::memory_order_release);
        return Result::failure(
            FISSION_SKIA_STATUS_DEVICE_LOST,
            "the Android Vulkan device is lost");
    }
    if (context.ganesh->abandoned()) {
        return Result::failure(
            FISSION_SKIA_STATUS_CONTEXT_LOST,
            "the Android Ganesh Vulkan context was abandoned");
    }
    return Result::success();
}

Result create_semaphore(
    AndroidVulkanContext::Impl& context,
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
    AndroidVulkanSurface::Impl& surface,
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
                    "the Android Vulkan device was lost while draining frame synchronization");
            }
            return Result::failure(
                FISSION_SKIA_STATUS_INTERNAL,
                "Ganesh could not drain Android frame synchronization work");
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
            "completed Ganesh work did not release an Android acquisition semaphore");
    }

    auto candidate = std::unique_ptr<AcquireSemaphore>(
        new (std::nothrow) AcquireSemaphore{});
    if (!candidate) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Android frame synchronization state could not be allocated");
    }
    Result status = create_semaphore(context, &candidate->handle);
    if (!status.ok()) return status;
    candidate->in_flight.store(true, std::memory_order_release);
    *out_semaphore = candidate.get();
    surface.acquire_semaphores.push_back(std::move(candidate));
    return Result::success();
}

Result ganesh_submit_active(
    AndroidVulkanSurface::Impl& surface,
    bool for_present,
    bool sync_cpu) {
    auto& context = surface.context->internal_state();
    Result health = current_context_health(context);
    if (!health.ok()) return health;
    if (surface.active_image >= surface.images.size() ||
        surface.active_acquire == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Android surface has no acquired swapchain image");
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
            "Ganesh could not submit work to the Android Vulkan device");
    }
    if (context.ganesh->oomed()) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Ganesh exhausted Android Vulkan memory while submitting the frame");
    }
    if (!submitted) {
        return Result::failure(
            FISSION_SKIA_STATUS_INTERNAL,
            "Ganesh rejected the Android Vulkan submission");
    }
    if (for_present && semaphore_result != GrSemaphoresSubmitted::kYes) {
        return Result::failure(
            FISSION_SKIA_STATUS_INTERNAL,
            "Ganesh did not submit the required Android presentation semaphore");
    }
    if (sync_cpu) context.ganesh->checkAsyncWorkCompletion();
    return Result::success();
}

void clear_active_frame(AndroidVulkanSurface::Impl& surface) {
    surface.active_image = kNoImage;
    surface.active_acquire = nullptr;
}

Result rebuild_attachment(AndroidVulkanSurface::Impl& surface) {
    // `surface.window` is borrowed metadata. Preserve a real native-window
    // reference across teardown so destroy_swapchain_attachment cannot release
    // the final reference before the replacement attachment acquires its own.
    ANativeWindow* replacement_window = surface.window.native_window;
    if (replacement_window != nullptr) {
        ANativeWindow_acquire(replacement_window);
    }
    destroy_swapchain_attachment(surface, true);
    if (surface.width == 0 || surface.height == 0) {
        if (replacement_window != nullptr) {
            ANativeWindow_release(replacement_window);
        }
        surface.window = AndroidWindow{};
        surface.state = SurfaceState::kSuspended;
        return Result::success();
    }
    surface.window.native_window = replacement_window;
    Result status = create_swapchain_attachment(surface);
    if (replacement_window != nullptr) {
        ANativeWindow_release(replacement_window);
    }
    if (!status.ok()) surface.state = SurfaceState::kLost;
    return status;
}

}  // namespace

Result create_android_surface(
    AndroidVulkanContext::Impl& context,
    ANativeWindow* window,
    VkSurfaceKHR* out_surface) {
    if (window == nullptr || out_surface == nullptr ||
        context.instance == VK_NULL_HANDLE) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Android native window or Vulkan surface output is invalid");
    }
    *out_surface = VK_NULL_HANDLE;
    auto create = instance_proc<PFN_vkCreateAndroidSurfaceKHR>(
        context.instance, "vkCreateAndroidSurfaceKHR");
    if (create == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "the Android Vulkan loader is missing vkCreateAndroidSurfaceKHR");
    }
    VkAndroidSurfaceCreateInfoKHR info{};
    info.sType = VK_STRUCTURE_TYPE_ANDROID_SURFACE_CREATE_INFO_KHR;
    info.window = window;
    const VkResult result = create(
        context.instance, &info, nullptr, out_surface);
    return classify_vulkan_result(context, result);
}

Result create_swapchain_attachment(AndroidVulkanSurface::Impl& surface) {
    auto& context = surface.context->internal_state();
    Result health = current_context_health(context);
    if (!health.ok()) {
        surface.state = SurfaceState::kLost;
        return health;
    }
    if (surface.width == 0 || surface.height == 0) {
        surface.window = AndroidWindow{};
        surface.state = SurfaceState::kSuspended;
        return Result::success();
    }
    if (!valid_android_window(surface.window) ||
        surface.width > static_cast<uint32_t>(INT_MAX) ||
        surface.height > static_cast<uint32_t>(INT_MAX) ||
        surface.retained_window != nullptr ||
        surface.native_surface != VK_NULL_HANDLE ||
        surface.swapchain != VK_NULL_HANDLE) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Android Ganesh attachment state or dimensions are invalid");
    }

    surface.retained_window = surface.window.native_window;
    ANativeWindow_acquire(surface.retained_window);
    Result status = create_android_surface(
        context, surface.retained_window, &surface.native_surface);
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
                "the Android Vulkan device has no usable queue for this fresh ANativeWindow"));
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
    VkPresentModeKHR selected_present_mode = VK_PRESENT_MODE_FIFO_KHR;
    status = choose_present_mode(present_modes, &selected_present_mode);
    if (!status.ok()) return fail_attachment(surface, status);
    if ((capabilities.supportedUsageFlags &
         VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT) == 0) {
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "the Android Vulkan surface images cannot be color attachments"));
    }
    if ((capabilities.supportedTransforms &
         VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR) == 0) {
        // The shared playback seam currently supplies logical frame bounds to
        // filters and readback. Pre-rotating only the canvas would leave those
        // device-space operations inconsistent. Let SurfaceFlinger perform the
        // transform when identity is supported; fail explicitly on the rare
        // surface that mandates application-side rotation or mirroring.
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "the Android Vulkan surface requires an application-side transform that Fission cannot yet represent safely"));
    }

    VkExtent2D extent{surface.width, surface.height};
    if (capabilities.currentExtent.width != UINT32_MAX) {
        extent = capabilities.currentExtent;
    }
    if (extent.width == 0 || extent.height == 0 ||
        extent.width > static_cast<uint32_t>(INT_MAX) ||
        extent.height > static_cast<uint32_t>(INT_MAX) ||
        extent.width != surface.width || extent.height != surface.height ||
        extent.width < capabilities.minImageExtent.width ||
        extent.height < capabilities.minImageExtent.height ||
        extent.width > capabilities.maxImageExtent.width ||
        extent.height > capabilities.maxImageExtent.height) {
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_SURFACE_LOST,
                "the Android host size does not match the current Vulkan surface extent"));
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
                "the Android Vulkan surface requires an unsupported swapchain image count"));
    }
    const VkCompositeAlphaFlagBitsKHR composite_alpha =
        choose_composite_alpha(capabilities.supportedCompositeAlpha);
    if (composite_alpha == 0) {
        return fail_attachment(
            surface,
            Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "the Android Vulkan surface exposes no composite-alpha mode"));
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
    surface.sharing_mode =
        surface.present_queue_family == context.graphics_queue_family
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
    swapchain_info.preTransform = VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR;
    swapchain_info.compositeAlpha = composite_alpha;
    swapchain_info.presentMode = selected_present_mode;
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
                "the Android Vulkan swapchain returned an unsupported image count"));
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
                    "Skia rejected an Android Vulkan swapchain render target"));
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
                    context.ganesh->oomed()
                        ? FISSION_SKIA_STATUS_OUT_OF_MEMORY
                        : FISSION_SKIA_STATUS_UNSUPPORTED,
                    context.ganesh->oomed()
                        ? "Ganesh exhausted memory wrapping an Android swapchain image"
                        : "Skia could not wrap an Android Vulkan swapchain image"));
        }
    }

    surface.state = SurfaceState::kIdle;
    surface.recreate_after_present = false;
    return Result::success();
}

void destroy_swapchain_attachment(
    AndroidVulkanSurface::Impl& surface,
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
    const bool preserve_callback_storage =
        context.device_lost.load(std::memory_order_acquire);
    for (auto& semaphore : surface.acquire_semaphores) {
        if (semaphore->handle != VK_NULL_HANDLE &&
            context.device_api.destroy_semaphore != nullptr) {
            context.device_api.destroy_semaphore(
                context.device, semaphore->handle, nullptr);
            semaphore->handle = VK_NULL_HANDLE;
        }
        if (preserve_callback_storage) {
            semaphore->retired_next =
                std::move(context.retired_acquire_semaphores);
            context.retired_acquire_semaphores = std::move(semaphore);
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
    if (surface.retained_window != nullptr) {
        ANativeWindow_release(surface.retained_window);
        surface.retained_window = nullptr;
    }
    surface.extent = {};
    surface.format = VK_FORMAT_UNDEFINED;
    surface.color_space = VK_COLOR_SPACE_SRGB_NONLINEAR_KHR;
    surface.usage = 0;
    surface.sharing_mode = VK_SHARING_MODE_EXCLUSIVE;
    surface.present_queue = VK_NULL_HANDLE;
    surface.present_queue_family = 0;
    surface.readback_supported = false;
    surface.recreate_after_present = false;
    clear_active_frame(surface);
}

AndroidVulkanSurface::AndroidVulkanSurface(AndroidVulkanContext& context)
    : impl_(new (std::nothrow) Impl(context)) {}

AndroidVulkanSurface::~AndroidVulkanSurface() {
    if (impl_) {
        destroy_swapchain_attachment(
            *impl_, !impl_->context->is_device_lost());
    }
}

Result AndroidVulkanSurface::create(
    AndroidVulkanContext& context,
    const AndroidWindow& window,
    uint32_t width,
    uint32_t height,
    std::unique_ptr<AndroidVulkanSurface>* out_surface) {
    if (out_surface == nullptr || !valid_android_window(window) ||
        width > static_cast<uint32_t>(INT_MAX) ||
        height > static_cast<uint32_t>(INT_MAX)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Android Ganesh surface output, window, or dimensions are invalid");
    }
    if (context.is_device_lost()) {
        return Result::failure(
            FISSION_SKIA_STATUS_DEVICE_LOST,
            "the Android Ganesh Vulkan device is lost");
    }
    out_surface->reset();
    auto surface = std::unique_ptr<AndroidVulkanSurface>(
        new (std::nothrow) AndroidVulkanSurface(context));
    if (!surface || !surface->impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the Android Ganesh surface state could not be allocated");
    }
    surface->impl_->window = window;
    surface->impl_->width = width;
    surface->impl_->height = height;
    Result status = create_swapchain_attachment(*surface->impl_);
    if (!status.ok()) return status;
    *out_surface = std::move(surface);
    return Result::success();
}

Result AndroidVulkanSurface::resize(
    const AndroidWindow& window,
    uint32_t width,
    uint32_t height) {
    if (!impl_ || !valid_android_window(window) ||
        width > static_cast<uint32_t>(INT_MAX) ||
        height > static_cast<uint32_t>(INT_MAX)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Android Ganesh resize window or dimensions are invalid");
    }
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "an Android Ganesh surface cannot resize while a frame is active");
    }

    destroy_swapchain_attachment(*impl_, true);
    impl_->window = window;
    impl_->width = width;
    impl_->height = height;
    Result status = create_swapchain_attachment(*impl_);
    if (!status.ok()) impl_->state = SurfaceState::kLost;
    return status;
}

Result AndroidVulkanSurface::suspend() {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Android Ganesh surface is not initialized");
    }
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "an Android Ganesh surface cannot suspend while a frame is active");
    }
    destroy_swapchain_attachment(*impl_, true);
    impl_->window = AndroidWindow{};
    impl_->width = 0;
    impl_->height = 0;
    impl_->state = impl_->context->is_device_lost()
        ? SurfaceState::kLost
        : SurfaceState::kSuspended;
    return impl_->state == SurfaceState::kLost
        ? Result::failure(
              FISSION_SKIA_STATUS_DEVICE_LOST,
              "the Android Vulkan device was lost while suspending")
        : Result::success();
}

Result AndroidVulkanSurface::resume(
    const AndroidWindow& window,
    uint32_t width,
    uint32_t height) {
    if (width == 0 || height == 0) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "a resumed Android Ganesh surface requires a non-zero extent");
    }
    return resize(window, width, height);
}

AndroidVulkanSurface::Frame AndroidVulkanSurface::begin_frame() {
    if (!impl_ || impl_->state != SurfaceState::kIdle ||
        impl_->swapchain == VK_NULL_HANDLE || impl_->retained_window == nullptr) {
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_INVALID_STATE,
                "the Android Ganesh surface is not ready to begin a frame"),
            nullptr,
        };
    }
    auto& context = impl_->context->internal_state();
    Result health = current_context_health(context);
    if (!health.ok()) {
        impl_->state = SurfaceState::kLost;
        return Frame{health, nullptr};
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
        if (is_loss_status(status.status)) {
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
                "Android Vulkan acquired an out-of-range swapchain image"),
            nullptr,
        };
    }

    impl_->recreate_after_present = result == VK_SUBOPTIMAL_KHR;
    auto wait = GrBackendSemaphores::MakeVk(acquired->handle);
    if (!context.ganesh->wait(1, &wait, false)) {
        acquired->in_flight.store(false, std::memory_order_release);
        const bool device_lost = context.ganesh->isDeviceLost();
        if (device_lost) {
            context.device_lost.store(true, std::memory_order_release);
        }
        destroy_swapchain_attachment(*impl_, !device_lost);
        impl_->state = SurfaceState::kLost;
        return Frame{
            Result::failure(
                device_lost
                    ? FISSION_SKIA_STATUS_DEVICE_LOST
                    : FISSION_SKIA_STATUS_INTERNAL,
                device_lost
                    ? "the Android Vulkan device was lost while waiting for a swapchain image"
                    : "Ganesh could not wait for the acquired Android swapchain image"),
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
                      "the Android Ganesh swapchain surface has no canvas")
                : cancelled,
            nullptr,
        };
    }
    return Frame{Result::success(), canvas};
}

Result AndroidVulkanSurface::finish_frame() {
    if (!impl_ || impl_->state != SurfaceState::kRecording) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Android Ganesh surface has no recording frame to finish");
    }
    impl_->state = SurfaceState::kReadyToPresent;
    return Result::success();
}

Result AndroidVulkanSurface::cancel_frame() {
    if (!impl_ || (impl_->state != SurfaceState::kRecording &&
        impl_->state != SurfaceState::kReadyToPresent)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Android Ganesh surface has no active frame to cancel");
    }
    Result status = ganesh_submit_active(*impl_, false, true);
    clear_active_frame(*impl_);
    Result rebuilt = rebuild_attachment(*impl_);
    if (!status.ok()) return status;
    return rebuilt;
}

Result AndroidVulkanSurface::read_pixels_rgba8888(
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
            "Android Ganesh readback is only valid after execution and before present");
    }
    if (!impl_->readback_supported) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "the Android Vulkan swapchain does not support transfer-source readback");
    }
    if (x < 0 || y < 0 || width == 0 || height == 0 ||
        destination == nullptr || width > static_cast<uint32_t>(INT_MAX) ||
        height > static_cast<uint32_t>(INT_MAX) ||
        width > std::numeric_limits<size_t>::max() / 4) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Android Ganesh readback request is invalid");
    }
    const size_t minimum_row_bytes = static_cast<size_t>(width) * 4;
    if (destination_row_bytes < minimum_row_bytes ||
        height > std::numeric_limits<size_t>::max() / destination_row_bytes ||
        destination_length < destination_row_bytes * height ||
        static_cast<uint64_t>(x) + width > impl_->width ||
        static_cast<uint64_t>(y) + height > impl_->height) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Android Ganesh readback bounds or destination stride are invalid");
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
        if (context.ganesh->isDeviceLost()) {
            context.device_lost.store(true, std::memory_order_release);
            return Result::failure(
                FISSION_SKIA_STATUS_DEVICE_LOST,
                "the Android Vulkan device was lost during readback");
        }
        if (context.ganesh->oomed()) {
            return Result::failure(
                FISSION_SKIA_STATUS_OUT_OF_MEMORY,
                "Ganesh exhausted Android Vulkan memory during readback");
        }
        return Result::failure(
            FISSION_SKIA_STATUS_SURFACE_LOST,
            "Ganesh could not read the Android Vulkan swapchain surface");
    }
    return current_context_health(context);
}

Result AndroidVulkanSurface::present() {
    if (!impl_ || impl_->state != SurfaceState::kReadyToPresent ||
        impl_->active_image >= impl_->images.size()) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Android Ganesh surface has no completed frame to present");
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
                ? "the Android swapchain was recreated; the frame must be redrawn"
                : "the out-of-date Android swapchain could not be recreated");
    }
    if (result == VK_ERROR_SURFACE_LOST_KHR) {
        destroy_swapchain_attachment(*impl_, true);
        impl_->state = SurfaceState::kLost;
        return Result::failure(
            FISSION_SKIA_STATUS_SURFACE_LOST,
            "the Android native Vulkan surface was lost");
    }
    status = classify_vulkan_result(context, result);
    destroy_swapchain_attachment(
        *impl_, status.status != FISSION_SKIA_STATUS_DEVICE_LOST);
    impl_->state = SurfaceState::kLost;
    return status;
}

uint32_t AndroidVulkanSurface::width() const {
    return impl_ ? impl_->width : 0;
}

uint32_t AndroidVulkanSurface::height() const {
    return impl_ ? impl_->height : 0;
}

bool AndroidVulkanSurface::is_zero_sized() const {
    return !impl_ || impl_->width == 0 || impl_->height == 0;
}

bool AndroidVulkanSurface::supports_readback() const {
    return impl_ && impl_->readback_supported;
}

}  // namespace fission::skia::ganesh::android_vulkan
