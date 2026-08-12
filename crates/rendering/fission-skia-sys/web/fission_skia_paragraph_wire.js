// Strict, pointer-free FSPQ/FSPR paragraph transport shared by Rust and CanvasKit.

export const PARAGRAPH_VERSION = 1;
export const MAX_PARAGRAPH_PACKET_BYTES = 16 * 1024 * 1024;

const REQUEST_MAGIC = [0x46, 0x53, 0x50, 0x51]; // FSPQ
const RESPONSE_MAGIC = [0x46, 0x53, 0x50, 0x52]; // FSPR
const REQUEST_HEADER_LEN = 144;
const RESPONSE_HEADER_LEN = 128;
const STYLE_HEADER_LEN = 72;
const INLINE_LEN = 40;
const FONT_HEADER_LEN = 16;
const LINE_LEN = 56;
const CLUSTER_LEN = 48;
const CARET_LEN = 40;
const HIT_REGION_LEN = 40;
const INLINE_BOX_LEN = 48;
const UNRESOLVED_GLYPH_LEN = 32;

const MAX_TEXT_BYTES = 4 * 1024 * 1024;
const MAX_STRING_BYTES = 1024 * 1024;
const MAX_STYLE_RUNS = 65_536;
const MAX_INLINE_OBJECTS = 65_536;
const MAX_FALLBACK_FAMILIES = 4_096;
const MAX_FONT_RESOURCES = 4_096;
const MAX_VARIATIONS = 65_536;
const MAX_FEATURES = 65_536;
const MAX_GEOMETRY_RECORDS = 1_048_576;
const MAX_UNRESOLVED_CODEPOINTS = 1_048_576;

const FLAG_WRAP = 1 << 0;
const FLAG_WIDTH = 1 << 1;
const FLAG_LOCALE = 1 << 2;
const FLAG_SELECTION = 1 << 3;
const FLAG_PREEDIT = 1 << 4;
const FLAG_MAX_LINES = 1 << 5;
const FLAG_STRUT = 1 << 6;
const FLAG_FIRST_ASCENT = 1 << 7;
const FLAG_LAST_DESCENT = 1 << 8;
const KNOWN_FLAGS = (1 << 9) - 1;

const STYLE_UNDERLINE = 1 << 0;
const STYLE_FAMILY = 1 << 1;
const STYLE_LOCALE = 1 << 2;
const STYLE_LINE_HEIGHT = 1 << 3;
const STYLE_BACKGROUND = 1 << 4;
const KNOWN_STYLE_FLAGS = (1 << 5) - 1;

const RESPONSE_FIRST_BASELINE = 1 << 0;
const RESPONSE_LAST_BASELINE = 1 << 1;
const RESPONSE_KNOWN_FLAGS = RESPONSE_FIRST_BASELINE | RESPONSE_LAST_BASELINE;
const ALL_CAPABILITIES = 0x1ffn;

const utf8Decoder = new TextDecoder("utf-8", { fatal: true });

export class ParagraphWireError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "ParagraphWireError";
    this.code = code;
  }
}

function reject(code, message) {
  throw new ParagraphWireError(code, message);
}

function toBytes(input) {
  if (input instanceof Uint8Array) return input;
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  if (input instanceof ArrayBuffer) return new Uint8Array(input);
  reject("invalid-buffer", "paragraph packet must be an ArrayBuffer or typed-array view");
}

class Reader {
  constructor(bytes) {
    this.bytes = bytes;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    this.position = 0;
  }

  remaining() {
    return this.bytes.byteLength - this.position;
  }

  require(length) {
    if (!Number.isSafeInteger(length) || length < 0 || length > this.remaining()) {
      reject("truncated", "paragraph packet is truncated");
    }
  }

  take(length) {
    this.require(length);
    const result = this.bytes.subarray(this.position, this.position + length);
    this.position += length;
    return result;
  }

  u8() {
    this.require(1);
    return this.view.getUint8(this.position++);
  }

  u16() {
    this.require(2);
    const value = this.view.getUint16(this.position, true);
    this.position += 2;
    return value;
  }

