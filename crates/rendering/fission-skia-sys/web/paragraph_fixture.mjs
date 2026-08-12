import assert from "node:assert/strict";

import {
  CanvasKitParagraphError,
  createCanvasKitParagraphHost,
} from "./fission_skia_paragraph.js";
import { ParagraphWireError } from "./fission_skia_paragraph_wire.js";

const encoder = new TextEncoder();

function enumGroup(names) {
  return Object.fromEntries(names.map((name, value) => [name, Object.freeze({ value })]));
}

function scalarMap(text) {
  const values = [];
  let utf16 = 0;
  let utf8 = 0;
  for (const character of text) {
    const bytes = encoder.encode(character).byteLength;
    values.push({
      character,
      codepoint: character.codePointAt(0),
      utf16Start: utf16,
      utf16End: utf16 + character.length,
      utf8Start: utf8,
      utf8End: utf8 + bytes,
    });
    utf16 += character.length;
    utf8 += bytes;
  }
  return { values, utf16, utf8 };
}

function fakeCanvasKit() {
  const state = {
    builders: [],
    paragraphs: [],
    providers: [],
    paragraphStyles: [],
  };
  const CanvasKit = {
    FontWeight: enumGroup([
      "Invisible", "Thin", "ExtraLight", "Light", "Normal", "Medium",
      "SemiBold", "Bold", "ExtraBold", "Black", "ExtraBlack",
    ]),
    FontWidth: enumGroup([
      "UltraCondensed", "ExtraCondensed", "Condensed", "SemiCondensed", "Normal",
      "SemiExpanded", "Expanded", "ExtraExpanded", "UltraExpanded",
    ]),
    FontSlant: enumGroup(["Upright", "Italic", "Oblique"]),
    TextAlign: enumGroup(["Left", "Right", "Center", "Justify", "Start", "End"]),
    TextDirection: enumGroup(["LTR", "RTL"]),
    TextHeightBehavior: enumGroup([
      "All", "DisableFirstAscent", "DisableLastDescent", "DisableAll",
    ]),
    PlaceholderAlignment: enumGroup([
      "Baseline", "AboveBaseline", "BelowBaseline", "Top", "Bottom", "Middle",
    ]),
    TextBaseline: enumGroup(["Alphabetic", "Ideographic"]),
    RectHeightStyle: enumGroup([
      "Tight", "Max", "IncludeLineSpacingMiddle", "IncludeLineSpacingTop",
      "IncludeLineSpacingBottom", "Strut",
    ]),
    RectWidthStyle: enumGroup(["Tight", "Max"]),
    UnderlineDecoration: 1,
    Color4f: (...values) => Float32Array.from(values),
    TextStyle: (value) => value,
    ParagraphStyle: (value) => {
      state.paragraphStyles.push(value);
      return value;
    },
  };

  class FakeParagraph {
    constructor(text, style, placeholders) {
      this.text = text;
      this.style = style;
      this.placeholders = placeholders;
      this.map = scalarMap(text);
      this.layoutWidths = [];
      this.deleted = false;
      state.paragraphs.push(this);
    }

    delete() { this.deleted = true; }
    isDeleted() { return this.deleted; }
    layout(width) { this.layoutWidths.push(width); }
    getHeight() { return 12; }
    getLongestLine() { return this.map.values.length * 10; }
    getMinIntrinsicWidth() { return this.map.values.length === 0 ? 0 : 10; }
    getMaxIntrinsicWidth() { return this.getLongestLine(); }
    getLineMetrics() {
      return [{
        startIndex: 0,
        endIndex: this.map.utf16,
        endIncludingNewline: this.map.utf16,
        isHardBreak: false,
        ascent: 9,
        descent: 3,
        height: 12,
        width: this.getLongestLine(),
        left: 0,
        baseline: 9,
        lineNumber: 0,
      }];
    }
    getLineNumberAt(offset) { return offset >= 0 && offset < this.map.utf8 ? 0 : -1; }
    getGlyphInfoAt(offset) {
      const item = this.map.values.find((value) => (
        value.utf16Start <= offset && offset < value.utf16End
      ));
      if (!item) return null;
      const position = this.map.values.indexOf(item) * 10;
      return {
        graphemeClusterTextRange: { start: item.utf16Start, end: item.utf16End },
        graphemeLayoutBounds: Float32Array.of(position, 0, position + 10, 12),
        dir: this.style.textDirection,
        isEllipsis: false,
      };
    }
    getRectsForRange(start, end) {
      const item = this.map.values.find((value) => (
        value.utf16Start === start && value.utf16End === end
      ));
      if (!item) return [];
      const position = this.map.values.indexOf(item) * 10;
      return [{
        rect: Float32Array.of(position, 0, position + 10, 12),
        dir: this.style.textDirection,
      }];
    }
    getRectsForPlaceholders() {
      return this.placeholders
        .map((placeholder) => {
          const item = this.map.values[placeholder.scalarIndex];
          const position = placeholder.scalarIndex * 10;
          return {
            rect: Float32Array.of(
              position,
              0,
              position + placeholder.width,
              placeholder.height,
            ),
            dir: this.style.textDirection,
            item,
          };
        });
    }
    getWordBoundary(offset) {
      const item = this.map.values.find((value) => value.utf16Start === offset);
      return item
        ? { start: item.utf16Start, end: item.utf16End }
        : { start: 0, end: 0 };
    }
    unresolvedCodepoints() {
      return this.map.values
        .filter((value) => value.character === "\u2603")
        .map((value) => value.codepoint);
    }
    getShapedLines() {
      const glyphs = Uint16Array.from(this.map.values.map((value) => (
        value.character === "\u2603" ? 0 : 1
      )));
      const offsets = Uint32Array.from([
        ...this.map.values.map((value) => value.utf8Start),
        this.map.utf8,
      ]);
      return [{ runs: [{ glyphs, offsets, typeface: null }] }];
    }
  }

  CanvasKit.TypefaceFontProvider = {
    Make() {
      const provider = {
        deleted: false,
        fonts: [],
        registerFont(bytes, family) {
          this.fonts.push({ bytes: Uint8Array.from(bytes), family });
          return undefined;
        },
        delete() { this.deleted = true; },
        isDeleted() { return this.deleted; },
      };
      state.providers.push(provider);
      return provider;
    },
  };

  CanvasKit.ParagraphBuilder = {
    MakeFromFontProvider(style, provider) {
      const parts = [];
      const placeholders = [];
      const builder = {
        deleted: false,
        pushStyle() {},
        addText(text) { parts.push(text); },
        addPlaceholder(width, height, _alignment, _baseline, offset) {
          const scalarIndex = scalarMap(parts.join("")).values.length;
          parts.push("\ufffc");
          placeholders.push({ width, height, offset, scalarIndex });
        },
        pop() {},
        build() { return new FakeParagraph(parts.join(""), style, placeholders); },
        delete() { this.deleted = true; },
        isDeleted() { return this.deleted; },
        provider,
      };
      state.builders.push(builder);
      return builder;
    },
  };
  return { CanvasKit, state };
}

