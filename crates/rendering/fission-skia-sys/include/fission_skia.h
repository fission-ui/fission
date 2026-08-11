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
typedef uint64_t fission_skia_paragraph_result_handle_t;

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
    FISSION_SKIA_FEATURE_PARAGRAPH = UINT64_C(1) << 7,
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

/*
 * Paragraph input strings and text ranges are UTF-8. Every range is half-open
 * and measured in UTF-8 bytes. Input pointers are borrowed only for the
 * duration of fission_skia_paragraph_layout; the result never retains them.
 */
typedef struct fission_skia_utf8_slice_t {
    const uint8_t* data;
    size_t length;
} fission_skia_utf8_slice_t;

typedef struct fission_skia_text_range_t {
    uint64_t start;
    uint64_t end;
} fission_skia_text_range_t;

typedef struct fission_skia_rgba8_t {
    uint8_t red;
    uint8_t green;
    uint8_t blue;
    uint8_t alpha;
} fission_skia_rgba8_t;

typedef struct fission_skia_font_variation_t {
    uint32_t tag;
    float value;
} fission_skia_font_variation_t;

typedef struct fission_skia_font_feature_t {
    uint32_t tag;
    uint32_t value;
} fission_skia_font_feature_t;

typedef enum fission_skia_font_slant_t {
    FISSION_SKIA_FONT_SLANT_NORMAL = 0,
    FISSION_SKIA_FONT_SLANT_ITALIC = 1
} fission_skia_font_slant_t;

enum fission_skia_text_style_flags_t {
    FISSION_SKIA_TEXT_STYLE_UNDERLINE = 1u << 0,
    FISSION_SKIA_TEXT_STYLE_HAS_LINE_HEIGHT = 1u << 1,
    FISSION_SKIA_TEXT_STYLE_HAS_BACKGROUND = 1u << 2
};

typedef struct fission_skia_text_style_run_t {
    uint32_t struct_size;
    uint32_t flags;
    fission_skia_text_range_t range;
    float font_size;
    fission_skia_rgba8_t color;
    fission_skia_utf8_slice_t font_family;
    fission_skia_utf8_slice_t locale;
    uint16_t font_weight;
    uint16_t font_slant;
    float line_height;
    float letter_spacing;
    fission_skia_rgba8_t background_color;
    float font_width;
    float word_spacing;
    const fission_skia_font_variation_t* variations;
    size_t variation_count;
    const fission_skia_font_feature_t* features;
    size_t feature_count;
} fission_skia_text_style_run_t;

typedef enum fission_skia_text_align_t {
    FISSION_SKIA_TEXT_ALIGN_LEFT = 0,
    FISSION_SKIA_TEXT_ALIGN_RIGHT = 1,
    FISSION_SKIA_TEXT_ALIGN_CENTER = 2,
    FISSION_SKIA_TEXT_ALIGN_JUSTIFY = 3,
    FISSION_SKIA_TEXT_ALIGN_START = 4,
    FISSION_SKIA_TEXT_ALIGN_END = 5
} fission_skia_text_align_t;

typedef enum fission_skia_text_overflow_t {
    FISSION_SKIA_TEXT_OVERFLOW_CLIP = 0,
    FISSION_SKIA_TEXT_OVERFLOW_ELLIPSIS = 1,
    FISSION_SKIA_TEXT_OVERFLOW_FADE = 2,
    FISSION_SKIA_TEXT_OVERFLOW_VISIBLE = 3
} fission_skia_text_overflow_t;

typedef enum fission_skia_text_direction_t {
    FISSION_SKIA_TEXT_DIRECTION_AUTO = 0,
    FISSION_SKIA_TEXT_DIRECTION_LTR = 1,
    FISSION_SKIA_TEXT_DIRECTION_RTL = 2
} fission_skia_text_direction_t;

typedef enum fission_skia_text_width_basis_t {
    FISSION_SKIA_TEXT_WIDTH_BASIS_PARENT = 0,
    FISSION_SKIA_TEXT_WIDTH_BASIS_LONGEST_LINE = 1
} fission_skia_text_width_basis_t;

