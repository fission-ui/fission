#include "fission_skia_internal.h"

#include <cmath>
#include <cstring>

namespace fission::skia::bridge {

Registry& registry() {
    static Registry value;
    return value;
}

uint64_t next_handle() {
    uint64_t handle = registry().next_handle.fetch_add(1, std::memory_order_relaxed);
    return handle == 0
        ? registry().next_handle.fetch_add(1, std::memory_order_relaxed)
        : handle;
}

void copy_text(char* destination, size_t capacity, const char* source) {
    if (capacity == 0) {
        return;
    }
    const size_t length = std::strlen(source);
    const size_t copied = length < capacity - 1 ? length : capacity - 1;
    std::memcpy(destination, source, copied);
    destination[copied] = '\0';
    if (copied + 1 < capacity) {
        std::memset(destination + copied + 1, 0, capacity - copied - 1);
    }
}

void clear_error(fission_skia_error_t* error) {
    if (error == nullptr || error->struct_size != sizeof(fission_skia_error_t)) {
        return;
    }
    error->code = FISSION_SKIA_STATUS_OK;
    error->sequence = 0;
    error->operation[0] = '\0';
    error->message[0] = '\0';
}

fission_skia_status_t fail(
    fission_skia_status_t status,
    const char* operation,
    const char* message,
    fission_skia_error_t* error) {
    if (error != nullptr && error->struct_size == sizeof(fission_skia_error_t)) {
        error->code = status;
        error->sequence = registry().next_error.fetch_add(1, std::memory_order_relaxed);
        copy_text(error->operation, sizeof(error->operation), operation);
        copy_text(error->message, sizeof(error->message), message);
    }
    return status;
}

bool finite(float value) {
    return std::isfinite(value);
}

namespace {

uint8_t ascii_lower(uint8_t value) {
    return value >= 'A' && value <= 'Z'
        ? static_cast<uint8_t>(value + ('a' - 'A'))
        : value;
}

bool contains_ascii_case_insensitive(
    const uint8_t* bytes,
    size_t length,
    const char* needle) {
    const size_t needle_length = std::strlen(needle);
    if (needle_length == 0 || needle_length > length) return false;
    for (size_t offset = 0; offset <= length - needle_length; ++offset) {
        bool matches = true;
        for (size_t index = 0; index < needle_length; ++index) {
            if (ascii_lower(bytes[offset + index]) !=
                ascii_lower(static_cast<uint8_t>(needle[index]))) {
                matches = false;
                break;
            }
        }
        if (matches) return true;
    }
    return false;
}

bool valid_utf8_without_nul(const uint8_t* bytes, size_t length) {
    size_t index = 0;
    while (index < length) {
        const uint8_t first = bytes[index];
        if (first == 0) return false;
        if (first <= 0x7f) {
            index += 1;
            continue;
        }
        if (first >= 0xc2 && first <= 0xdf) {
            if (index + 1 >= length || (bytes[index + 1] & 0xc0) != 0x80) return false;
            index += 2;
            continue;
        }
        if (first >= 0xe0 && first <= 0xef) {
            if (index + 2 >= length || (bytes[index + 2] & 0xc0) != 0x80) return false;
            const uint8_t second = bytes[index + 1];
            if ((first == 0xe0 && (second < 0xa0 || second > 0xbf)) ||
                (first == 0xed && (second < 0x80 || second > 0x9f)) ||
                (first != 0xe0 && first != 0xed && (second & 0xc0) != 0x80)) {
                return false;
            }
            index += 3;
            continue;
        }
        if (first >= 0xf0 && first <= 0xf4) {
            if (index + 3 >= length || (bytes[index + 2] & 0xc0) != 0x80 ||
                (bytes[index + 3] & 0xc0) != 0x80) {
                return false;
            }
            const uint8_t second = bytes[index + 1];
            if ((first == 0xf0 && (second < 0x90 || second > 0xbf)) ||
                (first == 0xf4 && (second < 0x80 || second > 0x8f)) ||
                (first != 0xf0 && first != 0xf4 && (second & 0xc0) != 0x80)) {
                return false;
            }
            index += 4;
            continue;
        }
        return false;
    }
    return true;
}

}  // namespace

bool valid_svg_source(const uint8_t* bytes, size_t length) {
    return bytes != nullptr && length != 0 &&
           length <= FISSION_SKIA_MAX_SVG_DOCUMENT_BYTES &&
           valid_utf8_without_nul(bytes, length) &&
           !contains_ascii_case_insensitive(bytes, length, "<!doctype") &&
           !contains_ascii_case_insensitive(bytes, length, "<!entity");
}

bool valid_color(const fission_skia_color_t& color) {
    const float values[] = {color.red, color.green, color.blue, color.alpha};
    for (float value : values) {
        if (!finite(value) || value < 0.0f || value > 1.0f) {
            return false;
        }
    }
    return true;
}

bool valid_rect(const fission_skia_rect_t& rect) {
    return finite(rect.x) && finite(rect.y) && finite(rect.width) &&
           finite(rect.height) && rect.width >= 0.0f && rect.height >= 0.0f;
}

bool valid_non_empty_rect(const fission_skia_rect_t& rect) {
    return valid_rect(rect) && rect.width > 0.0f && rect.height > 0.0f;
}

bool valid_native_window(const fission_skia_native_window_t* window) {
    if (window == nullptr || window->struct_size != sizeof(*window) ||
        window->window == 0) {
        return false;
    }
    switch (window->kind) {
        case FISSION_SKIA_NATIVE_WINDOW_WAYLAND:
            return window->display != 0 &&
                   window->display <= static_cast<uint64_t>(UINTPTR_MAX) &&
                   window->window <= static_cast<uint64_t>(UINTPTR_MAX) &&
                   window->visual_id == 0;
        case FISSION_SKIA_NATIVE_WINDOW_XLIB:
            return window->display != 0 &&
                   window->display <= static_cast<uint64_t>(UINTPTR_MAX) &&
                   window->window <= static_cast<uint64_t>(UINTPTR_MAX) &&
                   window->visual_id <= static_cast<uint64_t>(UINTPTR_MAX);
        case FISSION_SKIA_NATIVE_WINDOW_XCB:
            return window->display != 0 &&
                   window->display <= static_cast<uint64_t>(UINTPTR_MAX) &&
                   window->window <= UINT32_MAX && window->visual_id <= UINT32_MAX;
        case FISSION_SKIA_NATIVE_WINDOW_APPKIT:
        case FISSION_SKIA_NATIVE_WINDOW_UIKIT:
            return window->display == 0 && window->visual_id == 0 &&
                   window->window <= static_cast<uint64_t>(UINTPTR_MAX);
        default:
            return false;
    }
}

#if FISSION_SKIA_ENABLE_GANESH_NATIVE
bool native_ganesh_supports_window(uint32_t kind) {
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    return kind == FISSION_SKIA_NATIVE_WINDOW_WAYLAND ||
           kind == FISSION_SKIA_NATIVE_WINDOW_XLIB ||
           kind == FISSION_SKIA_NATIVE_WINDOW_XCB;
#elif FISSION_SKIA_ENABLE_GANESH_METAL
    return kind == FISSION_SKIA_NATIVE_WINDOW_APPKIT;
#elif FISSION_SKIA_ENABLE_GANESH_IOS_METAL
    return kind == FISSION_SKIA_NATIVE_WINDOW_UIKIT;
#endif
}

NativeGaneshResult create_native_ganesh_context(
    const fission_skia_native_window_t& window,
    std::unique_ptr<NativeGaneshContext>* out_context) {
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    return NativeGaneshContext::create(window, out_context);
#elif FISSION_SKIA_ENABLE_GANESH_METAL
    const ::fission::skia::ganesh::metal::MacOSWindow native{
        sizeof(::fission::skia::ganesh::metal::MacOSWindow),
        reinterpret_cast<const void*>(static_cast<uintptr_t>(window.window)),
    };
    return NativeGaneshContext::create(native, out_context);
#elif FISSION_SKIA_ENABLE_GANESH_IOS_METAL
    const ::fission::skia::ganesh::ios_metal::IOSView native{
        sizeof(::fission::skia::ganesh::ios_metal::IOSView),
        reinterpret_cast<const void*>(static_cast<uintptr_t>(window.window)),
    };
    return NativeGaneshContext::create(native, out_context);
#endif
}

NativeGaneshResult create_native_ganesh_surface(
    NativeGaneshContext& context,
    const fission_skia_native_window_t& window,
    uint32_t width,
    uint32_t height,
    std::unique_ptr<NativeGaneshSurface>* out_surface) {
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    return NativeGaneshSurface::create(
        context, window, width, height, out_surface);
#elif FISSION_SKIA_ENABLE_GANESH_METAL
    const ::fission::skia::ganesh::metal::MacOSWindow native{
        sizeof(::fission::skia::ganesh::metal::MacOSWindow),
        reinterpret_cast<const void*>(static_cast<uintptr_t>(window.window)),
    };
    return NativeGaneshSurface::create(
        context, native, width, height, out_surface);
#elif FISSION_SKIA_ENABLE_GANESH_IOS_METAL
    const ::fission::skia::ganesh::ios_metal::IOSView native{
        sizeof(::fission::skia::ganesh::ios_metal::IOSView),
        reinterpret_cast<const void*>(static_cast<uintptr_t>(window.window)),
    };
    return NativeGaneshSurface::create(
        context, native, width, height, out_surface);
#endif
}

NativeGaneshResult resize_native_ganesh_surface(
    NativeGaneshSurface& surface,
    const fission_skia_native_window_t& window,
    uint32_t width,
    uint32_t height) {
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    return surface.resize(window, width, height);
#elif FISSION_SKIA_ENABLE_GANESH_METAL
    const ::fission::skia::ganesh::metal::MacOSWindow native{
        sizeof(::fission::skia::ganesh::metal::MacOSWindow),
        reinterpret_cast<const void*>(static_cast<uintptr_t>(window.window)),
    };
    return surface.resize(native, width, height);
#elif FISSION_SKIA_ENABLE_GANESH_IOS_METAL
    const ::fission::skia::ganesh::ios_metal::IOSView native{
        sizeof(::fission::skia::ganesh::ios_metal::IOSView),
        reinterpret_cast<const void*>(static_cast<uintptr_t>(window.window)),
    };
    return surface.resize(native, width, height);
#endif
}
#endif

void write_image_info(
    const ImageState& image,
    fission_skia_image_info_t* out_info) {
    out_info->width = image.width;
    out_info->height = image.height;
    out_info->reserved = 0;
    out_info->approximate_decoded_bytes = image.approximate_decoded_bytes;
}

}  // namespace fission::skia::bridge
