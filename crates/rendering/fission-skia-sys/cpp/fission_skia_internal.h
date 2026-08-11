#ifndef FISSION_SKIA_INTERNAL_H
#define FISSION_SKIA_INTERNAL_H

#include "fission_skia.h"

#ifndef FISSION_SKIA_ENABLE_GANESH_VULKAN
#define FISSION_SKIA_ENABLE_GANESH_VULKAN 0
#endif

#if FISSION_SKIA_ENABLE_GANESH_VULKAN
#include "fission_skia_ganesh_vulkan.h"
#endif

#include "include/core/SkImage.h"
#include "include/core/SkPicture.h"
#include "include/core/SkRect.h"
#include "include/core/SkSize.h"
#include "include/core/SkSurface.h"
#include "modules/svg/include/SkSVGDOM.h"

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <mutex>
#include <thread>
#include <unordered_map>

class SkCanvas;

namespace fission::skia::bridge {

enum class ContextBackend { kRaster, kGaneshVulkan };
enum class SurfaceBackend { kRaster, kGaneshVulkan };

struct EngineState {
    std::thread::id owner;
    uint64_t live_contexts = 0;
};

struct ContextState {
    std::thread::id owner;
    fission_skia_engine_handle_t engine = 0;
    uint64_t live_surfaces = 0;
    ContextBackend backend = ContextBackend::kRaster;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    std::unique_ptr<::fission::skia::ganesh::VulkanContext> ganesh;
#endif
};

struct SurfaceState {
    std::thread::id owner;
    fission_skia_context_handle_t context = 0;
    uint32_t width = 0;
    uint32_t height = 0;
    SurfaceBackend backend = SurfaceBackend::kRaster;
    sk_sp<SkSurface> surface;
#if FISSION_SKIA_ENABLE_GANESH_VULKAN
    std::unique_ptr<::fission::skia::ganesh::VulkanSurface> ganesh;
#endif
};

struct ImageState {
    uint32_t width = 0;
    uint32_t height = 0;
    size_t approximate_decoded_bytes = 0;
    sk_sp<SkImage> image;
};

struct SvgDocumentState {
    SkSize intrinsic_size = SkSize::Make(0.0f, 0.0f);
    sk_sp<SkSVGDOM> document;
};

struct PictureState {
    sk_sp<SkPicture> picture;
};

// The process-wide registry is the sole owner and liveness authority for every
// opaque C ABI handle. Exported entrypoints lock it before dereferencing or
// mutating state; frame playback requires that lock to remain held so retained
// resources cannot disappear during a draw.
struct Registry {
    std::mutex mutex;
    std::unordered_map<uint64_t, std::unique_ptr<EngineState>> engines;
    std::unordered_map<uint64_t, std::unique_ptr<ContextState>> contexts;
    std::unordered_map<uint64_t, std::unique_ptr<SurfaceState>> surfaces;
    std::unordered_map<uint64_t, std::unique_ptr<ImageState>> images;
    std::unordered_map<uint64_t, std::unique_ptr<SvgDocumentState>> svg_documents;
    std::unordered_map<uint64_t, std::unique_ptr<PictureState>> pictures;
    std::atomic<uint64_t> next_handle{1};
    std::atomic<uint64_t> next_error{1};
};

Registry& registry();
uint64_t next_handle();

void copy_text(char* destination, size_t capacity, const char* source);
void clear_error(fission_skia_error_t* error);
fission_skia_status_t fail(
    fission_skia_status_t status,
    const char* operation,
    const char* message,
    fission_skia_error_t* error);

bool finite(float value);
bool valid_svg_source(const uint8_t* bytes, size_t length);
bool valid_color(const fission_skia_color_t& color);
bool valid_rect(const fission_skia_rect_t& rect);
bool valid_non_empty_rect(const fission_skia_rect_t& rect);
bool valid_native_window(const fission_skia_native_window_t* window);
void write_image_info(const ImageState& image, fission_skia_image_info_t* out_info);

fission_skia_status_t validate_frame(
    const fission_skia_frame_t* frame,
    bool recording,
    fission_skia_error_t* error);

SkRect sk_rect(const fission_skia_rect_t& rect);

// The caller must hold registry().mutex for the whole call. Playback resolves
// image, SVG, and picture handles directly from the authoritative registry.
fission_skia_status_t play_frame(
    SkCanvas* canvas,
    SurfaceState* surface,
    const fission_skia_frame_t& frame,
    const char* operation_name,
    fission_skia_error_t* error);

template <typename State>
fission_skia_status_t check_owner(
    const State& state,
    const char* operation,
    fission_skia_error_t* error) {
    if (state.owner != std::this_thread::get_id()) {
        return fail(FISSION_SKIA_STATUS_WRONG_THREAD, operation,
                    "native handle was used from a thread other than its owner", error);
    }
    return FISSION_SKIA_STATUS_OK;
}

#if FISSION_SKIA_ENABLE_GANESH_VULKAN
fission_skia_status_t cancel_ganesh_frame(
    ::fission::skia::ganesh::VulkanSurface& surface,
    fission_skia_status_t original_status,
    fission_skia_error_t* error);
#endif

}  // namespace fission::skia::bridge

#endif  // FISSION_SKIA_INTERNAL_H
