#define FISSION_SKIA_TEST_SHIM 1
#include "fission_skia.h"
#include "fission_skia_paragraph_internal.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace {

constexpr uint64_t kCapabilities =
    FISSION_SKIA_PARAGRAPH_BIDIRECTIONAL_TEXT |
    FISSION_SKIA_PARAGRAPH_VARIABLE_FONTS |
    FISSION_SKIA_PARAGRAPH_FONT_FEATURES |
    FISSION_SKIA_PARAGRAPH_INLINE_OBJECTS |
    FISSION_SKIA_PARAGRAPH_CLUSTER_MAPPING |
    FISSION_SKIA_PARAGRAPH_HIT_TESTING |
    FISSION_SKIA_PARAGRAPH_CARET_GEOMETRY |
    FISSION_SKIA_PARAGRAPH_SELECTION_GEOMETRY |
    FISSION_SKIA_PARAGRAPH_UNRESOLVED_GLYPHS;

constexpr uint32_t kRequestFlags =
    FISSION_SKIA_PARAGRAPH_REQUEST_WRAP |
    FISSION_SKIA_PARAGRAPH_REQUEST_HAS_WIDTH |
    FISSION_SKIA_PARAGRAPH_REQUEST_HAS_SELECTION |
    FISSION_SKIA_PARAGRAPH_REQUEST_HAS_PREEDIT;
constexpr uint32_t kParagraphFlags =
    FISSION_SKIA_PARAGRAPH_STYLE_HAS_MAX_LINES |
    FISSION_SKIA_PARAGRAPH_STYLE_HAS_STRUT_HEIGHT |
    FISSION_SKIA_PARAGRAPH_STYLE_APPLY_FIRST_ASCENT |
    FISSION_SKIA_PARAGRAPH_STYLE_APPLY_LAST_DESCENT;
constexpr uint32_t kStyleFlags =
    FISSION_SKIA_TEXT_STYLE_UNDERLINE |
    FISSION_SKIA_TEXT_STYLE_HAS_LINE_HEIGHT |
    FISSION_SKIA_TEXT_STYLE_HAS_BACKGROUND;

struct Scalar {
    uint32_t value;
    size_t start;
    size_t end;
};

struct PictureRect {
    fission_skia_paragraph_rect_t rect{};
    fission_skia_color_t color{};
};

struct Result {
    size_t approximate_bytes = 0;
    std::vector<PictureRect> picture;
    fission_skia_paragraph_size_t size{};
    float min_intrinsic = 0.0f;
    float max_intrinsic = 0.0f;
    float first_baseline = 0.0f;
    float last_baseline = 0.0f;
    bool has_baseline = false;
    std::vector<fission_skia_paragraph_line_t> lines;
    std::vector<fission_skia_paragraph_cluster_t> clusters;
    std::vector<fission_skia_paragraph_caret_t> carets;
    std::vector<fission_skia_paragraph_hit_region_t> hits;
    std::vector<fission_skia_paragraph_inline_box_t> inline_boxes;
    std::vector<fission_skia_unresolved_glyph_t> unresolved;
    std::vector<uint32_t> codepoints;
};

struct State {
    std::mutex mutex;
    std::unordered_map<uint64_t, std::unique_ptr<Result>> results;
    std::atomic<uint64_t> next{1};
    std::atomic<uint64_t> errors{1};
};

State& state() {
    static State value;
    return value;
}

void copy_text(char* destination, size_t capacity, const char* source) {
    if (capacity == 0) return;
    const size_t length = std::min(std::strlen(source), capacity - 1);
    std::memcpy(destination, source, length);
    destination[length] = '\0';
}

void clear_error(fission_skia_error_t* error) {
    if (error == nullptr || error->struct_size != sizeof(*error)) return;
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
    if (error != nullptr && error->struct_size == sizeof(*error)) {
        error->code = status;
        error->sequence = state().errors.fetch_add(1, std::memory_order_relaxed);
        copy_text(error->operation, sizeof(error->operation), operation);
        copy_text(error->message, sizeof(error->message), message);
    }
    return status;
}

bool pointer_count(const void* pointer, size_t count) {
    return count == 0 || pointer != nullptr;
}

