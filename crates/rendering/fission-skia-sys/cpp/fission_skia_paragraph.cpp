#include "fission_skia.h"
#include "fission_skia_paragraph_internal.h"

#include "include/core/SkCanvas.h"
#include "include/core/SkColor.h"
#include "include/core/SkData.h"
#include "include/core/SkFont.h"
#include "include/core/SkFontArguments.h"
#include "include/core/SkFontMgr.h"
#include "include/core/SkFontStyle.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPicture.h"
#include "include/core/SkPictureRecorder.h"
#include "include/core/SkString.h"
#include "include/core/SkTypeface.h"
#include "include/ports/SkFontScanner_FreeType.h"
#include "modules/skparagraph/include/FontCollection.h"
#include "modules/skparagraph/include/Metrics.h"
#include "modules/skparagraph/include/Paragraph.h"
#include "modules/skparagraph/include/ParagraphBuilder.h"
#include "modules/skparagraph/include/ParagraphStyle.h"
#include "modules/skparagraph/include/TextStyle.h"
#include "modules/skparagraph/include/TypefaceFontProvider.h"
#include "modules/skunicode/include/SkUnicode.h"
#include "modules/skunicode/include/SkUnicode_icu.h"

#if defined(__APPLE__)
#include "include/ports/SkFontMgr_mac_ct.h"
#elif defined(_WIN32)
#include "include/ports/SkTypeface_win.h"
#elif defined(__ANDROID__)
#include "include/ports/SkFontMgr_android.h"
#elif defined(__linux__)
#include "include/ports/SkFontMgr_fontconfig.h"
#endif

#ifndef U_DISABLE_RENAMING
#define U_DISABLE_RENAMING 1
#endif
#include <unicode/ubidi.h>

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
#include <optional>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

