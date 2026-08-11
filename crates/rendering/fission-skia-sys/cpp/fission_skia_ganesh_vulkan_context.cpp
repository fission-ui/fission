#include "fission_skia_ganesh_vulkan_internal.h"

#if !defined(__linux__)
#error "The first Fission Ganesh Vulkan runtime is Linux-only"
#endif

#if !defined(FISSION_SKIA_ENABLE_GANESH_VULKAN)
#error "Compile this source only for the native-ganesh profile"
#endif

#include "include/gpu/ganesh/vk/GrVkDirectContext.h"
#include "include/gpu/vk/VulkanBackendContext.h"
#include "src/gpu/GpuTypesPriv.h"
#include "src/gpu/vk/vulkanmemoryallocator/VulkanMemoryAllocatorPriv.h"

#include <chrono>
#include <cstring>
#include <new>
#include <string>
#include <utility>
#include <vector>

namespace fission::skia::ganesh {
namespace {

constexpr uint32_t kMaximumVulkanEnumerationCount = 16384u;
constexpr const char* kSurfaceExtension = "VK_KHR_surface";
constexpr const char* kSwapchainExtension = "VK_KHR_swapchain";

template <typename Function>
Function global_proc(const char* name) {
    return reinterpret_cast<Function>(
        vkGetInstanceProcAddr(VK_NULL_HANDLE, name));
}

template <typename Function>
Function instance_proc(VkInstance instance, const char* name) {
    return reinterpret_cast<Function>(vkGetInstanceProcAddr(instance, name));
}

template <typename Function>
Function device_proc(VkDevice device, const char* name) {
    return reinterpret_cast<Function>(vkGetDeviceProcAddr(device, name));
}

bool has_extension(
    const VkExtensionProperties* properties,
    uint32_t count,
    const char* name) {
    for (uint32_t index = 0; index < count; ++index) {
        if (std::strcmp(properties[index].extensionName, name) == 0) {
            return true;
        }
    }
    return false;
}

Result check_instance_extensions(
    PFN_vkEnumerateInstanceExtensionProperties enumerate_extensions,
    const char* wsi_extension) {
    uint32_t count = 0;
    VkResult result = enumerate_extensions(nullptr, &count, nullptr);
    if (result != VK_SUCCESS || count == 0 ||
        count > kMaximumVulkanEnumerationCount) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "Vulkan instance extensions could not be enumerated safely");
    }
    auto properties = std::unique_ptr<VkExtensionProperties[]>(
        new (std::nothrow) VkExtensionProperties[count]);
    if (!properties) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Vulkan instance extension storage could not be allocated");
    }
    result = enumerate_extensions(nullptr, &count, properties.get());
    if (result != VK_SUCCESS) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "Vulkan instance extension enumeration failed");
    }
    if (!has_extension(properties.get(), count, kSurfaceExtension) ||
        !has_extension(properties.get(), count, wsi_extension)) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "Vulkan does not expose the required Linux surface extensions");
    }
    return Result::success();
}

bool supports_device_extension(
    const InstanceDispatch& api,
    VkPhysicalDevice physical_device,
    const char* extension) {
    uint32_t count = 0;
    VkResult result = api.enumerate_device_extensions(
        physical_device, nullptr, &count, nullptr);
    if (result != VK_SUCCESS || count == 0 ||
        count > kMaximumVulkanEnumerationCount) {
        return false;
    }
    auto properties = std::unique_ptr<VkExtensionProperties[]>(
        new (std::nothrow) VkExtensionProperties[count]);
    if (!properties) return false;
    result = api.enumerate_device_extensions(
        physical_device, nullptr, &count, properties.get());
    return result == VK_SUCCESS &&
           has_extension(properties.get(), count, extension);
}

