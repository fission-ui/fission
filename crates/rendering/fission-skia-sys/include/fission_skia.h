#ifndef FISSION_SKIA_H
#define FISSION_SKIA_H

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32) && defined(FISSION_SKIA_SHARED)
#if defined(FISSION_SKIA_BUILDING_BRIDGE)
#define FISSION_SKIA_EXPORT __declspec(dllexport)
#else
#define FISSION_SKIA_EXPORT __declspec(dllimport)
#endif
#elif defined(__GNUC__) && defined(FISSION_SKIA_SHARED)
#define FISSION_SKIA_EXPORT __attribute__((visibility("default")))
#else
#define FISSION_SKIA_EXPORT
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define FISSION_SKIA_ABI_VERSION 2u
#define FISSION_SKIA_REVISION_LENGTH 41u
#define FISSION_SKIA_PROFILE_LENGTH 32u
#define FISSION_SKIA_ERROR_OPERATION_LENGTH 64u
#define FISSION_SKIA_ERROR_MESSAGE_LENGTH 512u

typedef uint64_t fission_skia_engine_handle_t;
typedef uint64_t fission_skia_context_handle_t;
typedef uint64_t fission_skia_surface_handle_t;

typedef enum fission_skia_status_t {
    FISSION_SKIA_STATUS_OK = 0,
    FISSION_SKIA_STATUS_INVALID_ARGUMENT = 1,
    FISSION_SKIA_STATUS_INVALID_HANDLE = 2,
    FISSION_SKIA_STATUS_INVALID_STATE = 3,
    FISSION_SKIA_STATUS_UNSUPPORTED = 4,
    FISSION_SKIA_STATUS_WRONG_THREAD = 5,
    FISSION_SKIA_STATUS_SURFACE_LOST = 6,
    FISSION_SKIA_STATUS_CONTEXT_LOST = 7,
    FISSION_SKIA_STATUS_DEVICE_LOST = 8,
    FISSION_SKIA_STATUS_OUT_OF_MEMORY = 9,
    FISSION_SKIA_STATUS_ABI_MISMATCH = 10,
    FISSION_SKIA_STATUS_INTERNAL = 11
} fission_skia_status_t;

typedef enum fission_skia_feature_t {
    FISSION_SKIA_FEATURE_RASTER_SURFACE = UINT64_C(1) << 0,
    FISSION_SKIA_FEATURE_BASIC_FRAME = UINT64_C(1) << 1,
    FISSION_SKIA_FEATURE_RGBA_READBACK = UINT64_C(1) << 2,
    FISSION_SKIA_FEATURE_STRUCTURED_ERRORS = UINT64_C(1) << 3,
    FISSION_SKIA_FEATURE_THREAD_AFFINITY = UINT64_C(1) << 4,
    FISSION_SKIA_FEATURE_MEMORY_PRESSURE = UINT64_C(1) << 5,
    FISSION_SKIA_FEATURE_PAINT_STATE = UINT64_C(1) << 6,
    FISSION_SKIA_FEATURE_TEST_SHIM = UINT64_C(1) << 63
} fission_skia_feature_t;

typedef struct fission_skia_abi_info_t {
    uint32_t struct_size;
    uint32_t abi_version;
    uint64_t feature_bits;
    char skia_revision[FISSION_SKIA_REVISION_LENGTH];
    char build_profile[FISSION_SKIA_PROFILE_LENGTH];
} fission_skia_abi_info_t;

typedef struct fission_skia_error_t {
    uint32_t struct_size;
    uint32_t code;
    uint64_t sequence;
    char operation[FISSION_SKIA_ERROR_OPERATION_LENGTH];
    char message[FISSION_SKIA_ERROR_MESSAGE_LENGTH];
} fission_skia_error_t;

typedef struct fission_skia_engine_config_t {
    uint32_t struct_size;
    uint32_t expected_abi_version;
    uint64_t required_feature_bits;
} fission_skia_engine_config_t;

typedef enum fission_skia_memory_pressure_t {
    FISSION_SKIA_MEMORY_PRESSURE_MODERATE = 1,
    FISSION_SKIA_MEMORY_PRESSURE_CRITICAL = 2
} fission_skia_memory_pressure_t;