namespace {

using skia::textlayout::Affinity;
using skia::textlayout::FontCollection;
using skia::textlayout::LineMetrics;
using skia::textlayout::Paragraph;
using skia::textlayout::ParagraphBuilder;
using skia::textlayout::ParagraphStyle;
using skia::textlayout::PlaceholderAlignment;
using skia::textlayout::PlaceholderStyle;
using skia::textlayout::RectHeightStyle;
using skia::textlayout::RectWidthStyle;
using skia::textlayout::TextAlign;
using skia::textlayout::TextBaseline;
using skia::textlayout::TextBox;
using skia::textlayout::TextDirection;
using skia::textlayout::TextHeightBehavior;
using skia::textlayout::TextStyle;

constexpr uint64_t kParagraphCapabilities =
    FISSION_SKIA_PARAGRAPH_BIDIRECTIONAL_TEXT |
    FISSION_SKIA_PARAGRAPH_VARIABLE_FONTS |
    FISSION_SKIA_PARAGRAPH_FONT_FEATURES |
    FISSION_SKIA_PARAGRAPH_INLINE_OBJECTS |
    FISSION_SKIA_PARAGRAPH_CLUSTER_MAPPING |
    FISSION_SKIA_PARAGRAPH_HIT_TESTING |
    FISSION_SKIA_PARAGRAPH_CARET_GEOMETRY |
    FISSION_SKIA_PARAGRAPH_SELECTION_GEOMETRY |
    FISSION_SKIA_PARAGRAPH_UNRESOLVED_GLYPHS;

constexpr uint32_t kKnownRequestFlags =
    FISSION_SKIA_PARAGRAPH_REQUEST_WRAP |
    FISSION_SKIA_PARAGRAPH_REQUEST_HAS_WIDTH |
    FISSION_SKIA_PARAGRAPH_REQUEST_HAS_SELECTION |
    FISSION_SKIA_PARAGRAPH_REQUEST_HAS_PREEDIT;
constexpr uint32_t kKnownParagraphStyleFlags =
    FISSION_SKIA_PARAGRAPH_STYLE_HAS_MAX_LINES |
    FISSION_SKIA_PARAGRAPH_STYLE_HAS_STRUT_HEIGHT |
    FISSION_SKIA_PARAGRAPH_STYLE_APPLY_FIRST_ASCENT |
    FISSION_SKIA_PARAGRAPH_STYLE_APPLY_LAST_DESCENT;
constexpr uint32_t kKnownTextStyleFlags =
    FISSION_SKIA_TEXT_STYLE_UNDERLINE |
    FISSION_SKIA_TEXT_STYLE_HAS_LINE_HEIGHT |
    FISSION_SKIA_TEXT_STYLE_HAS_BACKGROUND;
constexpr size_t kMaxFontCatalogFaces = 4096;
constexpr size_t kMaxFontCatalogBytes = 512 * 1024 * 1024;
constexpr size_t kMaxFontFaceAxes = 256;

struct DecodedScalar {
    uint32_t value;
    size_t start;
    size_t end;
    size_t utf16_start;
    size_t utf16_end;
};

struct ParagraphResultState {
    sk_sp<SkPicture> picture;
    size_t approximate_bytes = 0;
    fission_skia_paragraph_size_t size{};
    float min_intrinsic_width = 0.0f;
    float max_intrinsic_width = 0.0f;
    float first_baseline = 0.0f;
    float last_baseline = 0.0f;
    bool has_first_baseline = false;
    bool has_last_baseline = false;
    std::vector<fission_skia_paragraph_line_t> lines;
    std::vector<fission_skia_paragraph_cluster_t> clusters;
    std::vector<fission_skia_paragraph_caret_t> carets;
    std::vector<fission_skia_paragraph_hit_region_t> hit_regions;
    std::vector<fission_skia_paragraph_inline_box_t> inline_boxes;
    std::vector<fission_skia_unresolved_glyph_t> unresolved_glyphs;
    std::vector<uint32_t> unresolved_codepoints;
    sk_sp<FontCollection> font_collection;
};

struct ParagraphRegistry {
    std::mutex mutex;
    std::unordered_map<uint64_t, std::unique_ptr<ParagraphResultState>> results;
    std::unordered_map<uint64_t, sk_sp<FontCollection>> font_catalogs;
    std::atomic<uint64_t> next_handle{1};
    std::atomic<uint64_t> next_font_catalog{1};
    std::atomic<uint64_t> next_error{1};
};

ParagraphRegistry& paragraph_registry() {
    static ParagraphRegistry value;
    return value;
}

void copy_text(char* destination, size_t capacity, const char* source) {
    if (capacity == 0) return;
    const size_t source_length = std::strlen(source);
    const size_t length = std::min(source_length, capacity - 1);
    std::memcpy(destination, source, length);
    destination[length] = '\0';
    if (length + 1 < capacity) {
        std::memset(destination + length + 1, 0, capacity - length - 1);
    }
}

void clear_error(fission_skia_error_t* error) {
    if (error == nullptr || error->struct_size != sizeof(*error)) return;
    error->code = FISSION_SKIA_STATUS_OK;
    error->sequence = 0;
    std::memset(error->operation, 0, sizeof(error->operation));
    std::memset(error->message, 0, sizeof(error->message));
}

fission_skia_status_t fail(
    fission_skia_status_t status,
    const char* operation,
    const char* message,
    fission_skia_error_t* error) {
    if (error != nullptr && error->struct_size == sizeof(*error)) {
        error->code = status;
        error->sequence =
            paragraph_registry().next_error.fetch_add(1, std::memory_order_relaxed);
        copy_text(error->operation, sizeof(error->operation), operation);
        copy_text(error->message, sizeof(error->message), message);
    }
    return status;
}

bool finite(float value) {
    return std::isfinite(value);
}

bool valid_pointer_count(const void* pointer, size_t count) {
    return count == 0 || pointer != nullptr;
}

bool valid_utf8_slice_shape(const fission_skia_utf8_slice_t& value) {
    return valid_pointer_count(value.data, value.length);
}

bool zero_range(const fission_skia_text_range_t& range) {
    return range.start == 0 && range.end == 0;
}

bool zero_color(const fission_skia_rgba8_t& color) {
    return color.red == 0 && color.green == 0 && color.blue == 0 && color.alpha == 0;
}

bool decode_utf8(
    const uint8_t* data,
    size_t length,
    std::vector<DecodedScalar>* scalars,
    std::vector<size_t>* utf16_for_utf8) {
    scalars->clear();
    utf16_for_utf8->assign(length + 1, std::numeric_limits<size_t>::max());
    (*utf16_for_utf8)[0] = 0;
    size_t offset = 0;
    size_t utf16_offset = 0;
    while (offset < length) {
        const size_t start = offset;
        const uint8_t first = data[offset++];
        uint32_t value = 0;
        size_t continuation_count = 0;
        if (first <= 0x7f) {
            value = first;
        } else if (first >= 0xc2 && first <= 0xdf) {
            value = first & 0x1f;
            continuation_count = 1;
        } else if (first >= 0xe0 && first <= 0xef) {
            value = first & 0x0f;
            continuation_count = 2;
        } else if (first >= 0xf0 && first <= 0xf4) {
            value = first & 0x07;
            continuation_count = 3;
        } else {
            return false;
        }
        if (continuation_count > length - offset) return false;
        for (size_t index = 0; index < continuation_count; ++index) {
            const uint8_t next = data[offset++];
            if ((next & 0xc0) != 0x80) return false;
            value = (value << 6) | (next & 0x3f);
        }
        if ((continuation_count == 2 && value < 0x800) ||
            (continuation_count == 3 && value < 0x10000) ||
            value > 0x10ffff || (value >= 0xd800 && value <= 0xdfff)) {
            return false;
        }
        const size_t units = value > 0xffff ? 2 : 1;
        (*utf16_for_utf8)[start] = utf16_offset;
        if (utf16_offset > std::numeric_limits<size_t>::max() - units) return false;
        const size_t next_utf16 = utf16_offset + units;
        (*utf16_for_utf8)[offset] = next_utf16;
        scalars->push_back({value, start, offset, utf16_offset, next_utf16});
        utf16_offset = next_utf16;
    }
    return true;
}

bool valid_text_range(
    const fission_skia_text_range_t& range,
    size_t text_length,
    const std::vector<size_t>& utf16_for_utf8) {
    if (range.start > range.end || range.end > text_length) return false;
    const size_t start = static_cast<size_t>(range.start);
    const size_t end = static_cast<size_t>(range.end);
    return utf16_for_utf8[start] != std::numeric_limits<size_t>::max() &&
           utf16_for_utf8[end] != std::numeric_limits<size_t>::max();
}

bool utf8_offset_for_utf16(
    const std::vector<DecodedScalar>& scalars,
    size_t utf16_offset,
    size_t text_length,
    size_t* output) {
    if (output == nullptr) return false;
    if (utf16_offset == 0) {
        *output = 0;
        return true;
    }
    for (const auto& scalar : scalars) {
        if (scalar.utf16_start == utf16_offset) {
            *output = scalar.start;
            return true;
        }
        if (scalar.utf16_end == utf16_offset) {
            *output = scalar.end;
            return true;
        }
        if (scalar.utf16_start < utf16_offset && utf16_offset < scalar.utf16_end) {
            return false;
        }
    }
    if (scalars.empty() && utf16_offset == 0) {
        *output = text_length;
        return true;
    }
    return false;
}

bool valid_string_slice(
    const fission_skia_utf8_slice_t& value,
    bool permit_empty,
    bool permit_embedded_nul) {
    if (!valid_utf8_slice_shape(value)) return false;
    if (!permit_empty && value.length == 0) return false;
    if (!permit_embedded_nul && value.length != 0 &&
        std::memchr(value.data, 0, value.length) != nullptr) {
        return false;
    }
    std::vector<DecodedScalar> scalars;
    std::vector<size_t> mapping;
    return decode_utf8(value.data, value.length, &scalars, &mapping);
}

std::string string_from(const fission_skia_utf8_slice_t& value) {
    if (value.length == 0) return {};
    return std::string(reinterpret_cast<const char*>(value.data), value.length);
}

bool is_placeholder_range(
    const fission_skia_paragraph_request_t& request,
    const fission_skia_text_range_t& range) {
    for (size_t index = 0; index < request.inline_object_count; ++index) {
        const auto& inline_object = request.inline_objects[index];
        if (inline_object.range.start == range.start && inline_object.range.end == range.end) {
            return true;
        }
    }
    return false;
}

fission_skia_status_t validate_request(
    const fission_skia_paragraph_request_t* request,
    std::vector<DecodedScalar>* scalars,
    std::vector<size_t>* utf16_for_utf8,
    fission_skia_error_t* error) {
    if (request == nullptr || request->struct_size != sizeof(*request) ||
        request->reserved != 0 || (request->flags & ~kKnownRequestFlags) != 0 ||
        !valid_utf8_slice_shape(request->text) ||
        !valid_pointer_count(request->style_runs, request->style_run_count) ||
        !valid_pointer_count(request->inline_objects, request->inline_object_count) ||
        !valid_pointer_count(request->fallback_families, request->fallback_family_count)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "paragraph request layout, flags, or pointer/count pair is invalid", error);
    }
    if (request->text.length > static_cast<size_t>(std::numeric_limits<int>::max()) ||
        !decode_utf8(request->text.data, request->text.length, scalars, utf16_for_utf8)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "paragraph text must be valid UTF-8 within SkParagraph's length range", error);
    }
    if (!valid_string_slice(request->locale, true, false)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "paragraph locale must be a valid NUL-free UTF-8 string", error);
    }
    for (size_t index = 0; index < request->fallback_family_count; ++index) {
        if (!valid_string_slice(request->fallback_families[index], false, false)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "fallback font families must be nonempty NUL-free UTF-8 strings", error);
        }
    }

    const auto& paragraph = request->paragraph_style;
    if (paragraph.struct_size != sizeof(paragraph) || paragraph.reserved != 0 ||
        (paragraph.flags & ~kKnownParagraphStyleFlags) != 0 ||
        paragraph.text_align > FISSION_SKIA_TEXT_ALIGN_END ||
        paragraph.overflow > FISSION_SKIA_TEXT_OVERFLOW_VISIBLE ||
        paragraph.text_direction > FISSION_SKIA_TEXT_DIRECTION_RTL ||
        paragraph.text_width_basis > FISSION_SKIA_TEXT_WIDTH_BASIS_LONGEST_LINE) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "paragraph style layout, flags, or enum value is invalid", error);
    }
    const bool has_max_lines =
        (paragraph.flags & FISSION_SKIA_PARAGRAPH_STYLE_HAS_MAX_LINES) != 0;
    const bool has_strut =
        (paragraph.flags & FISSION_SKIA_PARAGRAPH_STYLE_HAS_STRUT_HEIGHT) != 0;
    if ((!has_max_lines && paragraph.max_lines != 0) ||
        (has_max_lines && (paragraph.max_lines == 0 ||
                           paragraph.max_lines > std::numeric_limits<size_t>::max())) ||
        (!has_strut && paragraph.strut_line_height != 0.0f) ||
        (has_strut && (!finite(paragraph.strut_line_height) ||
                       paragraph.strut_line_height <= 0.0f))) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "paragraph style option payload does not match its presence flags", error);
    }
    if (paragraph.overflow == FISSION_SKIA_TEXT_OVERFLOW_FADE ||
        (paragraph.overflow == FISSION_SKIA_TEXT_OVERFLOW_VISIBLE && has_max_lines)) {
        return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "paragraph_layout",
                    "SkParagraph cannot represent the requested overflow policy exactly", error);
    }
    const bool has_width =
        (request->flags & FISSION_SKIA_PARAGRAPH_REQUEST_HAS_WIDTH) != 0;
    if ((!has_width && request->width_constraint != 0.0f) ||
        (has_width && (!finite(request->width_constraint) || request->width_constraint < 0.0f))) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "width constraint does not match its presence flag", error);
    }
    if ((request->flags & FISSION_SKIA_PARAGRAPH_REQUEST_HAS_SELECTION) == 0) {
        if (!zero_range(request->selection)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "absent selection must have a zero payload", error);
        }
    } else if (!valid_text_range(request->selection, request->text.length, *utf16_for_utf8)) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "selection is not a valid source UTF-8 range", error);
    }
    if ((request->flags & FISSION_SKIA_PARAGRAPH_REQUEST_HAS_PREEDIT) == 0) {
        if (!zero_range(request->preedit.range) || !zero_range(request->preedit.selection)) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "absent preedit must have a zero payload", error);
        }
    } else if (!valid_text_range(request->preedit.range, request->text.length,
                                 *utf16_for_utf8) ||
               !valid_text_range(request->preedit.selection, request->text.length,
                                 *utf16_for_utf8) ||
               request->preedit.selection.start < request->preedit.range.start ||
               request->preedit.selection.end > request->preedit.range.end) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "preedit ranges are invalid or the selection is not contained", error);
    }

    uint64_t covered = 0;
    if (request->text.length != 0 && request->style_run_count == 0) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "nonempty text requires contiguous style runs", error);
    }
    if (request->text.length == 0 && request->style_run_count > 1) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "empty text accepts at most one empty style run", error);
    }
    for (size_t index = 0; index < request->style_run_count; ++index) {
        const auto& run = request->style_runs[index];
        if (run.struct_size != sizeof(run) || (run.flags & ~kKnownTextStyleFlags) != 0 ||
            run.range.start != covered ||
            !valid_text_range(run.range, request->text.length, *utf16_for_utf8) ||
            (request->text.length != 0 && run.range.start == run.range.end) ||
            !finite(run.font_size) || run.font_size <= 0.0f ||
            run.font_weight == 0 || run.font_weight > 1000 ||
            run.font_slant > FISSION_SKIA_FONT_SLANT_ITALIC ||
            !finite(run.letter_spacing) || !finite(run.font_width) || run.font_width <= 0.0f ||
            !finite(run.word_spacing) || !valid_string_slice(run.font_family, true, false) ||
            !valid_string_slice(run.locale, true, false) ||
            !valid_pointer_count(run.variations, run.variation_count) ||
            !valid_pointer_count(run.features, run.feature_count) ||
            run.variation_count > static_cast<size_t>(std::numeric_limits<int>::max())) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "text style run is malformed or does not contiguously cover UTF-8 text",
                        error);
        }
        const bool has_line_height =
            (run.flags & FISSION_SKIA_TEXT_STYLE_HAS_LINE_HEIGHT) != 0;
        const bool has_background =
            (run.flags & FISSION_SKIA_TEXT_STYLE_HAS_BACKGROUND) != 0;
        if ((!has_line_height && run.line_height != 0.0f) ||
            (has_line_height && (!finite(run.line_height) || run.line_height <= 0.0f)) ||
            (!has_background && !zero_color(run.background_color))) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "text style optional payload does not match its flags", error);
        }
        for (size_t variation = 0; variation < run.variation_count; ++variation) {
            if (!finite(run.variations[variation].value)) {
                return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                            "font variation values must be finite", error);
            }
        }
        for (size_t feature = 0; feature < run.feature_count; ++feature) {
            if (run.features[feature].value >
                static_cast<uint32_t>(std::numeric_limits<int>::max())) {
                return fail(FISSION_SKIA_STATUS_UNSUPPORTED, "paragraph_layout",
                            "font feature value exceeds SkParagraph's signed integer range",
                            error);
            }
        }
        covered = run.range.end;
    }
    if (covered != request->text.length) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "style runs do not cover the complete UTF-8 source", error);
    }

    uint64_t previous_inline_end = 0;
    std::unordered_set<uint64_t> inline_ids;
    for (size_t index = 0; index < request->inline_object_count; ++index) {
        const auto& inline_object = request->inline_objects[index];
        if (inline_object.struct_size != sizeof(inline_object) ||
            inline_object.reserved != 0 || inline_object.reserved_scalar != 0.0f ||
            !valid_text_range(inline_object.range, request->text.length, *utf16_for_utf8) ||
            inline_object.range.start == inline_object.range.end ||
            (index != 0 && inline_object.range.start < previous_inline_end) ||
            !inline_ids.insert(inline_object.id).second ||
            !finite(inline_object.width) || inline_object.width < 0.0f ||
            !finite(inline_object.height) || inline_object.height < 0.0f ||
            !finite(inline_object.baseline) || inline_object.baseline < 0.0f ||
            inline_object.baseline > inline_object.height) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "inline object layout, identity, range, or geometry is invalid", error);
        }
        const size_t start = static_cast<size_t>(inline_object.range.start);
        const size_t length = static_cast<size_t>(inline_object.range.end -
                                                  inline_object.range.start);
        if (length != 3 || request->text.data[start] != 0xef ||
            request->text.data[start + 1] != 0xbf || request->text.data[start + 2] != 0xbc) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "inline object range must contain exactly one U+FFFC placeholder", error);
        }
        const bool contained_by_run = std::any_of(
            request->style_runs,
            request->style_runs + request->style_run_count,
            [&](const fission_skia_text_style_run_t& run) {
                return run.range.start <= inline_object.range.start &&
                       inline_object.range.end <= run.range.end;
            });
        if (!contained_by_run) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                        "inline object must be contained by one text style run", error);
        }
        previous_inline_end = inline_object.range.end;
    }
    return FISSION_SKIA_STATUS_OK;
}