enum fission_skia_paragraph_style_flags_t {
    FISSION_SKIA_PARAGRAPH_STYLE_HAS_MAX_LINES = 1u << 0,
    FISSION_SKIA_PARAGRAPH_STYLE_HAS_STRUT_HEIGHT = 1u << 1,
    FISSION_SKIA_PARAGRAPH_STYLE_APPLY_FIRST_ASCENT = 1u << 2,
    FISSION_SKIA_PARAGRAPH_STYLE_APPLY_LAST_DESCENT = 1u << 3
};

typedef struct fission_skia_paragraph_style_t {
    uint32_t struct_size;
    uint32_t flags;
    uint32_t text_align;
    uint32_t overflow;
    uint32_t text_direction;
    uint32_t text_width_basis;
    uint64_t max_lines;
    float strut_line_height;
    uint32_t reserved;
} fission_skia_paragraph_style_t;

typedef struct fission_skia_inline_object_t {
    uint32_t struct_size;
    uint32_t reserved;
    uint64_t id;
    fission_skia_text_range_t range;
    float width;
    float height;
    float baseline;
    float reserved_scalar;
} fission_skia_inline_object_t;

typedef struct fission_skia_preedit_t {
    fission_skia_text_range_t range;
    fission_skia_text_range_t selection;
} fission_skia_preedit_t;

enum fission_skia_paragraph_request_flags_t {
    FISSION_SKIA_PARAGRAPH_REQUEST_WRAP = 1u << 0,
    FISSION_SKIA_PARAGRAPH_REQUEST_HAS_WIDTH = 1u << 1,
    FISSION_SKIA_PARAGRAPH_REQUEST_HAS_SELECTION = 1u << 2,
    FISSION_SKIA_PARAGRAPH_REQUEST_HAS_PREEDIT = 1u << 3
};

typedef struct fission_skia_paragraph_request_t {
    uint32_t struct_size;
    uint32_t flags;
    fission_skia_utf8_slice_t text;
    const fission_skia_text_style_run_t* style_runs;
    size_t style_run_count;
    fission_skia_paragraph_style_t paragraph_style;
    float width_constraint;
    uint32_t reserved;
    fission_skia_utf8_slice_t locale;
    const fission_skia_inline_object_t* inline_objects;
    size_t inline_object_count;
    fission_skia_text_range_t selection;
    fission_skia_preedit_t preedit;
    uint64_t font_catalog_generation;
    const fission_skia_utf8_slice_t* fallback_families;
    size_t fallback_family_count;
} fission_skia_paragraph_request_t;

typedef enum fission_skia_paragraph_capability_t {
    FISSION_SKIA_PARAGRAPH_BIDIRECTIONAL_TEXT = UINT64_C(1) << 0,
    FISSION_SKIA_PARAGRAPH_VARIABLE_FONTS = UINT64_C(1) << 1,
    FISSION_SKIA_PARAGRAPH_FONT_FEATURES = UINT64_C(1) << 2,
    FISSION_SKIA_PARAGRAPH_INLINE_OBJECTS = UINT64_C(1) << 3,
    FISSION_SKIA_PARAGRAPH_CLUSTER_MAPPING = UINT64_C(1) << 4,
    FISSION_SKIA_PARAGRAPH_HIT_TESTING = UINT64_C(1) << 5,
    FISSION_SKIA_PARAGRAPH_CARET_GEOMETRY = UINT64_C(1) << 6,
    FISSION_SKIA_PARAGRAPH_SELECTION_GEOMETRY = UINT64_C(1) << 7,
    FISSION_SKIA_PARAGRAPH_UNRESOLVED_GLYPHS = UINT64_C(1) << 8
} fission_skia_paragraph_capability_t;

typedef enum fission_skia_index_encoding_t {
    FISSION_SKIA_INDEX_UTF8 = 0,
    FISSION_SKIA_INDEX_UTF16 = 1
} fission_skia_index_encoding_t;

typedef enum fission_skia_resolved_direction_t {
    FISSION_SKIA_DIRECTION_LTR = 0,
    FISSION_SKIA_DIRECTION_RTL = 1
} fission_skia_resolved_direction_t;