bool surface_is_usable(
    const InstanceDispatch& api,
    VkPhysicalDevice physical_device,
    VkSurfaceKHR surface) {
    VkSurfaceCapabilitiesKHR capabilities{};
    if (api.get_surface_capabilities(
            physical_device, surface, &capabilities) != VK_SUCCESS ||
        (capabilities.supportedUsageFlags &
            VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT) == 0) {
        return false;
    }
    uint32_t format_count = 0;
    uint32_t present_mode_count = 0;
    if (api.get_surface_formats(
            physical_device, surface, &format_count, nullptr) != VK_SUCCESS ||
        api.get_present_modes(
            physical_device, surface, &present_mode_count, nullptr) != VK_SUCCESS ||
        format_count == 0 || format_count > kMaximumVulkanEnumerationCount ||
        present_mode_count == 0 ||
        present_mode_count > kMaximumVulkanEnumerationCount) {
        return false;
    }
    auto formats = std::unique_ptr<VkSurfaceFormatKHR[]>(
        new (std::nothrow) VkSurfaceFormatKHR[format_count]);
    auto modes = std::unique_ptr<VkPresentModeKHR[]>(
        new (std::nothrow) VkPresentModeKHR[present_mode_count]);
    if (!formats || !modes ||
        api.get_surface_formats(
            physical_device, surface, &format_count, formats.get()) != VK_SUCCESS ||
        api.get_present_modes(
            physical_device, surface, &present_mode_count, modes.get()) != VK_SUCCESS) {
        return false;
    }
    bool format_supported = false;
    for (uint32_t index = 0; index < format_count; ++index) {
        const bool any_format = formats[index].format == VK_FORMAT_UNDEFINED;
        if ((any_format || formats[index].format == VK_FORMAT_B8G8R8A8_SRGB) &&
            formats[index].colorSpace == VK_COLOR_SPACE_SRGB_NONLINEAR_KHR) {
            format_supported = true;
            break;
        }
    }
    bool fifo_supported = false;
    for (uint32_t index = 0; index < present_mode_count; ++index) {
        if (modes[index] == VK_PRESENT_MODE_FIFO_KHR) {
            fifo_supported = true;
            break;
        }
    }
    return format_supported && fifo_supported;
}

struct DeviceChoice {
    VkPhysicalDevice physical_device = VK_NULL_HANDLE;
    uint32_t graphics_queue_family = 0;
    uint32_t present_queue_family = 0;
    int score = -1;
};

int device_type_score(VkPhysicalDeviceType type) {
    switch (type) {
        case VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU:
            return 400;
        case VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU:
            return 300;
        case VK_PHYSICAL_DEVICE_TYPE_VIRTUAL_GPU:
            return 200;
        case VK_PHYSICAL_DEVICE_TYPE_CPU:
            return 100;
        default:
            return 0;
    }
}

bool choose_queue_families(
    const InstanceDispatch& api,
    VkPhysicalDevice physical_device,
    VkSurfaceKHR surface,
    uint32_t* out_graphics,
    uint32_t* out_present) {
    uint32_t count = 0;
    api.get_queue_family_properties(physical_device, &count, nullptr);
    if (count == 0 || count > kMaximumVulkanEnumerationCount) return false;
    auto properties = std::unique_ptr<VkQueueFamilyProperties[]>(
        new (std::nothrow) VkQueueFamilyProperties[count]);
    if (!properties) return false;
    api.get_queue_family_properties(physical_device, &count, properties.get());

    uint32_t first_graphics = count;
    uint32_t first_present = count;
    for (uint32_t index = 0; index < count; ++index) {
        const bool graphics = properties[index].queueCount != 0 &&
            (properties[index].queueFlags & VK_QUEUE_GRAPHICS_BIT) != 0;
        VkBool32 present = VK_FALSE;
        if (api.get_surface_support(
                physical_device, index, surface, &present) != VK_SUCCESS) {
            continue;
        }
        if (graphics && present == VK_TRUE) {
            *out_graphics = index;
            *out_present = index;
            return true;
        }
        if (graphics && first_graphics == count) first_graphics = index;
        if (present == VK_TRUE && properties[index].queueCount != 0 &&
            first_present == count) {
            first_present = index;
        }
    }
    if (first_graphics == count || first_present == count) return false;
    *out_graphics = first_graphics;
    *out_present = first_present;
    return true;
}