SkColor sk_color(const fission_skia_rgba8_t& color) {
    return SkColorSetARGB(color.alpha, color.red, color.green, color.blue);
}

std::vector<SkString> font_families(
    const fission_skia_paragraph_request_t& request,
    const fission_skia_text_style_run_t& run) {
    std::vector<SkString> result;
    if (run.font_family.length != 0) {
        result.emplace_back(reinterpret_cast<const char*>(run.font_family.data),
                            run.font_family.length);
    }
    for (size_t index = 0; index < request.fallback_family_count; ++index) {
        const auto& family = request.fallback_families[index];
        const std::string value = string_from(family);
        const bool duplicate = std::any_of(result.begin(), result.end(), [&](const SkString& item) {
            return item.equals(value.c_str());
        });
        if (!duplicate) result.emplace_back(value.c_str(), value.size());
    }
    if (result.empty()) result.emplace_back("sans-serif");
    return result;
}

int sk_font_width(float ratio) {
    const int width = static_cast<int>(std::lround(ratio * SkFontStyle::kNormal_Width));
    return std::clamp(width, static_cast<int>(SkFontStyle::kUltraCondensed_Width),
                      static_cast<int>(SkFontStyle::kUltraExpanded_Width));
}

TextStyle sk_text_style(
    const fission_skia_paragraph_request_t& request,
    const fission_skia_text_style_run_t& run) {
    TextStyle style;
    style.setColor(sk_color(run.color));
    style.setFontSize(run.font_size);
    style.setFontFamilies(font_families(request, run));
    style.setFontStyle(SkFontStyle(
        run.font_weight,
        sk_font_width(run.font_width),
        run.font_slant == FISSION_SKIA_FONT_SLANT_ITALIC
            ? SkFontStyle::kItalic_Slant
            : SkFontStyle::kUpright_Slant));
    if (run.locale.length != 0) {
        style.setLocale(SkString(reinterpret_cast<const char*>(run.locale.data), run.locale.length));
    } else if (request.locale.length != 0) {
        style.setLocale(
            SkString(reinterpret_cast<const char*>(request.locale.data), request.locale.length));
    }
    if ((run.flags & FISSION_SKIA_TEXT_STYLE_UNDERLINE) != 0) {
        style.setDecoration(skia::textlayout::TextDecoration::kUnderline);
        style.setDecorationColor(sk_color(run.color));
    }
    if ((run.flags & FISSION_SKIA_TEXT_STYLE_HAS_LINE_HEIGHT) != 0) {
        style.setHeight(run.line_height / run.font_size);
        style.setHeightOverride(true);
    }
    style.setLetterSpacing(run.letter_spacing);
    style.setWordSpacing(run.word_spacing);
    if ((run.flags & FISSION_SKIA_TEXT_STYLE_HAS_BACKGROUND) != 0) {
        SkPaint background;
        background.setColor(sk_color(run.background_color));
        style.setBackgroundPaint(std::move(background));
    }
    for (size_t index = 0; index < run.feature_count; ++index) {
        const auto& feature = run.features[index];
        const char tag[4] = {
            static_cast<char>((feature.tag >> 24) & 0xff),
            static_cast<char>((feature.tag >> 16) & 0xff),
            static_cast<char>((feature.tag >> 8) & 0xff),
            static_cast<char>(feature.tag & 0xff),
        };
        style.addFontFeature(SkString(tag, sizeof(tag)), static_cast<int>(feature.value));
    }
    if (run.variation_count != 0) {
        std::vector<SkFontArguments::VariationPosition::Coordinate> coordinates;
        coordinates.reserve(run.variation_count);
        for (size_t index = 0; index < run.variation_count; ++index) {
            coordinates.push_back({run.variations[index].tag, run.variations[index].value});
        }
        SkFontArguments arguments;
        arguments.setVariationDesignPosition({coordinates.data(),
                                              static_cast<int>(coordinates.size())});
        style.setFontArguments(std::optional<SkFontArguments>(arguments));
    }
    return style;
}

