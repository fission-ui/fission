// Copyright 2026 The Fission Authors. All rights reserved.
// Use of this source code is governed by the license in the repository root.

import {
  decodeParagraphRequest,
  encodeParagraphResponse,
} from "./fission_skia_paragraph_wire.js";
import { resolveParagraphDirection } from "./fission_skia_paragraph_unicode.js";

const FONT_RESOURCE_KIND = 3;
const ALL_CAPABILITIES = 0x1ffn;
const MAX_U32 = 0xffffffff;
const MAX_GEOMETRY_RECORDS = 1_048_576;
const MAX_UNRESOLVED_CODEPOINTS = 1_048_576;
const PROBE_WIDTH = 3.4028234663852886e38 / 1024;

export class CanvasKitParagraphError extends Error {
  constructor(code, message, options) {
    super(message, options);
    this.name = "CanvasKitParagraphError";
    this.code = code;
  }
}

function fail(code, message, cause) {
  throw new CanvasKitParagraphError(code, message, cause === undefined ? undefined : { cause });
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function safeDelete(value) {
  if (!value || typeof value.delete !== "function") return;
  try {
    if (typeof value.isDeleted !== "function" || !value.isDeleted()) value.delete();
  } catch (_error) {
    // Continue teardown so one failed Embind destructor cannot leak its siblings.
  }
}

function requireFunction(value, label) {
  if (typeof value !== "function") fail("unsupported-api", `CanvasKit does not expose ${label}`);
  return value;
}

function requireEnum(group, name, label) {
  const value = group?.[name];
  if (value === undefined || value === null) {
    fail("unsupported-api", `CanvasKit does not expose ${label}.${name}`);
  }
  return value;
}

function enumEquals(left, right) {
  if (left === right) return true;
  return left?.value !== undefined && right?.value !== undefined && left.value === right.value;
}

function finite(value, label) {
  if (!Number.isFinite(value)) fail("invalid-geometry", `CanvasKit returned invalid ${label}`);
  return value;
}

function nonNegative(value, label) {
  finite(value, label);
  if (value < 0) fail("invalid-geometry", `CanvasKit returned negative ${label}`);
  return value;
}

function index(value, maximum, label) {
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) {
    fail("invalid-geometry", `CanvasKit returned invalid ${label}`);
  }
  return value;
}

function rect(value, label) {
  if (!value || typeof value.length !== "number" || value.length < 4) {
    fail("invalid-geometry", `CanvasKit returned invalid ${label}`);
  }
  const left = finite(Number(value[0]), `${label} left`);
  const top = finite(Number(value[1]), `${label} top`);
  const right = finite(Number(value[2]), `${label} right`);
  const bottom = finite(Number(value[3]), `${label} bottom`);
  if (right < left || bottom < top) {
    fail("invalid-geometry", `CanvasKit returned inverted ${label}`);
  }
  return { x: left, y: top, width: right - left, height: bottom - top };
}

function inlineKey(start, end) {
  return `${start}:${end}`;
}

function color(CanvasKit, rgba) {
  return CanvasKit.Color4f(rgba[0] / 255, rgba[1] / 255, rgba[2] / 255, rgba[3] / 255);
}

function fontFamilies(request, style) {
  const families = [];
  if (style?.fontFamily) families.push(style.fontFamily);
  for (const fallback of request.fallbackFamilies) {
    if (!families.includes(fallback)) families.push(fallback);
  }
  if (families.length === 0) families.push("sans-serif");
  return families;
}

function tagString(tag, label) {
  const bytes = [tag >>> 24, (tag >>> 16) & 0xff, (tag >>> 8) & 0xff, tag & 0xff];
  if (bytes.some((byte) => byte < 0x20 || byte > 0x7e)) {
    fail(
      "unsupported-api",
      `${label} must be a four-byte printable OpenType tag for CanvasKit`,
    );
  }
  return String.fromCharCode(...bytes);
}

function fontWeight(CanvasKit, weight) {
  const names = new Map([
    [100, "Thin"],
    [200, "ExtraLight"],
    [300, "Light"],
    [400, "Normal"],
    [500, "Medium"],
    [600, "SemiBold"],
    [700, "Bold"],
    [800, "ExtraBold"],
    [900, "Black"],
    [1000, "ExtraBlack"],
  ]);
  const name = names.get(weight);
  if (!name) {
    fail(
      "unsupported-api",
      `CanvasKit cannot represent the exact requested font weight ${weight}`,
    );
  }
  return requireEnum(CanvasKit.FontWeight, name, "FontWeight");
}

function fontWidth(CanvasKit, ratio) {
  const names = [
    "UltraCondensed",
    "ExtraCondensed",
    "Condensed",
    "SemiCondensed",
    "Normal",
    "SemiExpanded",
    "Expanded",
    "ExtraExpanded",
    "UltraExpanded",
  ];
  const width = Math.max(1, Math.min(9, Math.round(ratio * 5)));
  return requireEnum(CanvasKit.FontWidth, names[width - 1], "FontWidth");
}