Result choose_physical_device(VulkanContext::Impl& context) {
    uint32_t count = 0;
    VkResult result = context.instance_api.enumerate_physical_devices(
        context.instance, &count, nullptr);
    if (result != VK_SUCCESS || count == 0 ||
        count > kMaximumVulkanEnumerationCount) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "no safely enumerable Vulkan physical device is available");
    }
    auto devices = std::unique_ptr<VkPhysicalDevice[]>(
        new (std::nothrow) VkPhysicalDevice[count]);
    if (!devices) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Vulkan physical-device storage could not be allocated");
    }
    result = context.instance_api.enumerate_physical_devices(
        context.instance, &count, devices.get());
    if (result != VK_SUCCESS) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "Vulkan physical-device enumeration failed");
    }

    DeviceChoice best;
    for (uint32_t index = 0; index < count; ++index) {
        VkPhysicalDeviceProperties properties{};
        context.instance_api.get_physical_device_properties(
            devices[index], &properties);
        if (properties.apiVersion < VK_API_VERSION_1_1 ||
            !supports_device_extension(
                context.instance_api, devices[index], kSwapchainExtension) ||
            !surface_is_usable(
                context.instance_api, devices[index], context.probe_surface)) {
            continue;
        }
        uint32_t graphics_family = 0;
        uint32_t present_family = 0;
        if (!choose_queue_families(
                context.instance_api,
                devices[index],
                context.probe_surface,
                &graphics_family,
                &present_family)) {
            continue;
        }
        int score = device_type_score(properties.deviceType);
        if (graphics_family == present_family) score += 1000;
        if (score > best.score) {
            best.physical_device = devices[index];
            best.graphics_queue_family = graphics_family;
            best.present_queue_family = present_family;
            best.score = score;
        }
    }
    if (best.physical_device == VK_NULL_HANDLE) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "no Vulkan 1.1 device supports graphics, swapchains, and this window");
    }
    context.physical_device = best.physical_device;
    context.graphics_queue_family = best.graphics_queue_family;
    context.initial_present_queue_family = best.present_queue_family;
    return Result::success();
}

void ganesh_device_lost(
    skgpu::VulkanDeviceLostContext raw_context,
    const std::string&,
    const std::vector<VkDeviceFaultAddressInfoEXT>&,
    const std::vector<VkDeviceFaultVendorInfoEXT>&,
    const std::vector<std::byte>&) {
    auto* context = static_cast<VulkanContext::Impl*>(raw_context);
    if (context != nullptr) {
        context->device_lost.store(true, std::memory_order_release);
    }
}

}  // namespace

Result classify_vulkan_result(VulkanContext::Impl& context, VkResult result) {
    switch (result) {
        case VK_SUCCESS:
            return Result::success();
        case VK_ERROR_OUT_OF_HOST_MEMORY:
        case VK_ERROR_OUT_OF_DEVICE_MEMORY:
            return Result::failure(
                FISSION_SKIA_STATUS_OUT_OF_MEMORY,
                "Vulkan could not allocate a required resource");
        case VK_ERROR_DEVICE_LOST:
            context.device_lost.store(true, std::memory_order_release);
            return Result::failure(
                FISSION_SKIA_STATUS_DEVICE_LOST,
                "the Vulkan device was lost");
        case VK_ERROR_SURFACE_LOST_KHR:
        case VK_ERROR_OUT_OF_DATE_KHR:
            return Result::failure(
                FISSION_SKIA_STATUS_SURFACE_LOST,
                "the Vulkan native surface must be recreated");
        case VK_ERROR_NATIVE_WINDOW_IN_USE_KHR:
            return Result::failure(
                FISSION_SKIA_STATUS_INVALID_STATE,
                "the native window is already owned by another Vulkan surface");
        case VK_ERROR_INCOMPATIBLE_DRIVER:
        case VK_ERROR_EXTENSION_NOT_PRESENT:
        case VK_ERROR_FEATURE_NOT_PRESENT:
        case VK_ERROR_FORMAT_NOT_SUPPORTED:
            return Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "the Vulkan driver does not support the requested capability");
        default:
            return Result::failure(
                FISSION_SKIA_STATUS_INTERNAL,
                "an unexpected Vulkan operation failed");
    }
}