TextDirection resolve_direction(
    const fission_skia_paragraph_request_t& request,
    const std::vector<DecodedScalar>& scalars) {
    if (request.paragraph_style.text_direction == FISSION_SKIA_TEXT_DIRECTION_RTL) {
        return TextDirection::kRtl;
    }
    if (request.paragraph_style.text_direction == FISSION_SKIA_TEXT_DIRECTION_LTR ||
        scalars.empty()) {
        return TextDirection::kLtr;
    }
    const std::u16string utf16 = SkUnicode::convertUtf8ToUtf16(
        reinterpret_cast<const char*>(request.text.data),
        static_cast<int>(request.text.length));
    if (utf16.size() > static_cast<size_t>(std::numeric_limits<int32_t>::max())) {
        return TextDirection::kLtr;
    }
    const UBiDiDirection direction = ubidi_getBaseDirection(
        reinterpret_cast<const UChar*>(utf16.data()), static_cast<int32_t>(utf16.size()));
    return direction == UBIDI_RTL ? TextDirection::kRtl : TextDirection::kLtr;
}

TextAlign sk_text_align(uint32_t value) {
    switch (value) {
        case FISSION_SKIA_TEXT_ALIGN_LEFT: return TextAlign::kLeft;
        case FISSION_SKIA_TEXT_ALIGN_RIGHT: return TextAlign::kRight;
        case FISSION_SKIA_TEXT_ALIGN_CENTER: return TextAlign::kCenter;
        case FISSION_SKIA_TEXT_ALIGN_JUSTIFY: return TextAlign::kJustify;
        case FISSION_SKIA_TEXT_ALIGN_START: return TextAlign::kStart;
        case FISSION_SKIA_TEXT_ALIGN_END: return TextAlign::kEnd;
    }
    return TextAlign::kLeft;
}

sk_sp<SkFontMgr> make_platform_font_manager() {
#if defined(__APPLE__)
    return SkFontMgr_New_CoreText(nullptr);
#elif defined(_WIN32)
    return SkFontMgr_New_DirectWrite();
#elif defined(__ANDROID__)
    return SkFontMgr_New_Android(nullptr, SkFontScanner_Make_FreeType());
#elif defined(__linux__)
    return SkFontMgr_New_FontConfig(nullptr, SkFontScanner_Make_FreeType());
#else
    return nullptr;
#endif
}

sk_sp<FontCollection> thread_font_collection() {
    thread_local sk_sp<FontCollection> collection = [] {
        sk_sp<SkFontMgr> manager = make_platform_font_manager();
        if (manager == nullptr) return sk_sp<FontCollection>();
        sk_sp<FontCollection> fonts = sk_make_sp<FontCollection>();
        fonts->setDefaultFontManager(std::move(manager));
        return fonts;
    }();
    return collection;
}

sk_sp<FontCollection> font_collection_for(uint64_t generation) {
    if (generation == 0) return thread_font_collection();
    std::lock_guard<std::mutex> lock(paragraph_registry().mutex);
    const auto found = paragraph_registry().font_catalogs.find(generation);
    return found == paragraph_registry().font_catalogs.end() ? nullptr : found->second;
}