bool decode(const fission_skia_utf8_slice_t& text, std::vector<Scalar>* output) {
    if (!pointer_count(text.data, text.length)) return false;
    output->clear();
    size_t offset = 0;
    while (offset < text.length) {
        const size_t start = offset;
        const uint8_t first = text.data[offset++];
        uint32_t value = 0;
        size_t count = 0;
        if (first <= 0x7f) {
            value = first;
        } else if (first >= 0xc2 && first <= 0xdf) {
            value = first & 0x1f;
            count = 1;
        } else if (first >= 0xe0 && first <= 0xef) {
            value = first & 0x0f;
            count = 2;
        } else if (first >= 0xf0 && first <= 0xf4) {
            value = first & 0x07;
            count = 3;
        } else {
            return false;
        }
        if (count > text.length - offset) return false;
        for (size_t index = 0; index < count; ++index) {
            const uint8_t next = text.data[offset++];
            if ((next & 0xc0) != 0x80) return false;
            value = (value << 6) | (next & 0x3f);
        }
        if ((count == 2 && value < 0x800) || (count == 3 && value < 0x10000) ||
            value > 0x10ffff || (value >= 0xd800 && value <= 0xdfff)) {
            return false;
        }
        output->push_back({value, start, offset});
    }
    return true;
}

bool boundary(const std::vector<Scalar>& scalars, size_t text_length, uint64_t value) {
    if (value == 0 || value == text_length) return true;
    return std::any_of(scalars.begin(), scalars.end(),
                       [&](const Scalar& scalar) { return scalar.start == value; });
}

bool valid_range(
    const fission_skia_text_range_t& range,
    const std::vector<Scalar>& scalars,
    size_t text_length) {
    return range.start <= range.end && range.end <= text_length &&
           boundary(scalars, text_length, range.start) &&
           boundary(scalars, text_length, range.end);
}

bool valid_string(const fission_skia_utf8_slice_t& value, bool allow_empty) {
    if (!allow_empty && value.length == 0) return false;
    if (!pointer_count(value.data, value.length) ||
        (value.length != 0 && std::memchr(value.data, 0, value.length) != nullptr)) {
        return false;
    }
    std::vector<Scalar> decoded;
    return decode(value, &decoded);
}

bool zero_range(const fission_skia_text_range_t& range) {
    return range.start == 0 && range.end == 0;
}

bool zero_color(const fission_skia_rgba8_t& color) {
    return color.red == 0 && color.green == 0 && color.blue == 0 && color.alpha == 0;
}