class Bytes {
  constructor() { this.values = []; }
  raw(values) { this.values.push(...values); }
  u8(value) { this.values.push(value & 0xff); }
  u16(value) { this.u8(value); this.u8(value >>> 8); }
  u32(value) {
    this.u8(value);
    this.u8(value >>> 8);
    this.u8(value >>> 16);
    this.u8(value >>> 24);
  }
  u64(value) {
    let remaining = BigInt(value);
    for (let index = 0; index < 8; index += 1) {
      this.u8(Number(remaining & 0xffn));
      remaining >>= 8n;
    }
  }
  f32(value) {
    const bytes = new Uint8Array(4);
    new DataView(bytes.buffer).setFloat32(0, value, true);
    this.raw(bytes);
  }
  patchU32(offset, value) {
    const bytes = new Uint8Array(4);
    new DataView(bytes.buffer).setUint32(0, value, true);
    this.values.splice(offset, 4, ...bytes);
  }
  finish() { return Uint8Array.from(this.values); }
}

export function requestPacket({ text = "ab", weight = 400, inline = null } = {}) {
  const textBytes = encoder.encode(text);
  const family = encoder.encode("Fixture");
  const bytes = new Bytes();
  bytes.raw([0x46, 0x53, 0x50, 0x51]);
  bytes.u16(1);
  bytes.u16(0);
  bytes.u32(0);
  bytes.u32(1 | 2 | 128 | 256);
  bytes.u32(textBytes.length);
  bytes.u32(1);
  bytes.u32(inline ? 1 : 0);
  bytes.u32(0);
  bytes.u64(7n);
  bytes.f32(100);
  bytes.u8(0);
  bytes.u8(0);
  bytes.u8(0);
  bytes.u8(0);
  bytes.u64(0n);
  bytes.f32(0);
  bytes.u32(0);
  bytes.u32(1);
  bytes.u32(0);
  for (let index = 0; index < 6; index += 1) bytes.u64(0n);
  bytes.raw(new Uint8Array(24));
  assert.equal(bytes.values.length, 144);
  bytes.raw(textBytes);

  const styleStart = bytes.values.length;
  bytes.u32(0);
  bytes.u32(2);
  bytes.u64(0n);
  bytes.u64(BigInt(textBytes.length));
  bytes.f32(14);
  bytes.raw([15, 25, 35, 255]);
  bytes.u16(weight);
  bytes.u8(0);
  bytes.u8(0);
  bytes.f32(0);
  bytes.f32(0);
  bytes.raw([0, 0, 0, 0]);
  bytes.f32(1);
  bytes.f32(0);
  bytes.u32(family.length);
  bytes.u32(0);
  bytes.u32(0);
  bytes.u32(0);
  assert.equal(bytes.values.length - styleStart, 72);
  bytes.raw(family);
  bytes.patchU32(styleStart, bytes.values.length - styleStart);

  if (inline) {
    bytes.u64(BigInt(inline.id));
    bytes.u64(BigInt(inline.start));
    bytes.u64(BigInt(inline.end));
    bytes.f32(inline.width);
    bytes.f32(inline.height);
    bytes.f32(inline.baseline);
    bytes.u32(0);
  }
  bytes.u32(5);
  bytes.u32(1);
  bytes.u32(family.length);
  bytes.u32(0);
  bytes.raw(family);
  bytes.patchU32(8, bytes.values.length);
  return bytes.finish();
}

