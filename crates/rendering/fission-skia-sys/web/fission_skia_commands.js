import { ErrorCode, ResourceKind } from "./fission_skia_web.js";

export const COMMAND_MAGIC = Object.freeze([0x46, 0x53, 0x43, 0x4d]); // FSCM
export const COMMAND_VERSION = 1;
export const COMMAND_HEADER_LEN = 16;
export const MAX_COMMAND_STREAM_BYTES = 32 * 1024 * 1024;
export const MAX_COMMANDS = 262144;
export const MAX_PATH_COMMANDS = 1048576;
export const MAX_GRADIENT_STOPS = 65536;
export const MAX_DASH_INTERVALS = 65536;

const ENTRY_HEADER_LEN = 8;
const PATH_ENTRY_LEN = 28;

export class CommandStreamError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "CommandStreamError";
    this.code = code;
    this.protocolCode =
      code === "unsupported-version"
        ? ErrorCode.UNSUPPORTED_VERSION
        : ErrorCode.INVALID_PACKET;
  }
}

export class CommandExecutionError extends Error {
  constructor(protocolCode, message) {
    super(message);
    this.name = "CommandExecutionError";
    this.protocolCode = protocolCode;
  }
}

function reject(code, message) {
  throw new CommandStreamError(code, message);
}

function executionFailure(code, message) {
  throw new CommandExecutionError(code, message);
}