fission_skia_status_t build_font_collection(
    const fission_skia_paragraph_font_face_t* faces,
    size_t face_count,
    sk_sp<FontCollection>* output,
    fission_skia_error_t* error) {
    if (output == nullptr || faces == nullptr || face_count == 0 ||
        face_count > kMaxFontCatalogFaces) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT,
                    "paragraph_font_catalog_create",
                    "font catalogue requires a bounded, nonempty face array", error);
    }
    sk_sp<SkFontMgr> parser = make_platform_font_manager();
    if (parser == nullptr) {
        return fail(FISSION_SKIA_STATUS_UNSUPPORTED,
                    "paragraph_font_catalog_create",
                    "the selected native profile has no font decoder", error);
    }
    auto provider = sk_make_sp<skia::textlayout::TypefaceFontProvider>();
    size_t total_bytes = 0;
    for (size_t index = 0; index < face_count; ++index) {
        const auto& face = faces[index];
        if (face.struct_size != sizeof(face) || face.reserved != 0 ||
            face.reserved_scalar != 0 ||
            !valid_string_slice(face.family, false, false) || face.data == nullptr ||
            face.data_length == 0 || face.weight == 0 || face.weight > 1000 ||
            face.slant > FISSION_SKIA_FONT_SLANT_OBLIQUE ||
            !valid_pointer_count(face.axes, face.axis_count) ||
            face.axis_count > kMaxFontFaceAxes ||
            face.data_length > kMaxFontCatalogBytes - total_bytes) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT,
                        "paragraph_font_catalog_create",
                        "font face metadata, bytes, or catalogue budget is invalid", error);
        }
        total_bytes += face.data_length;
        std::vector<SkFontArguments::VariationPosition::Coordinate> coordinates;
        coordinates.reserve(face.axis_count);
        std::unordered_set<uint32_t> tags;
        for (size_t axis = 0; axis < face.axis_count; ++axis) {
            if (!finite(face.axes[axis].value) || !tags.insert(face.axes[axis].tag).second) {
                return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT,
                            "paragraph_font_catalog_create",
                            "font face axes must be finite and unique", error);
            }
            coordinates.push_back({face.axes[axis].tag, face.axes[axis].value});
        }
        sk_sp<SkData> data = SkData::MakeWithCopy(face.data, face.data_length);
        sk_sp<SkTypeface> typeface = data == nullptr
            ? nullptr
            : parser->makeFromData(std::move(data));
        if (typeface == nullptr) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT,
                        "paragraph_font_catalog_create",
                        "font face bytes could not be decoded by Skia", error);
        }
        if (!coordinates.empty()) {
            SkFontArguments arguments;
            arguments.setVariationDesignPosition(
                {coordinates.data(), static_cast<int>(coordinates.size())});
            typeface = typeface->makeClone(arguments);
            if (typeface == nullptr) {
                return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT,
                            "paragraph_font_catalog_create",
                            "font face variation defaults could not be applied", error);
            }
        }
        // Face selection remains authoritative in the encoded font. Fission's
        // declared weight/slant are validated at the boundary and carried by
        // paragraph TextStyle; aliases intentionally allow app family names
        // that differ from the font's internal name.
        const SkString family(reinterpret_cast<const char*>(face.family.data),
                              face.family.length);
        if (provider->registerTypeface(std::move(typeface), family) == 0) {
            return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT,
                        "paragraph_font_catalog_create",
                        "font face could not be registered under its Fission family", error);
        }
    }
    sk_sp<FontCollection> collection = sk_make_sp<FontCollection>();
    collection->setAssetFontManager(std::move(provider));
    collection->setDefaultFontManager(std::move(parser));
    *output = std::move(collection);
    return FISSION_SKIA_STATUS_OK;
}

ParagraphStyle sk_paragraph_style(
    const fission_skia_paragraph_request_t& request,
    const std::vector<DecodedScalar>& scalars) {
    ParagraphStyle style;
    const TextStyle default_style = request.style_run_count == 0
        ? TextStyle()
        : sk_text_style(request, request.style_runs[0]);
    style.setTextStyle(default_style);
    style.setTextAlign(sk_text_align(request.paragraph_style.text_align));
    style.setTextDirection(resolve_direction(request, scalars));
    if ((request.paragraph_style.flags &
         FISSION_SKIA_PARAGRAPH_STYLE_HAS_MAX_LINES) != 0) {
        style.setMaxLines(static_cast<size_t>(request.paragraph_style.max_lines));
    }
    if (request.paragraph_style.overflow == FISSION_SKIA_TEXT_OVERFLOW_ELLIPSIS) {
        style.setEllipsis(SkString("\xe2\x80\xa6"));
    }
    int behavior = TextHeightBehavior::kAll;
    if ((request.paragraph_style.flags &
         FISSION_SKIA_PARAGRAPH_STYLE_APPLY_FIRST_ASCENT) == 0) {
        behavior |= TextHeightBehavior::kDisableFirstAscent;
    }
    if ((request.paragraph_style.flags &
         FISSION_SKIA_PARAGRAPH_STYLE_APPLY_LAST_DESCENT) == 0) {
        behavior |= TextHeightBehavior::kDisableLastDescent;
    }
    style.setTextHeightBehavior(static_cast<TextHeightBehavior>(behavior));
    if ((request.paragraph_style.flags &
         FISSION_SKIA_PARAGRAPH_STYLE_HAS_STRUT_HEIGHT) != 0) {
        auto strut = style.getStrutStyle();
        const float font_size = request.style_run_count == 0
            ? 14.0f
            : request.style_runs[0].font_size;
        strut.setFontSize(font_size);
        if (request.style_run_count != 0) {
            strut.setFontFamilies(font_families(request, request.style_runs[0]));
        }
        strut.setHeight(request.paragraph_style.strut_line_height / font_size);
        strut.setHeightOverride(true);
        strut.setStrutEnabled(true);
        style.setStrutStyle(std::move(strut));
    }
    return style;
}

bool add_styled_text(
    ParagraphBuilder* builder,
    const fission_skia_paragraph_request_t& request) {
    size_t inline_index = 0;
    for (size_t run_index = 0; run_index < request.style_run_count; ++run_index) {
        const auto& run = request.style_runs[run_index];
        builder->pushStyle(sk_text_style(request, run));
        size_t cursor = static_cast<size_t>(run.range.start);
        const size_t run_end = static_cast<size_t>(run.range.end);
        while (inline_index < request.inline_object_count &&
               request.inline_objects[inline_index].range.end <= run.range.start) {
            ++inline_index;
        }
        size_t scan_inline = inline_index;
        while (scan_inline < request.inline_object_count &&
               request.inline_objects[scan_inline].range.start < run.range.end) {
            const auto& inline_object = request.inline_objects[scan_inline];
            const size_t inline_start = static_cast<size_t>(inline_object.range.start);
            const size_t inline_end = static_cast<size_t>(inline_object.range.end);
            if (cursor < inline_start) {
                builder->addText(
                    reinterpret_cast<const char*>(request.text.data + cursor),
                    inline_start - cursor);
            }
            builder->addPlaceholder(PlaceholderStyle(
                inline_object.width,
                inline_object.height,
                PlaceholderAlignment::kBaseline,
                TextBaseline::kAlphabetic,
                inline_object.baseline));
            cursor = inline_end;
            ++scan_inline;
        }
        if (cursor < run_end) {
            builder->addText(reinterpret_cast<const char*>(request.text.data + cursor),
                             run_end - cursor);
        }
        builder->pop();
        inline_index = scan_inline;
    }
    return inline_index == request.inline_object_count;
}

fission_skia_paragraph_rect_t paragraph_rect(const SkRect& rect) {
    return {rect.fLeft, rect.fTop, rect.width(), rect.height()};
}

uint32_t output_direction(TextDirection direction) {
    return direction == TextDirection::kRtl ? FISSION_SKIA_DIRECTION_RTL
                                            : FISSION_SKIA_DIRECTION_LTR;
}

bool collect_breaks(
    SkUnicode* unicode,
    const std::string& text,
    const std::string& locale,
    SkUnicode::BreakType type,
    std::vector<size_t>* output) {
    output->clear();
    auto iterator = unicode->makeBreakIterator(locale.empty() ? nullptr : locale.c_str(), type);
    if (iterator == nullptr ||
        !iterator->setText(text.data(), static_cast<int>(text.size()))) {
        return false;
    }
    for (int position = iterator->first(); !iterator->isDone(); position = iterator->next()) {
        if (position < 0 || static_cast<size_t>(position) > text.size()) return false;
        output->push_back(static_cast<size_t>(position));
    }
    if (output->empty() || output->front() != 0 || output->back() != text.size()) {
        return false;
    }
    output->erase(std::unique(output->begin(), output->end()), output->end());
    return true;
}

bool range_is_inline(
    const fission_skia_paragraph_request_t& request,
    size_t start,
    size_t end) {
    const fission_skia_text_range_t range{start, end};
    return is_placeholder_range(request, range);
}