function responseHeader(response) {
  const view = new DataView(response.buffer, response.byteOffset, response.byteLength);
  assert.deepEqual([...response.subarray(0, 4)], [0x46, 0x53, 0x50, 0x52]);
  assert.equal(view.getUint32(8, true), response.byteLength);
  return {
    slot: view.getUint32(16, true),
    generation: view.getUint32(20, true),
    capabilities: view.getBigUint64(24, true),
    encoding: view.getUint32(32, true),
    lines: view.getUint32(36, true),
    clusters: view.getUint32(40, true),
    carets: view.getUint32(44, true),
    hits: view.getUint32(48, true),
    inline: view.getUint32(52, true),
    unresolvedGlyphs: view.getUint32(56, true),
    unresolvedCodepoints: view.getUint32(60, true),
  };
}

function fixtureHost(CanvasKit, entries) {
  return createCanvasKitParagraphHost({
    CanvasKit,
    resolveResource: (slot) => entries.get(slot),
  });
}

{
  const { CanvasKit, state } = fakeCanvasKit();
  const resources = new Map([[5, { generation: 1, kind: 3, bytes: Uint8Array.of(1, 2, 3) }]]);
  const host = fixtureHost(CanvasKit, resources);
  const first = responseHeader(host.layout(requestPacket()));
  assert.deepEqual(first, {
    slot: 1,
    generation: 1,
    capabilities: 0x1ffn,
    encoding: 0,
    lines: 1,
    clusters: 2,
    carets: 4,
    hits: 4,
    inline: 0,
    unresolvedGlyphs: 0,
    unresolvedCodepoints: 0,
  });
  assert.equal(state.providers[0].fonts[0].family, "Fixture");
  assert.deepEqual([...state.providers[0].fonts[0].bytes], [1, 2, 3]);
  assert.deepEqual(state.paragraphs[0].layoutWidths, [100]);

  const prepared = host.prepare({ slot: first.slot, generation: first.generation });
  const calls = [];
  const canvas = {
    save() { calls.push(["save"]); },
    translate(x, y) { calls.push(["translate", x, y]); },
    scale(x, y) { calls.push(["scale", x, y]); },
    drawParagraph(paragraph, x, y) { calls.push(["draw", paragraph, x, y]); },
    restore() { calls.push(["restore"]); },
  };
  prepared.paint(canvas, 3, 4, 2);
  assert.equal(calls[3][1], state.paragraphs[0]);
  assert.deepEqual(calls.map((call) => call[0]), ["save", "translate", "scale", "draw", "restore"]);

  assert.equal(host.destroy(first), true);
  assert.equal(state.paragraphs[0].deleted, true);
  assert.equal(state.providers[0].deleted, true);
  assert.throws(() => host.prepare(first), (error) => (
    error instanceof CanvasKitParagraphError && error.code === "stale-handle"
  ));
  const secondResponse = host.layout(requestPacket({ text: "\u05d0" }));
  const second = responseHeader(secondResponse);
  assert.equal(second.slot, first.slot);
  assert.equal(second.generation, 2);
  assert.equal(state.paragraphStyles.at(-1).textDirection, CanvasKit.TextDirection.RTL);
  assert.equal(new DataView(secondResponse.buffer).getUint8(128 + 49), 1);
  host.clear();
  assert.equal(host.resolve(second), null);
  const afterClear = responseHeader(host.layout(requestPacket({ text: "fresh" })));
  assert.equal(afterClear.slot, second.slot);
  assert.equal(afterClear.generation, 3);
  host.clear();
}