function textStyle(CanvasKit, request, style) {
  const value = {
    color: color(CanvasKit, style.color),
    fontFamilies: fontFamilies(request, style),
    fontSize: style.fontSize,
    fontStyle: {
      weight: fontWeight(CanvasKit, style.fontWeight),
      width: fontWidth(CanvasKit, style.fontWidth),
      slant: requireEnum(
        CanvasKit.FontSlant,
        style.fontSlant === 1 ? "Italic" : "Upright",
        "FontSlant",
      ),
    },
    letterSpacing: style.letterSpacing,
    wordSpacing: style.wordSpacing,
    locale: style.locale ?? request.locale ?? undefined,
    fontFeatures: style.features.map((feature) => ({
      name: tagString(feature.tag, "font feature"),
      value: feature.value,
    })),
    fontVariations: style.variations.map((variation) => ({
      axis: tagString(variation.tag, "font variation"),
      value: variation.value,
    })),
  };
  if (style.underline) {
    if (CanvasKit.UnderlineDecoration === undefined) {
      fail("unsupported-api", "CanvasKit does not expose UnderlineDecoration");
    }
    value.decoration = CanvasKit.UnderlineDecoration;
    value.decorationColor = color(CanvasKit, style.color);
  }
  if (style.lineHeight !== null) value.heightMultiplier = style.lineHeight / style.fontSize;
  if (style.backgroundColor !== null) {
    value.backgroundColor = color(CanvasKit, style.backgroundColor);
  }
  return CanvasKit.TextStyle(value);
}

function defaultTextStyle(CanvasKit, request) {
  return CanvasKit.TextStyle({
    color: CanvasKit.Color4f(0, 0, 0, 1),
    fontFamilies: fontFamilies(request, null),
    fontSize: 14,
    fontStyle: {
      weight: requireEnum(CanvasKit.FontWeight, "Normal", "FontWeight"),
      width: requireEnum(CanvasKit.FontWidth, "Normal", "FontWidth"),
      slant: requireEnum(CanvasKit.FontSlant, "Upright", "FontSlant"),
    },
  });
}

function paragraphDirection(CanvasKit, request) {
  const direction = request.paragraphStyle.textDirection === 2
    ? 1
    : request.paragraphStyle.textDirection === 1
      ? 0
      : resolveParagraphDirection(request.textInfo.scalars);
  return {
    wire: direction,
    canvasKit: requireEnum(
      CanvasKit.TextDirection,
      direction === 1 ? "RTL" : "LTR",
      "TextDirection",
    ),
  };
}

function textHeightBehavior(CanvasKit, style) {
  let name;
  if (style.applyHeightToFirstAscent && style.applyHeightToLastDescent) name = "All";
  else if (!style.applyHeightToFirstAscent && style.applyHeightToLastDescent) {
    name = "DisableFirstAscent";
  } else if (style.applyHeightToFirstAscent && !style.applyHeightToLastDescent) {
    name = "DisableLastDescent";
  } else name = "DisableAll";
  return requireEnum(CanvasKit.TextHeightBehavior, name, "TextHeightBehavior");
}

function makeParagraphStyle(CanvasKit, request, direction) {
  if (request.paragraphStyle.overflow === 2) {
    fail("unsupported-api", "CanvasKit SkParagraph cannot represent fade overflow exactly");
  }
  if (request.paragraphStyle.overflow === 3 && request.paragraphStyle.maxLines !== null) {
    fail(
      "unsupported-api",
      "CanvasKit SkParagraph cannot combine visible overflow with a line limit exactly",
    );
  }
  if (request.paragraphStyle.maxLines !== null && request.paragraphStyle.maxLines > 0xffffffffn) {
    fail("unsupported-api", "CanvasKit's WebAssembly size_t cannot represent maxLines");
  }
  const aligns = ["Left", "Right", "Center", "Justify", "Start", "End"];
  const first = request.styles[0] ?? null;
  const value = {
    textStyle: first
      ? textStyle(CanvasKit, request, first)
      : defaultTextStyle(CanvasKit, request),
    textAlign: requireEnum(
      CanvasKit.TextAlign,
      aligns[request.paragraphStyle.textAlign],
      "TextAlign",
    ),
    textDirection: direction.canvasKit,
    textHeightBehavior: textHeightBehavior(CanvasKit, request.paragraphStyle),
  };
  if (request.paragraphStyle.maxLines !== null) {
    value.maxLines = Number(request.paragraphStyle.maxLines);
  }
  if (request.paragraphStyle.overflow === 1) value.ellipsis = "\u2026";
  if (request.paragraphStyle.strutLineHeight !== null) {
    const fontSize = first?.fontSize ?? 14;
    value.strutStyle = {
      strutEnabled: true,
      fontFamilies: fontFamilies(request, first),
      fontStyle: first
        ? {
            weight: fontWeight(CanvasKit, first.fontWeight),
            width: fontWidth(CanvasKit, first.fontWidth),
            slant: requireEnum(
              CanvasKit.FontSlant,
              first.fontSlant === 1 ? "Italic" : "Upright",
              "FontSlant",
            ),
          }
        : {
            weight: requireEnum(CanvasKit.FontWeight, "Normal", "FontWeight"),
            width: requireEnum(CanvasKit.FontWidth, "Normal", "FontWidth"),
            slant: requireEnum(CanvasKit.FontSlant, "Upright", "FontSlant"),
          },
      fontSize,
      heightMultiplier: request.paragraphStyle.strutLineHeight / fontSize,
    };
  }
  return CanvasKit.ParagraphStyle(value);
}

function hasRegularText(request) {
  const inline = new Set(request.inlineObjects.map((item) => (
    inlineKey(item.range.start, item.range.end)
  )));
  return request.textInfo.scalars.some((scalar) => (
    scalar.codepoint !== 0x0a &&
    scalar.codepoint !== 0x0d &&
    !inline.has(inlineKey(scalar.utf8Start, scalar.utf8End))
  ));
}