fission_skia_status_t validate(
    const fission_skia_paragraph_request_t* request,
    std::vector<Scalar>* scalars,
    fission_skia_error_t* error) {
    if (request == nullptr || request->struct_size != sizeof(*request) ||
        request->reserved != 0 || (request->flags & ~kRequestFlags) != 0 ||
        !pointer_count(request->style_runs, request->style_run_count) ||
        !pointer_count(request->inline_objects, request->inline_object_count) ||
        !pointer_count(request->fallback_families, request->fallback_family_count) ||
        !decode(request->text, scalars) || !valid_string(request->locale, true)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "invalid test paragraph request layout or UTF-8", error);
    }
    if (request->font_catalog_generation != 0) {
        return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "paragraph_layout",
                    "test shim rejects nonzero font catalog generations", error);
    }
    const auto& paragraph = request->paragraph_style;
    if (paragraph.struct_size != sizeof(paragraph) || paragraph.reserved != 0 ||
        (paragraph.flags & ~kParagraphFlags) != 0 ||
        paragraph.text_align > FISSION_SKIA_TEXT_ALIGN_END ||
        paragraph.overflow > FISSION_SKIA_TEXT_OVERFLOW_VISIBLE ||
        paragraph.text_direction > FISSION_SKIA_TEXT_DIRECTION_RTL ||
        paragraph.text_width_basis > FISSION_SKIA_TEXT_WIDTH_BASIS_LONGEST_LINE ||
        paragraph.overflow == FISSION_SKIA_TEXT_OVERFLOW_FADE) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "invalid or unsupported test paragraph style", error);
    }
    const bool max_lines =
        (paragraph.flags & FISSION_SKIA_PARAGRAPH_STYLE_HAS_MAX_LINES) != 0;
    const bool strut =
        (paragraph.flags & FISSION_SKIA_PARAGRAPH_STYLE_HAS_STRUT_HEIGHT) != 0;
    if ((!max_lines && paragraph.max_lines != 0) || (max_lines && paragraph.max_lines == 0) ||
        (!strut && paragraph.strut_line_height != 0.0f) ||
        (strut && (!std::isfinite(paragraph.strut_line_height) ||
                   paragraph.strut_line_height <= 0.0f)) ||
        (paragraph.overflow == FISSION_SKIA_TEXT_OVERFLOW_VISIBLE && max_lines)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "test paragraph option flags disagree with their payload", error);
    }
    const bool width = (request->flags & FISSION_SKIA_PARAGRAPH_REQUEST_HAS_WIDTH) != 0;
    if ((!width && request->width_constraint != 0.0f) ||
        (width && (!std::isfinite(request->width_constraint) ||
                   request->width_constraint < 0.0f))) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "invalid test width option", error);
    }
    if (((request->flags & FISSION_SKIA_PARAGRAPH_REQUEST_HAS_SELECTION) == 0 &&
         !zero_range(request->selection)) ||
        ((request->flags & FISSION_SKIA_PARAGRAPH_REQUEST_HAS_SELECTION) != 0 &&
         !valid_range(request->selection, *scalars, request->text.length))) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "invalid test selection range", error);
    }
    if ((request->flags & FISSION_SKIA_PARAGRAPH_REQUEST_HAS_PREEDIT) == 0) {
        if (!zero_range(request->preedit.range) || !zero_range(request->preedit.selection)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "absent test preedit has nonzero payload", error);
        }
    } else if (!valid_range(request->preedit.range, *scalars, request->text.length) ||
               !valid_range(request->preedit.selection, *scalars, request->text.length) ||
               request->preedit.selection.start < request->preedit.range.start ||
               request->preedit.selection.end > request->preedit.range.end) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "invalid test preedit range", error);
    }

    uint64_t covered = 0;
    if (request->text.length != 0 && request->style_run_count == 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "test text requires style coverage", error);
    }
    for (size_t index = 0; index < request->style_run_count; ++index) {
        const auto& run = request->style_runs[index];
        if (run.struct_size != sizeof(run) || (run.flags & ~kStyleFlags) != 0 ||
            run.range.start != covered ||
            !valid_range(run.range, *scalars, request->text.length) ||
            !std::isfinite(run.font_size) || run.font_size <= 0.0f ||
            run.font_weight == 0 || run.font_weight > 1000 ||
            run.font_slant > FISSION_SKIA_FONT_SLANT_ITALIC ||
            !std::isfinite(run.font_width) || run.font_width <= 0.0f ||
            !std::isfinite(run.letter_spacing) || !std::isfinite(run.word_spacing) ||
            !valid_string(run.font_family, true) || !valid_string(run.locale, true) ||
            !pointer_count(run.variations, run.variation_count) ||
            !pointer_count(run.features, run.feature_count)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "invalid test text style run", error);
        }
        const bool line_height = (run.flags & FISSION_SKIA_TEXT_STYLE_HAS_LINE_HEIGHT) != 0;
        const bool background = (run.flags & FISSION_SKIA_TEXT_STYLE_HAS_BACKGROUND) != 0;
        if ((!line_height && run.line_height != 0.0f) ||
            (line_height && (!std::isfinite(run.line_height) || run.line_height <= 0.0f)) ||
            (!background && !zero_color(run.background_color))) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "test style option payload disagrees with flags", error);
        }
        for (size_t item = 0; item < run.variation_count; ++item) {
            if (!std::isfinite(run.variations[item].value)) {
                return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                            "non-finite test variation", error);
            }
        }
        covered = run.range.end;
    }
    if (covered != request->text.length ||
        (request->text.length == 0 && request->style_run_count > 1)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "test style runs do not cover source", error);
    }
    for (size_t index = 0; index < request->fallback_family_count; ++index) {
        if (!valid_string(request->fallback_families[index], false)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "invalid test fallback family", error);
        }
    }
    std::unordered_set<uint64_t> ids;
    uint64_t previous_end = 0;
    for (size_t index = 0; index < request->inline_object_count; ++index) {
        const auto& object = request->inline_objects[index];
        if (object.struct_size != sizeof(object) || object.reserved != 0 ||
            object.reserved_scalar != 0.0f ||
            !valid_range(object.range, *scalars, request->text.length) ||
            object.range.start == object.range.end ||
            (index != 0 && object.range.start < previous_end) ||
            !ids.insert(object.id).second || !std::isfinite(object.width) || object.width < 0.0f ||
            !std::isfinite(object.height) || object.height < 0.0f ||
            !std::isfinite(object.baseline) || object.baseline < 0.0f ||
            object.baseline > object.height) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "invalid test inline object", error);
        }
        const size_t start = static_cast<size_t>(object.range.start);
        if (object.range.end - object.range.start != 3 ||
            request->text.data[start] != 0xef || request->text.data[start + 1] != 0xbf ||
            request->text.data[start + 2] != 0xbc) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "test inline object does not cover U+FFFC", error);
        }
        previous_end = object.range.end;
    }
    return FISSION_SKIA_STATUS_OK;
}