function toBytes(input) {
  if (input instanceof Uint8Array) return input;
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  if (input instanceof ArrayBuffer) return new Uint8Array(input);
  reject("invalid-buffer", "command stream must be an ArrayBuffer or typed-array view");
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

  require(count) {
    if (!Number.isSafeInteger(count) || count < 0 || count > this.remaining()) {
      reject("truncated", "command stream is truncated");
    }
  }

  take(count) {
    this.require(count);
    const value = this.bytes.subarray(this.position, this.position + count);
    this.position += count;
    return value;
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

  f32() {
    this.require(4);
    const value = this.view.getFloat32(this.position, true);
    this.position += 4;
    return value;
  }

  finish() {
    if (this.remaining() !== 0) reject("length-mismatch", "command payload has trailing bytes");
  }
}

function requireLimit(field, actual, maximum) {
  if (actual > maximum) {
    reject("limit-exceeded", `${field} ${actual} exceeds ${maximum}`);
  }
}

function requireZero(bytes, field) {
  if (bytes.some((value) => value !== 0)) {
    reject("nonzero-reserved", `${field} reserved bytes must be zero`);
  }
}

function finite(value, field) {
  if (!Number.isFinite(value)) reject("invalid-value", `${field} must be finite`);
  return value;
}

function nonNegative(value, field) {
  value = finite(value, field);
  if (value < 0) reject("invalid-value", `${field} must be non-negative`);
  return value;
}

function positive(value, field) {
  value = finite(value, field);
  if (value <= 0) reject("invalid-value", `${field} must be positive`);
  return value;
}

function unit(value, field) {
  value = finite(value, field);
  if (value < 0 || value > 1) reject("invalid-value", `${field} must be in 0..=1`);
  return value;
}

function decodeColor(reader) {
  return [
    unit(reader.f32(), "color component"),
    unit(reader.f32(), "color component"),
    unit(reader.f32(), "color component"),
    unit(reader.f32(), "color component"),
  ];
}

function decodePoint(reader) {
  return {
    x: finite(reader.f32(), "point"),
    y: finite(reader.f32(), "point"),
  };
}

function decodeRect(reader, field = "rectangle", requireNonEmpty = false) {
  const rect = {
    x: finite(reader.f32(), field),
    y: finite(reader.f32(), field),
    width: nonNegative(reader.f32(), field),
    height: nonNegative(reader.f32(), field),
  };
  if (
    !Number.isFinite(Math.fround(rect.x + rect.width)) ||
    !Number.isFinite(Math.fround(rect.y + rect.height))
  ) {
    reject("invalid-value", `${field} extent must be finite`);
  }
  if (requireNonEmpty && (rect.width <= 0 || rect.height <= 0)) {
    reject("invalid-value", `${field} must be non-empty`);
  }
  return rect;
}

function decodeHandle(reader) {
  const handle = { slot: reader.u32(), generation: reader.u32() };
  if (handle.slot === 0 || handle.generation === 0) {
    reject("invalid-value", "resource handle must be non-zero");
  }
  return handle;
}

function isZeroPoint(point) {
  return point.x === 0 && point.y === 0;
}

function isTransparent(color) {
  return color.every((component) => component === 0);
}

function decodePaint(reader) {
  reader.require(44);
  const kind = reader.u8();
  requireZero(reader.take(3), "paint");
  const solid = decodeColor(reader);
  const start = decodePoint(reader);
  const end = decodePoint(reader);
  const radius = nonNegative(reader.f32(), "gradient radius");
  const stopCount = reader.u32();
  requireLimit("gradient stops", stopCount, MAX_GRADIENT_STOPS);
  if (stopCount > Math.floor(reader.remaining() / 20)) {
    reject("truncated", "gradient stops are truncated");
  }
  const stops = [];
  for (let index = 0; index < stopCount; index += 1) {
    stops.push({
      offset: unit(reader.f32(), "gradient stop"),
      color: decodeColor(reader),
    });
  }
  for (let index = 1; index < stops.length; index += 1) {
    if (stops[index - 1].offset >= stops[index].offset) {
      reject("invalid-value", "gradient stop offsets must be strictly increasing");
    }
  }

  if (
    kind === 1 &&
    stopCount === 0 &&
    isZeroPoint(start) &&
    isZeroPoint(end) &&
    radius === 0
  ) {
    return { kind: "solid", color: solid };
  }
  if (kind === 2 && isTransparent(solid) && radius === 0) {
    return { kind: "linear-gradient", start, end, stops };
  }
  if (kind === 3 && isTransparent(solid) && isZeroPoint(end)) {
    return { kind: "radial-gradient", center: start, radius, stops };
  }
  reject("invalid-value", "paint kind or payload is noncanonical");
}

function decodeStroke(reader) {
  reader.require(12);
  const width = nonNegative(reader.f32(), "stroke width");
  const lineCap = reader.u8();
  if (lineCap < 1 || lineCap > 3) reject("invalid-value", "invalid line cap");
  const lineJoin = reader.u8();
  if (lineJoin < 1 || lineJoin > 3) reject("invalid-value", "invalid line join");
  requireZero(reader.take(2), "stroke");
  const dashCount = reader.u32();
  requireLimit("dash intervals", dashCount, MAX_DASH_INTERVALS);
  const paint = decodePaint(reader);
  if (dashCount > Math.floor(reader.remaining() / 4)) {
    reject("truncated", "dash intervals are truncated");
  }
  const dashes = [];
  for (let index = 0; index < dashCount; index += 1) {
    dashes.push(nonNegative(reader.f32(), "dash interval"));
  }
  if (dashCount % 2 !== 0 || (dashes.length > 0 && dashes.every((dash) => dash === 0))) {
    reject("invalid-value", "dash intervals are noncanonical");
  }
  return { width, lineCap, lineJoin, paint, dashes };
}

function decodePath(reader) {
  reader.require(8);
  const fillRule = reader.u8();
  if (fillRule !== 1 && fillRule !== 2) reject("invalid-value", "invalid fill rule");
  requireZero(reader.take(3), "path");
  const count = reader.u32();
  if (count === 0) reject("invalid-value", "path must not be empty");
  requireLimit("path commands", count, MAX_PATH_COMMANDS);
  if (count > Math.floor(reader.remaining() / PATH_ENTRY_LEN)) {
    reject("truncated", "path commands are truncated");
  }

  const commands = [];
  let current = false;
  for (let index = 0; index < count; index += 1) {
    const kind = reader.u8();
    requireZero(reader.take(3), "path command");
    const values = [];
    for (let value = 0; value < 6; value += 1) {
      values.push(finite(reader.f32(), "path coordinate"));
    }
    const zeroFrom = (offset) => values.slice(offset).every((value) => value === 0);
    switch (kind) {
      case 1:
        if (!zeroFrom(2)) reject("invalid-value", "move path payload is noncanonical");
        current = true;
        commands.push({ kind: "move", x: values[0], y: values[1] });
        break;
      case 2:
        if (!current || !zeroFrom(2)) reject("invalid-value", "line path payload is invalid");
        commands.push({ kind: "line", x: values[0], y: values[1] });
        break;
      case 3:
        if (!current || !zeroFrom(4)) reject("invalid-value", "quad path payload is invalid");
        commands.push({
          kind: "quad",
          cx: values[0],
          cy: values[1],
          x: values[2],
          y: values[3],
        });
        break;
      case 4:
        if (!current) reject("invalid-value", "cubic path has no contour");
        commands.push({
          kind: "cubic",
          c1x: values[0],
          c1y: values[1],
          c2x: values[2],
          c2y: values[3],
          x: values[4],
          y: values[5],
        });
        break;
      case 5:
        if (!current || !zeroFrom(0)) reject("invalid-value", "close path payload is invalid");
        commands.push({ kind: "close" });
        break;
      default:
        reject("invalid-value", `unknown path command ${kind}`);
    }
  }
  return { fillRule, commands };
}

function decodeCommand(kind, reader, state) {
  switch (kind) {
    case 1:
      return { kind: "clear", color: decodeColor(reader) };
    case 2:
      state.saveDepth += 1;
      return { kind: "save" };
    case 3:
      if (state.saveDepth === 0) reject("unbalanced-restore", "restore has no matching save");
      state.saveDepth -= 1;
      return { kind: "restore" };
    case 4: {
      const command = {
        kind: "opacity-layer",
        bounds: decodeRect(reader),
        alpha: unit(reader.f32(), "opacity alpha"),
      };
      state.saveDepth += 1;
      return command;
    }
    case 5:
      return { kind: "clip-rect", rect: decodeRect(reader) };
    case 6:
      return {
        kind: "clip-rounded-rect",
        rect: decodeRect(reader),
        radius: nonNegative(reader.f32(), "clip radius"),
      };
    case 7:
      return {
        kind: "concat-affine",
        affine: [
          finite(reader.f32(), "affine"),
          finite(reader.f32(), "affine"),
          finite(reader.f32(), "affine"),
          finite(reader.f32(), "affine"),
          finite(reader.f32(), "affine"),
          finite(reader.f32(), "affine"),
        ],
      };
    case 8:
      return {
        kind: "fill-rect",
        rect: decodeRect(reader),
        radius: nonNegative(reader.f32(), "rectangle radius"),
        paint: decodePaint(reader),
      };
    case 9:
      return {
        kind: "stroke-rect",
        rect: decodeRect(reader),
        radius: nonNegative(reader.f32(), "rectangle radius"),
        stroke: decodeStroke(reader),
      };
    case 10:
      return { kind: "fill-path", path: decodePath(reader), paint: decodePaint(reader) };
    case 11:
      return { kind: "stroke-path", path: decodePath(reader), stroke: decodeStroke(reader) };
    case 12: {
      const command = {
        kind: "box-shadow",
        rect: decodeRect(reader),
        radius: nonNegative(reader.f32(), "shadow radius"),
        color: decodeColor(reader),
        blurRadius: nonNegative(reader.f32(), "shadow blur"),
        spreadRadius: finite(reader.f32(), "shadow spread"),
        offset: decodePoint(reader),
      };
      const inset = reader.u8();
      if (inset !== 0 && inset !== 1) reject("invalid-value", "shadow inset is invalid");
      command.inset = inset === 1;
      requireZero(reader.take(3), "box shadow");
      return command;
    }
    case 13:
      return {
        kind: "draw-paragraph",
        handle: decodeHandle(reader),
        origin: decodePoint(reader),
        scaleFactor: positive(reader.f32(), "paragraph scale factor"),
      };
    case 14: {
      const command = {
        kind: "draw-image",
        handle: decodeHandle(reader),
        source: decodeRect(reader, "image source", true),
        destination: decodeRect(reader, "image destination", true),
      };
      const sampling = reader.u8();
      if (sampling !== 1 && sampling !== 2) reject("invalid-value", "image sampling is invalid");
      command.sampling = sampling;
      requireZero(reader.take(3), "draw image");
      return command;
    }
    case 15:
      return {
        kind: "backdrop-blur",
        bounds: decodeRect(reader),
        cornerRadius: nonNegative(reader.f32(), "backdrop radius"),
        sigma: nonNegative(reader.f32(), "backdrop sigma"),
      };
    case 16:
      return {
        kind: "draw-svg",
        handle: decodeHandle(reader),
        destination: decodeRect(reader, "SVG destination", true),
      };
    case 17:
      return { kind: "draw-picture", handle: decodeHandle(reader) };
    case 18: {
      const command = {
        kind: "draw-image-fit",
        handle: decodeHandle(reader),
        target: decodeRect(reader, "image target", true),
      };
      const fit = reader.u8();
      if (fit < 1 || fit > 4) reject("invalid-value", "image fit is invalid");
      command.fit = fit;
      const alignment = reader.u8();
      if (alignment < 1 || alignment > 9) {
        reject("invalid-value", "image alignment is invalid");
      }
      command.alignment = alignment;
      const sampling = reader.u8();
      if (sampling !== 1 && sampling !== 2) reject("invalid-value", "image sampling is invalid");
      command.sampling = sampling;
      if (reader.u8() !== 0) reject("nonzero-reserved", "draw image fit reserved byte must be zero");
      return command;
    }
    default:
      reject("unknown-command", `unknown command ${kind}`);
  }
}

/** Decode FSCM v1 into owned JavaScript values; no input views are retained. */
export function decodeCommandStream(input) {
  const bytes = toBytes(input);
  requireLimit("command stream bytes", bytes.byteLength, MAX_COMMAND_STREAM_BYTES);
  if (bytes.byteLength < COMMAND_HEADER_LEN) reject("truncated", "command header is truncated");
  const reader = new Reader(bytes);
  for (const expected of COMMAND_MAGIC) {
    if (reader.u8() !== expected) reject("invalid-magic", "command stream does not begin FSCM");
  }
  const version = reader.u16();
  if (version !== COMMAND_VERSION) {
    reject("unsupported-version", `unsupported command version ${version}`);
  }
  requireZero(reader.take(2), "command header");
  const declaredLength = reader.u32();
  if (declaredLength !== bytes.byteLength) {
    reject("length-mismatch", "command stream length does not match its header");
  }
  const commandCount = reader.u32();
  requireLimit("commands", commandCount, MAX_COMMANDS);
  if (commandCount > Math.floor(reader.remaining() / ENTRY_HEADER_LEN)) {
    reject("truncated", "command entries are truncated");
  }

  const commands = [];
  const state = { saveDepth: 0 };
  for (let index = 0; index < commandCount; index += 1) {
    const kind = reader.u16();
    requireZero(reader.take(2), "command entry");
    const entryLength = reader.u32();
    if (entryLength < ENTRY_HEADER_LEN) reject("length-mismatch", "command entry is too short");
    const payload = new Reader(reader.take(entryLength - ENTRY_HEADER_LEN));
    commands.push(decodeCommand(kind, payload, state));
    payload.finish();
  }
  reader.finish();
  if (state.saveDepth !== 0) {
    reject("unclosed-save", `command stream leaves ${state.saveDepth} save scopes open`);
  }
  return commands;
}

function safeDelete(value) {
  if (!value || typeof value.delete !== "function") return;
  try {
    if (typeof value.isDeleted !== "function" || !value.isDeleted()) value.delete();
  } catch (_error) {
    // Continue cleaning the rest of the transient command objects.
  }
}

function requireFunction(owner, name) {
  const value = owner?.[name];
  if (typeof value !== "function") {
    executionFailure(ErrorCode.INVALID_STATE, `CanvasKit API ${name} is unavailable`);
  }
  return value.bind(owner);
}

function requireEnum(owner, name) {
  const value = owner?.[name];
  if (value === undefined || value === null) {
    executionFailure(ErrorCode.INVALID_STATE, `CanvasKit enum ${name} is unavailable`);
  }
  return value;
}

function rect(CanvasKit, value) {
  return requireFunction(CanvasKit, "XYWHRect")(
    value.x,
    value.y,
    value.width,
    value.height,
  );
}

function roundedRect(CanvasKit, value, radius) {
  return requireFunction(CanvasKit, "RRectXY")(rect(CanvasKit, value), radius, radius);
}

function color(CanvasKit, value) {
  return requireFunction(CanvasKit, "Color4f")(...value);
}

function makePaint(CanvasKit, source) {
  const Paint = CanvasKit.Paint;
  if (typeof Paint !== "function") {
    executionFailure(ErrorCode.INVALID_STATE, "CanvasKit Paint constructor is unavailable");
  }
  const paint = new Paint();
  const owned = [];
  try {
    paint.setAntiAlias(true);
    if (source.kind === "solid") {
      paint.setColor(color(CanvasKit, source.color), CanvasKit.ColorSpace?.SRGB);
      return { paint, owned };
    }

    if (source.stops.length === 0) {
      paint.setColor(color(CanvasKit, [0, 0, 0, 0]), CanvasKit.ColorSpace?.SRGB);
      return { paint, owned };
    }
    if (source.stops.length === 1) {
      paint.setColor(color(CanvasKit, source.stops[0].color), CanvasKit.ColorSpace?.SRGB);
      return { paint, owned };
    }
    if (
      source.kind === "linear-gradient" &&
      source.start.x === source.end.x &&
      source.start.y === source.end.y
    ) {
      paint.setColor(color(CanvasKit, source.stops.at(-1).color), CanvasKit.ColorSpace?.SRGB);
      return { paint, owned };
    }
    if (source.kind === "radial-gradient" && source.radius <= 0) {
      paint.setColor(color(CanvasKit, source.stops.at(-1).color), CanvasKit.ColorSpace?.SRGB);
      return { paint, owned };
    }

    const colors = source.stops.map((stop) => color(CanvasKit, stop.color));
    const positions = source.stops.map((stop) => stop.offset);
    const tileMode = requireEnum(CanvasKit.TileMode, "Clamp");
    const shader =
      source.kind === "linear-gradient"
        ? requireFunction(CanvasKit.Shader, "MakeLinearGradient")(
            [source.start.x, source.start.y],
            [source.end.x, source.end.y],
            colors,
            positions,
            tileMode,
          )
        : requireFunction(CanvasKit.Shader, "MakeRadialGradient")(
            [source.center.x, source.center.y],
            source.radius,
            colors,
            positions,
            tileMode,
          );
    if (!shader || typeof shader.delete !== "function") {
      executionFailure(ErrorCode.INTERNAL, "CanvasKit rejected a validated gradient");
    }
    owned.push(shader);
    paint.setShader(shader);
    return { paint, owned };
  } catch (error) {
    for (const object of owned) safeDelete(object);
    safeDelete(paint);
    throw error;
  }
}

function configureStroke(CanvasKit, prepared, stroke) {
  const { paint, owned } = prepared;
  paint.setStyle(requireEnum(CanvasKit.PaintStyle, "Stroke"));
  paint.setStrokeWidth(stroke.width);
  paint.setStrokeCap(
    requireEnum(CanvasKit.StrokeCap, ["Butt", "Round", "Square"][stroke.lineCap - 1]),
  );
  paint.setStrokeJoin(
    requireEnum(CanvasKit.StrokeJoin, ["Miter", "Round", "Bevel"][stroke.lineJoin - 1]),
  );
  if (stroke.dashes.length > 0) {
    const effect = requireFunction(CanvasKit.PathEffect, "MakeDash")(stroke.dashes, 0);
    if (!effect || typeof effect.delete !== "function") {
      executionFailure(ErrorCode.INTERNAL, "CanvasKit rejected validated dash intervals");
    }
    owned.push(effect);
    paint.setPathEffect(effect);
  }
}

function disposePaint(prepared) {
  safeDelete(prepared?.paint);
  for (const object of prepared?.owned ?? []) safeDelete(object);
}

function makePath(CanvasKit, source) {
  const PathBuilder = CanvasKit.PathBuilder;
  if (typeof PathBuilder !== "function") {
    executionFailure(ErrorCode.INVALID_STATE, "CanvasKit PathBuilder constructor is unavailable");
  }
  let builder = new PathBuilder();
  try {
    for (const command of source.commands) {
      switch (command.kind) {
        case "move":
          builder.moveTo(command.x, command.y);
          break;
        case "line":
          builder.lineTo(command.x, command.y);
          break;
        case "quad":
          builder.quadTo(command.cx, command.cy, command.x, command.y);
          break;
        case "cubic":
          builder.cubicTo(
            command.c1x,
            command.c1y,
            command.c2x,
            command.c2y,
            command.x,
            command.y,
          );
          break;
        case "close":
          builder.close();
          break;
        default:
          executionFailure(ErrorCode.INTERNAL, "decoded path command is invalid");
      }
    }
    const path = builder.detachAndDelete();
    builder = null;
    if (!path || typeof path.delete !== "function") {
      executionFailure(ErrorCode.INTERNAL, "CanvasKit rejected a validated path");
    }
    path.setFillType(
      source.fillRule === 2
        ? requireEnum(CanvasKit.FillType, "EvenOdd")
        : requireEnum(CanvasKit.FillType, "Winding"),
    );
    return path;
  } finally {
    safeDelete(builder);
  }
}

function requireResource(resolveResource, handle, expectedKind, label) {
  const resource = resolveResource(handle.slot);
  if (!resource) {
    executionFailure(ErrorCode.RESOURCE_FAILURE, `${label} resource slot is not live`);
  }
  if (resource.generation !== handle.generation) {
    executionFailure(ErrorCode.RESOURCE_FAILURE, `${label} resource generation is stale`);
  }
  if (resource.kind !== expectedKind) {
    executionFailure(ErrorCode.RESOURCE_FAILURE, `${label} resource kind is invalid`);
  }
  if (!resource.object || typeof resource.object.delete !== "function") {
    executionFailure(ErrorCode.RESOURCE_FAILURE, `${label} CanvasKit object is unavailable`);
  }
  if (typeof resource.object.isDeleted === "function" && resource.object.isDeleted()) {
    executionFailure(ErrorCode.RESOURCE_FAILURE, `${label} CanvasKit object was released`);
  }
  return resource.object;
}

function preflightResources(commands, resolveResource) {
  const resolved = new Map();
  commands.forEach((command, index) => {
    switch (command.kind) {
      case "draw-image":
      case "draw-image-fit":
        resolved.set(
          index,
          requireResource(resolveResource, command.handle, ResourceKind.IMAGE, "image"),
        );
        break;
      case "draw-paragraph":
        executionFailure(
          ErrorCode.INVALID_STATE,
          "paragraph resources await the versioned CanvasKit paragraph schema",
        );
        break;
      case "draw-svg":
        executionFailure(
          ErrorCode.INVALID_STATE,
          "DrawSvg is invalid on Web; SVG must be lowered into neutral paint commands",
        );
        break;
      case "draw-picture":
        executionFailure(
          ErrorCode.INVALID_STATE,
          "DrawPicture is invalid on Web; cached scenes must expand through the compiler",
        );
        break;
      case "clear":
      case "save":
      case "restore":
      case "opacity-layer":
      case "clip-rect":
      case "clip-rounded-rect":
      case "concat-affine":
      case "fill-rect":
      case "stroke-rect":
      case "fill-path":
      case "stroke-path":
      case "box-shadow":
      case "backdrop-blur":
        break;
      default:
        executionFailure(
          ErrorCode.INTERNAL,
          `decoded command ${String(command.kind)} is invalid`,
        );
    }
  });
  return resolved;
}

function samplingOptions(CanvasKit, sampling) {
  return [
    sampling === 1
      ? requireEnum(CanvasKit.FilterMode, "Nearest")
      : requireEnum(CanvasKit.FilterMode, "Linear"),
    requireEnum(CanvasKit.MipmapMode, "None"),
  ];
}

function alignedOffset(extraWidth, extraHeight, alignment) {
  const column = (alignment - 1) % 3;
  const row = Math.floor((alignment - 1) / 3);
  return {
    x: column === 0 ? 0 : column === 1 ? Math.fround(extraWidth / 2) : extraWidth,
    y: row === 0 ? 0 : row === 1 ? Math.fround(extraHeight / 2) : extraHeight,
  };
}

function drawImageFit(CanvasKit, canvas, image, command) {
  const widthFunction = requireFunction(image, "width");
  const heightFunction = requireFunction(image, "height");
  const imageWidth = widthFunction.call(image);
  const imageHeight = heightFunction.call(image);
  if (
    !Number.isFinite(imageWidth) ||
    !Number.isFinite(imageHeight) ||
    !Number.isFinite(Math.fround(imageWidth)) ||
    !Number.isFinite(Math.fround(imageHeight)) ||
    imageWidth <= 0 ||
    imageHeight <= 0
  ) {
    executionFailure(ErrorCode.RESOURCE_FAILURE, "decoded image has invalid intrinsic dimensions");
  }

  const { target } = command;
  const sourceWidth = Math.fround(imageWidth);
  const sourceHeight = Math.fround(imageHeight);
  let width;
  let height;
  if (command.fit === 3) {
    width = target.width;
    height = target.height;
  } else if (command.fit === 4) {
    width = sourceWidth;
    height = sourceHeight;
  } else {
    const scale = command.fit === 1
      ? Math.min(
          Math.fround(target.width / sourceWidth),
          Math.fround(target.height / sourceHeight),
        )
      : Math.max(
          Math.fround(target.width / sourceWidth),
          Math.fround(target.height / sourceHeight),
        );
    width = Math.fround(sourceWidth * scale);
    height = Math.fround(sourceHeight * scale);
  }
  const offset = command.fit === 3 || command.fit === 4
    ? { x: 0, y: 0 }
    : alignedOffset(
        Math.fround(target.width - width),
        Math.fround(target.height - height),
        command.alignment,
      );
  const destination = {
    x: Math.fround(target.x + offset.x),
    y: Math.fround(target.y + offset.y),
    width,
    height,
  };
  if (
    !Object.values(destination).every(
      (value) => Number.isFinite(value) && Number.isFinite(Math.fround(value)),
    )
  ) {
    executionFailure(ErrorCode.RESOURCE_FAILURE, "decoded image placement is invalid");
  }
  const [filterMode, mipmapMode] = samplingOptions(CanvasKit, command.sampling);
  canvas.save();
  try {
    canvas.clipRect(
      rect(CanvasKit, target),
      requireEnum(CanvasKit.ClipOp, "Intersect"),
      true,
    );
    canvas.drawImageRectOptions(
      image,
      rect(CanvasKit, { x: 0, y: 0, width: sourceWidth, height: sourceHeight }),
      rect(CanvasKit, destination),
      filterMode,
      mipmapMode,
      null,
    );
  } finally {
    canvas.restore();
  }
}

function drawBoxShadow(CanvasKit, canvas, command) {
  const prepared = makePaint(CanvasKit, { kind: "solid", color: command.color });
  let filter = null;
  let path = null;
  try {
    const sigma = command.blurRadius * 0.5;
    if (sigma > 0) {
      filter = requireFunction(CanvasKit.MaskFilter, "MakeBlur")(
        requireEnum(CanvasKit.BlurStyle, "Normal"),
        sigma,
        true,
      );
      if (!filter || typeof filter.delete !== "function") {
        executionFailure(ErrorCode.INTERNAL, "CanvasKit rejected a validated shadow blur");
      }
      prepared.paint.setMaskFilter(filter);
    }

    if (!command.inset) {
      const expanded = {
        x: command.rect.x + command.offset.x - command.spreadRadius,
        y: command.rect.y + command.offset.y - command.spreadRadius,
        width: Math.max(0, command.rect.width + command.spreadRadius * 2),
        height: Math.max(0, command.rect.height + command.spreadRadius * 2),
      };
      const radius = Math.max(0, command.radius + command.spreadRadius);
      if (![expanded.x, expanded.y, expanded.width, expanded.height, radius].every(Number.isFinite)) {
        executionFailure(ErrorCode.INVALID_PACKET, "shadow derived geometry is invalid");
      }
      canvas.drawRRect(roundedRect(CanvasKit, expanded, radius), prepared.paint);
      return;
    }

    const hole = {
      x: command.rect.x + command.spreadRadius + command.offset.x,
      y: command.rect.y + command.spreadRadius + command.offset.y,
      width: Math.max(0, command.rect.width - command.spreadRadius * 2),
      height: Math.max(0, command.rect.height - command.spreadRadius * 2),
    };
    const holeRadius = Math.max(0, command.radius - command.spreadRadius);
    if (![hole.x, hole.y, hole.width, hole.height, holeRadius].every(Number.isFinite)) {
      executionFailure(ErrorCode.INVALID_PACKET, "inset shadow derived geometry is invalid");
    }
    const PathBuilder = CanvasKit.PathBuilder;
    if (typeof PathBuilder !== "function") {
      executionFailure(ErrorCode.INVALID_STATE, "CanvasKit PathBuilder constructor is unavailable");
    }
    const builder = new PathBuilder();
    try {
      builder.addRRect(roundedRect(CanvasKit, command.rect, command.radius));
      builder.addRRect(roundedRect(CanvasKit, hole, holeRadius));
      path = builder.detachAndDelete();
    } catch (error) {
      safeDelete(builder);
      throw error;
    }
    path.setFillType(requireEnum(CanvasKit.FillType, "EvenOdd"));
    canvas.save();
    try {
      canvas.clipRRect(
        roundedRect(CanvasKit, command.rect, command.radius),
        requireEnum(CanvasKit.ClipOp, "Intersect"),
        true,
      );
      canvas.drawPath(path, prepared.paint);
    } finally {
      canvas.restore();
    }
  } finally {
    safeDelete(path);
    disposePaint(prepared);
    safeDelete(filter);
  }
}

function drawBackdropBlur(CanvasKit, canvas, command) {
  if (command.sigma === 0 || command.bounds.width === 0 || command.bounds.height === 0) return;
  const filter = requireFunction(CanvasKit.ImageFilter, "MakeBlur")(
    command.sigma,
    command.sigma,
    requireEnum(CanvasKit.TileMode, "Clamp"),
    null,
  );
  if (!filter || typeof filter.delete !== "function") {
    executionFailure(ErrorCode.INTERNAL, "CanvasKit rejected a validated backdrop blur");
  }
  let saved = false;
  try {
    canvas.save();
    saved = true;
    canvas.clipRRect(
      roundedRect(CanvasKit, command.bounds, command.cornerRadius),
      requireEnum(CanvasKit.ClipOp, "Intersect"),
      true,
    );
    canvas.saveLayer(
      null,
      rect(CanvasKit, command.bounds),
      filter,
      0,
      requireEnum(CanvasKit.TileMode, "Clamp"),
    );
    canvas.restore();
  } finally {
    try {
      if (saved) canvas.restore();
    } finally {
      safeDelete(filter);
    }
  }
}

/** Execute already-decoded commands against a staging CanvasKit canvas. */
export function executeCommandStream({ CanvasKit, canvas, commands, resolveResource }) {
  if (typeof resolveResource !== "function") {
    throw new TypeError("resolveResource must be a function");
  }
  const resolved = preflightResources(commands, resolveResource);
  const initialSaveCount = canvas.getSaveCount();
  try {
    commands.forEach((command, index) => {
      switch (command.kind) {
        case "clear":
          canvas.clear(color(CanvasKit, command.color));
          break;
        case "save":
          canvas.save();
          break;
        case "restore":
          canvas.restore();
          break;
        case "opacity-layer": {
          const Paint = CanvasKit.Paint;
          if (typeof Paint !== "function") {
            executionFailure(ErrorCode.INVALID_STATE, "CanvasKit Paint constructor is unavailable");
          }
          const paint = new Paint();
          try {
            paint.setAlphaf(command.alpha);
            const bounds = rect(CanvasKit, command.bounds);
            canvas.saveLayer(paint, bounds);
            canvas.clipRect(bounds, requireEnum(CanvasKit.ClipOp, "Intersect"), false);
          } finally {
            safeDelete(paint);
          }
          break;
        }
        case "clip-rect":
          canvas.clipRect(
            rect(CanvasKit, command.rect),
            requireEnum(CanvasKit.ClipOp, "Intersect"),
            true,
          );
          break;
        case "clip-rounded-rect":
          canvas.clipRRect(
            roundedRect(CanvasKit, command.rect, command.radius),
            requireEnum(CanvasKit.ClipOp, "Intersect"),
            true,
          );
          break;
        case "concat-affine": {
          const [scaleX, skewX, translateX, skewY, scaleY, translateY] = command.affine;
          canvas.concat([
            scaleX,
            skewX,
            translateX,
            skewY,
            scaleY,
            translateY,
            0,
            0,
            1,
          ]);
          break;
        }
        case "fill-rect": {
          const prepared = makePaint(CanvasKit, command.paint);
          try {
            prepared.paint.setStyle(requireEnum(CanvasKit.PaintStyle, "Fill"));
            canvas.drawRRect(
              roundedRect(CanvasKit, command.rect, command.radius),
              prepared.paint,
            );
          } finally {
            disposePaint(prepared);
          }
          break;
        }
        case "stroke-rect": {
          if (command.stroke.width === 0) break;
          const prepared = makePaint(CanvasKit, command.stroke.paint);
          try {
            configureStroke(CanvasKit, prepared, command.stroke);
            canvas.drawRRect(
              roundedRect(CanvasKit, command.rect, command.radius),
              prepared.paint,
            );
          } finally {
            disposePaint(prepared);
          }
          break;
        }
        case "fill-path": {
          const path = makePath(CanvasKit, command.path);
          let prepared = null;
          try {
            prepared = makePaint(CanvasKit, command.paint);
            prepared.paint.setStyle(requireEnum(CanvasKit.PaintStyle, "Fill"));
            canvas.drawPath(path, prepared.paint);
          } finally {
            disposePaint(prepared);
            safeDelete(path);
          }
          break;
        }
        case "stroke-path": {
          if (command.stroke.width === 0) break;
          const path = makePath(CanvasKit, command.path);
          let prepared = null;
          try {
            prepared = makePaint(CanvasKit, command.stroke.paint);
            configureStroke(CanvasKit, prepared, command.stroke);
            canvas.drawPath(path, prepared.paint);
          } finally {
            disposePaint(prepared);
            safeDelete(path);
          }
          break;
        }
        case "box-shadow":
          drawBoxShadow(CanvasKit, canvas, command);
          break;
        case "draw-image": {
          const [filterMode, mipmapMode] = samplingOptions(CanvasKit, command.sampling);
          canvas.drawImageRectOptions(
            resolved.get(index),
            rect(CanvasKit, command.source),
            rect(CanvasKit, command.destination),
            filterMode,
            mipmapMode,
            null,
          );
          break;
        }
        case "draw-image-fit":
          drawImageFit(CanvasKit, canvas, resolved.get(index), command);
          break;
        case "backdrop-blur":
          drawBackdropBlur(CanvasKit, canvas, command);
          break;
        case "draw-paragraph":
        case "draw-svg":
        case "draw-picture":
          executionFailure(ErrorCode.INTERNAL, "unsupported resource command escaped preflight");
          break;
        default:
          executionFailure(ErrorCode.INTERNAL, `decoded command ${command.kind} is invalid`);
      }
    });
  } finally {
    canvas.restoreToCount(initialSaveCount);
  }
}