typedef struct fission_skia_color_t {
    /* Finite, unpremultiplied sRGB components in the inclusive range 0..1. */
    float red;
    float green;
    float blue;
    float alpha;
} fission_skia_color_t;

typedef struct fission_skia_rect_t {
    float x;
    float y;
    float width;
    float height;
} fission_skia_rect_t;

typedef struct fission_skia_point_t {
    float x;
    float y;
} fission_skia_point_t;

/* A finite two-dimensional affine matrix in Skia's six-scalar order. */
typedef struct fission_skia_affine_t {
    float scale_x;
    float skew_x;
    float translate_x;
    float skew_y;
    float scale_y;
    float translate_y;
} fission_skia_affine_t;

typedef enum fission_skia_path_verb_t {
    FISSION_SKIA_PATH_MOVE = 1,
    FISSION_SKIA_PATH_LINE = 2,
    FISSION_SKIA_PATH_QUAD = 3,
    FISSION_SKIA_PATH_CUBIC = 4,
    FISSION_SKIA_PATH_CLOSE = 5
} fission_skia_path_verb_t;

typedef struct fission_skia_path_command_t {
    uint32_t struct_size;
    uint32_t verb;
    float x1;
    float y1;
    float x2;
    float y2;
    float x3;
    float y3;
} fission_skia_path_command_t;

typedef enum fission_skia_fill_rule_t {
    FISSION_SKIA_FILL_NON_ZERO = 1,
    FISSION_SKIA_FILL_EVEN_ODD = 2
} fission_skia_fill_rule_t;

typedef struct fission_skia_gradient_stop_t {
    float offset;
    fission_skia_color_t color;
} fission_skia_gradient_stop_t;

typedef enum fission_skia_paint_kind_t {
    FISSION_SKIA_PAINT_SOLID = 1,
    FISSION_SKIA_PAINT_LINEAR_GRADIENT = 2,
    FISSION_SKIA_PAINT_RADIAL_GRADIENT = 3
} fission_skia_paint_kind_t;

typedef struct fission_skia_paint_t {
    uint32_t struct_size;
    uint32_t kind;
    fission_skia_color_t color;
    fission_skia_point_t start;
    fission_skia_point_t end;
    float radius;
    uint32_t stop_offset;
    uint32_t stop_count;
} fission_skia_paint_t;

/*
 * Gradient stops are ordered by offset. The bridge preserves coincident-stop
 * order by separating hard-stop colors by adjacent f32 values for Skia.
 * An empty gradient is transparent; a one-stop gradient is that solid color.
 * A zero-radius radial gradient or a linear gradient with identical endpoints
 * resolves to its terminal stop (or transparent when empty), so no accepted
 * gradient silently loses its paint.
 */

typedef enum fission_skia_line_cap_t {
    FISSION_SKIA_LINE_CAP_BUTT = 1,
    FISSION_SKIA_LINE_CAP_ROUND = 2,
    FISSION_SKIA_LINE_CAP_SQUARE = 3
} fission_skia_line_cap_t;

typedef enum fission_skia_line_join_t {
    FISSION_SKIA_LINE_JOIN_MITER = 1,
    FISSION_SKIA_LINE_JOIN_ROUND = 2,
    FISSION_SKIA_LINE_JOIN_BEVEL = 3
} fission_skia_line_join_t;

typedef struct fission_skia_stroke_t {
    uint32_t struct_size;
    float width;
    uint32_t line_cap;
    uint32_t line_join;
    uint32_t dash_offset;
    uint32_t dash_count;
} fission_skia_stroke_t;

typedef struct fission_skia_box_shadow_t {
    uint32_t struct_size;
    uint32_t inset;
    fission_skia_color_t color;
    float blur_radius;
    float spread_radius;
    float offset_x;
    float offset_y;
} fission_skia_box_shadow_t;