bool combining(uint32_t value) {
    return (value >= 0x0300 && value <= 0x036f) ||
           (value >= 0x1ab0 && value <= 0x1aff) ||
           (value >= 0x1dc0 && value <= 0x1dff) ||
           (value >= 0xfe00 && value <= 0xfe0f) ||
           (value >= 0x1f3fb && value <= 0x1f3ff) ||
           (value >= 0xfe20 && value <= 0xfe2f);
}

bool rtl(uint32_t value) {
    return (value >= 0x0590 && value <= 0x08ff) ||
           (value >= 0xfb1d && value <= 0xfdff) ||
           (value >= 0xfe70 && value <= 0xfeff);
}

const fission_skia_inline_object_t* inline_at(
    const fission_skia_paragraph_request_t& request,
    size_t start,
    size_t end) {
    for (size_t index = 0; index < request.inline_object_count; ++index) {
        const auto& object = request.inline_objects[index];
        if (object.range.start == start && object.range.end == end) return &object;
    }
    return nullptr;
}

const fission_skia_text_style_run_t* style_at(
    const fission_skia_paragraph_request_t& request,
    size_t byte_index) {
    for (size_t index = 0; index < request.style_run_count; ++index) {
        const auto& style = request.style_runs[index];
        if (style.range.start <= byte_index && byte_index < style.range.end) return &style;
    }
    return nullptr;
}

fission_skia_color_t color_from_rgba8(const fission_skia_rgba8_t& color) {
    constexpr float kChannelScale = 1.0f / 255.0f;
    return {
        color.red * kChannelScale,
        color.green * kChannelScale,
        color.blue * kChannelScale,
        color.alpha * kChannelScale,
    };
}