Result load_instance_dispatch(VulkanContext::Impl& context) {
#define FISSION_LOAD_INSTANCE(field, type, name) \
    context.instance_api.field = instance_proc<type>(context.instance, name); \
    if (context.instance_api.field == nullptr) { \
        return Result::failure( \
            FISSION_SKIA_STATUS_UNSUPPORTED, \
            "the Vulkan loader is missing a required instance function"); \
    }
    FISSION_LOAD_INSTANCE(destroy_instance, PFN_vkDestroyInstance, "vkDestroyInstance")
    FISSION_LOAD_INSTANCE(
        enumerate_physical_devices,
        PFN_vkEnumeratePhysicalDevices,
        "vkEnumeratePhysicalDevices")
    FISSION_LOAD_INSTANCE(
        get_physical_device_properties,
        PFN_vkGetPhysicalDeviceProperties,
        "vkGetPhysicalDeviceProperties")
    FISSION_LOAD_INSTANCE(
        get_physical_device_features,
        PFN_vkGetPhysicalDeviceFeatures,
        "vkGetPhysicalDeviceFeatures")
    FISSION_LOAD_INSTANCE(
        get_queue_family_properties,
        PFN_vkGetPhysicalDeviceQueueFamilyProperties,
        "vkGetPhysicalDeviceQueueFamilyProperties")
    FISSION_LOAD_INSTANCE(
        get_surface_support,
        PFN_vkGetPhysicalDeviceSurfaceSupportKHR,
        "vkGetPhysicalDeviceSurfaceSupportKHR")
    FISSION_LOAD_INSTANCE(
        get_surface_capabilities,
        PFN_vkGetPhysicalDeviceSurfaceCapabilitiesKHR,
        "vkGetPhysicalDeviceSurfaceCapabilitiesKHR")
    FISSION_LOAD_INSTANCE(
        get_surface_formats,
        PFN_vkGetPhysicalDeviceSurfaceFormatsKHR,
        "vkGetPhysicalDeviceSurfaceFormatsKHR")
    FISSION_LOAD_INSTANCE(
        get_present_modes,
        PFN_vkGetPhysicalDeviceSurfacePresentModesKHR,
        "vkGetPhysicalDeviceSurfacePresentModesKHR")
    FISSION_LOAD_INSTANCE(
        enumerate_device_extensions,
        PFN_vkEnumerateDeviceExtensionProperties,
        "vkEnumerateDeviceExtensionProperties")
    FISSION_LOAD_INSTANCE(create_device, PFN_vkCreateDevice, "vkCreateDevice")
    FISSION_LOAD_INSTANCE(
        destroy_surface,
        PFN_vkDestroySurfaceKHR,
        "vkDestroySurfaceKHR")
#undef FISSION_LOAD_INSTANCE
    return Result::success();
}

Result load_device_dispatch(VulkanContext::Impl& context) {
#define FISSION_LOAD_DEVICE(field, type, name) \
    context.device_api.field = device_proc<type>(context.device, name); \
    if (context.device_api.field == nullptr) { \
        return Result::failure( \
            FISSION_SKIA_STATUS_UNSUPPORTED, \
            "the Vulkan loader is missing a required device function"); \
    }
    FISSION_LOAD_DEVICE(destroy_device, PFN_vkDestroyDevice, "vkDestroyDevice")
    FISSION_LOAD_DEVICE(get_device_queue, PFN_vkGetDeviceQueue, "vkGetDeviceQueue")
    FISSION_LOAD_DEVICE(device_wait_idle, PFN_vkDeviceWaitIdle, "vkDeviceWaitIdle")
    FISSION_LOAD_DEVICE(queue_wait_idle, PFN_vkQueueWaitIdle, "vkQueueWaitIdle")
    FISSION_LOAD_DEVICE(
        create_swapchain,
        PFN_vkCreateSwapchainKHR,
        "vkCreateSwapchainKHR")
    FISSION_LOAD_DEVICE(
        destroy_swapchain,
        PFN_vkDestroySwapchainKHR,
        "vkDestroySwapchainKHR")
    FISSION_LOAD_DEVICE(
        get_swapchain_images,
        PFN_vkGetSwapchainImagesKHR,
        "vkGetSwapchainImagesKHR")
    FISSION_LOAD_DEVICE(
        create_semaphore,
        PFN_vkCreateSemaphore,
        "vkCreateSemaphore")
    FISSION_LOAD_DEVICE(
        destroy_semaphore,
        PFN_vkDestroySemaphore,
        "vkDestroySemaphore")
    FISSION_LOAD_DEVICE(
        acquire_next_image,
        PFN_vkAcquireNextImageKHR,
        "vkAcquireNextImageKHR")
    FISSION_LOAD_DEVICE(queue_present, PFN_vkQueuePresentKHR, "vkQueuePresentKHR")
#undef FISSION_LOAD_DEVICE
    return Result::success();
}