bool line_has_regular_text(
    const fission_skia_paragraph_request_t& request,
    const std::vector<DecodedScalar>& scalars,
    const fission_skia_paragraph_line_t& line) {
    for (const auto& scalar : scalars) {
        if (scalar.start < line.range.start || scalar.end > line.range.end) continue;
        if (scalar.value == '\r' || scalar.value == '\n' ||
            range_is_inline(request, scalar.start, scalar.end)) {
            continue;
        }
        return true;
    }
    return false;
}

bool build_geometry(
    const fission_skia_paragraph_request_t& request,
    const std::vector<DecodedScalar>& scalars,
    const std::vector<size_t>& utf16_for_utf8,
    SkUnicode* unicode,
    Paragraph* paragraph,
    ParagraphResultState* output) {
    std::vector<LineMetrics> metrics;
    paragraph->getLineMetrics(metrics);
    const uint32_t base_direction = output_direction(resolve_direction(request, scalars));
    double previous_height = 0.0;
    for (const auto& line : metrics) {
        // Current SkParagraph LineMetrics indices are UTF-16 even though
        // shaped-cluster APIs and getLineNumberAt use UTF-8 TextIndex values.
        // Fission's paragraph ABI declares one index space for every record,
        // so normalize line ranges at the bridge boundary.
        size_t line_start = 0;
        size_t line_end = 0;
        if (!utf8_offset_for_utf16(scalars, line.fStartIndex, request.text.length,
                                   &line_start) ||
            !utf8_offset_for_utf16(scalars, line.fEndIncludingNewline,
                                   request.text.length, &line_end) ||
            line_start > line_end) {
            return false;
        }
        const double line_height = std::max(0.0, line.fHeight - previous_height);
        output->lines.push_back({
            {static_cast<uint64_t>(line_start), static_cast<uint64_t>(line_end)},
            {static_cast<float>(line.fLeft), static_cast<float>(previous_height),
             static_cast<float>(std::max(0.0, line.fWidth)),
             static_cast<float>(line_height)},
            static_cast<float>(line.fBaseline),
            static_cast<float>(line.fAscent),
            static_cast<float>(line.fDescent),
            static_cast<float>(std::max(0.0, line_height - line.fAscent - line.fDescent)),
            line.fHardBreak ? 1u : 0u,
            base_direction,
        });
        previous_height = line.fHeight;
    }
    if (!output->lines.empty()) {
        output->has_first_baseline = true;
        output->has_last_baseline = true;
        output->first_baseline = output->lines.front().baseline;
        output->last_baseline = output->lines.back().baseline;
    }

    const std::string text = request.text.length == 0
        ? std::string()
        : std::string(reinterpret_cast<const char*>(request.text.data), request.text.length);
    const std::string locale = string_from(request.locale);
    std::vector<size_t> grapheme_breaks;
    std::vector<size_t> word_breaks;
    const bool has_breaks = collect_breaks(
        unicode, text, locale, SkUnicode::BreakType::kGraphemes, &grapheme_breaks);
    const bool has_words = text.empty() ||
        unicode->getUtf8Words(text.data(), static_cast<int>(text.size()),
                              locale.empty() ? nullptr : locale.c_str(), &word_breaks);
    if (!has_breaks || !has_words) {
        return false;
    }
    if (text.empty()) word_breaks.push_back(0);
    std::sort(word_breaks.begin(), word_breaks.end());
    word_breaks.erase(std::unique(word_breaks.begin(), word_breaks.end()), word_breaks.end());

    std::unordered_set<uint64_t> seen_clusters;
    for (const auto& scalar : scalars) {
        Paragraph::GlyphClusterInfo info;
        if (!paragraph->getGlyphClusterAt(scalar.start, &info)) continue;
        const size_t start = info.fClusterTextRange.start;
        const size_t end = info.fClusterTextRange.end;
        if (start >= end || end > request.text.length || range_is_inline(request, start, end)) {
            continue;
        }
        const uint64_t key = (static_cast<uint64_t>(start) << 32) ^ end;
        if (!seen_clusters.insert(key).second) continue;
        const int line_index = paragraph->getLineNumberAt(start);
        if (line_index < 0 || static_cast<size_t>(line_index) >= output->lines.size()) continue;
        output->clusters.push_back({
            {static_cast<uint64_t>(start), static_cast<uint64_t>(end)},
            paragraph_rect(info.fBounds),
            static_cast<uint64_t>(line_index),
            output_direction(info.fGlyphClusterPosition),
            std::binary_search(grapheme_breaks.begin(), grapheme_breaks.end(), start) ? 1u : 0u,
            std::binary_search(word_breaks.begin(), word_breaks.end(), start) ? 1u : 0u,
            0,
        });
    }

    for (size_t index = 0; index + 1 < grapheme_breaks.size(); ++index) {
        const size_t start = grapheme_breaks[index];
        const size_t end = grapheme_breaks[index + 1];
        if (start == end || range_is_inline(request, start, end) ||
            utf16_for_utf8[start] == std::numeric_limits<size_t>::max() ||
            utf16_for_utf8[end] == std::numeric_limits<size_t>::max() ||
            utf16_for_utf8[start] > std::numeric_limits<unsigned>::max() ||
            utf16_for_utf8[end] > std::numeric_limits<unsigned>::max()) {
            continue;
        }
        const auto boxes = paragraph->getRectsForRange(
            static_cast<unsigned>(utf16_for_utf8[start]),
            static_cast<unsigned>(utf16_for_utf8[end]),
            RectHeightStyle::kTight,
            RectWidthStyle::kTight);
        if (boxes.empty()) continue;
        const int line_index = paragraph->getLineNumberAt(start);
        if (line_index < 0 || static_cast<size_t>(line_index) >= output->lines.size()) continue;
        const TextBox& box = boxes.front();
        const auto rect = paragraph_rect(box.rect);
        const bool rtl = box.direction == TextDirection::kRtl;
        const float start_x = rtl ? box.rect.fRight : box.rect.fLeft;
        const float end_x = rtl ? box.rect.fLeft : box.rect.fRight;
        const float caret_height = std::max(1.0f, box.rect.height());
        output->carets.push_back({
            static_cast<uint64_t>(start), FISSION_SKIA_AFFINITY_DOWNSTREAM, 0,
            {start_x, box.rect.fTop, 1.0f, caret_height},
            static_cast<uint64_t>(line_index),
        });
        output->carets.push_back({
            static_cast<uint64_t>(end), FISSION_SKIA_AFFINITY_UPSTREAM, 0,
            {end_x, box.rect.fTop, 1.0f, caret_height},
            static_cast<uint64_t>(line_index),
        });
        const float hit_width = std::max(1.0f, rect.width);
        const float half = hit_width * 0.5f;
        const uint64_t left_index = rtl ? end : start;
        const uint64_t right_index = rtl ? start : end;
        const uint32_t left_affinity = rtl ? FISSION_SKIA_AFFINITY_UPSTREAM
                                           : FISSION_SKIA_AFFINITY_DOWNSTREAM;
        const uint32_t right_affinity = rtl ? FISSION_SKIA_AFFINITY_DOWNSTREAM
                                            : FISSION_SKIA_AFFINITY_UPSTREAM;
        output->hit_regions.push_back({
            {rect.x, rect.y, half, std::max(1.0f, rect.height)},
            left_index, left_affinity, 0, static_cast<uint64_t>(line_index),
        });
        output->hit_regions.push_back({
            {rect.x + half, rect.y, hit_width - half, std::max(1.0f, rect.height)},
            right_index, right_affinity, 0, static_cast<uint64_t>(line_index),
        });
    }

    const auto placeholder_boxes = paragraph->getRectsForPlaceholders();
    size_t box_index = 0;
    for (size_t index = 0; index < request.inline_object_count; ++index) {
        const auto& input = request.inline_objects[index];
        const auto line = std::find_if(
            output->lines.begin(), output->lines.end(), [&](const auto& candidate) {
                return candidate.range.start <= input.range.start &&
                       input.range.end <= candidate.range.end;
            });
        if (line == output->lines.end()) continue;
        if (input.width == 0.0f && input.height == 0.0f) {
            float x = line->rect.x;
            const auto caret = std::find_if(
                output->carets.begin(), output->carets.end(), [&](const auto& candidate) {
                    return candidate.index == input.range.start ||
                           candidate.index == input.range.end;
                });
            if (caret != output->carets.end()) x = caret->rect.x;
            output->inline_boxes.push_back({
                input.id,
                input.range,
                {x, line->rect.y, 0.0f, 0.0f},
                input.baseline,
                0,
            });
            continue;
        }
        if (box_index >= placeholder_boxes.size()) {
            // SkParagraph truncation may omit only a trailing invisible suffix.
            break;
        }
        output->inline_boxes.push_back({
            input.id,
            input.range,
            paragraph_rect(placeholder_boxes[box_index].rect),
            input.baseline,
            0,
        });
        ++box_index;
    }
    if (box_index != placeholder_boxes.size()) return false;

    const auto unresolved = paragraph->unresolvedCodepoints();
    for (const auto& scalar : scalars) {
        if (unresolved.find(static_cast<SkUnichar>(scalar.value)) == unresolved.end()) continue;
        const SkFont font = paragraph->getFontAt(scalar.start);
        if (font.getTypeface() != nullptr &&
            font.unicharToGlyph(static_cast<SkUnichar>(scalar.value)) != 0) {
            continue;
        }
        const uint64_t codepoint_start = output->unresolved_codepoints.size();
        output->unresolved_codepoints.push_back(scalar.value);
        output->unresolved_glyphs.push_back({
            {static_cast<uint64_t>(scalar.start), static_cast<uint64_t>(scalar.end)},
            codepoint_start,
            1,
        });
    }

    for (size_t line_index = 0; line_index < output->lines.size(); ++line_index) {
        const auto& line = output->lines[line_index];
        const bool has_cluster = std::any_of(
            output->clusters.begin(), output->clusters.end(), [&](const auto& cluster) {
                return cluster.line_index == line_index;
            });
        const bool has_caret = std::any_of(
            output->carets.begin(), output->carets.end(), [&](const auto& caret) {
                return caret.line_index == line_index;
            });
        const bool has_hit = std::any_of(
            output->hit_regions.begin(), output->hit_regions.end(), [&](const auto& hit) {
                return hit.line_index == line_index;
            });
        if (line_has_regular_text(request, scalars, line) &&
            (!has_cluster || !has_caret || !has_hit)) {
            return false;
        }
        if (!has_caret || !has_hit) {
            const float height = std::max(1.0f, line.rect.height);
            output->carets.push_back({
                line.range.start,
                FISSION_SKIA_AFFINITY_DOWNSTREAM,
                0,
                {line.rect.x, line.rect.y, 1.0f, height},
                static_cast<uint64_t>(line_index),
            });
            output->hit_regions.push_back({
                {line.rect.x, line.rect.y, 1.0f, height},
                line.range.start,
                FISSION_SKIA_AFFINITY_DOWNSTREAM,
                0,
                static_cast<uint64_t>(line_index),
            });
        }
    }
    return true;
}