std::unique_ptr<Result> shape(
    const fission_skia_paragraph_request_t& request,
    const std::vector<Scalar>& scalars) {
    std::unique_ptr<Result> result(new (std::nothrow) Result());
    if (result == nullptr) return nullptr;
    std::vector<std::pair<size_t, size_t>> graphemes;
    for (const auto& scalar : scalars) {
        if (graphemes.empty() ||
            (!combining(scalar.value) && scalar.value != 0x200d &&
             scalars[&scalar - scalars.data() - 1].value != 0x200d)) {
            graphemes.emplace_back(scalar.start, scalar.end);
        } else {
            graphemes.back().second = scalar.end;
        }
    }

    uint64_t line_start = 0;
    size_t line_index = 0;
    float x = 0.0f;
    float y = 0.0f;
    float max_width = 0.0f;
    auto finish_line = [&](uint64_t end, bool hard_break) {
        result->lines.push_back({
            {line_start, end},
            {0.0f, y, x, 20.0f},
            y + 15.0f,
            12.0f,
            4.0f,
            4.0f,
            hard_break ? 1u : 0u,
            request.paragraph_style.text_direction == FISSION_SKIA_TEXT_DIRECTION_RTL
                ? FISSION_SKIA_DIRECTION_RTL
                : FISSION_SKIA_DIRECTION_LTR,
        });
        max_width = std::max(max_width, x);
        line_start = end;
        x = 0.0f;
        y += 20.0f;
        ++line_index;
    };

    for (const auto& range : graphemes) {
        const Scalar* first = nullptr;
        for (const auto& scalar : scalars) {
            if (scalar.start == range.first) {
                first = &scalar;
                break;
            }
        }
        if (first == nullptr) continue;
        if (first->value == '\n') {
            finish_line(range.second, true);
            continue;
        }
        const auto* object = inline_at(request, range.first, range.second);
        const float width = object == nullptr ? 10.0f : object->width;
        const float height = object == nullptr ? 20.0f : object->height;
        const uint32_t direction = rtl(first->value) ? FISSION_SKIA_DIRECTION_RTL
                                                     : FISSION_SKIA_DIRECTION_LTR;
        if (object == nullptr) {
            result->clusters.push_back({
                {range.first, range.second},
                {x, y, width, height},
                line_index,
                direction,
                1,
                1,
                0,
            });
            const auto* style = style_at(request, range.first);
            if (style == nullptr) return nullptr;
            result->picture.push_back({
                {x, y, width, height},
                color_from_rgba8(style->color),
            });
        } else {
            result->inline_boxes.push_back({
                object->id,
                object->range,
                {x, y, width, height},
                object->baseline,
                0,
            });
        }
        const float start_x = direction == FISSION_SKIA_DIRECTION_RTL ? x + width : x;
        const float end_x = direction == FISSION_SKIA_DIRECTION_RTL ? x : x + width;
        result->carets.push_back({
            range.first, FISSION_SKIA_AFFINITY_DOWNSTREAM, 0,
            {start_x, y, 1.0f, std::max(1.0f, height)}, line_index,
        });
        result->carets.push_back({
            range.second, FISSION_SKIA_AFFINITY_UPSTREAM, 0,
            {end_x, y, 1.0f, std::max(1.0f, height)}, line_index,
        });
        result->hits.push_back({
            {x, y, std::max(1.0f, width), std::max(1.0f, height)},
            range.first,
            FISSION_SKIA_AFFINITY_DOWNSTREAM,
            0,
            line_index,
        });
        x += width;
    }
    if (result->lines.empty() || line_start < request.text.length ||
        (!scalars.empty() && scalars.back().value == '\n')) {
        finish_line(request.text.length, false);
    }
    for (size_t index = 0; index < result->lines.size(); ++index) {
        const bool caret = std::any_of(result->carets.begin(), result->carets.end(),
                                       [&](const auto& value) { return value.line_index == index; });
        if (!caret) {
            const auto& line = result->lines[index];
            result->carets.push_back({
                line.range.start, FISSION_SKIA_AFFINITY_DOWNSTREAM, 0,
                {line.rect.x, line.rect.y, 1.0f, std::max(1.0f, line.rect.height)}, index,
            });
            result->hits.push_back({
                {line.rect.x, line.rect.y, 1.0f, std::max(1.0f, line.rect.height)},
                line.range.start, FISSION_SKIA_AFFINITY_DOWNSTREAM, 0, index,
            });
        }
    }
    result->min_intrinsic = max_width;
    result->max_intrinsic = max_width;
    result->size.width =
        request.paragraph_style.text_width_basis == FISSION_SKIA_TEXT_WIDTH_BASIS_PARENT &&
                (request.flags & FISSION_SKIA_PARAGRAPH_REQUEST_HAS_WIDTH) != 0
            ? request.width_constraint
            : max_width;
    result->size.height = result->lines.size() * 20.0f;
    if (!result->lines.empty()) {
        result->has_baseline = true;
        result->first_baseline = result->lines.front().baseline;
        result->last_baseline = result->lines.back().baseline;
    }
    result->approximate_bytes = result->picture.size() * sizeof(PictureRect);
    return result;
}

template <typename T>
const T* data(const std::vector<T>& values) {
    return values.empty() ? nullptr : values.data();
}

}  // namespace

