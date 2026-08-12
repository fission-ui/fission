import {
  BackendPreference,
  ErrorCode,
  PacketKind,
  ResourceKind,
  ResourceOperation,
  decodeMessage,
  encodeError,
} from "./fission_skia_web.js";
import {
  CommandStreamError,
  decodeCommandStream,
} from "./fission_skia_commands.js";
import { createCanvasKitExecutor } from "./fission_skia_executor.js";

const SESSION = 11n;

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function assertEqual(actual, expected, message) {
  assert(actual === expected, `${message}: expected ${expected}, got ${actual}`);
}

function startsWithLog(entry, prefix) {
  return typeof entry === "string" && entry.startsWith(prefix);
}

function makePacket(kind, sequence, payload, session = SESSION) {
  const bytes = new Uint8Array(32 + payload.byteLength);
  bytes.set([0x46, 0x53, 0x4b, 0x4e], 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(4, 1, true);
  view.setUint16(6, kind, true);
  view.setUint32(8, bytes.byteLength, true);
  view.setUint32(12, 0, true);
  view.setBigUint64(16, session, true);
  view.setBigUint64(24, BigInt(sequence), true);
  bytes.set(payload, 32);
  return bytes;
}

function surfacePayload(width, height, scaleFactor) {
  const bytes = new Uint8Array(12);
  const view = new DataView(bytes.buffer);
  view.setUint32(0, width, true);
  view.setUint32(4, height, true);
  view.setFloat32(8, scaleFactor, true);
  return bytes;
}

function initPacket(
  backend = BackendPreference.WEB_GL,
  {
    session = SESSION,
    width = 640,
    height = 480,
    scaleFactor = 2,
    colorSpace = 1,
    alphaMode = 2,
  } = {},
) {
  const payload = new Uint8Array(16);
  payload.set(surfacePayload(width, height, scaleFactor));
  payload[12] = backend;
  payload[13] = colorSpace;
  payload[14] = alphaMode;
  payload[15] = 0;
  return makePacket(PacketKind.INIT, 1, payload, session);
}

function resizePacket(sequence, width, height, scaleFactor = 2) {
  return makePacket(
    PacketKind.RESIZE,
    sequence,
    surfacePayload(width, height, scaleFactor),
  );
}

function resourceBatchPacket(sequence, epoch, updates) {
  const byteLength =
    16 + updates.reduce((length, update) => length + 24 + update.bytes.byteLength, 0);
  const payload = new Uint8Array(byteLength);
  const view = new DataView(payload.buffer);
  view.setBigUint64(0, BigInt(epoch), true);
  view.setUint32(8, updates.length, true);
  view.setUint32(12, 0, true);
  let offset = 16;
  for (const update of updates) {
    view.setUint32(offset, update.slot, true);
    view.setUint32(offset + 4, update.generation, true);
    view.setUint8(offset + 8, update.operation);
    view.setUint8(offset + 9, update.kind);
    view.setUint16(offset + 10, 0, true);
    view.setBigUint64(offset + 12, BigInt(update.contentId), true);
    view.setUint32(offset + 20, update.bytes.byteLength, true);
    payload.set(update.bytes, offset + 24);
    offset += 24 + update.bytes.byteLength;
  }
  return makePacket(PacketKind.RESOURCE_BATCH, sequence, payload);
}

function upsert(slot, generation, bytes, kind = ResourceKind.IMAGE) {
  return {
    slot,
    generation,
    operation: ResourceOperation.UPSERT,
    kind,
    contentId: BigInt(slot * 100 + generation),
    bytes: Uint8Array.from(bytes),
  };
}

function release(slot, generation, kind = ResourceKind.IMAGE) {
  return {
    slot,
    generation,
    operation: ResourceOperation.RELEASE,
    kind,
    contentId: 0n,
    bytes: new Uint8Array(),
  };
}

class WireWriter {
  constructor() {
    this.values = [];
  }

  u8(value) {
    this.values.push(value & 0xff);
  }

  u16(value) {
    this.u8(value);
    this.u8(value >>> 8);
  }

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

  raw(bytes) {
    this.values.push(...bytes);
  }

  patchU32(offset, value) {
    const bytes = new Uint8Array(4);
    new DataView(bytes.buffer).setUint32(0, value, true);
    this.values.splice(offset, 4, ...bytes);
  }

  finish() {
    return Uint8Array.from(this.values);
  }
}

function command(kind, writePayload = () => {}) {
  const payload = new WireWriter();
  writePayload(payload);
  return { kind, payload: payload.finish() };
}

function commandStream(entries, version = 1) {
  const body = new WireWriter();
  for (const entry of entries) {
    body.u16(entry.kind);
    body.u16(0);
    body.u32(8 + entry.payload.byteLength);
    body.raw(entry.payload);
  }
  const bodyBytes = body.finish();
  const stream = new Uint8Array(16 + bodyBytes.byteLength);
  stream.set([0x46, 0x53, 0x43, 0x4d], 0);
  const view = new DataView(stream.buffer);
  view.setUint16(4, version, true);
  view.setUint16(6, 0, true);
  view.setUint32(8, stream.byteLength, true);
  view.setUint32(12, entries.length, true);
  stream.set(bodyBytes, 16);
  return stream;
}

function writeColor(writer, value) {
  for (const component of value) writer.f32(component);
}

function writePoint(writer, value) {
  writer.f32(value.x);
  writer.f32(value.y);
}

function writeRect(writer, value) {
  writer.f32(value.x);
  writer.f32(value.y);
  writer.f32(value.width);
  writer.f32(value.height);
}

function writeHandle(writer, slot, generation) {
  writer.u32(slot);
  writer.u32(generation);
}

function solidPaint(value) {
  return { kind: 1, solid: value, start: { x: 0, y: 0 }, end: { x: 0, y: 0 }, radius: 0, stops: [] };
}

function linearPaint(start, end, stops) {
  return { kind: 2, solid: [0, 0, 0, 0], start, end, radius: 0, stops };
}

function radialPaint(center, radius, stops) {
  return {
    kind: 3,
    solid: [0, 0, 0, 0],
    start: center,
    end: { x: 0, y: 0 },
    radius,
    stops,
  };
}

function writePaint(writer, paint) {
  writer.u8(paint.kind);
  writer.raw([0, 0, 0]);
  writeColor(writer, paint.solid);
  writePoint(writer, paint.start);
  writePoint(writer, paint.end);
  writer.f32(paint.radius);
  writer.u32(paint.stops.length);
  for (const stop of paint.stops) {
    writer.f32(stop.offset);
    writeColor(writer, stop.color);
  }
}

function writeStroke(writer, stroke) {
  writer.f32(stroke.width);
  writer.u8(stroke.lineCap);
  writer.u8(stroke.lineJoin);
  writer.u16(0);
  writer.u32(stroke.dashes.length);
  writePaint(writer, stroke.paint);
  for (const dash of stroke.dashes) writer.f32(dash);
}

function writePath(writer, fillRule, commands) {
  writer.u8(fillRule);
  writer.raw([0, 0, 0]);
  writer.u32(commands.length);
  for (const pathCommand of commands) {
    writer.u8(pathCommand.kind);
    writer.raw([0, 0, 0]);
    for (const value of pathCommand.values) writer.f32(value);
  }
}

function emptyCommandStream() {
  return commandStream([]);
}

function framePacket(
  sequence,
  resourceEpoch,
  commands = emptyCommandStream(),
  { width = 640, height = 480, scaleFactor = 2 } = {},
) {
  const suppliedCommands = Uint8Array.from(commands);
  const commandBytes = suppliedCommands.byteLength === 0
    ? emptyCommandStream()
    : suppliedCommands;
  const payload = new Uint8Array(64 + commandBytes.byteLength);
  const view = new DataView(payload.buffer);
  view.setBigUint64(0, BigInt(sequence), true);
  view.setBigUint64(8, BigInt(resourceEpoch), true);
  view.setBigUint64(16, 1n, true);
  payload.set(surfacePayload(width, height, scaleFactor), 24);
  view.setUint32(36, 0, true);
  view.setUint32(40, commandBytes.byteLength, true);
  view.setFloat32(44, 0.1, true);
  view.setFloat32(48, 0.2, true);
  view.setFloat32(52, 0.3, true);
  view.setFloat32(56, 1, true);
  view.setUint32(60, 0, true);
  payload.set(commandBytes, 64);
  return makePacket(PacketKind.FRAME, sequence, payload);
}

function destroyPacket(sequence) {
  const payload = new Uint8Array(4);
  return makePacket(PacketKind.DESTROY, sequence, payload);
}

function paragraphRequestPacket(text = "paragraph") {
  const textBytes = new TextEncoder().encode(text);
  const family = new TextEncoder().encode("Fixture");
  const bytes = new WireWriter();
  bytes.raw([0x46, 0x53, 0x50, 0x51]);
  bytes.u16(1);
  bytes.u16(0);
  bytes.u32(0);
  bytes.u32(1 | 2 | 128 | 256);
  bytes.u32(textBytes.byteLength);
  bytes.u32(1);
  bytes.u32(0);
  bytes.u32(0);
  bytes.u64(1n);
  bytes.f32(200);
  bytes.raw([0, 0, 0, 0]);
  bytes.u64(0n);
  bytes.f32(0);
  bytes.u32(0);
  bytes.u32(1);
  bytes.u32(0);
  for (let index = 0; index < 6; index += 1) bytes.u64(0n);
  bytes.raw(new Uint8Array(24));
  assertEqual(bytes.values.length, 144, "paragraph request header length");
  bytes.raw(textBytes);

  const styleStart = bytes.values.length;
  bytes.u32(0);
  bytes.u32(2);
  bytes.u64(0n);
  bytes.u64(BigInt(textBytes.byteLength));
  bytes.f32(14);
  bytes.raw([10, 20, 30, 255]);
  bytes.u16(400);
  bytes.raw([0, 0]);
  bytes.f32(0);
  bytes.f32(0);
  bytes.raw([0, 0, 0, 0]);
  bytes.f32(1);
  bytes.f32(0);
  bytes.u32(family.byteLength);
  bytes.u32(0);
  bytes.u32(0);
  bytes.u32(0);
  assertEqual(bytes.values.length - styleStart, 72, "paragraph style header length");
  bytes.raw(family);
  bytes.patchU32(styleStart, bytes.values.length - styleStart);

  bytes.u32(7);
  bytes.u32(1);
  bytes.u32(family.byteLength);
  bytes.u32(0);
  bytes.raw(family);
  bytes.patchU32(8, bytes.values.length);
  return bytes.finish();
}

function paragraphResponseHandle(response) {
  const view = new DataView(response.buffer, response.byteOffset, response.byteLength);
  assertEqual(view.getUint32(0, false), 0x46535052, "paragraph response magic");
  assertEqual(view.getUint32(8, true), response.byteLength, "paragraph response length");
  return {
    slot: view.getUint32(16, true),
    generation: view.getUint32(20, true),
  };
}

function assertAck(bytes, acknowledgedSequence) {
  const message = decodeMessage(bytes);
  assertEqual(message.envelope.kind, PacketKind.ACK, "response packet kind");
  assertEqual(
    message.packet.acknowledgedSequence,
    BigInt(acknowledgedSequence),
    "acknowledged sequence",
  );
  return message;
}

function assertError(bytes, code, failedSequence) {
  const message = decodeMessage(bytes);
  assertEqual(message.envelope.kind, PacketKind.ERROR, "response packet kind");
  assertEqual(message.packet.code, code, "error code");
  assertEqual(
    message.packet.failedSequence,
    BigInt(failedSequence),
    "failed sequence",
  );
  return message;
}

function makeCanvas() {
  const listeners = new Map();
  return {
    width: 0,
    height: 0,
    addEventListener(type, callback) {
      listeners.set(type, callback);
    },
    removeEventListener(type, callback) {
      if (listeners.get(type) === callback) listeners.delete(type);
    },
    dispatch(type) {
      let prevented = false;
      listeners.get(type)?.({
        preventDefault() {
          prevented = true;
        },
      });
      return prevented;
    },
    listenerCount() {
      return listeners.size;
    },
  };
}

function makeFakeCanvasKit() {
  const log = [];
  const objects = [];
  let nextId = 1;
  let nextContext = 40;
  const controls = {
    webGlAvailable: true,
    failNextGaneshContext: false,
    failNextImageDecode: false,
    failNextSurface: false,
  };

  function owned(type, extra = {}) {
    const object = {
      id: nextId++,
      type,
      deleted: false,
      ...extra,
      isDeleted() {
        return this.deleted;
      },
      delete() {
        assert(!this.deleted, `${type} ${this.id} deleted twice`);
        this.deleted = true;
        log.push(`delete:${type}:${this.id}`);
      },
    };
    objects.push(object);
    log.push(`create:${type}:${object.id}`);
    return object;
  }

  function makeSurface(type, width, height, canStage = true) {
    if (controls.failNextSurface) {
      controls.failNextSurface = false;
      return null;
    }
    let surface;
    let saveCount = 1;
    const skCanvas = {
      clear(color) {
        log.push(`clear:${color.join(",")}`);
      },
      drawImage(image, x, y, paint) {
        log.push(`draw-image:${image.id}:${x},${y}:${paint.id}`);
      },
      drawImageRectOptions(image, source, destination, filterMode, mipmapMode) {
        log.push({
          type: "draw-image-rect",
          surface: surface.id,
          image: image.id,
          source,
          destination,
          filterMode,
          mipmapMode,
        });
      },
      drawRRect(rrect, paint) {
        log.push({ type: "draw-rrect", surface: surface.id, rrect, paint: paint.id });
      },
      drawPath(path, paint) {
        log.push({ type: "draw-path", surface: surface.id, path: path.id, paint: paint.id });
      },
      clipRect(value, operation, antialias) {
        log.push({ type: "clip-rect", surface: surface.id, value, operation, antialias });
      },
      clipRRect(value, operation, antialias) {
        log.push({ type: "clip-rrect", surface: surface.id, value, operation, antialias });
      },
      concat(matrix) {
        log.push({ type: "concat", surface: surface.id, matrix: Array.from(matrix) });
      },
      translate(x, y) {
        log.push({ type: "translate", surface: surface.id, x, y });
      },
      scale(x, y) {
        log.push({ type: "scale", surface: surface.id, x, y });
      },
      drawParagraph(paragraph, x, y) {
        assert(!paragraph.deleted, "retained paragraph must be live while painting");
        log.push({ type: "draw-paragraph", surface: surface.id, paragraph: paragraph.id, x, y });
      },
      getSaveCount() {
        return saveCount;
      },
      save() {
        saveCount += 1;
        log.push({ type: "save", surface: surface.id, saveCount });
        return saveCount;
      },
      saveLayer(...arguments_) {
        saveCount += 1;
        log.push({ type: "save-layer", surface: surface.id, arguments_, saveCount });
        return saveCount;
      },
      restore() {
        saveCount = Math.max(1, saveCount - 1);
        log.push({ type: "restore", surface: surface.id, saveCount });
      },
      restoreToCount(count) {
        saveCount = count;
        log.push({ type: "restore-to-count", surface: surface.id, saveCount });
      },
    };
    surface = owned(type, {
      getCanvas() {
        return skCanvas;
      },
      imageInfo() {
        return { width, height };
      },
      makeSurface(info) {
        if (!canStage) return null;
        log.push(`make-staging-surface:${info.width}x${info.height}`);
        return makeSurface("staging-surface", info.width, info.height, false);
      },
      makeImageSnapshot() {
        return owned("snapshot", {
          width: () => width,
          height: () => height,
        });
      },
      flush() {
        log.push(`flush:${this.id}`);
      },
      dispose() {
        log.push(`dispose:${type}:${this.id}`);
        this.delete();
      },
    });
    return surface;
  }

  const CanvasKit = {
    gpu: true,
    ColorSpace: {
      SRGB: { name: "srgb" },
      DISPLAY_P3: { name: "display-p3" },
    },
    Color4f(...components) {
      return components;
    },
    XYWHRect(x, y, width, height) {
      return { x, y, width, height };
    },
    RRectXY(value, radiusX, radiusY) {
      return { rect: value, radiusX, radiusY };
    },
    BlendMode: { Src: 0 },
    PaintStyle: { Fill: 0, Stroke: 1 },
    StrokeCap: { Butt: 0, Round: 1, Square: 2 },
    StrokeJoin: { Miter: 0, Round: 1, Bevel: 2 },
    TileMode: { Clamp: 0 },
    FilterMode: { Nearest: 0, Linear: 1 },
    MipmapMode: { None: 0 },
    ClipOp: { Intersect: 0 },
    FillType: { Winding: 0, EvenOdd: 1 },
    BlurStyle: { Normal: 0 },
    Paint: class {
      constructor() {
        return owned("paint", {
          setAntiAlias(value) {
            log.push(`paint-antialias:${value}`);
          },
          setColor(value) {
            log.push(`paint-color:${value.join(",")}`);
          },
          setShader(shader) {
            log.push(`paint-shader:${shader.id}`);
          },
          setStyle(style) {
            log.push(`paint-style:${style}`);
          },
          setStrokeWidth(width) {
            log.push(`paint-stroke-width:${width}`);
          },
          setStrokeCap(cap) {
            log.push(`paint-stroke-cap:${cap}`);
          },
          setStrokeJoin(join) {
            log.push(`paint-stroke-join:${join}`);
          },
          setPathEffect(effect) {
            log.push(`paint-path-effect:${effect.id}`);
          },
          setMaskFilter(filter) {
            log.push(`paint-mask-filter:${filter.id}`);
          },
          setAlphaf(alpha) {
            log.push(`paint-alpha:${alpha}`);
          },
          setBlendMode(mode) {
            log.push(`paint-blend:${mode}`);
          },
        });
      }
    },
    Shader: {
      MakeLinearGradient(start, end, colors, positions, mode) {
        return owned("linear-shader", { start, end, colors, positions, mode });
      },
      MakeRadialGradient(center, radius, colors, positions, mode) {
        return owned("radial-shader", { center, radius, colors, positions, mode });
      },
    },
    PathEffect: {
      MakeDash(intervals, phase) {
        return owned("dash-effect", { intervals: Array.from(intervals), phase });
      },
    },
    MaskFilter: {
      MakeBlur(style, sigma, respectTransform) {
        return owned("mask-filter", { style, sigma, respectTransform });
      },
    },
    ImageFilter: {
      MakeBlur(sigmaX, sigmaY, mode, input) {
        return owned("image-filter", { sigmaX, sigmaY, mode, input });
      },
    },
    PathBuilder: class {
      constructor() {
        const pathCommands = [];
        return owned("path-builder", {
          moveTo(...values) {
            pathCommands.push(["move", ...values]);
          },
          lineTo(...values) {
            pathCommands.push(["line", ...values]);
          },
          quadTo(...values) {
            pathCommands.push(["quad", ...values]);
          },
          cubicTo(...values) {
            pathCommands.push(["cubic", ...values]);
          },
          close() {
            pathCommands.push(["close"]);
          },
          addRRect(value) {
            pathCommands.push(["rrect", value]);
          },
          detachAndDelete() {
            this.delete();
            return owned("path", {
              pathCommands,
              setFillType(fillType) {
                log.push(`path-fill:${this.id}:${fillType}`);
              },
            });
          },
        });
      }
    },
    GetWebGLContext() {
      log.push("get-webgl-context");
      return controls.webGlAvailable ? nextContext++ : 0;
    },
    MakeWebGLContext(handle) {
      if (controls.failNextGaneshContext) {
        controls.failNextGaneshContext = false;
        return null;
      }
      return owned("gr-context", {
        handle,
        releaseResourcesAndAbandonContext() {
          log.push(`abandon:gr-context:${this.id}`);
        },
      });
    },
    MakeOnScreenGLSurface(_context, width, height) {
      log.push(`make-webgl-surface:${width}x${height}`);
      return makeSurface("webgl-surface", width, height);
    },
    MakeSWCanvasSurface(canvas) {
      log.push(`make-software-surface:${canvas.width}x${canvas.height}`);
      return makeSurface("software-surface", canvas.width, canvas.height);
    },
    deleteContext(handle) {
      log.push(`delete-context:${handle}`);
    },
    MakeImageFromEncoded(bytes) {
      const snapshot = Array.from(bytes);
      log.push(`decode-image:${snapshot.join(".")}`);
      if (controls.failNextImageDecode) {
        controls.failNextImageDecode = false;
        return null;
      }
      if (bytes[0] === 0xff) return null;
      return owned("image", { snapshot, width: () => 64, height: () => 32 });
    },
    Typeface: {
      MakeTypefaceFromData(buffer) {
        const snapshot = Array.from(new Uint8Array(buffer));
        log.push(`decode-font:${snapshot.join(".")}`);
        if (snapshot[0] === 0xff) return null;
        return owned("typeface", { snapshot });
      },
    },
  };

  function enumValues(names) {
    return Object.fromEntries(names.map((name, value) => [name, Object.freeze({ value })]));
  }

  CanvasKit.FontWeight = enumValues([
    "Invisible", "Thin", "ExtraLight", "Light", "Normal", "Medium",
    "SemiBold", "Bold", "ExtraBold", "Black", "ExtraBlack",
  ]);
  CanvasKit.FontWidth = enumValues([
    "UltraCondensed", "ExtraCondensed", "Condensed", "SemiCondensed", "Normal",
    "SemiExpanded", "Expanded", "ExtraExpanded", "UltraExpanded",
  ]);
  CanvasKit.FontSlant = enumValues(["Upright", "Italic", "Oblique"]);
  CanvasKit.TextAlign = enumValues(["Left", "Right", "Center", "Justify", "Start", "End"]);
  CanvasKit.TextDirection = enumValues(["LTR", "RTL"]);
  CanvasKit.TextHeightBehavior = enumValues([
    "All", "DisableFirstAscent", "DisableLastDescent", "DisableAll",
  ]);
  CanvasKit.PlaceholderAlignment = enumValues([
    "Baseline", "AboveBaseline", "BelowBaseline", "Top", "Bottom", "Middle",
  ]);
  CanvasKit.TextBaseline = enumValues(["Alphabetic", "Ideographic"]);
  CanvasKit.RectHeightStyle = enumValues([
    "Tight", "Max", "IncludeLineSpacingMiddle", "IncludeLineSpacingTop",
    "IncludeLineSpacingBottom", "Strut",
  ]);
  CanvasKit.RectWidthStyle = enumValues(["Tight", "Max"]);
  CanvasKit.UnderlineDecoration = 1;
  CanvasKit.TextStyle = (value) => value;
  CanvasKit.ParagraphStyle = (value) => value;
  CanvasKit.TypefaceFontProvider = {
    Make() {
      return owned("font-provider", {
        fonts: [],
        registerFont(bytes, family) {
          this.fonts.push({ bytes: Uint8Array.from(bytes), family });
          log.push(`register-font:${family}:${bytes.byteLength}`);
        },
      });
    },
  };
  CanvasKit.ParagraphBuilder = {
    MakeFromFontProvider(style, provider) {
      const parts = [];
      return owned("paragraph-builder", {
        style,
        provider,
        pushStyle() {},
        addText(text) { parts.push(text); },
        addPlaceholder() { parts.push("\ufffc"); },
        pop() {},
        build() {
          const text = parts.join("");
          const paragraph = owned("paragraph", {
            text,
            style,
            layoutWidths: [],
            layout(width) {
              this.layoutWidths.push(width);
              log.push(`layout-paragraph:${this.id}:${width}`);
            },
            getHeight() { return text.length === 0 ? 0 : 14; },
            getLongestLine() { return text.length * 8; },
            getMinIntrinsicWidth() { return text.length === 0 ? 0 : 8; },
            getMaxIntrinsicWidth() { return text.length * 8; },
            getLineMetrics() {
              return text.length === 0 ? [] : [{
                startIndex: 0,
                endIndex: text.length,
                endIncludingNewline: text.length,
                isHardBreak: true,
                ascent: 10,
                descent: 4,
                height: 14,
                width: text.length * 8,
                left: 0,
                baseline: 10,
                lineNumber: 0,
              }];
            },
            getGlyphInfoAt(offset) {
              if (offset < 0 || offset >= text.length) return null;
              return {
                graphemeClusterTextRange: { start: offset, end: offset + 1 },
                graphemeLayoutBounds: [offset * 8, 0, offset * 8 + 8, 14],
                dir: style.textDirection,
                isEllipsis: false,
              };
            },
            getRectsForRange(start, end) {
              if (start < 0 || end !== start + 1 || end > text.length) return [];
              return [{
                rect: [start * 8, 0, start * 8 + 8, 14],
                dir: style.textDirection,
              }];
            },
            getRectsForPlaceholders() { return []; },
            getWordBoundary(offset) { return { start: offset, end: offset + 1 }; },
            unresolvedCodepoints() { return []; },
            getShapedLines() { return []; },
          });
          log.push(`build-paragraph:${paragraph.id}`);
          return paragraph;
        },
      });
    },
  };

  return { CanvasKit, controls, log, objects };
}

function latestObject(fake, type) {
  return [...fake.objects].reverse().find((object) => object.type === type);
}

function drawImageCommand(kind, slot, generation, target, options) {
  return command(kind, (writer) => {
    writeHandle(writer, slot, generation);
    if (kind === 14) {
      writeRect(writer, options.source);
      writeRect(writer, target);
      writer.u8(options.sampling);
      writer.raw([0, 0, 0]);
      return;
    }
    writeRect(writer, target);
    writer.u8(options.fit);
    writer.u8(options.alignment);
    writer.u8(options.sampling);
    writer.u8(0);
  });
}

function drawParagraphCommand(slot, generation, origin = { x: 12, y: 18 }, scale = 1.5) {
  return command(13, (writer) => {
    writeHandle(writer, slot, generation);
    writePoint(writer, origin);
    writer.f32(scale);
  });
}

function supportedCommandStream() {
  const geometry = { x: 4, y: 6, width: 40, height: 30 };
  const target = { x: 10, y: 20, width: 100, height: 100 };
  const path = [
    { kind: 1, values: [0, 0, 0, 0, 0, 0] },
    { kind: 2, values: [20, 0, 0, 0, 0, 0] },
    { kind: 3, values: [25, 5, 20, 10, 0, 0] },
    { kind: 4, values: [15, 15, 5, 15, 0, 10] },
    { kind: 5, values: [0, 0, 0, 0, 0, 0] },
  ];
  const stops = [
    { offset: 0, color: [1, 0, 0, 1] },
    { offset: 1, color: [0, 0, 1, 0.5] },
  ];
  const dashedStroke = {
    width: 2,
    lineCap: 2,
    lineJoin: 3,
    paint: linearPaint({ x: 0, y: 0 }, { x: 40, y: 30 }, stops),
    dashes: [2, 3],
  };

  return commandStream([
    command(1, (writer) => writeColor(writer, [0.2, 0.3, 0.4, 1])),
    command(2),
    command(5, (writer) => writeRect(writer, geometry)),
    command(6, (writer) => {
      writeRect(writer, geometry);
      writer.f32(5);
    }),
    command(7, (writer) => {
      for (const value of [1, 0.2, 3, 0.1, 1, 4]) writer.f32(value);
    }),
    command(8, (writer) => {
      writeRect(writer, geometry);
      writer.f32(4);
      writePaint(writer, solidPaint([0.1, 0.2, 0.3, 0.9]));
    }),
    command(8, (writer) => {
      writeRect(writer, geometry);
      writer.f32(0);
      writePaint(writer, linearPaint({ x: 0, y: 0 }, { x: 40, y: 30 }, stops));
    }),
    command(8, (writer) => {
      writeRect(writer, geometry);
      writer.f32(0);
      writePaint(writer, radialPaint({ x: 20, y: 15 }, 12, stops));
    }),
    command(9, (writer) => {
      writeRect(writer, geometry);
      writer.f32(3);
      writeStroke(writer, dashedStroke);
    }),
    command(10, (writer) => {
      writePath(writer, 2, path);
      writePaint(writer, solidPaint([0.8, 0.2, 0.1, 1]));
    }),
    command(11, (writer) => {
      writePath(writer, 1, path);
      writeStroke(writer, { ...dashedStroke, dashes: [] });
    }),
    command(12, (writer) => {
      writeRect(writer, geometry);
      writer.f32(4);
      writeColor(writer, [0, 0, 0, 0.5]);
      writer.f32(8);
      writer.f32(2);
      writePoint(writer, { x: 3, y: 4 });
      writer.u8(0);
      writer.raw([0, 0, 0]);
    }),
    command(12, (writer) => {
      writeRect(writer, geometry);
      writer.f32(4);
      writeColor(writer, [0, 0, 0, 0.35]);
      writer.f32(6);
      writer.f32(1);
      writePoint(writer, { x: 1, y: 2 });
      writer.u8(1);
      writer.raw([0, 0, 0]);
    }),
    drawImageCommand(14, 1, 1, { x: 40, y: 50, width: 128, height: 64 }, {
      source: { x: 0, y: 0, width: 64, height: 32 },
      sampling: 2,
    }),
    drawImageCommand(18, 1, 1, target, { fit: 1, alignment: 9, sampling: 2 }),
    drawImageCommand(18, 1, 1, target, { fit: 2, alignment: 3, sampling: 1 }),
    drawImageCommand(18, 1, 1, target, { fit: 3, alignment: 5, sampling: 2 }),
    drawImageCommand(18, 1, 1, target, { fit: 4, alignment: 5, sampling: 1 }),
    command(15, (writer) => {
      writeRect(writer, geometry);
      writer.f32(4);
      writer.f32(3);
    }),
    command(3),
    command(4, (writer) => {
      writeRect(writer, geometry);
      writer.f32(0.5);
    }),
    command(8, (writer) => {
      writeRect(writer, geometry);
      writer.f32(0);
      writePaint(writer, solidPaint([1, 1, 1, 1]));
    }),
    command(3),
  ]);
}

function assertRect(actual, expected, message) {
  for (const field of ["x", "y", "width", "height"]) {
    assertEqual(actual[field], expected[field], `${message} ${field}`);
  }
}

function assertCommandError(callback, code) {
  try {
    callback();
  } catch (error) {
    assert(error instanceof CommandStreamError, `expected CommandStreamError, got ${error}`);
    assertEqual(error.code, code, "command error code");
    return error;
  }
  throw new Error(`expected command error ${code}`);
}

function assertExecutorFault(callback, code) {
  try {
    callback();
  } catch (error) {
    assertEqual(error?.protocolCode, code, "executor fault code");
    return error;
  }
  throw new Error(`expected executor fault ${code}`);
}

function testStrictCommandDecoder() {
  const supported = supportedCommandStream();
  const decoded = decodeCommandStream(supported);
  assertEqual(decoded.length, 23, "supported command count");
  assertEqual(decoded[14].kind, "draw-image-fit", "DrawImageFit decoded kind");

  assertCommandError(() => decodeCommandStream(commandStream([], 2)), "unsupported-version");
  assertCommandError(() => decodeCommandStream(commandStream([command(99)])), "unknown-command");
  assertCommandError(() => decodeCommandStream(commandStream([command(2)])), "unclosed-save");
  assertCommandError(() => decodeCommandStream(commandStream([command(3)])), "unbalanced-restore");

  const noncanonicalPaint = command(8, (writer) => {
    writeRect(writer, { x: 0, y: 0, width: 1, height: 1 });
    writer.f32(0);
    const paint = solidPaint([1, 1, 1, 1]);
    paint.start = { x: 1, y: 0 };
    writePaint(writer, paint);
  });
  assertCommandError(
    () => decodeCommandStream(commandStream([noncanonicalPaint])),
    "invalid-value",
  );
}

function testValuePaintCommandsAndImageFit() {
  const canvas = makeCanvas();
  const fake = makeFakeCanvasKit();
  const executor = createCanvasKitExecutor({ CanvasKit: fake.CanvasKit, canvas });
  assertAck(executor.submit(initPacket()), 1);
  assertAck(
    executor.submit(resourceBatchPacket(2, 1, [upsert(1, 1, [1, 2, 3])])),
    2,
  );

  const commandStart = fake.log.length;
  assertAck(executor.submit(framePacket(3, 1, supportedCommandStream())), 3);
  const commandLog = fake.log.slice(commandStart);
  const imageDraws = commandLog.filter((entry) => entry?.type === "draw-image-rect");
  assertEqual(imageDraws.length, 5, "all image commands executed");
  assertRect(
    imageDraws[0].destination,
    { x: 40, y: 50, width: 128, height: 64 },
    "explicit image destination",
  );
  assertRect(
    imageDraws[1].destination,
    { x: 10, y: 70, width: 100, height: 50 },
    "contain destination",
  );
  assertRect(
    imageDraws[2].destination,
    { x: -90, y: 20, width: 200, height: 100 },
    "cover destination",
  );
  assertRect(
    imageDraws[3].destination,
    { x: 10, y: 20, width: 100, height: 100 },
    "fill destination",
  );
  assertRect(
    imageDraws[4].destination,
    { x: 10, y: 20, width: 64, height: 32 },
    "natural-size destination",
  );
  assert(
    commandLog.some((entry) => startsWithLog(entry, "create:linear-shader")),
    "linear value paint should create a shader",
  );
  assert(
    commandLog.some((entry) => startsWithLog(entry, "create:radial-shader")),
    "radial value paint should create a shader",
  );
  assert(
    commandLog.some((entry) => startsWithLog(entry, "create:dash-effect")),
    "dashed stroke should create a path effect",
  );
  for (const type of ["linear-shader", "radial-shader", "dash-effect", "path"]) {
    const values = fake.objects.filter((object) => object.type === type);
    assert(values.length > 0, `${type} should be created`);
    assert(values.every((object) => object.deleted), `${type} should be released after the frame`);
  }
  const staging = latestObject(fake, "staging-surface");
  assert(staging.deleted, "transactional staging surface should be disposed");
  assert(
    commandLog.some((entry) => entry === `dispose:staging-surface:${staging.id}`),
    "staging disposal must use CanvasKit Surface.dispose",
  );
  executor.destroy();
}

function testUnsupportedCommandsFailAndRetry() {
  const canvas = makeCanvas();
  const fake = makeFakeCanvasKit();
  const executor = createCanvasKitExecutor({ CanvasKit: fake.CanvasKit, canvas });
  const initResponse = assertAck(executor.submit(initPacket()), 1);
  assertEqual(initResponse.envelope.sequence, 1n, "first response sequence");

  const unsupported = [
    {
      label: "paragraph",
      code: ErrorCode.RESOURCE_FAILURE,
      entry: command(13, (writer) => {
        writeHandle(writer, 1, 1);
        writePoint(writer, { x: 2, y: 3 });
        writer.f32(1);
      }),
    },
    {
      label: "SVG",
      code: ErrorCode.INVALID_STATE,
      entry: command(16, (writer) => {
        writeHandle(writer, 1, 1);
        writeRect(writer, { x: 0, y: 0, width: 10, height: 10 });
      }),
    },
    {
      label: "Picture",
      code: ErrorCode.INVALID_STATE,
      entry: command(17, (writer) => writeHandle(writer, 1, 1)),
    },
  ];

  let responseSequence = 2n;
  for (const unsupportedCommand of unsupported) {
    const response = assertError(
      executor.submit(framePacket(2, 0, commandStream([unsupportedCommand.entry]))),
      unsupportedCommand.code,
      2,
    );
    assertEqual(response.envelope.sequence, responseSequence, "error response sequence");
    responseSequence += 1n;
    assert(
      response.packet.message.includes(unsupportedCommand.label),
      `${unsupportedCommand.label} failure should be explicit`,
    );
  }

  const unknown = assertError(
    executor.submit(framePacket(2, 0, commandStream([command(99)]))),
    ErrorCode.INVALID_PACKET,
    2,
  );
  assertEqual(unknown.envelope.sequence, responseSequence++, "unknown-command response sequence");
  const futureVersion = assertError(
    executor.submit(framePacket(2, 0, commandStream([], 2))),
    ErrorCode.UNSUPPORTED_VERSION,
    2,
  );
  assertEqual(futureVersion.envelope.sequence, responseSequence++, "version response sequence");
  const badSessionPacket = framePacket(2, 0);
  badSessionPacket.fill(0, 16, 24);
  const badSession = assertError(
    executor.submit(badSessionPacket),
    ErrorCode.INVALID_PACKET,
    2,
  );
  assertEqual(badSession.envelope.session, SESSION, "malformed command response session");
  assertEqual(badSession.envelope.sequence, responseSequence++, "malformed response sequence");
  assert(
    !fake.log.some((entry) => startsWithLog(entry, "draw-image:")),
    "failed frames must not reach the live surface",
  );

  const retry = assertAck(executor.submit(framePacket(2, 0)), 2);
  assertEqual(retry.envelope.sequence, responseSequence, "retry response sequence");
  executor.destroy();
}

function testGenerationalResourcesFailClosed() {
  const canvas = makeCanvas();
  const fake = makeFakeCanvasKit();
  const executor = createCanvasKitExecutor({ CanvasKit: fake.CanvasKit, canvas });
  assertAck(executor.submit(initPacket()), 1);
  assertAck(
    executor.submit(resourceBatchPacket(2, 1, [upsert(1, 1, [1, 2, 3])])),
    2,
  );
  const generationOne = latestObject(fake, "image");
  assertAck(
    executor.submit(resourceBatchPacket(3, 2, [
      release(1, 1),
      upsert(1, 2, [4, 5, 6]),
    ])),
    3,
  );
  assert(generationOne.deleted, "released generation should be deleted on commit");
  const generationTwo = latestObject(fake, "image");

  const destination = { x: 0, y: 0, width: 64, height: 32 };
  const staleDraw = commandStream([
    drawImageCommand(14, 1, 1, destination, {
      source: destination,
      sampling: 2,
    }),
  ]);
  assertError(
    executor.submit(framePacket(4, 2, staleDraw)),
    ErrorCode.RESOURCE_FAILURE,
    4,
  );
  assert(!generationTwo.deleted, "stale draw must preserve the live generation");

  const liveDraw = commandStream([
    drawImageCommand(14, 1, 2, destination, {
      source: destination,
      sampling: 2,
    }),
  ]);
  assertAck(executor.submit(framePacket(4, 2, liveDraw)), 4);

  const decodeCount = fake.log.filter((entry) => startsWithLog(entry, "decode-image:")).length;
  assertError(
    executor.submit(resourceBatchPacket(5, 3, [upsert(1, 3, [7, 8, 9])])),
    ErrorCode.INVALID_STATE,
    5,
  );
  assertEqual(
    fake.log.filter((entry) => startsWithLog(entry, "decode-image:")).length,
    decodeCount,
    "occupied generation must fail before CanvasKit mutation",
  );
  assert(!generationTwo.deleted, "rejected generation must preserve the committed resource");
  assertAck(
    executor.submit(resourceBatchPacket(5, 3, [release(1, 2)])),
    5,
  );
  assert(generationTwo.deleted, "retry release should commit");
  assertAck(
    executor.submit(resourceBatchPacket(6, 4, [upsert(1, 3, [7, 8, 9])])),
    6,
  );
  executor.destroy();
}

function testRetainedParagraphLifecycleAndPaint() {
  const canvas = makeCanvas();
  const fake = makeFakeCanvasKit();
  const executor = createCanvasKitExecutor({ CanvasKit: fake.CanvasKit, canvas });
  const request = paragraphRequestPacket("retained");

  assertExecutorFault(
    () => executor.layoutParagraph(request),
    ErrorCode.INVALID_STATE,
  );
  assertAck(executor.submit(initPacket()), 1);
  assertAck(
    executor.submit(resourceBatchPacket(2, 1, [
      upsert(7, 1, [1, 2, 3, 4], ResourceKind.FONT),
    ])),
    2,
  );

  const response = executor.layoutParagraph(request);
  const handle = paragraphResponseHandle(response);
  const paragraph = latestObject(fake, "paragraph");
  const provider = latestObject(fake, "font-provider");
  assert(paragraph && !paragraph.deleted, "layout should retain a live CanvasKit Paragraph");
  assert(provider && !provider.deleted, "layout should retain its owned font provider");
  assertEqual(paragraph.layoutWidths.length, 1, "paragraph should be laid out exactly once");

  assert(canvas.dispatch("webglcontextlost"), "paragraph test should enter context loss");
  assert(!paragraph.deleted, "CPU paragraph must survive recoverable WebGL context loss");
  assertExecutorFault(
    () => executor.layoutParagraph(request),
    ErrorCode.SURFACE_LOST,
  );
  canvas.dispatch("webglcontextrestored");

  const draw = commandStream([
    drawParagraphCommand(handle.slot, handle.generation),
  ]);
  const paintStart = fake.log.length;
  assertAck(executor.submit(framePacket(3, 1, draw)), 3);
  const paintLog = fake.log.slice(paintStart);
  const paragraphDraws = paintLog.filter((entry) => entry?.type === "draw-paragraph");
  assertEqual(paragraphDraws.length, 1, "retained paragraph draw count");
  assertEqual(paragraphDraws[0].paragraph, paragraph.id, "paint must use the layout paragraph");
  assertEqual(paragraph.layoutWidths.length, 1, "paint must never re-layout the paragraph");
  assert(
    paintLog.some((entry) => entry?.type === "translate" && entry.x === 12 && entry.y === 18),
    "paragraph paint should apply its origin",
  );
  assert(
    paintLog.some((entry) => entry?.type === "scale" && entry.x === 1.5 && entry.y === 1.5),
    "paragraph paint should apply its scale factor",
  );

  const stale = commandStream([
    drawParagraphCommand(handle.slot, handle.generation + 1),
  ]);
  assertError(
    executor.submit(framePacket(4, 1, stale)),
    ErrorCode.RESOURCE_FAILURE,
    4,
  );
  assertEqual(
    fake.log.filter((entry) => entry?.type === "draw-paragraph").length,
    1,
    "stale paragraph preflight must prevent painting",
  );
  assertAck(executor.submit(framePacket(4, 1, draw)), 4);

  executor.destroyParagraph(handle);
  assert(paragraph.deleted, "explicit paragraph destruction should delete the paragraph");
  assert(provider.deleted, "explicit paragraph destruction should delete its provider");
  assertExecutorFault(
    () => executor.destroyParagraph(handle),
    ErrorCode.RESOURCE_FAILURE,
  );
  assertError(
    executor.submit(framePacket(5, 1, draw)),
    ErrorCode.RESOURCE_FAILURE,
    5,
  );

  const replacementResponse = executor.layoutParagraph(paragraphRequestPacket("replacement"));
  const replacementHandle = paragraphResponseHandle(replacementResponse);
  const replacement = latestObject(fake, "paragraph");
  assertEqual(replacementHandle.slot, handle.slot, "paragraph slot should be reused");
  assertEqual(
    replacementHandle.generation,
    handle.generation + 1,
    "paragraph slot reuse must advance the generation",
  );
  assertAck(executor.submit(destroyPacket(5)), 5);
  assert(replacement.deleted, "session Destroy must clear retained paragraphs");
  assertExecutorFault(
    () => executor.layoutParagraph(request),
    ErrorCode.INVALID_STATE,
  );
  executor.destroy();
  executor.destroy();
}

function testContextEventsDoNotHideResponseSequences() {
  const canvas = makeCanvas();
  const fake = makeFakeCanvasKit();
  const executor = createCanvasKitExecutor({ CanvasKit: fake.CanvasKit, canvas });
  assertEqual(assertAck(executor.submit(initPacket()), 1).envelope.sequence, 1n, "init response");
  assert(canvas.dispatch("webglcontextlost"), "WebGL loss should be retained for restoration");
  const lostFrame = assertError(
    executor.submit(framePacket(2, 0)),
    ErrorCode.SURFACE_LOST,
    2,
  );
  assertEqual(
    lostFrame.envelope.sequence,
    2n,
    "an unobserved lifecycle event must not consume a response sequence",
  );
  canvas.dispatch("webglcontextrestored");
  const restoredFrame = assertAck(executor.submit(framePacket(2, 0)), 2);
  assertEqual(
    restoredFrame.envelope.sequence,
    3n,
    "restoration without an event sink must not consume a response sequence",
  );
  executor.destroy();
}

function testWebGlLifecycleAndResources() {
  const canvas = makeCanvas();
  const fake = makeFakeCanvasKit();
  const events = [];
  const executor = createCanvasKitExecutor({
    CanvasKit: fake.CanvasKit,
    canvas,
    eventSink: (event) => events.push(event),
  });

  assertEqual(canvas.listenerCount(), 2, "context listeners installed");
  assertAck(executor.submit(initPacket()), 1);
  assertEqual(canvas.width, 640, "initial canvas width");
  assertEqual(canvas.height, 480, "initial canvas height");
  assert(
    fake.log.findIndex((entry) => entry === "get-webgl-context") <
      fake.log.findIndex((entry) => startsWithLog(entry, "create:gr-context")) &&
      fake.log.findIndex((entry) => startsWithLog(entry, "create:gr-context")) <
        fake.log.findIndex((entry) => startsWithLog(entry, "make-webgl-surface")),
    "WebGL initialization should create Ganesh before the on-screen surface",
  );

  const firstBatch = resourceBatchPacket(2, 1, [upsert(1, 1, [1, 2, 3])]);
  assertAck(executor.submit(firstBatch), 2);
  const firstImage = latestObject(fake, "image");
  assert(firstImage && !firstImage.deleted, "first image should be live");

  const replacementStart = fake.log.length;
  assertAck(
    executor.submit(resourceBatchPacket(3, 2, [upsert(1, 1, [4, 5, 6])])),
    3,
  );
  const replacementLog = fake.log.slice(replacementStart);
  const replacementImage = latestObject(fake, "image");
  assert(replacementImage !== firstImage, "replacement should own a new image");
  assert(firstImage.deleted, "replaced image should be deleted");
  assert(
    replacementLog.findIndex((entry) => startsWithLog(entry, "create:image")) <
      replacementLog.findIndex((entry) => entry === `delete:image:${firstImage.id}`),
    "replacement must be decoded before the old object is deleted",
  );

  const failedBatch = resourceBatchPacket(4, 3, [
    upsert(2, 1, [7, 8]),
    upsert(3, 1, [0xff, 9]),
  ]);
  const failedResponse = assertError(
    executor.submit(failedBatch),
    ErrorCode.RESOURCE_FAILURE,
    4,
  );
  assert(
    failedResponse.packet.message.includes("image resource"),
    "resource failure should retain useful diagnostics",
  );
  const stagedImage = fake.objects.find(
    (object) => object.type === "image" && object.snapshot?.[0] === 7,
  );
  assert(stagedImage?.deleted, "staged resource should be deleted on rollback");
  assert(!replacementImage.deleted, "rollback must preserve the committed resource");

  assertAck(
    executor.submit(resourceBatchPacket(4, 3, [release(1, 1)])),
    4,
  );
  assert(replacementImage.deleted, "released resource should be deleted on commit");

  assertAck(executor.submit(framePacket(5, 3)), 5);
  const flushCount = fake.log.filter((entry) => startsWithLog(entry, "flush:")).length;
  assertError(executor.submit(framePacket(6, 3, [1])), ErrorCode.INVALID_PACKET, 6);
  assertEqual(
    fake.log.filter((entry) => startsWithLog(entry, "flush:")).length,
    flushCount,
    "nonempty command stream must not be presented",
  );
  assertAck(executor.submit(framePacket(6, 3)), 6);

  const oldSurface = latestObject(fake, "webgl-surface");
  fake.controls.failNextSurface = true;
  assertError(
    executor.submit(resizePacket(7, 800, 600)),
    ErrorCode.SURFACE_LOST,
    7,
  );
  assert(!oldSurface.deleted, "failed resize must preserve the committed surface");
  assertEqual(canvas.width, 640, "failed resize restores canvas width");
  assertEqual(canvas.height, 480, "failed resize restores canvas height");
  assertAck(executor.submit(resizePacket(7, 800, 600)), 7);
  assert(oldSurface.deleted, "resize should delete the replaced surface");
  assertEqual(canvas.width, 800, "resized canvas width");
  assertEqual(canvas.height, 600, "resized canvas height");

  const retainedBatch = resourceBatchPacket(8, 4, [upsert(4, 1, [10, 11, 12])]);
  assertAck(executor.submit(retainedBatch), 8);
  retainedBatch.fill(0);
  const retainedImage = latestObject(fake, "image");
  const contextLossStart = fake.log.length;
  assert(canvas.dispatch("webglcontextlost"), "context loss must call preventDefault");
  assert(retainedImage.deleted, "context loss should drop CanvasKit resource objects");
  const contextLossLog = fake.log.slice(contextLossStart);
  assert(
    contextLossLog.findIndex((entry) => entry === `delete:image:${retainedImage.id}`) <
      contextLossLog.findIndex((entry) => startsWithLog(entry, "delete:webgl-surface")),
    "context loss must release resources before the surface",
  );
  assert(
    contextLossLog.findIndex((entry) => startsWithLog(entry, "delete:webgl-surface")) <
      contextLossLog.findIndex((entry) => startsWithLog(entry, "abandon:gr-context")),
    "context loss must release the surface before abandoning Ganesh",
  );
  assertEqual(events.at(-1).type, "context-lost", "context-loss event type");
  assertError(events.at(-1).packet, ErrorCode.SURFACE_LOST, 8);
  const lostDecodeCount = fake.log.filter((entry) => startsWithLog(entry, "decode-image:")).length;
  assertError(
    executor.submit(resourceBatchPacket(9, 5, [upsert(5, 1, [13, 14, 15])])),
    ErrorCode.SURFACE_LOST,
    9,
  );
  assertEqual(
    fake.log.filter((entry) => startsWithLog(entry, "decode-image:")).length,
    lostDecodeCount,
    "resource batches must not mutate CanvasKit during context loss",
  );
  assertError(
    executor.submit(framePacket(9, 4, [], { width: 800, height: 600 })),
    ErrorCode.SURFACE_LOST,
    9,
  );

  fake.controls.failNextImageDecode = true;
  canvas.dispatch("webglcontextrestored");
  assertEqual(
    events.at(-1).type,
    "context-restore-failed",
    "failed restoration event type",
  );
  assertError(events.at(-1).packet, ErrorCode.RESOURCE_FAILURE, 8);
  const failedRestoreSurface = latestObject(fake, "webgl-surface");
  assert(failedRestoreSurface.deleted, "failed restoration must dispose its candidate surface");

  canvas.dispatch("webglcontextrestored");
  assertEqual(events.at(-1).type, "context-restored", "context-restored event type");
  assertAck(events.at(-1).packet, 8);
  const restoredImage = latestObject(fake, "image");
  assert(
    restoredImage.snapshot.join(",") === "10,11,12",
    "restoration must use the owned resource copy, not the submitted Wasm view",
  );
  assertAck(
    executor.submit(framePacket(9, 4, [], { width: 800, height: 600 })),
    9,
  );

  const destroyStart = fake.log.length;
  assertAck(executor.submit(destroyPacket(10)), 10);
  assert(restoredImage.deleted, "session destroy should delete restored resources");
  const activeSurface = latestObject(fake, "webgl-surface");
  assert(activeSurface.deleted, "session destroy should delete the surface");
  const destroyLog = fake.log.slice(destroyStart);
  assert(
    destroyLog.findIndex((entry) => entry === `delete:image:${restoredImage.id}`) <
      destroyLog.findIndex((entry) => entry === `delete:webgl-surface:${activeSurface.id}`),
    "teardown must release resources before the surface",
  );
  assert(
    destroyLog.findIndex((entry) => entry === `delete:webgl-surface:${activeSurface.id}`) <
      destroyLog.findIndex((entry) => startsWithLog(entry, "abandon:gr-context")),
    "teardown must release the surface before abandoning Ganesh",
  );

  executor.destroy();
  executor.destroy();
  assertEqual(canvas.listenerCount(), 0, "executor destroy removes listeners");
}

function testSoftwareFallbackAndExplicitWebGlFailure() {
  const fallbackCanvas = makeCanvas();
  const fallback = makeFakeCanvasKit();
  fallback.controls.webGlAvailable = false;
  const fallbackExecutor = createCanvasKitExecutor({
    CanvasKit: fallback.CanvasKit,
    canvas: fallbackCanvas,
  });
  assertAck(fallbackExecutor.submit(initPacket(BackendPreference.AUTO)), 1);
  assert(
    fallback.log.some((entry) => startsWithLog(entry, "make-software-surface")),
    "auto mode should use software when no WebGL context was acquired",
  );
  assert(
    !fallbackCanvas.dispatch("webglcontextlost"),
    "software surface must ignore WebGL context-loss events",
  );
  assertAck(fallbackExecutor.submit(framePacket(2, 0)), 2);
  const softwareSurface = latestObject(fallback, "software-surface");
  fallbackExecutor.destroy();
  assert(softwareSurface.deleted, "software surface should be destroyed");
  assert(
    fallback.log.includes(`dispose:software-surface:${softwareSurface.id}`),
    "software teardown must use CanvasKit Surface.dispose",
  );

  const graphiteCanvas = makeCanvas();
  const graphite = makeFakeCanvasKit();
  const graphiteExecutor = createCanvasKitExecutor({
    CanvasKit: graphite.CanvasKit,
    canvas: graphiteCanvas,
  });
  const graphiteError = assertError(
    graphiteExecutor.submit(initPacket(BackendPreference.GRAPHITE)),
    ErrorCode.INVALID_STATE,
    1,
  );
  assert(
    graphiteError.packet.message.includes("Graphite/Dawn"),
    "Graphite failure should name the unfinished backend",
  );
  assert(
    !graphite.log.some((entry) => startsWithLog(entry, "get-webgl-context")) &&
      !graphite.log.some((entry) => startsWithLog(entry, "make-software-surface")),
    "Graphite must not be silently reinterpreted as another backend",
  );
  assertAck(graphiteExecutor.submit(initPacket(BackendPreference.SOFTWARE)), 1);
  graphiteExecutor.destroy();

  const wideGamutCanvas = makeCanvas();
  const wideGamut = makeFakeCanvasKit();
  const wideGamutExecutor = createCanvasKitExecutor({
    CanvasKit: wideGamut.CanvasKit,
    canvas: wideGamutCanvas,
  });
  const wideGamutError = assertError(
    wideGamutExecutor.submit(
      initPacket(BackendPreference.SOFTWARE, { colorSpace: 2 }),
    ),
    ErrorCode.INVALID_STATE,
    1,
  );
  assert(
    wideGamutError.packet.message.includes("non-sRGB"),
    "software color-space downgrade must fail explicitly",
  );
  assertEqual(wideGamutCanvas.width, 0, "failed init restores canvas width");
  assertEqual(wideGamutCanvas.height, 0, "failed init restores canvas height");
  wideGamutExecutor.destroy();

  const strictCanvas = makeCanvas();
  const strict = makeFakeCanvasKit();
  strict.controls.webGlAvailable = false;
  const strictExecutor = createCanvasKitExecutor({
    CanvasKit: strict.CanvasKit,
    canvas: strictCanvas,
    backendPreference: "webgl",
  });
  assertError(
    strictExecutor.submit(initPacket(BackendPreference.AUTO)),
    ErrorCode.SURFACE_LOST,
    1,
  );
  assert(
    !strict.log.some((entry) => startsWithLog(entry, "make-software-surface")),
    "explicit WebGL mode must not silently fall back",
  );
  strictExecutor.destroy();
}

function testBoundedErrorEncoding() {
  const encoded = encodeError({
    session: SESSION,
    sequence: 1n,
    failedSequence: 1n,
    code: ErrorCode.INTERNAL,
    message: "🦀".repeat(10_000),
  });
  assert(encoded.byteLength <= 32 + 16 + 4096, "error packet exceeded its bound");
  const decoded = decodeMessage(encoded);
  assertEqual(decoded.packet.code, ErrorCode.INTERNAL, "bounded error code");
  assert(new TextEncoder().encode(decoded.packet.message).byteLength <= 4096, "UTF-8 bound");
}

testStrictCommandDecoder();
testValuePaintCommandsAndImageFit();
testUnsupportedCommandsFailAndRetry();
testGenerationalResourcesFailClosed();
testRetainedParagraphLifecycleAndPaint();
testContextEventsDoNotHideResponseSequences();
testWebGlLifecycleAndResources();
testSoftwareFallbackAndExplicitWebGlFailure();
testBoundedErrorEncoding();

console.log("CanvasKit executor fixtures passed");