function resourceBytes(entry) {
  if (entry?.bytes instanceof Uint8Array) return entry.bytes;
  if (entry?.bytes instanceof ArrayBuffer) return new Uint8Array(entry.bytes);
  if (ArrayBuffer.isView(entry?.bytes)) {
    return new Uint8Array(entry.bytes.buffer, entry.bytes.byteOffset, entry.bytes.byteLength);
  }
  return null;
}

function makeFontProvider(CanvasKit, request, resolveResource) {
  if (hasRegularText(request) && request.fonts.length === 0) {
    fail(
      "resource-failure",
      "CanvasKit paragraph text requires at least one owned Fission font resource",
    );
  }
  let provider;
  try {
    provider = CanvasKit.TypefaceFontProvider.Make();
  } catch (error) {
    fail("resource-failure", `CanvasKit could not create a font provider: ${errorMessage(error)}`, error);
  }
  if (
    !provider ||
    typeof provider.registerFont !== "function" ||
    typeof provider.delete !== "function"
  ) {
    safeDelete(provider);
    fail("unsupported-api", "CanvasKit TypefaceFontProvider is unavailable");
  }
  let fontBytes = 0;
  try {
    for (const font of request.fonts) {
      let entry;
      try {
        entry = resolveResource(font.handle.slot);
      } catch (error) {
        fail(
          "resource-failure",
          `font resource ${font.handle.slot} could not be resolved: ${errorMessage(error)}`,
          error,
        );
      }
      if (!entry || entry.generation !== font.handle.generation) {
        fail("stale-handle", `font resource ${font.handle.slot} is not live at its generation`);
      }
      if (entry.kind !== FONT_RESOURCE_KIND) {
        fail("resource-failure", `resource ${font.handle.slot} is not a font`);
      }
      const bytes = resourceBytes(entry);
      if (!bytes || bytes.byteLength === 0) {
        fail("resource-failure", `font resource ${font.handle.slot} has no owned bytes`);
      }
      // registerFont decodes into provider-owned SkData. The copy also prevents
      // a concurrent Wasm-memory growth from invalidating an external view.
      const owned = Uint8Array.from(bytes);
      let result;
      try {
        result = provider.registerFont(owned, font.family);
      } catch (error) {
        fail(
          "resource-failure",
          `CanvasKit could not decode font resource ${font.handle.slot}: ${errorMessage(error)}`,
          error,
        );
      }
      if (result === null || result === false) {
        fail("resource-failure", `CanvasKit could not decode font resource ${font.handle.slot}`);
      }
      fontBytes += owned.byteLength;
    }
    return { provider, fontBytes };
  } catch (error) {
    safeDelete(provider);
    throw error;
  }
}

function fontProviderKey(request) {
  const fonts = request.fonts.map((font) => (
    `${font.handle.slot}:${font.handle.generation}:${JSON.stringify(font.family)}`
  ));
  return `${request.fontCatalogGeneration.toString()}|${fonts.join("|")}`;
}

function addTextRuns(CanvasKit, builder, request) {
  let inlineIndex = 0;
  for (const style of request.styles) {
    builder.pushStyle(textStyle(CanvasKit, request, style));
    let cursor = style.range.start;
    while (
      inlineIndex < request.inlineObjects.length &&
      request.inlineObjects[inlineIndex].range.end <= style.range.start
    ) {
      inlineIndex += 1;
    }
    let scanInline = inlineIndex;
    while (
      scanInline < request.inlineObjects.length &&
      request.inlineObjects[scanInline].range.start < style.range.end
    ) {
      const inline = request.inlineObjects[scanInline];
      if (cursor < inline.range.start) {
        builder.addText(request.textInfo.text.slice(
          request.textInfo.byteToUtf16[cursor],
          request.textInfo.byteToUtf16[inline.range.start],
        ));
      }
      builder.addPlaceholder(
        inline.width,
        inline.height,
        requireEnum(CanvasKit.PlaceholderAlignment, "Baseline", "PlaceholderAlignment"),
        requireEnum(CanvasKit.TextBaseline, "Alphabetic", "TextBaseline"),
        inline.baseline,
      );
      cursor = inline.range.end;
      scanInline += 1;
    }
    if (cursor < style.range.end) {
      builder.addText(request.textInfo.text.slice(
        request.textInfo.byteToUtf16[cursor],
        request.textInfo.byteToUtf16[style.range.end],
      ));
    }
    builder.pop();
    inlineIndex = scanInline;
  }
  if (inlineIndex !== request.inlineObjects.length) {
    fail("layout-failure", "normalized inline objects were not consumed by CanvasKit");
  }
}

function requireParagraphApi(paragraph) {
  for (const method of [
    "delete",
    "layout",
    "getHeight",
    "getLongestLine",
    "getMinIntrinsicWidth",
    "getMaxIntrinsicWidth",
    "getLineMetrics",
    "getGlyphInfoAt",
    "getRectsForRange",
    "getRectsForPlaceholders",
    "getWordBoundary",
    "unresolvedCodepoints",
    "getShapedLines",
  ]) {
    requireFunction(paragraph?.[method], `Paragraph.${method}`);
  }
}