  u32() {
    this.require(4);
    const value = this.view.getUint32(this.position, true);
    this.position += 4;
    return value;
  }

  u64() {
    this.require(8);
    const value = this.view.getBigUint64(this.position, true);
    this.position += 8;
    return value;
  }

  f32() {
    this.require(4);
    const value = this.view.getFloat32(this.position, true);
    this.position += 4;
    return value;
  }

  f32WithBits() {
    this.require(4);
    const bits = this.view.getUint32(this.position, true);
    const value = this.view.getFloat32(this.position, true);
    this.position += 4;
    return { value, bits };
  }

  finish() {
    if (this.remaining() !== 0) reject("length-mismatch", "paragraph packet has trailing bytes");
  }
}

class Writer {
  constructor(length) {
    this.bytes = new Uint8Array(length);
    this.view = new DataView(this.bytes.buffer);
    this.position = 0;
  }

  raw(bytes) {
    this.bytes.set(bytes, this.position);
    this.position += bytes.byteLength;
  }

  zeros(length) {
    this.position += length;
  }

  u8(value) {
    this.view.setUint8(this.position, value);
    this.position += 1;
  }

  u16(value) {
    this.view.setUint16(this.position, value, true);
    this.position += 2;
  }

  u32(value) {
    this.view.setUint32(this.position, value, true);
    this.position += 4;
  }

  u64(value) {
    this.view.setBigUint64(this.position, BigInt(value), true);
    this.position += 8;
  }

  f32(value) {
    this.view.setFloat32(this.position, value, true);
    this.position += 4;
  }

  finish() {
    if (this.position !== this.bytes.byteLength) {
      throw new Error("paragraph response encoder length mismatch");
    }
    return this.bytes;
  }
}

function requireMagic(reader, expected) {
  const actual = reader.take(4);
  if (expected.some((value, index) => actual[index] !== value)) {
    reject("invalid-magic", "paragraph packet has invalid magic");
  }
}

function requireZero(value, field) {
  if (value !== 0) reject("nonzero-reserved", `${field} reserved value must be zero`);
}

function requireZeroBytes(bytes, field) {
  if (bytes.some((value) => value !== 0)) {
    reject("nonzero-reserved", `${field} reserved bytes must be zero`);
  }
}

function requireLimit(field, actual, maximum) {
  if (!Number.isSafeInteger(actual) || actual < 0 || actual > maximum) {
    reject("limit-exceeded", `${field} ${actual} exceeds ${maximum}`);
  }
}

function requireRemaining(reader, count, recordLength, field) {
  const needed = count * recordLength;
  if (!Number.isSafeInteger(needed) || needed > reader.remaining()) {
    reject("truncated", `${field} records are truncated`);
  }
}

function finite(value, field) {
  if (!Number.isFinite(value)) reject("invalid-value", `${field} must be finite`);
  return value;
}

function nonNegative(value, field) {
  finite(value, field);
  if (value < 0) reject("invalid-value", `${field} must be non-negative`);
  return value;
}

function positive(value, field) {
  finite(value, field);
  if (value <= 0) reject("invalid-value", `${field} must be positive`);
  return value;
}

function utf8(bytes, field) {
  let value;
  try {
    value = utf8Decoder.decode(bytes);
  } catch (_error) {
    reject("invalid-utf8", `${field} must be valid UTF-8`);
  }
  return value;
}

function requiredString(bytes, field) {
  requireLimit(field, bytes.byteLength, MAX_STRING_BYTES);
  const value = utf8(bytes, field);
  if (value.length === 0 || value.includes("\0")) {
    reject("invalid-value", `${field} must be nonempty and NUL-free`);
  }
  return value;
}

function optionalString(bytes, present, field) {
  requireLimit(field, bytes.byteLength, MAX_STRING_BYTES);
  const value = utf8(bytes, field);
  if (present) {
    if (value.length === 0 || value.includes("\0")) {
      reject("invalid-value", `${field} must be nonempty and NUL-free`);
    }
    return value;
  }
  if (value.length !== 0) reject("invalid-value", `${field} has an absent payload`);
  return null;
}