bool valid_native_window(const fission_skia_native_window_t& window) {
    if (window.struct_size != sizeof(window) || window.display == 0 ||
        window.window == 0 ||
        window.display > static_cast<uint64_t>(UINTPTR_MAX)) {
        return false;
    }
    switch (window.kind) {
        case FISSION_SKIA_NATIVE_WINDOW_WAYLAND:
            return window.window <= static_cast<uint64_t>(UINTPTR_MAX) &&
                   window.visual_id == 0;
        case FISSION_SKIA_NATIVE_WINDOW_XLIB:
            return window.window <= static_cast<uint64_t>(UINTPTR_MAX) &&
                   window.visual_id <= static_cast<uint64_t>(UINTPTR_MAX);
        case FISSION_SKIA_NATIVE_WINDOW_XCB:
            return window.window <= UINT32_MAX && window.visual_id <= UINT32_MAX;
        default:
            return false;
    }
}

VulkanContext::VulkanContext()
    : impl_(new (std::nothrow) Impl{}) {}

VulkanContext::~VulkanContext() {
    if (!impl_) return;
    if (impl_->device != VK_NULL_HANDLE && impl_->device_api.device_wait_idle != nullptr &&
        !impl_->device_lost.load(std::memory_order_acquire)) {
        const VkResult wait = impl_->device_api.device_wait_idle(impl_->device);
        if (wait == VK_ERROR_DEVICE_LOST) {
            impl_->device_lost.store(true, std::memory_order_release);
        }
    }
    if (impl_->ganesh) {
        if (impl_->device_lost.load(std::memory_order_acquire) ||
            impl_->ganesh->isDeviceLost()) {
            impl_->ganesh->abandonContext();
        } else {
            impl_->ganesh->releaseResourcesAndAbandonContext();
        }
        impl_->ganesh.reset();
    }
    impl_->allocator.reset();
    if (impl_->probe_surface != VK_NULL_HANDLE &&
        impl_->instance_api.destroy_surface != nullptr) {
        impl_->instance_api.destroy_surface(
            impl_->instance, impl_->probe_surface, nullptr);
        impl_->probe_surface = VK_NULL_HANDLE;
    }
    if (impl_->device != VK_NULL_HANDLE && impl_->device_api.destroy_device != nullptr) {
        impl_->device_api.destroy_device(impl_->device, nullptr);
        impl_->device = VK_NULL_HANDLE;
    }
    if (impl_->instance != VK_NULL_HANDLE &&
        impl_->instance_api.destroy_instance != nullptr) {
        impl_->instance_api.destroy_instance(impl_->instance, nullptr);
        impl_->instance = VK_NULL_HANDLE;
    }
}