function layoutParagraph(paragraph, request) {
  if (request.wrap && request.widthConstraint !== null) {
    paragraph.layout(request.widthConstraint);
    return;
  }
  paragraph.layout(PROBE_WIDTH);
  const measured = Math.max(
    nonNegative(paragraph.getLongestLine(), "longest line"),
    nonNegative(paragraph.getMaxIntrinsicWidth(), "maximum intrinsic width"),
  );
  paragraph.layout(Math.max(measured, request.widthConstraint ?? 0));
}

function collectLines(paragraph, request, direction) {
  const metrics = paragraph.getLineMetrics();
  if (!Array.isArray(metrics)) fail("invalid-geometry", "CanvasKit returned invalid line metrics");
  const lines = [];
  let previousHeight = 0;
  for (const metric of metrics) {
    if (lines.length === MAX_GEOMETRY_RECORDS) {
      fail("resource-exhausted", "CanvasKit paragraph line geometry exceeds the wire limit");
    }
    // CanvasKit exposes line metrics in UTF-16 even though shaped-run offsets
    // and Paragraph::getLineNumberAt use SkParagraph's internal UTF-8 indices.
    // Convert at this boundary so Fission only ever observes source UTF-8.
    const utf16Start = index(
      metric.startIndex,
      request.textInfo.text.length,
      "line UTF-16 start",
    );
    const utf16End = index(
      metric.endIncludingNewline,
      request.textInfo.text.length,
      "line UTF-16 end",
    );
    const start = request.textInfo.utf16ToByte[utf16Start];
    const end = request.textInfo.utf16ToByte[utf16End];
    if (start < 0 || end < 0 || start > end) {
      fail("invalid-geometry", "CanvasKit returned a line outside Unicode boundaries");
    }
    const cumulativeHeight = nonNegative(Number(metric.height), "line cumulative height");
    if (cumulativeHeight < previousHeight) {
      fail("invalid-geometry", "CanvasKit returned decreasing line heights");
    }
    const height = cumulativeHeight - previousHeight;
    const ascent = nonNegative(Number(metric.ascent), "line ascent");
    const descent = nonNegative(Number(metric.descent), "line descent");
    lines.push({
      range: { start, end },
      rect: {
        x: finite(Number(metric.left), "line left"),
        y: previousHeight,
        width: nonNegative(Number(metric.width), "line width"),
        height,
      },
      baseline: finite(Number(metric.baseline), "line baseline"),
      ascent,
      descent,
      leading: Math.max(0, height - ascent - descent),
      hardBreak: metric.isHardBreak === true,
      direction,
    });
    previousHeight = cumulativeHeight;
  }
  return lines;
}

function lineNumber(lines, start, end) {
  let low = 0;
  let high = lines.length - 1;
  while (low <= high) {
    const middle = (low + high) >>> 1;
    const line = lines[middle];
    if (start < line.range.start) high = middle - 1;
    else if (end > line.range.end) low = middle + 1;
    else return middle;
  }
  return -1;
}

function collectGraphemes(CanvasKit, paragraph, request, lines) {
  const inline = new Set(request.inlineObjects.map((item) => (
    inlineKey(item.range.start, item.range.end)
  )));
  const seen = new Set();
  const graphemes = [];
  let coveredUtf16End = 0;
  let wordBoundary = null;
  for (const scalar of request.textInfo.scalars) {
    if (inline.has(inlineKey(scalar.utf8Start, scalar.utf8End))) continue;
    if (scalar.utf16Start < coveredUtf16End) continue;
    const info = paragraph.getGlyphInfoAt(scalar.utf16Start);
    if (!info) continue;
    const utf16Start = index(
      info.graphemeClusterTextRange?.start,
      request.textInfo.text.length,
      "grapheme UTF-16 start",
    );
    const utf16End = index(
      info.graphemeClusterTextRange?.end,
      request.textInfo.text.length,
      "grapheme UTF-16 end",
    );
    if (
      utf16Start >= utf16End ||
      utf16Start > scalar.utf16Start ||
      scalar.utf16Start >= utf16End
    ) {
      fail("invalid-geometry", "CanvasKit returned an invalid grapheme range");
    }
    coveredUtf16End = utf16End;
    const start = request.textInfo.utf16ToByte[utf16Start];
    const end = request.textInfo.utf16ToByte[utf16End];
    if (start < 0 || end < 0 || start >= end) {
      fail("invalid-geometry", "CanvasKit returned a grapheme outside UTF-8 boundaries");
    }
    const key = inlineKey(start, end);
    if (seen.has(key) || inline.has(key)) continue;
    seen.add(key);
    const line = lineNumber(lines, start, end);
    if (line < 0) continue;
    if (
      wordBoundary === null ||
      utf16Start < wordBoundary.start ||
      utf16Start >= wordBoundary.end
    ) {
      wordBoundary = paragraph.getWordBoundary(utf16Start);
    }
    const boundary = wordBoundary;
    if (
      !boundary ||
      !Number.isSafeInteger(boundary.start) ||
      !Number.isSafeInteger(boundary.end) ||
      boundary.start < 0 ||
      boundary.start > boundary.end ||
      boundary.end > request.textInfo.text.length
    ) {
      fail("invalid-geometry", "CanvasKit returned an invalid UTF-16 word boundary");
    }
    const wordStart = request.textInfo.utf16ToByte[boundary.start];
    const wordEnd = request.textInfo.utf16ToByte[boundary.end];
    if (wordStart < 0 || wordEnd < 0 || wordStart > wordEnd) {
      fail("invalid-geometry", "CanvasKit returned a word boundary inside a Unicode scalar");
    }
    if (graphemes.length === MAX_GEOMETRY_RECORDS) {
      fail("resource-exhausted", "CanvasKit grapheme geometry exceeds the wire limit");
    }
    graphemes.push({
      start,
      end,
      lineIndex: line,
      direction: enumEquals(info.dir, CanvasKit.TextDirection.RTL) ? 1 : 0,
      rect: rect(info.graphemeLayoutBounds, "grapheme rectangle"),
      startsWord: wordStart === start,
    });
  }
  return graphemes;
}