function checkedIndex(value, maximum, field) {
  if (value > BigInt(maximum)) reject("invalid-value", `${field} is outside paragraph text`);
  return Number(value);
}

function decodeRange(reader) {
  return { start: reader.u64(), end: reader.u64() };
}

function validateRange(range, textInfo, field) {
  const start = checkedIndex(range.start, textInfo.bytes, field);
  const end = checkedIndex(range.end, textInfo.bytes, field);
  if (start > end || textInfo.byteToUtf16[start] < 0 || textInfo.byteToUtf16[end] < 0) {
    reject("invalid-value", `${field} is not a valid UTF-8 range`);
  }
  return { start, end };
}

function decodeText(bytes) {
  const text = utf8(bytes, "paragraph text");
  const byteToUtf16 = new Int32Array(bytes.byteLength + 1);
  byteToUtf16.fill(-1);
  const utf16ToByte = new Int32Array(text.length + 1);
  utf16ToByte.fill(-1);
  const scalars = [];
  let byteOffset = 0;
  let utf16Offset = 0;
  byteToUtf16[0] = 0;
  utf16ToByte[0] = 0;
  for (const character of text) {
    const codepoint = character.codePointAt(0);
    const utf8Length = codepoint <= 0x7f ? 1 : codepoint <= 0x7ff ? 2 : codepoint <= 0xffff ? 3 : 4;
    const utf16Length = character.length;
    scalars.push({
      codepoint,
      character,
      utf8Start: byteOffset,
      utf8End: byteOffset + utf8Length,
      utf16Start: utf16Offset,
      utf16End: utf16Offset + utf16Length,
    });
    byteOffset += utf8Length;
    utf16Offset += utf16Length;
    byteToUtf16[byteOffset] = utf16Offset;
    utf16ToByte[utf16Offset] = byteOffset;
  }
  if (byteOffset !== bytes.byteLength) {
    throw new Error("fatal UTF-8 decoder produced an inconsistent byte mapping");
  }
  return { text, bytes: bytes.byteLength, byteToUtf16, utf16ToByte, scalars };
}

function decodeStyle(reader, textInfo, totals) {
  const entryLength = reader.u32();
  if (entryLength < STYLE_HEADER_LEN || entryLength - 4 > reader.remaining()) {
    reject("length-mismatch", "paragraph style entry length is invalid");
  }
  const entry = new Reader(reader.take(entryLength - 4));
  const flags = entry.u32();
  if ((flags & ~KNOWN_STYLE_FLAGS) !== 0) reject("invalid-flags", "style flags are invalid");
  const rawRange = decodeRange(entry);
  const fontSize = positive(entry.f32(), "style font size");
  const color = [entry.u8(), entry.u8(), entry.u8(), entry.u8()];
  const fontWeight = entry.u16();
  if (fontWeight < 1 || fontWeight > 1000) {
    reject("invalid-value", "style font weight must be in 1..=1000");
  }
  const fontSlant = entry.u8();
  if (fontSlant > 1) reject("invalid-enum", "style font slant is invalid");
  requireZero(entry.u8(), "style");
  const lineHeightValue = entry.f32WithBits();
  const letterSpacing = finite(entry.f32(), "style letter spacing");
  const backgroundColor = [entry.u8(), entry.u8(), entry.u8(), entry.u8()];
  const fontWidth = positive(entry.f32(), "style font width");
  const wordSpacing = finite(entry.f32(), "style word spacing");
  const familyLength = entry.u32();
  const localeLength = entry.u32();
  const variationCount = entry.u32();
  const featureCount = entry.u32();
  requireLimit("style font family bytes", familyLength, MAX_STRING_BYTES);
  requireLimit("style locale bytes", localeLength, MAX_STRING_BYTES);
  totals.variations += variationCount;
  totals.features += featureCount;
  requireLimit("style variations", totals.variations, MAX_VARIATIONS);
  requireLimit("style features", totals.features, MAX_FEATURES);
  requireRemaining(entry, variationCount + featureCount, 8, "style variation/feature");
  const fontFamily = optionalString(
    entry.take(familyLength),
    (flags & STYLE_FAMILY) !== 0,
    "style font family",
  );
  const locale = optionalString(
    entry.take(localeLength),
    (flags & STYLE_LOCALE) !== 0,
    "style locale",
  );
  const variations = [];
  for (let index = 0; index < variationCount; index += 1) {
    variations.push({ tag: entry.u32(), value: finite(entry.f32(), "style variation value") });
  }
  const features = [];
  for (let index = 0; index < featureCount; index += 1) {
    features.push({ tag: entry.u32(), value: entry.u32() });
  }
  entry.finish();
  const hasLineHeight = (flags & STYLE_LINE_HEIGHT) !== 0;
  const lineHeight = hasLineHeight
    ? positive(lineHeightValue.value, "style line height")
    : null;
  if (!hasLineHeight && lineHeightValue.bits !== 0) {
    reject("invalid-value", "style line height has an absent payload");
  }
  const hasBackground = (flags & STYLE_BACKGROUND) !== 0;
  if (!hasBackground && backgroundColor.some((value) => value !== 0)) {
    reject("invalid-value", "style background color has an absent payload");
  }
  return {
    range: validateRange(rawRange, textInfo, "style range"),
    fontSize,
    color,
    underline: (flags & STYLE_UNDERLINE) !== 0,
    fontFamily,
    locale,
    fontWeight,
    fontSlant,
    lineHeight,
    letterSpacing,
    backgroundColor: hasBackground ? backgroundColor : null,
    fontWidth,
    wordSpacing,
    variations,
    features,
  };
}