extern "C" {

fission_skia_status_t fission_skia_paragraph_capabilities(
    uint64_t* out_capabilities,
    fission_skia_error_t* out_error) {
    if (out_capabilities == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_capabilities",
                    "null test capability output", out_error);
    }
    *out_capabilities = kCapabilities;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_layout(
    const fission_skia_paragraph_request_t* request,
    fission_skia_paragraph_result_handle_t* out_result,
    fission_skia_error_t* out_error) {
    if (out_result == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "null test result output", out_error);
    }
    *out_result = 0;
    std::vector<Scalar> scalars;
    const auto status = validate(request, &scalars, out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    std::unique_ptr<Result> result = shape(*request, scalars);
    if (result == nullptr) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "paragraph_layout",
                    "test result allocation failed", out_error);
    }
    uint64_t handle = 0;
    {
        std::lock_guard<std::mutex> lock(state().mutex);
        do {
            handle = state().next.fetch_add(1, std::memory_order_relaxed);
        } while (handle == 0 || state().results.find(handle) != state().results.end());
        state().results.emplace(handle, std::move(result));
    }
    *out_result = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_result_get_view(
    fission_skia_paragraph_result_handle_t result,
    fission_skia_paragraph_result_view_t* out_view,
    fission_skia_error_t* out_error) {
    if (out_view == nullptr || out_view->struct_size != sizeof(*out_view)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_result_get_view",
                    "invalid test result view", out_error);
    }
    std::lock_guard<std::mutex> lock(state().mutex);
    const auto found = state().results.find(result);
    if (found == state().results.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "paragraph_result_get_view",
                    "invalid test paragraph result", out_error);
    }
    const Result& value = *found->second;
    *out_view = {
        sizeof(*out_view),
        FISSION_SKIA_INDEX_UTF8,
        kCapabilities,
        value.size,
        value.min_intrinsic,
        value.max_intrinsic,
        value.first_baseline,
        value.last_baseline,
        value.has_baseline ? 1u : 0u,
        value.has_baseline ? 1u : 0u,
        data(value.lines),
        value.lines.size(),
        data(value.clusters),
        value.clusters.size(),
        data(value.carets),
        value.carets.size(),
        data(value.hits),
        value.hits.size(),
        data(value.inline_boxes),
        value.inline_boxes.size(),
        data(value.unresolved),
        value.unresolved.size(),
        data(value.codepoints),
        value.codepoints.size(),
    };
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_result_get_approximate_bytes(
    fission_skia_paragraph_result_handle_t result,
    size_t* out_approximate_bytes,
    fission_skia_error_t* out_error) {
    if (out_approximate_bytes == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT,
                    "paragraph_result_get_approximate_bytes",
                    "null test approximate byte output", out_error);
    }
    *out_approximate_bytes = 0;
    std::lock_guard<std::mutex> lock(state().mutex);
    const auto found = state().results.find(result);
    if (found == state().results.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE,
                    "paragraph_result_get_approximate_bytes",
                    "invalid test paragraph result", out_error);
    }
    *out_approximate_bytes = found->second->approximate_bytes;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_result_destroy(
    fission_skia_paragraph_result_handle_t result,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(state().mutex);
    const auto found = state().results.find(result);
    if (found == state().results.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "paragraph_result_destroy",
                    "invalid test paragraph result", out_error);
    }
    state().results.erase(found);
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

}  // extern "C"

fission_skia_status_t fission_skia_paragraph_validate_draw(
    fission_skia_paragraph_result_handle_t result,
    float x,
    float y,
    float scale_factor,
    fission_skia_error_t* out_error) {
    if (!std::isfinite(x) || !std::isfinite(y) || !std::isfinite(scale_factor) ||
        scale_factor <= 0.0f) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "draw_paragraph",
                    "invalid test paragraph origin or scale factor", out_error);
    }
    std::lock_guard<std::mutex> lock(state().mutex);
    if (state().results.find(result) == state().results.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "draw_paragraph",
                    "test paragraph draw handle is not live", out_error);
    }
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_draw_test_picture(
    fission_skia_paragraph_result_handle_t result,
    float x,
    float y,
    float scale_factor,
    void* context,
    fission_skia_test_paragraph_rect_callback_t draw_rect,
    fission_skia_error_t* out_error) {
    if (context == nullptr || draw_rect == nullptr || !std::isfinite(x) ||
        !std::isfinite(y) || !std::isfinite(scale_factor) || scale_factor <= 0.0f) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "draw_paragraph",
                    "invalid test paragraph playback arguments", out_error);
    }
    std::vector<PictureRect> picture;
    {
        std::lock_guard<std::mutex> lock(state().mutex);
        const auto found = state().results.find(result);
        if (found == state().results.end()) {
            return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "draw_paragraph",
                        "test paragraph draw handle is not live", out_error);
        }
        picture = found->second->picture;
    }
    for (const auto& command : picture) {
        const fission_skia_paragraph_rect_t rect = {
            x + command.rect.x * scale_factor,
            y + command.rect.y * scale_factor,
            command.rect.width * scale_factor,
            command.rect.height * scale_factor,
        };
        draw_rect(context, rect, command.color);
    }
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}