function collectTextGeometry(CanvasKit, paragraph, request, lines, graphemes) {
  const carets = [];
  const hitRegions = [];
  const tightHeight = requireEnum(CanvasKit.RectHeightStyle, "Tight", "RectHeightStyle");
  const tightWidth = requireEnum(CanvasKit.RectWidthStyle, "Tight", "RectWidthStyle");
  for (const grapheme of graphemes) {
    const boxes = paragraph.getRectsForRange(
      request.textInfo.byteToUtf16[grapheme.start],
      request.textInfo.byteToUtf16[grapheme.end],
      tightHeight,
      tightWidth,
    );
    if (!Array.isArray(boxes) || boxes.length === 0) continue;
    const box = boxes[0];
    const bounds = rect(box?.rect, "grapheme tight rectangle");
    const rtl = enumEquals(box?.dir, CanvasKit.TextDirection.RTL);
    const startX = rtl ? bounds.x + bounds.width : bounds.x;
    const endX = rtl ? bounds.x : bounds.x + bounds.width;
    const height = Math.max(1, bounds.height);
    if (
      carets.length > MAX_GEOMETRY_RECORDS - 2 ||
      hitRegions.length > MAX_GEOMETRY_RECORDS - 2
    ) {
      fail("resource-exhausted", "CanvasKit caret/hit geometry exceeds the wire limit");
    }
    carets.push(
      {
        index: BigInt(grapheme.start),
        affinity: 0,
        rect: { x: startX, y: bounds.y, width: 1, height },
        lineIndex: grapheme.lineIndex,
      },
      {
        index: BigInt(grapheme.end),
        affinity: 1,
        rect: { x: endX, y: bounds.y, width: 1, height },
        lineIndex: grapheme.lineIndex,
      },
    );
    const hitWidth = Math.max(1, bounds.width);
    const half = hitWidth * 0.5;
    hitRegions.push(
      {
        rect: { x: bounds.x, y: bounds.y, width: half, height },
        index: BigInt(rtl ? grapheme.end : grapheme.start),
        affinity: rtl ? 1 : 0,
        lineIndex: grapheme.lineIndex,
      },
      {
        rect: { x: bounds.x + half, y: bounds.y, width: hitWidth - half, height },
        index: BigInt(rtl ? grapheme.start : grapheme.end),
        affinity: rtl ? 0 : 1,
        lineIndex: grapheme.lineIndex,
      },
    );
  }
  return { carets, hitRegions };
}

function collectInlineBoxes(paragraph, request, lines, carets) {
  const boxes = paragraph.getRectsForPlaceholders();
  if (!Array.isArray(boxes)) {
    fail("invalid-geometry", "CanvasKit returned invalid placeholder rectangles");
  }
  const result = [];
  let boxIndex = 0;
  for (const input of request.inlineObjects) {
    const visibleLine = lineNumber(lines, input.range.start, input.range.end);
    if (visibleLine < 0) continue;
    const line = lines[visibleLine];
    if (input.width === 0 && input.height === 0) {
      const caret = carets.find((candidate) => (
        candidate.index === BigInt(input.range.start) || candidate.index === BigInt(input.range.end)
      ));
      let x = caret?.rect.x ?? line.rect.x;
      if (boxIndex < boxes.length) {
        const candidate = rect(boxes[boxIndex]?.rect, "zero-size placeholder rectangle");
        // CanvasKit versions differ on whether they return zero-area
        // placeholders. Consume one only when it is unambiguously this object.
        if (candidate.width === 0 && candidate.height === 0) {
          x = candidate.x;
          boxIndex += 1;
        }
      }
      result.push({
        id: input.id,
        range: { start: BigInt(input.range.start), end: BigInt(input.range.end) },
        rect: { x, y: line.rect.y, width: 0, height: 0 },
        baseline: input.baseline,
      });
      continue;
    }
    if (boxIndex >= boxes.length) break;
    result.push({
      id: input.id,
      range: { start: BigInt(input.range.start), end: BigInt(input.range.end) },
      rect: rect(boxes[boxIndex]?.rect, "placeholder rectangle"),
      baseline: input.baseline,
    });
    boxIndex += 1;
  }
  if (boxIndex !== boxes.length) {
    fail("invalid-geometry", "CanvasKit returned unmatched placeholder rectangles");
  }
  return result;
}