function decodeInline(reader, textInfo) {
  const id = reader.u64();
  const range = validateRange(decodeRange(reader), textInfo, "inline object range");
  const width = nonNegative(reader.f32(), "inline object width");
  const height = nonNegative(reader.f32(), "inline object height");
  const baseline = nonNegative(reader.f32(), "inline object baseline");
  requireZero(reader.u32(), "inline object");
  if (baseline > height) reject("invalid-value", "inline object baseline exceeds its height");
  return { id, range, width, height, baseline };
}

function decodeFont(reader) {
  const handle = { slot: reader.u32(), generation: reader.u32() };
  if (handle.slot === 0 || handle.generation === 0) {
    reject("invalid-value", "paragraph font handle must be nonzero");
  }
  const familyLength = reader.u32();
  requireZero(reader.u32(), "paragraph font");
  return {
    handle,
    family: requiredString(reader.take(familyLength), "paragraph font family"),
  };
}

function validateNormalizedRequest(request) {
  const { textInfo, styles, inlineObjects } = request;
  if (textInfo.bytes !== 0 && styles.length === 0) {
    reject("invalid-value", "style ranges do not cover paragraph text");
  }
  if (textInfo.bytes === 0 && styles.length > 1) {
    reject("invalid-value", "empty text accepts at most one style run");
  }
  let covered = 0;
  for (const style of styles) {
    if (style.range.start !== covered || (textInfo.bytes !== 0 && style.range.start === style.range.end)) {
      reject("invalid-value", "style ranges must cover text contiguously");
    }
    covered = style.range.end;
  }
  if (covered !== textInfo.bytes) reject("invalid-value", "style ranges do not cover paragraph text");

  let previousInlineEnd = 0;
  const ids = new Set();
  for (const inline of inlineObjects) {
    if (inline.range.start === inline.range.end || inline.range.start < previousInlineEnd) {
      reject("invalid-value", "inline object ranges overlap or are empty");
    }
    const start = textInfo.byteToUtf16[inline.range.start];
    const end = textInfo.byteToUtf16[inline.range.end];
    if (textInfo.text.slice(start, end) !== "\ufffc") {
      reject("invalid-value", "inline object range must contain exactly U+FFFC");
    }
    if (!styles.some((style) => style.range.start <= inline.range.start && inline.range.end <= style.range.end)) {
      reject("invalid-value", "inline object must be contained by one style run");
    }
    const key = inline.id.toString();
    if (ids.has(key)) reject("invalid-value", "inline object ids must be unique");
    ids.add(key);
    previousInlineEnd = inline.range.end;
  }
  const fontHandles = new Set();
  for (const font of request.fonts) {
    const key = `${font.handle.slot}:${font.handle.generation}`;
    if (fontHandles.has(key)) reject("invalid-value", "paragraph font handles must be unique");
    fontHandles.add(key);
  }
}