Result VulkanContext::create(
    const fission_skia_native_window_t& compatible_window,
    std::unique_ptr<VulkanContext>* out_context) {
    if (out_context == nullptr || !valid_native_window(compatible_window)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh context output or native window is invalid");
    }
    out_context->reset();
    const char* wsi_extension = native_surface_extension(compatible_window.kind);
    if (wsi_extension == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "the native window system has no Vulkan surface implementation");
    }

    auto enumerate_version = global_proc<PFN_vkEnumerateInstanceVersion>(
        "vkEnumerateInstanceVersion");
    auto enumerate_extensions =
        global_proc<PFN_vkEnumerateInstanceExtensionProperties>(
            "vkEnumerateInstanceExtensionProperties");
    auto create_instance = global_proc<PFN_vkCreateInstance>("vkCreateInstance");
    if (enumerate_version == nullptr || enumerate_extensions == nullptr ||
        create_instance == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "the Vulkan loader does not expose the required global functions");
    }
    uint32_t loader_version = 0;
    if (enumerate_version(&loader_version) != VK_SUCCESS ||
        loader_version < VK_API_VERSION_1_1) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "Fission Ganesh requires a Vulkan 1.1 loader");
    }
    Result status = check_instance_extensions(enumerate_extensions, wsi_extension);
    if (!status.ok()) return status;

    auto context = std::unique_ptr<VulkanContext>(
        new (std::nothrow) VulkanContext());
    if (!context || !context->impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the Ganesh Vulkan context state could not be allocated");
    }
    context->impl_->window_kind = compatible_window.kind;

    const char* instance_extensions[] = {kSurfaceExtension, wsi_extension};
    VkApplicationInfo application_info{};
    application_info.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO;
    application_info.pApplicationName = "Fission";
    application_info.applicationVersion = 0;
    application_info.pEngineName = "Fission Skia";
    application_info.engineVersion = 0;
    application_info.apiVersion = context->impl_->api_version;
    VkInstanceCreateInfo create_info{};
    create_info.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO;
    create_info.pApplicationInfo = &application_info;
    create_info.enabledExtensionCount = 2;
    create_info.ppEnabledExtensionNames = instance_extensions;
    VkResult vk_result = create_instance(
        &create_info, nullptr, &context->impl_->instance);
    status = classify_vulkan_result(*context->impl_, vk_result);
    if (!status.ok()) return status;
    status = load_instance_dispatch(*context->impl_);
    if (!status.ok()) return status;
    status = create_native_surface(
        *context->impl_, compatible_window, &context->impl_->probe_surface);
    if (!status.ok()) return status;
    status = choose_physical_device(*context->impl_);
    if (!status.ok()) return status;

    float queue_priority = 1.0f;
    VkDeviceQueueCreateInfo queues[2]{};
    uint32_t queue_count = 1;
    queues[0].sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO;
    queues[0].queueFamilyIndex = context->impl_->graphics_queue_family;
    queues[0].queueCount = 1;
    queues[0].pQueuePriorities = &queue_priority;
    if (context->impl_->initial_present_queue_family !=
        context->impl_->graphics_queue_family) {
        queue_count = 2;
        queues[1].sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO;
        queues[1].queueFamilyIndex = context->impl_->initial_present_queue_family;
        queues[1].queueCount = 1;
        queues[1].pQueuePriorities = &queue_priority;
    }
    const char* device_extensions[] = {kSwapchainExtension};
    VkDeviceCreateInfo device_info{};
    device_info.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO;
    device_info.queueCreateInfoCount = queue_count;
    device_info.pQueueCreateInfos = queues;
    device_info.enabledExtensionCount = 1;
    device_info.ppEnabledExtensionNames = device_extensions;
    device_info.pEnabledFeatures = &context->impl_->enabled_features;
    vk_result = context->impl_->instance_api.create_device(
        context->impl_->physical_device,
        &device_info,
        nullptr,
        &context->impl_->device);
    status = classify_vulkan_result(*context->impl_, vk_result);
    if (!status.ok()) return status;
    status = load_device_dispatch(*context->impl_);
    if (!status.ok()) return status;
    context->impl_->device_api.get_device_queue(
        context->impl_->device,
        context->impl_->graphics_queue_family,
        0,
        &context->impl_->graphics_queue);
    context->impl_->device_api.get_device_queue(
        context->impl_->device,
        context->impl_->initial_present_queue_family,
        0,
        &context->impl_->initial_present_queue);
    if (context->impl_->graphics_queue == VK_NULL_HANDLE ||
        context->impl_->initial_present_queue == VK_NULL_HANDLE) {
        return Result::failure(
            FISSION_SKIA_STATUS_INTERNAL,
            "Vulkan returned a null graphics or presentation queue");
    }

    context->impl_->instance_api.destroy_surface(
        context->impl_->instance, context->impl_->probe_surface, nullptr);
    context->impl_->probe_surface = VK_NULL_HANDLE;

    context->impl_->extensions.init(
        [](const char* name, VkInstance instance, VkDevice device) {
            return device != VK_NULL_HANDLE
                ? vkGetDeviceProcAddr(device, name)
                : vkGetInstanceProcAddr(instance, name);
        },
        context->impl_->instance,
        context->impl_->physical_device,
        2,
        instance_extensions,
        1,
        device_extensions);

    skgpu::VulkanBackendContext backend;
    backend.fInstance = context->impl_->instance;
    backend.fPhysicalDevice = context->impl_->physical_device;
    backend.fDevice = context->impl_->device;
    backend.fQueue = context->impl_->graphics_queue;
    backend.fGraphicsQueueIndex = context->impl_->graphics_queue_family;
    backend.fMaxAPIVersion = context->impl_->api_version;
    backend.fVkExtensions = &context->impl_->extensions;
    backend.fDeviceFeatures = &context->impl_->enabled_features;
    backend.fGetProc = [](const char* name, VkInstance instance, VkDevice device) {
        return device != VK_NULL_HANDLE
            ? vkGetDeviceProcAddr(device, name)
            : vkGetInstanceProcAddr(instance, name);
    };
    backend.fDeviceLostContext = context->impl_.get();
    backend.fDeviceLostProc = ganesh_device_lost;
    context->impl_->allocator = skgpu::VulkanMemoryAllocators::Make(
        backend, skgpu::ThreadSafe::kNo);
    if (!context->impl_->allocator) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Skia could not create its Vulkan memory allocator");
    }
    backend.fMemoryAllocator = context->impl_->allocator;
    context->impl_->ganesh = GrDirectContexts::MakeVulkan(backend);
    if (!context->impl_->ganesh) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "Skia could not create a Ganesh Vulkan context");
    }
    *out_context = std::move(context);
    return Result::success();
}