function shapedGlyphs(paragraph) {
  const shaped = paragraph.getShapedLines();
  if (!Array.isArray(shaped)) fail("invalid-geometry", "CanvasKit returned invalid shaped lines");
  const byOffset = new Map();
  try {
    for (const line of shaped) {
      if (!Array.isArray(line?.runs)) fail("invalid-geometry", "CanvasKit returned invalid shaped runs");
      for (const run of line.runs) {
        const glyphs = run?.glyphs;
        const offsets = run?.offsets;
        if (!glyphs || !offsets || offsets.length !== glyphs.length + 1) {
          fail("invalid-geometry", "CanvasKit returned invalid shaped glyph offsets");
        }
        for (let glyph = 0; glyph < glyphs.length; glyph += 1) {
          const offset = Number(offsets[glyph]);
          if (!Number.isSafeInteger(offset) || offset < 0) {
            fail("invalid-geometry", "CanvasKit returned invalid shaped glyph offset");
          }
          const values = byOffset.get(offset) ?? [];
          values.push(Number(glyphs[glyph]));
          byOffset.set(offset, values);
        }
      }
    }
    return byOffset;
  } finally {
    for (const line of shaped) {
      if (!Array.isArray(line?.runs)) continue;
      for (const run of line.runs) safeDelete(run?.typeface);
    }
  }
}

function collectUnresolved(paragraph, request) {
  const values = paragraph.unresolvedCodepoints();
  if (!Array.isArray(values)) {
    fail("invalid-geometry", "CanvasKit returned invalid unresolved codepoints");
  }
  const unresolved = new Set();
  for (const value of values) {
    if (
      !Number.isInteger(value) ||
      value < 0 ||
      value > 0x10ffff ||
      (value >= 0xd800 && value <= 0xdfff)
    ) {
      fail("invalid-geometry", "CanvasKit returned an invalid unresolved Unicode scalar");
    }
    unresolved.add(value);
  }
  if (unresolved.size === 0) return { unresolvedGlyphs: [], unresolvedCodepoints: [] };
  const inline = new Set(request.inlineObjects.map((item) => (
    inlineKey(item.range.start, item.range.end)
  )));
  const glyphs = shapedGlyphs(paragraph);
  const unresolvedGlyphs = [];
  const unresolvedCodepoints = [];
  const found = new Set();
  for (const scalar of request.textInfo.scalars) {
    if (!unresolved.has(scalar.codepoint)) continue;
    if (inline.has(inlineKey(scalar.utf8Start, scalar.utf8End))) continue;
    const ids = glyphs.get(scalar.utf8Start);
    if (!ids) {
      fail(
        "unsupported-api",
        "CanvasKit cannot attribute an unresolved glyph to its exact UTF-8 occurrence",
      );
    }
    if (!ids.includes(0)) continue;
    if (
      unresolvedGlyphs.length === MAX_GEOMETRY_RECORDS ||
      unresolvedCodepoints.length === MAX_UNRESOLVED_CODEPOINTS
    ) {
      fail("resource-exhausted", "CanvasKit unresolved glyph geometry exceeds the wire limit");
    }
    found.add(scalar.codepoint);
    const codepointStart = BigInt(unresolvedCodepoints.length);
    unresolvedCodepoints.push(scalar.codepoint);
    unresolvedGlyphs.push({
      range: { start: BigInt(scalar.utf8Start), end: BigInt(scalar.utf8End) },
      codepointStart,
      codepointCount: 1n,
    });
  }
  for (const value of unresolved) {
    if (!found.has(value)) {
      fail(
        "unsupported-api",
        "CanvasKit reported an unresolved codepoint without an attributable missing glyph",
      );
    }
  }
  return { unresolvedGlyphs, unresolvedCodepoints };
}

function ensureLineGeometry(request, lines, clusters, carets, hitRegions) {
  const inline = new Set(request.inlineObjects.map((item) => (
    inlineKey(item.range.start, item.range.end)
  )));
  const clusterLines = new Set(clusters.map((item) => item.lineIndex));
  const caretLines = new Set(carets.map((item) => item.lineIndex));
  const hitLines = new Set(hitRegions.map((item) => item.lineIndex));
  const regularLines = new Set();
  for (const scalar of request.textInfo.scalars) {
    if (
      scalar.codepoint === 0x0a ||
      scalar.codepoint === 0x0d ||
      inline.has(inlineKey(scalar.utf8Start, scalar.utf8End))
    ) {
      continue;
    }
    const visibleLine = lineNumber(lines, scalar.utf8Start, scalar.utf8End);
    if (visibleLine >= 0) regularLines.add(visibleLine);
  }
  lines.forEach((line, lineIndex) => {
    const hasCluster = clusterLines.has(lineIndex);
    const hasCaret = caretLines.has(lineIndex);
    const hasHit = hitLines.has(lineIndex);
    if (regularLines.has(lineIndex) && (!hasCluster || !hasCaret || !hasHit)) {
      fail("invalid-geometry", "CanvasKit returned incomplete immutable text geometry");
    }
    if (!hasCaret || !hasHit) {
      if (
        carets.length === MAX_GEOMETRY_RECORDS ||
        hitRegions.length === MAX_GEOMETRY_RECORDS
      ) {
        fail("resource-exhausted", "CanvasKit empty-line geometry exceeds the wire limit");
      }
      const height = Math.max(1, line.rect.height);
      const fallback = {
        rect: { x: line.rect.x, y: line.rect.y, width: 1, height },
        index: BigInt(line.range.start),
        affinity: 0,
        lineIndex,
      };
      carets.push({ ...fallback, rect: { ...fallback.rect } });
      hitRegions.push(fallback);
    }
  });
}