export function decodeParagraphRequest(input) {
  const bytes = toBytes(input);
  requireLimit("paragraph packet bytes", bytes.byteLength, MAX_PARAGRAPH_PACKET_BYTES);
  if (bytes.byteLength < REQUEST_HEADER_LEN) reject("truncated", "paragraph request header is truncated");
  const reader = new Reader(bytes);
  requireMagic(reader, REQUEST_MAGIC);
  const version = reader.u16();
  if (version !== PARAGRAPH_VERSION) reject("unsupported-version", `unsupported paragraph version ${version}`);
  requireZero(reader.u16(), "paragraph request");
  if (reader.u32() !== bytes.byteLength) reject("length-mismatch", "paragraph request length is invalid");
  const flags = reader.u32();
  if ((flags & ~KNOWN_FLAGS) !== 0) reject("invalid-flags", "paragraph request flags are invalid");
  const textLength = reader.u32();
  const styleCount = reader.u32();
  const inlineCount = reader.u32();
  const fallbackCount = reader.u32();
  const fontCatalogGeneration = reader.u64();
  const widthValue = reader.f32WithBits();
  const textAlign = reader.u8();
  const overflow = reader.u8();
  const textDirection = reader.u8();
  const textWidthBasis = reader.u8();
  if (textAlign > 5) reject("invalid-enum", "paragraph text alignment is invalid");
  if (overflow > 3) reject("invalid-enum", "paragraph overflow is invalid");
  if (textDirection > 2) reject("invalid-enum", "paragraph text direction is invalid");
  if (textWidthBasis > 1) reject("invalid-enum", "paragraph width basis is invalid");
  const maxLinesValue = reader.u64();
  const strutValue = reader.f32WithBits();
  const localeLength = reader.u32();
  const fontCount = reader.u32();
  requireZero(reader.u32(), "paragraph request");
  const selectionRaw = decodeRange(reader);
  const preeditRangeRaw = decodeRange(reader);
  const preeditSelectionRaw = decodeRange(reader);
  requireZeroBytes(reader.take(24), "paragraph request");

  requireLimit("paragraph text bytes", textLength, MAX_TEXT_BYTES);
  requireLimit("paragraph style runs", styleCount, MAX_STYLE_RUNS);
  requireLimit("paragraph inline objects", inlineCount, MAX_INLINE_OBJECTS);
  requireLimit("paragraph fallback families", fallbackCount, MAX_FALLBACK_FAMILIES);
  requireLimit("paragraph fonts", fontCount, MAX_FONT_RESOURCES);
  requireLimit("paragraph locale bytes", localeLength, MAX_STRING_BYTES);
  const textInfo = decodeText(reader.take(textLength));
  const locale = optionalString(reader.take(localeLength), (flags & FLAG_LOCALE) !== 0, "paragraph locale");

  const widthConstraint = (flags & FLAG_WIDTH) !== 0
    ? nonNegative(widthValue.value, "paragraph width constraint")
    : null;
  if ((flags & FLAG_WIDTH) === 0 && widthValue.bits !== 0) {
    reject("invalid-value", "paragraph width has an absent payload");
  }
  const maxLines = (flags & FLAG_MAX_LINES) !== 0 ? maxLinesValue : null;
  if ((flags & FLAG_MAX_LINES) !== 0 && maxLinesValue === 0n) {
    reject("invalid-value", "paragraph max lines must be positive");
  }
  if ((flags & FLAG_MAX_LINES) === 0 && maxLinesValue !== 0n) {
    reject("invalid-value", "paragraph max lines has an absent payload");
  }
  const strutLineHeight = (flags & FLAG_STRUT) !== 0
    ? positive(strutValue.value, "paragraph strut line height")
    : null;
  if ((flags & FLAG_STRUT) === 0 && strutValue.bits !== 0) {
    reject("invalid-value", "paragraph strut height has an absent payload");
  }

  requireRemaining(reader, styleCount, STYLE_HEADER_LEN, "paragraph style");
  const totals = { variations: 0, features: 0 };
  const styles = [];
  for (let index = 0; index < styleCount; index += 1) {
    styles.push(decodeStyle(reader, textInfo, totals));
  }
  requireRemaining(reader, inlineCount, INLINE_LEN, "paragraph inline object");
  const inlineObjects = [];
  for (let index = 0; index < inlineCount; index += 1) {
    inlineObjects.push(decodeInline(reader, textInfo));
  }
  requireRemaining(reader, fallbackCount, 4, "paragraph fallback family");
  const fallbackFamilies = [];
  for (let index = 0; index < fallbackCount; index += 1) {
    const length = reader.u32();
    fallbackFamilies.push(requiredString(reader.take(length), "paragraph fallback family"));
  }
  requireRemaining(reader, fontCount, FONT_HEADER_LEN, "paragraph font");
  const fonts = [];
  for (let index = 0; index < fontCount; index += 1) fonts.push(decodeFont(reader));
  reader.finish();

  const zeroRange = (range) => range.start === 0n && range.end === 0n;
  const selection = (flags & FLAG_SELECTION) !== 0
    ? validateRange(selectionRaw, textInfo, "paragraph selection")
    : null;
  if ((flags & FLAG_SELECTION) === 0 && !zeroRange(selectionRaw)) {
    reject("invalid-value", "paragraph selection has an absent payload");
  }
  let preedit = null;
  if ((flags & FLAG_PREEDIT) !== 0) {
    const range = validateRange(preeditRangeRaw, textInfo, "paragraph preedit");
    const selectionRange = validateRange(preeditSelectionRaw, textInfo, "paragraph preedit selection");
    if (selectionRange.start < range.start || selectionRange.end > range.end) {
      reject("invalid-value", "paragraph preedit selection is not contained by preedit");
    }
    preedit = { range, selection: selectionRange };
  } else if (!zeroRange(preeditRangeRaw) || !zeroRange(preeditSelectionRaw)) {
    reject("invalid-value", "paragraph preedit has an absent payload");
  }

  const request = {
    packetBytes: bytes.byteLength,
    textInfo,
    styles,
    inlineObjects,
    fallbackFamilies,
    fonts,
    fontCatalogGeneration,
    widthConstraint,
    wrap: (flags & FLAG_WRAP) !== 0,
    locale,
    selection,
    preedit,
    paragraphStyle: {
      textAlign,
      overflow,
      textDirection,
      textWidthBasis,
      maxLines,
      strutLineHeight,
      applyHeightToFirstAscent: (flags & FLAG_FIRST_ASCENT) !== 0,
      applyHeightToLastDescent: (flags & FLAG_LAST_DESCENT) !== 0,
    },
  };
  validateNormalizedRequest(request);
  return request;
}