Result VulkanContext::set_resource_cache_limit(uint64_t limit_bytes) {
    if (!impl_ || !impl_->ganesh) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh context is not initialized");
    }
    if (limit_bytes > static_cast<uint64_t>(std::numeric_limits<size_t>::max())) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the GPU resource-cache limit exceeds this platform's address range");
    }
    if (is_device_lost()) {
        return Result::failure(
            FISSION_SKIA_STATUS_DEVICE_LOST,
            "the Ganesh Vulkan device is lost");
    }
    impl_->ganesh->setResourceCacheLimit(static_cast<size_t>(limit_bytes));
    return Result::success();
}

Result VulkanContext::resource_cache_usage(
    uint64_t* out_resource_count,
    uint64_t* out_resource_bytes) const {
    if (out_resource_count == nullptr || out_resource_bytes == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the GPU resource-cache usage outputs are null");
    }
    *out_resource_count = 0;
    *out_resource_bytes = 0;
    if (!impl_ || !impl_->ganesh) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh context is not initialized");
    }
    if (is_device_lost()) {
        return Result::failure(
            FISSION_SKIA_STATUS_DEVICE_LOST,
            "the Ganesh Vulkan device is lost");
    }
    int resource_count = 0;
    size_t resource_bytes = 0;
    impl_->ganesh->getResourceCacheUsage(&resource_count, &resource_bytes);
    if (resource_count < 0) {
        return Result::failure(
            FISSION_SKIA_STATUS_INTERNAL,
            "Skia reported a negative GPU resource-cache count");
    }
    *out_resource_count = static_cast<uint64_t>(resource_count);
    *out_resource_bytes = static_cast<uint64_t>(resource_bytes);
    return Result::success();
}

Result VulkanContext::trim_memory(uint32_t pressure) {
    if (!impl_ || !impl_->ganesh) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh context is not initialized");
    }
    if (is_device_lost()) {
        return Result::failure(
            FISSION_SKIA_STATUS_DEVICE_LOST,
            "the Ganesh Vulkan device is lost");
    }
    if (pressure == FISSION_SKIA_MEMORY_PRESSURE_MODERATE) {
        impl_->ganesh->performDeferredCleanup(
            std::chrono::milliseconds(0),
            GrPurgeResourceOptions::kScratchResourcesOnly);
    } else if (pressure == FISSION_SKIA_MEMORY_PRESSURE_CRITICAL) {
        impl_->ganesh->purgeUnlockedResources(
            GrPurgeResourceOptions::kAllResources);
    } else {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the memory-pressure value is unknown");
    }
    return Result::success();
}

bool VulkanContext::is_device_lost() const {
    return !impl_ || impl_->device_lost.load(std::memory_order_acquire);
}

uint32_t VulkanContext::window_kind() const {
    return impl_ ? impl_->window_kind : 0;
}

VulkanContext::Impl& VulkanContext::internal_state() {
    return *impl_;
}

const VulkanContext::Impl& VulkanContext::internal_state() const {
    return *impl_;
}

}  // namespace fission::skia::ganesh