function makeOutput(CanvasKit, paragraph, request, direction) {
  const lines = collectLines(paragraph, request, direction.wire);
  const graphemes = collectGraphemes(CanvasKit, paragraph, request, lines);
  const clusters = graphemes.map((item) => ({
    range: { start: BigInt(item.start), end: BigInt(item.end) },
    rect: item.rect,
    lineIndex: item.lineIndex,
    direction: item.direction,
    startsGrapheme: true,
    startsWord: item.startsWord,
  }));
  const { carets, hitRegions } = collectTextGeometry(
    CanvasKit,
    paragraph,
    request,
    lines,
    graphemes,
  );
  const inlineBoxes = collectInlineBoxes(paragraph, request, lines, carets);
  const { unresolvedGlyphs, unresolvedCodepoints } = collectUnresolved(paragraph, request);
  ensureLineGeometry(request, lines, clusters, carets, hitRegions);
  const minIntrinsicWidth = nonNegative(
    paragraph.getMinIntrinsicWidth(),
    "minimum intrinsic width",
  );
  const maxIntrinsicWidth = nonNegative(
    paragraph.getMaxIntrinsicWidth(),
    "maximum intrinsic width",
  );
  if (minIntrinsicWidth > maxIntrinsicWidth) {
    fail("invalid-geometry", "CanvasKit returned unordered intrinsic widths");
  }
  const width = request.paragraphStyle.textWidthBasis === 0 && request.widthConstraint !== null
    ? request.widthConstraint
    : nonNegative(paragraph.getLongestLine(), "paragraph width");
  return {
    capabilities: ALL_CAPABILITIES,
    indexEncoding: 0,
    size: { width, height: nonNegative(paragraph.getHeight(), "paragraph height") },
    minIntrinsicWidth,
    maxIntrinsicWidth,
    firstBaseline: lines.length === 0 ? null : lines[0].baseline,
    lastBaseline: lines.length === 0 ? null : lines[lines.length - 1].baseline,
    lines: lines.map((line) => ({
      ...line,
      range: { start: BigInt(line.range.start), end: BigInt(line.range.end) },
    })),
    clusters,
    carets,
    hitRegions,
    inlineBoxes,
    unresolvedGlyphs,
    unresolvedCodepoints,
  };
}

function approximateBytes(request, output, fontBytes) {
  // CanvasKit does not expose Paragraph::approximateBytesUsed. Account for all
  // known retained payloads plus conservative JS/Wasm record overhead. This is
  // intentionally an estimate; the cache must also enforce a retained-count cap.
  const records =
    output.lines.length * 160 +
    output.clusters.length * 176 +
    output.carets.length * 112 +
    output.hitRegions.length * 112 +
    output.inlineBoxes.length * 144 +
    output.unresolvedGlyphs.length * 96 +
    output.unresolvedCodepoints.length * 4;
  return BigInt(
    request.packetBytes +
    request.textInfo.text.length * 2 +
    fontBytes +
    records +
    2048,
  );
}

function requireCanvasKit(CanvasKit, resolveResource) {
  if (!CanvasKit || typeof CanvasKit !== "object") {
    throw new TypeError("CanvasKit must be an initialized CanvasKit module");
  }
  if (typeof resolveResource !== "function") {
    throw new TypeError("resolveResource must resolve Fission resource slots");
  }
  requireFunction(CanvasKit.Color4f, "Color4f");
  requireFunction(CanvasKit.TextStyle, "TextStyle");
  requireFunction(CanvasKit.ParagraphStyle, "ParagraphStyle");
  requireFunction(CanvasKit.TypefaceFontProvider?.Make, "TypefaceFontProvider.Make");
  requireFunction(
    CanvasKit.ParagraphBuilder?.MakeFromFontProvider,
    "ParagraphBuilder.MakeFromFontProvider",
  );
}

function validHandle(handle) {
  return (
    handle &&
    Number.isInteger(handle.slot) &&
    handle.slot > 0 &&
    handle.slot <= MAX_U32 &&
    Number.isInteger(handle.generation) &&
    handle.generation > 0 &&
    handle.generation <= MAX_U32
  );
}

/**
 * Owns immutable CanvasKit SkParagraphs for the FSPQ/FSPR Web transport.
 *
 * `resolveResource(slot)` must return the executor's generation-aware resource
 * entry `{ generation, kind, bytes }`. Paragraph objects never escape this host;
 * `prepare()` returns an opaque painter that can only draw the exact retained
 * layout that produced the response geometry.
 */