template <typename T>
const T* data_or_null(const std::vector<T>& values) {
    return values.empty() ? nullptr : values.data();
}

}  // namespace

extern "C" {

fission_skia_status_t fission_skia_paragraph_capabilities(
    uint64_t* out_capabilities,
    fission_skia_error_t* out_error) {
    if (out_capabilities == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_capabilities",
                    "capability output pointer is null", out_error);
    }
    *out_capabilities = kParagraphCapabilities;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_font_catalog_create(
    const fission_skia_paragraph_font_face_t* faces,
    size_t face_count,
    fission_skia_font_catalog_handle_t* out_catalog,
    fission_skia_error_t* out_error) {
    if (out_catalog == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT,
                    "paragraph_font_catalog_create",
                    "font catalogue output pointer is null", out_error);
    }
    *out_catalog = 0;
    sk_sp<FontCollection> collection;
    const auto status = build_font_collection(
        faces, face_count, &collection, out_error);
    if (status != FISSION_SKIA_STATUS_OK) return status;
    uint64_t handle = 0;
    {
        std::lock_guard<std::mutex> lock(paragraph_registry().mutex);
        do {
            handle = paragraph_registry().next_font_catalog.fetch_add(
                1, std::memory_order_relaxed);
        } while (handle == 0 || paragraph_registry().font_catalogs.find(handle) !=
                                    paragraph_registry().font_catalogs.end());
        paragraph_registry().font_catalogs.emplace(handle, std::move(collection));
    }
    *out_catalog = handle;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_font_catalog_destroy(
    fission_skia_font_catalog_handle_t catalog,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(paragraph_registry().mutex);
    const auto found = paragraph_registry().font_catalogs.find(catalog);
    if (found == paragraph_registry().font_catalogs.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE,
                    "paragraph_font_catalog_destroy",
                    "font catalogue handle is not live", out_error);
    }
    paragraph_registry().font_catalogs.erase(found);
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_layout(
    const fission_skia_paragraph_request_t* request,
    fission_skia_paragraph_result_handle_t* out_result,
    fission_skia_error_t* out_error) {
    if (out_result == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "paragraph_layout",
                    "paragraph result output pointer is null", out_error);
    }
    *out_result = 0;
    std::vector<DecodedScalar> scalars;
    std::vector<size_t> utf16_for_utf8;
    const auto validation = validate_request(request, &scalars, &utf16_for_utf8, out_error);
    if (validation != FISSION_SKIA_STATUS_OK) return validation;

    sk_sp<FontCollection> fonts = font_collection_for(request->font_catalog_generation);
    sk_sp<SkUnicode> unicode = SkUnicodes::ICU::Make();
    if (fonts == nullptr || unicode == nullptr) {
        return fail(
            request->font_catalog_generation == 0 ? FISSION_SKIA_STATUS_UNSUPPORTED
                                                  : FISSION_SKIA_STATUS_INVALID_HANDLE,
            "paragraph_layout",
            request->font_catalog_generation == 0
                ? "the selected Skia profile has no platform FontMgr or ICU implementation"
                : "paragraph font catalogue generation is not live",
            out_error);
    }
    auto builder = ParagraphBuilder::make(
        sk_paragraph_style(*request, scalars), fonts, unicode);
    if (builder == nullptr || !add_styled_text(builder.get(), *request)) {
        return fail(FISSION_SKIA_STATUS_INTERNAL, "paragraph_layout",
                    "failed to construct the normalized SkParagraph input", out_error);
    }
    std::unique_ptr<Paragraph> paragraph = builder->Build();
    if (paragraph == nullptr) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "paragraph_layout",
                    "SkParagraph construction failed", out_error);
    }

    const bool wrap = (request->flags & FISSION_SKIA_PARAGRAPH_REQUEST_WRAP) != 0;
    const bool has_width =
        (request->flags & FISSION_SKIA_PARAGRAPH_REQUEST_HAS_WIDTH) != 0;
    constexpr float kProbeWidth = std::numeric_limits<float>::max() / 1024.0f;
    if (wrap && has_width) {
        paragraph->layout(request->width_constraint);
    } else {
        paragraph->layout(kProbeWidth);
        const float measured = std::max(paragraph->getLongestLine(),
                                        paragraph->getMaxIntrinsicWidth());
        const float requested = has_width ? request->width_constraint : 0.0f;
        paragraph->layout(std::max(measured, requested));
    }

    std::unique_ptr<ParagraphResultState> result(new (std::nothrow) ParagraphResultState());
    if (result == nullptr) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "paragraph_layout",
                    "paragraph result allocation failed", out_error);
    }
    result->min_intrinsic_width = paragraph->getMinIntrinsicWidth();
    // Keep the selected catalogue alive with the retained SkPicture. Destroying
    // the public catalogue handle prevents future layouts but cannot invalidate
    // an already laid-out paragraph that still owns cloned typefaces.
    result->font_collection = fonts;
    result->max_intrinsic_width = paragraph->getMaxIntrinsicWidth();
    result->size.width =
        request->paragraph_style.text_width_basis == FISSION_SKIA_TEXT_WIDTH_BASIS_PARENT &&
                has_width
            ? request->width_constraint
            : paragraph->getLongestLine();
    result->size.height = paragraph->getHeight();
    if (!finite(result->size.width) || result->size.width < 0.0f ||
        !finite(result->size.height) || result->size.height < 0.0f ||
        !finite(result->min_intrinsic_width) || result->min_intrinsic_width < 0.0f ||
        !finite(result->max_intrinsic_width) ||
        result->max_intrinsic_width < result->min_intrinsic_width ||
        !build_geometry(*request, scalars, utf16_for_utf8, unicode.get(), paragraph.get(),
                        result.get())) {
        return fail(FISSION_SKIA_STATUS_INTERNAL, "paragraph_layout",
                    "SkParagraph returned incomplete or invalid immutable geometry", out_error);
    }

    // Record the exact laid-out paragraph once. Playback later consumes this
    // immutable picture and cannot accidentally shape or lay out a second time.
    // The broad finite cull avoids discarding pathological font overhangs while
    // keeping SkPicture's scalar calculations finite.
    SkPictureRecorder recorder;
    SkCanvas* recording_canvas = recorder.beginRecording(SkRect::MakeLTRB(
        -kProbeWidth, -kProbeWidth, kProbeWidth, kProbeWidth));
    if (recording_canvas == nullptr) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "paragraph_layout",
                    "SkPicture recording canvas allocation failed", out_error);
    }
    paragraph->paint(recording_canvas, 0.0f, 0.0f);
    result->picture = recorder.finishRecordingAsPicture();
    if (result->picture == nullptr) {
        return fail(FISSION_SKIA_STATUS_OUT_OF_MEMORY, "paragraph_layout",
                    "SkParagraph picture recording failed", out_error);
    }
    result->approximate_bytes = result->picture->approximateBytesUsed();

    uint64_t handle = 0;
    {
        std::lock_guard<std::mutex> lock(paragraph_registry().mutex);
        do {
            handle = paragraph_registry().next_handle.fetch_add(1, std::memory_order_relaxed);
        } while (handle == 0 || paragraph_registry().results.find(handle) !=
                                    paragraph_registry().results.end());
        paragraph_registry().results.emplace(handle, std::move(result));
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
                    "paragraph result view has an incompatible layout", out_error);
    }
    std::lock_guard<std::mutex> lock(paragraph_registry().mutex);
    const auto found = paragraph_registry().results.find(result);
    if (found == paragraph_registry().results.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "paragraph_result_get_view",
                    "paragraph result handle is not live", out_error);
    }
    const ParagraphResultState& value = *found->second;
    *out_view = {
        sizeof(*out_view),
        FISSION_SKIA_INDEX_UTF8,
        kParagraphCapabilities,
        value.size,
        value.min_intrinsic_width,
        value.max_intrinsic_width,
        value.first_baseline,
        value.last_baseline,
        value.has_first_baseline ? 1u : 0u,
        value.has_last_baseline ? 1u : 0u,
        data_or_null(value.lines),
        value.lines.size(),
        data_or_null(value.clusters),
        value.clusters.size(),
        data_or_null(value.carets),
        value.carets.size(),
        data_or_null(value.hit_regions),
        value.hit_regions.size(),
        data_or_null(value.inline_boxes),
        value.inline_boxes.size(),
        data_or_null(value.unresolved_glyphs),
        value.unresolved_glyphs.size(),
        data_or_null(value.unresolved_codepoints),
        value.unresolved_codepoints.size(),
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
                    "approximate byte output pointer is null", out_error);
    }
    *out_approximate_bytes = 0;
    std::lock_guard<std::mutex> lock(paragraph_registry().mutex);
    const auto found = paragraph_registry().results.find(result);
    if (found == paragraph_registry().results.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE,
                    "paragraph_result_get_approximate_bytes",
                    "paragraph result handle is not live", out_error);
    }
    *out_approximate_bytes = found->second->approximate_bytes;
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_result_destroy(
    fission_skia_paragraph_result_handle_t result,
    fission_skia_error_t* out_error) {
    std::lock_guard<std::mutex> lock(paragraph_registry().mutex);
    const auto found = paragraph_registry().results.find(result);
    if (found == paragraph_registry().results.end()) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "paragraph_result_destroy",
                    "paragraph result handle is not live", out_error);
    }
    paragraph_registry().results.erase(found);
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
    if (!finite(x) || !finite(y) || !finite(scale_factor) || scale_factor <= 0.0f) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "draw_paragraph",
                    "paragraph origin or scale factor is invalid", out_error);
    }
    std::lock_guard<std::mutex> lock(paragraph_registry().mutex);
    const auto found = paragraph_registry().results.find(result);
    if (found == paragraph_registry().results.end() || found->second->picture == nullptr) {
        return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "draw_paragraph",
                    "paragraph draw handle is not live", out_error);
    }
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}

fission_skia_status_t fission_skia_paragraph_draw_picture(
    fission_skia_paragraph_result_handle_t result,
    SkCanvas* canvas,
    float x,
    float y,
    float scale_factor,
    fission_skia_error_t* out_error) {
    if (canvas == nullptr || !finite(x) || !finite(y) || !finite(scale_factor) ||
        scale_factor <= 0.0f) {
        return fail(FISSION_SKIA_STATUS_INVALID_ARGUMENT, "draw_paragraph",
                    "paragraph canvas, origin, or scale factor is invalid", out_error);
    }
    sk_sp<SkPicture> picture;
    {
        std::lock_guard<std::mutex> lock(paragraph_registry().mutex);
        const auto found = paragraph_registry().results.find(result);
        if (found == paragraph_registry().results.end() || found->second->picture == nullptr) {
            return fail(FISSION_SKIA_STATUS_INVALID_HANDLE, "draw_paragraph",
                        "paragraph draw handle is not live", out_error);
        }
        picture = found->second->picture;
    }
    canvas->save();
    canvas->translate(x, y);
    canvas->scale(scale_factor, scale_factor);
    canvas->drawPicture(picture);
    canvas->restore();
    clear_error(out_error);
    return FISSION_SKIA_STATUS_OK;
}