typedef enum fission_skia_frame_op_kind_t {
    FISSION_SKIA_FRAME_CLEAR = 1,
    FISSION_SKIA_FRAME_SAVE = 2,
    FISSION_SKIA_FRAME_RESTORE = 3,
    FISSION_SKIA_FRAME_CLIP_RECT = 4,
    FISSION_SKIA_FRAME_CLIP_ROUNDED_RECT = 5,
    FISSION_SKIA_FRAME_CONCAT_AFFINE = 6,
    FISSION_SKIA_FRAME_FILL_RECT = 7,
    FISSION_SKIA_FRAME_STROKE_RECT = 8,
    FISSION_SKIA_FRAME_FILL_PATH = 9,
    FISSION_SKIA_FRAME_STROKE_PATH = 10,
    FISSION_SKIA_FRAME_BOX_SHADOW = 11
} fission_skia_frame_op_kind_t;

/*
 * Operations are deliberately fixed-width and pointer-free. Offset/count
 * pairs address the enclosing frame's path, gradient, and dash arrays. Unused
 * fields must be zero, allowing future ABI versions to give them meaning
 * explicitly. Odd dash arrays are duplicated by the safe Rust encoder; empty
 * and all-zero arrays are encoded as a solid stroke.
 */
typedef struct fission_skia_frame_op_t {
    uint32_t struct_size;
    uint32_t kind;
    fission_skia_paint_t paint;
    fission_skia_stroke_t stroke;
    fission_skia_box_shadow_t shadow;
    fission_skia_rect_t rect;
    fission_skia_affine_t affine;
    float radius;
    uint32_t path_offset;
    uint32_t path_count;
    uint32_t fill_rule;
    uint32_t reserved;
} fission_skia_frame_op_t;

typedef struct fission_skia_frame_t {
    uint32_t struct_size;
    uint32_t reserved;
    const fission_skia_frame_op_t* operations;
    size_t operation_count;
    const fission_skia_path_command_t* path_commands;
    size_t path_command_count;
    const fission_skia_gradient_stop_t* gradient_stops;
    size_t gradient_stop_count;
    const float* dash_intervals;
    size_t dash_interval_count;
} fission_skia_frame_t;

typedef struct fission_skia_pixel_rect_t {
    int32_t x;
    int32_t y;
    uint32_t width;
    uint32_t height;
} fission_skia_pixel_rect_t;

FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_get_abi_info(
    fission_skia_abi_info_t* out_info,
    fission_skia_error_t* out_error);

FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_engine_create(
    const fission_skia_engine_config_t* config,
    fission_skia_engine_handle_t* out_engine,
    fission_skia_error_t* out_error);
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_engine_destroy(
    fission_skia_engine_handle_t engine,
    fission_skia_error_t* out_error);

FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_context_create_raster(
    fission_skia_engine_handle_t engine,
    fission_skia_context_handle_t* out_context,
    fission_skia_error_t* out_error);
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_context_trim_memory(
    fission_skia_context_handle_t context,
    uint32_t pressure,
    fission_skia_error_t* out_error);
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_context_destroy(
    fission_skia_context_handle_t context,
    fission_skia_error_t* out_error);

FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_surface_create_raster(
    fission_skia_context_handle_t context,
    uint32_t width,
    uint32_t height,
    fission_skia_surface_handle_t* out_surface,
    fission_skia_error_t* out_error);
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_surface_execute_frame(
    fission_skia_surface_handle_t surface,
    const fission_skia_frame_t* frame,
    fission_skia_error_t* out_error);
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_surface_read_pixels_rgba8888(
    fission_skia_surface_handle_t surface,
    const fission_skia_pixel_rect_t* source_rect,
    uint8_t* destination,
    size_t destination_length,
    size_t destination_row_bytes,
    size_t* out_required_length,
    fission_skia_error_t* out_error);
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_surface_destroy(
    fission_skia_surface_handle_t surface,
    fission_skia_error_t* out_error);

#if defined(FISSION_SKIA_TEST_SHIM)
typedef struct fission_skia_test_counts_t {
    uint64_t engines;
    uint64_t contexts;
    uint64_t surfaces;
} fission_skia_test_counts_t;
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_test_live_counts(
    fission_skia_test_counts_t* out_counts,
    fission_skia_error_t* out_error);
#endif

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* FISSION_SKIA_H */