typedef enum fission_skia_affinity_t {
    FISSION_SKIA_AFFINITY_UPSTREAM = 0,
    FISSION_SKIA_AFFINITY_DOWNSTREAM = 1
} fission_skia_affinity_t;

typedef struct fission_skia_paragraph_size_t {
    float width;
    float height;
} fission_skia_paragraph_size_t;

typedef struct fission_skia_paragraph_rect_t {
    float x;
    float y;
    float width;
    float height;
} fission_skia_paragraph_rect_t;

typedef struct fission_skia_paragraph_line_t {
    fission_skia_text_range_t range;
    fission_skia_paragraph_rect_t rect;
    float baseline;
    float ascent;
    float descent;
    float leading;
    uint32_t hard_break;
    uint32_t direction;
} fission_skia_paragraph_line_t;

typedef struct fission_skia_paragraph_cluster_t {
    fission_skia_text_range_t range;
    fission_skia_paragraph_rect_t rect;
    uint64_t line_index;
    uint32_t direction;
    uint32_t starts_grapheme;
    uint32_t starts_word;
    uint32_t reserved;
} fission_skia_paragraph_cluster_t;

typedef struct fission_skia_paragraph_caret_t {
    uint64_t index;
    uint32_t affinity;
    uint32_t reserved;
    fission_skia_paragraph_rect_t rect;
    uint64_t line_index;
} fission_skia_paragraph_caret_t;

typedef struct fission_skia_paragraph_hit_region_t {
    fission_skia_paragraph_rect_t rect;
    uint64_t index;
    uint32_t affinity;
    uint32_t reserved;
    uint64_t line_index;
} fission_skia_paragraph_hit_region_t;

typedef struct fission_skia_paragraph_inline_box_t {
    uint64_t id;
    fission_skia_text_range_t range;
    fission_skia_paragraph_rect_t rect;
    float baseline;
    uint32_t reserved;
} fission_skia_paragraph_inline_box_t;

typedef struct fission_skia_unresolved_glyph_t {
    fission_skia_text_range_t range;
    uint64_t codepoint_start;
    uint64_t codepoint_count;
} fission_skia_unresolved_glyph_t;

/*
 * This view contains only C scalar records. Its pointers remain valid until
 * the matching result handle is destroyed. The caller must not concurrently
 * inspect and destroy the same raw handle. Safe Rust callers copy the records
 * into owned arrays before deterministic destruction.
 */
typedef struct fission_skia_paragraph_result_view_t {
    uint32_t struct_size;
    uint32_t index_encoding;
    uint64_t capabilities;
    fission_skia_paragraph_size_t size;
    float min_intrinsic_width;
    float max_intrinsic_width;
    float first_baseline;
    float last_baseline;
    uint32_t has_first_baseline;
    uint32_t has_last_baseline;
    const fission_skia_paragraph_line_t* lines;
    size_t line_count;
    const fission_skia_paragraph_cluster_t* clusters;
    size_t cluster_count;
    const fission_skia_paragraph_caret_t* carets;
    size_t caret_count;
    const fission_skia_paragraph_hit_region_t* hit_regions;
    size_t hit_region_count;
    const fission_skia_paragraph_inline_box_t* inline_boxes;
    size_t inline_box_count;
    const fission_skia_unresolved_glyph_t* unresolved_glyphs;
    size_t unresolved_glyph_count;
    const uint32_t* unresolved_codepoints;
    size_t unresolved_codepoint_count;
} fission_skia_paragraph_result_view_t;

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

FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_paragraph_capabilities(
    uint64_t* out_capabilities,
    fission_skia_error_t* out_error);
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_paragraph_layout(
    const fission_skia_paragraph_request_t* request,
    fission_skia_paragraph_result_handle_t* out_result,
    fission_skia_error_t* out_error);
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_paragraph_result_get_view(
    fission_skia_paragraph_result_handle_t result,
    fission_skia_paragraph_result_view_t* out_view,
    fission_skia_error_t* out_error);
FISSION_SKIA_EXPORT fission_skia_status_t fission_skia_paragraph_result_destroy(
    fission_skia_paragraph_result_handle_t result,
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