function checkedCount(values, field) {
  const count = values.length;
  requireLimit(field, count, MAX_GEOMETRY_RECORDS);
  return count;
}

function responseLength(output) {
  const records = [
    [output.lines.length, LINE_LEN],
    [output.clusters.length, CLUSTER_LEN],
    [output.carets.length, CARET_LEN],
    [output.hitRegions.length, HIT_REGION_LEN],
    [output.inlineBoxes.length, INLINE_BOX_LEN],
    [output.unresolvedGlyphs.length, UNRESOLVED_GLYPH_LEN],
    [output.unresolvedCodepoints.length, 4],
  ];
  let length = RESPONSE_HEADER_LEN;
  for (const [count, size] of records) {
    const addition = count * size;
    if (!Number.isSafeInteger(addition) || !Number.isSafeInteger(length + addition)) {
      reject("length-mismatch", "paragraph response length overflowed");
    }
    length += addition;
  }
  requireLimit("paragraph response bytes", length, MAX_PARAGRAPH_PACKET_BYTES);
  return length;
}

function validateRect(rect, field) {
  finite(rect.x, field);
  finite(rect.y, field);
  nonNegative(rect.width, field);
  nonNegative(rect.height, field);
  if (!Number.isFinite(Math.fround(rect.x + rect.width)) || !Number.isFinite(Math.fround(rect.y + rect.height))) {
    reject("invalid-value", `${field} extent must remain finite in f32`);
  }
}