{
  const { CanvasKit } = fakeCanvasKit();
  const host = fixtureHost(CanvasKit, new Map([[
    5,
    { generation: 1, kind: 3, bytes: Uint8Array.of(9) },
  ]]));
  const unresolved = responseHeader(host.layout(requestPacket({ text: "\u2603" })));
  assert.equal(unresolved.unresolvedGlyphs, 1);
  assert.equal(unresolved.unresolvedCodepoints, 1);

  const placeholderBytes = encoder.encode("a").length;
  const inlineEnd = placeholderBytes + encoder.encode("\ufffc").length;
  const inline = responseHeader(host.layout(requestPacket({
    text: "a\ufffcb",
    inline: { id: 41, start: placeholderBytes, end: inlineEnd, width: 6, height: 8, baseline: 6 },
  })));
  assert.equal(inline.inline, 1);
  assert.equal(inline.clusters, 2);

  const zeroInline = responseHeader(host.layout(requestPacket({
    text: "a\ufffcb",
    inline: { id: 42, start: placeholderBytes, end: inlineEnd, width: 0, height: 0, baseline: 0 },
  })));
  assert.equal(zeroInline.inline, 1);
  assert.equal(zeroInline.clusters, 2);
  host.clear();
}

{
  const { CanvasKit, state } = fakeCanvasKit();
  const stale = fixtureHost(CanvasKit, new Map([[
    5,
    { generation: 2, kind: 3, bytes: Uint8Array.of(1) },
  ]]));
  assert.throws(() => stale.layout(requestPacket()), (error) => (
    error instanceof CanvasKitParagraphError && error.code === "stale-handle"
  ));
  assert.equal(state.providers.at(-1).deleted, true);

  const wrongKind = fixtureHost(CanvasKit, new Map([[
    5,
    { generation: 1, kind: 1, bytes: Uint8Array.of(1) },
  ]]));
  assert.throws(() => wrongKind.layout(requestPacket()), (error) => (
    error instanceof CanvasKitParagraphError && error.code === "resource-failure"
  ));

  const exactWeight = fixtureHost(CanvasKit, new Map([[
    5,
    { generation: 1, kind: 3, bytes: Uint8Array.of(1) },
  ]]));
  assert.throws(() => exactWeight.layout(requestPacket({ weight: 450 })), (error) => (
    error instanceof CanvasKitParagraphError && error.code === "unsupported-api"
  ));

  const malformed = requestPacket();
  malformed[7] = 1;
  assert.throws(() => exactWeight.layout(malformed), (error) => error instanceof ParagraphWireError);
}

console.log("CanvasKit paragraph fixtures passed");