export function createCanvasKitParagraphHost({ CanvasKit, resolveResource }) {
  requireCanvasKit(CanvasKit, resolveResource);
  const entries = new Map();
  const fontProviders = new Map();
  const freeSlots = [];
  let nextSlot = 1;

  function allocateHandle() {
    while (freeSlots.length > 0) {
      const handle = freeSlots.pop();
      if (handle.generation !== 0) return handle;
    }
    if (nextSlot > MAX_U32) fail("resource-exhausted", "paragraph handle slots are exhausted");
    const handle = { slot: nextSlot, generation: 1 };
    nextSlot += 1;
    return handle;
  }

  function releaseHandle(handle) {
    if (handle.generation < MAX_U32) {
      freeSlots.push({ slot: handle.slot, generation: handle.generation + 1 });
    }
  }

  function liveEntry(handle) {
    if (!validHandle(handle)) return null;
    const entry = entries.get(handle.slot);
    return entry?.handle.generation === handle.generation ? entry : null;
  }

  function retainFontProvider(request) {
    const key = fontProviderKey(request);
    const existing = fontProviders.get(key);
    if (existing) {
      existing.references += 1;
      return { key, provider: existing.provider };
    }
    const created = makeFontProvider(CanvasKit, request, resolveResource);
    fontProviders.set(key, {
      provider: created.provider,
      fontBytes: created.fontBytes,
      references: 1,
    });
    return { key, provider: created.provider };
  }

  function releaseFontProvider(key) {
    const entry = fontProviders.get(key);
    if (!entry) return;
    entry.references -= 1;
    if (entry.references > 0) return;
    fontProviders.delete(key);
    safeDelete(entry.provider);
  }

  function disposeEntry(entry) {
    safeDelete(entry.paragraph);
    releaseFontProvider(entry.fontProviderKey);
  }

  function paint(handle, canvas, x, y, scaleFactor) {
    const entry = liveEntry(handle);
    if (!entry) fail("stale-handle", "paragraph draw handle is not live");
    if (!canvas || typeof canvas.drawParagraph !== "function") {
      fail("unsupported-api", "CanvasKit canvas does not expose drawParagraph");
    }
    if (
      !Number.isFinite(x) ||
      !Number.isFinite(y) ||
      !Number.isFinite(scaleFactor) ||
      scaleFactor <= 0
    ) {
      fail("invalid-draw", "paragraph origin and scale factor must be finite and positive");
    }
    requireFunction(canvas.save, "Canvas.save");
    requireFunction(canvas.translate, "Canvas.translate");
    requireFunction(canvas.scale, "Canvas.scale");
    requireFunction(canvas.restore, "Canvas.restore");
    canvas.save();
    try {
      canvas.translate(x, y);
      canvas.scale(scaleFactor, scaleFactor);
      canvas.drawParagraph(entry.paragraph, 0, 0);
    } finally {
      canvas.restore();
    }
  }

  function layout(packet) {
    const request = decodeParagraphRequest(packet);
    let fontProviderLease = null;
    let builder = null;
    let paragraph = null;
    let retainedHandle = null;
    try {
      fontProviderLease = retainFontProvider(request);
      const direction = paragraphDirection(CanvasKit, request);
      const style = makeParagraphStyle(CanvasKit, request, direction);
      builder = CanvasKit.ParagraphBuilder.MakeFromFontProvider(
        style,
        fontProviderLease.provider,
      );
      if (!builder || typeof builder.build !== "function" || typeof builder.delete !== "function") {
        fail("layout-failure", "CanvasKit could not create a ParagraphBuilder");
      }
      for (const method of ["pushStyle", "addText", "addPlaceholder", "pop"]) {
        requireFunction(builder[method], `ParagraphBuilder.${method}`);
      }
      addTextRuns(CanvasKit, builder, request);
      paragraph = builder.build();
      safeDelete(builder);
      builder = null;
      if (!paragraph) fail("layout-failure", "CanvasKit could not build SkParagraph");
      requireParagraphApi(paragraph);
      layoutParagraph(paragraph, request);
      const output = makeOutput(CanvasKit, paragraph, request, direction);
      // Shared provider bytes belong to the executor resource table, so the
      // paragraph registry charges only memory retained per paragraph.
      const retainedBytes = approximateBytes(request, output, 0);
      retainedHandle = allocateHandle();
      const entry = {
        handle: retainedHandle,
        paragraph,
        fontProviderKey: fontProviderLease.key,
        approximateBytes: retainedBytes,
      };
      entries.set(retainedHandle.slot, entry);
      paragraph = null;
      fontProviderLease = null;
      return encodeParagraphResponse({
        handle: retainedHandle,
        approximateBytes: retainedBytes,
        output,
      });
    } catch (error) {
      if (retainedHandle !== null) {
        const entry = entries.get(retainedHandle.slot);
        if (entry?.handle.generation === retainedHandle.generation) {
          entries.delete(retainedHandle.slot);
          disposeEntry(entry);
          releaseHandle(retainedHandle);
        }
      }
      safeDelete(paragraph);
      safeDelete(builder);
      if (fontProviderLease !== null) releaseFontProvider(fontProviderLease.key);
      if (
        error instanceof CanvasKitParagraphError ||
        error?.name === "ParagraphWireError"
      ) {
        throw error;
      }
      fail("layout-failure", `CanvasKit paragraph layout failed: ${errorMessage(error)}`, error);
    }
  }

  function resolve(handle) {
    const entry = liveEntry(handle);
    if (!entry) return null;
    return Object.freeze({
      handle: Object.freeze({ ...entry.handle }),
      approximateBytes: entry.approximateBytes,
    });
  }

  function prepare(handle) {
    const entry = liveEntry(handle);
    if (!entry) fail("stale-handle", "paragraph draw handle is not live");
    const ownedHandle = Object.freeze({ ...entry.handle });
    return Object.freeze({
      handle: ownedHandle,
      paint: (canvas, x, y, scaleFactor) => paint(ownedHandle, canvas, x, y, scaleFactor),
    });
  }

  function destroy(handle) {
    const entry = liveEntry(handle);
    if (!entry) return false;
    entries.delete(handle.slot);
    disposeEntry(entry);
    releaseHandle(entry.handle);
    return true;
  }

  function clear() {
    for (const entry of entries.values()) {
      disposeEntry(entry);
      releaseHandle(entry.handle);
    }
    entries.clear();
    // Every live paragraph owns one provider reference. Be defensive if an
    // Embind failure interrupted normal reference retirement.
    for (const entry of fontProviders.values()) safeDelete(entry.provider);
    fontProviders.clear();
  }

  return Object.freeze({ layout, resolve, prepare, destroy, clear });
}