function writeRange(writer, range) {
  writer.u64(range.start);
  writer.u64(range.end);
}

function writeRect(writer, rect) {
  writer.f32(rect.x);
  writer.f32(rect.y);
  writer.f32(rect.width);
  writer.f32(rect.height);
}

function validateOutput(output) {
  if (output.indexEncoding !== 0 && output.indexEncoding !== 1) {
    reject("invalid-enum", "paragraph output index encoding is invalid");
  }
  if ((BigInt(output.capabilities) & ~ALL_CAPABILITIES) !== 0n) {
    reject("invalid-value", "paragraph output capabilities contain unknown bits");
  }
  nonNegative(output.size.width, "paragraph width");
  nonNegative(output.size.height, "paragraph height");
  nonNegative(output.minIntrinsicWidth, "paragraph minimum intrinsic width");
  nonNegative(output.maxIntrinsicWidth, "paragraph maximum intrinsic width");
  if (output.minIntrinsicWidth > output.maxIntrinsicWidth) {
    reject("invalid-value", "paragraph intrinsic widths are unordered");
  }
  if (output.firstBaseline !== null) finite(output.firstBaseline, "paragraph first baseline");
  if (output.lastBaseline !== null) finite(output.lastBaseline, "paragraph last baseline");
  checkedCount(output.lines, "paragraph lines");
  checkedCount(output.clusters, "paragraph clusters");
  checkedCount(output.carets, "paragraph carets");
  checkedCount(output.hitRegions, "paragraph hit regions");
  checkedCount(output.inlineBoxes, "paragraph inline boxes");
  checkedCount(output.unresolvedGlyphs, "paragraph unresolved glyphs");
  requireLimit(
    "paragraph unresolved codepoints",
    output.unresolvedCodepoints.length,
    MAX_UNRESOLVED_CODEPOINTS,
  );
  for (const line of output.lines) {
    if (line.range.start > line.range.end) reject("invalid-value", "paragraph line range is inverted");
    validateRect(line.rect, "paragraph line rectangle");
    finite(line.baseline, "paragraph line baseline");
    finite(line.ascent, "paragraph line ascent");
    finite(line.descent, "paragraph line descent");
    finite(line.leading, "paragraph line leading");
    if (line.direction !== 0 && line.direction !== 1) reject("invalid-enum", "paragraph line direction is invalid");
  }
  const validateLineIndex = (value, field) => {
    if (!Number.isSafeInteger(value) || value < 0 || value >= output.lines.length) {
      reject("invalid-value", `${field} line index is invalid`);
    }
  };
  for (const cluster of output.clusters) {
    if (cluster.range.start > cluster.range.end) reject("invalid-value", "paragraph cluster range is inverted");
    validateRect(cluster.rect, "paragraph cluster rectangle");
    validateLineIndex(cluster.lineIndex, "paragraph cluster");
    if (cluster.direction !== 0 && cluster.direction !== 1) reject("invalid-enum", "paragraph cluster direction is invalid");
  }
  for (const caret of output.carets) {
    validateRect(caret.rect, "paragraph caret rectangle");
    validateLineIndex(caret.lineIndex, "paragraph caret");
    if (caret.affinity !== 0 && caret.affinity !== 1) reject("invalid-enum", "paragraph caret affinity is invalid");
  }
  for (const hit of output.hitRegions) {
    validateRect(hit.rect, "paragraph hit rectangle");
    validateLineIndex(hit.lineIndex, "paragraph hit");
    if (hit.affinity !== 0 && hit.affinity !== 1) reject("invalid-enum", "paragraph hit affinity is invalid");
  }
  for (const inline of output.inlineBoxes) {
    if (inline.range.start > inline.range.end) reject("invalid-value", "paragraph inline range is inverted");
    validateRect(inline.rect, "paragraph inline rectangle");
    finite(inline.baseline, "paragraph inline baseline");
  }
  for (const glyph of output.unresolvedGlyphs) {
    if (glyph.range.start > glyph.range.end) reject("invalid-value", "unresolved glyph range is inverted");
    const start = Number(glyph.codepointStart);
    const count = Number(glyph.codepointCount);
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(count) || start < 0 || count < 0 || start + count > output.unresolvedCodepoints.length) {
      reject("invalid-value", "unresolved glyph codepoint span is invalid");
    }
  }
  for (const codepoint of output.unresolvedCodepoints) {
    if (!Number.isInteger(codepoint) || codepoint < 0 || codepoint > 0x10ffff || (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
      reject("invalid-value", "unresolved glyph codepoint is not a Unicode scalar");
    }
  }
}

