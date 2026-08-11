#ifndef FISSION_SKIA_PARAGRAPH_INTERNAL_H
#define FISSION_SKIA_PARAGRAPH_INTERNAL_H

#include "fission_skia.h"

#include <cstddef>
#include <cstdint>

inline fission_skia_paragraph_result_handle_t fission_skia_paragraph_handle_from_frame_op(
    const fission_skia_frame_op_t& operation) {
    return static_cast<uint64_t>(operation.path_offset) |
           (static_cast<uint64_t>(operation.path_count) << 32);
}

fission_skia_status_t fission_skia_paragraph_validate_draw(
    fission_skia_paragraph_result_handle_t result,
    float x,
    float y,
    float scale_factor,
    fission_skia_error_t* out_error);

#if defined(FISSION_SKIA_TEST_SHIM)

using fission_skia_test_paragraph_rect_callback_t = void (*)(
    void* context,
    const fission_skia_paragraph_rect_t& rect,
    const fission_skia_color_t& color);

fission_skia_status_t fission_skia_paragraph_draw_test_picture(
    fission_skia_paragraph_result_handle_t result,
    float x,
    float y,
    float scale_factor,
    void* context,
    fission_skia_test_paragraph_rect_callback_t draw_rect,
    fission_skia_error_t* out_error);

#else

class SkCanvas;

fission_skia_status_t fission_skia_paragraph_draw_picture(
    fission_skia_paragraph_result_handle_t result,
    SkCanvas* canvas,
    float x,
    float y,
    float scale_factor,
    fission_skia_error_t* out_error);

#endif

#endif  // FISSION_SKIA_PARAGRAPH_INTERNAL_H