export function encodeParagraphResponse(response) {
  if (!response?.handle || response.handle.slot === 0 || response.handle.generation === 0) {
    reject("invalid-value", "paragraph response handle must be nonzero");
  }
  const output = response.output;
  validateOutput(output);
  const length = responseLength(output);
  const writer = new Writer(length);
  writer.raw(Uint8Array.from(RESPONSE_MAGIC));
  writer.u16(PARAGRAPH_VERSION);
  writer.u16(0);
  writer.u32(length);
  let flags = 0;
  if (output.firstBaseline !== null) flags |= RESPONSE_FIRST_BASELINE;
  if (output.lastBaseline !== null) flags |= RESPONSE_LAST_BASELINE;
  if ((flags & ~RESPONSE_KNOWN_FLAGS) !== 0) throw new Error("invalid response flags");
  writer.u32(flags);
  writer.u32(response.handle.slot);
  writer.u32(response.handle.generation);
  writer.u64(output.capabilities);
  writer.u32(output.indexEncoding);
  writer.u32(output.lines.length);
  writer.u32(output.clusters.length);
  writer.u32(output.carets.length);
  writer.u32(output.hitRegions.length);
  writer.u32(output.inlineBoxes.length);
  writer.u32(output.unresolvedGlyphs.length);
  writer.u32(output.unresolvedCodepoints.length);
  writer.f32(output.size.width);
  writer.f32(output.size.height);
  writer.f32(output.minIntrinsicWidth);
  writer.f32(output.maxIntrinsicWidth);
  writer.f32(output.firstBaseline ?? 0);
  writer.f32(output.lastBaseline ?? 0);
  writer.u64(response.approximateBytes);
  writer.zeros(32);

  for (const line of output.lines) {
    writeRange(writer, line.range);
    writeRect(writer, line.rect);
    writer.f32(line.baseline);
    writer.f32(line.ascent);
    writer.f32(line.descent);
    writer.f32(line.leading);
    writer.u8(line.hardBreak ? 1 : 0);
    writer.u8(line.direction);
    writer.zeros(6);
  }
  for (const cluster of output.clusters) {
    writeRange(writer, cluster.range);
    writeRect(writer, cluster.rect);
    writer.u64(cluster.lineIndex);
    writer.u8(cluster.direction);
    writer.u8(cluster.startsGrapheme ? 1 : 0);
    writer.u8(cluster.startsWord ? 1 : 0);
    writer.zeros(5);
  }
  for (const caret of output.carets) {
    writer.u64(caret.index);
    writer.u8(caret.affinity);
    writer.zeros(7);
    writeRect(writer, caret.rect);
    writer.u64(caret.lineIndex);
  }
  for (const hit of output.hitRegions) {
    writeRect(writer, hit.rect);
    writer.u64(hit.index);
    writer.u8(hit.affinity);
    writer.zeros(7);
    writer.u64(hit.lineIndex);
  }
  for (const inline of output.inlineBoxes) {
    writer.u64(inline.id);
    writeRange(writer, inline.range);
    writeRect(writer, inline.rect);
    writer.f32(inline.baseline);
    writer.u32(0);
  }
  for (const glyph of output.unresolvedGlyphs) {
    writeRange(writer, glyph.range);
    writer.u64(glyph.codepointStart);
    writer.u64(glyph.codepointCount);
  }
  for (const codepoint of output.unresolvedCodepoints) writer.u32(codepoint);
  return writer.finish();
}
