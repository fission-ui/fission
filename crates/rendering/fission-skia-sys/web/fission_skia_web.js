// Fission CanvasKit bridge protocol decoder. This module deliberately contains
// no CanvasKit calls; it is the bounded binary boundary used by the later host.

export const MAGIC = Object.freeze([0x46, 0x53, 0x4b, 0x4e]); // FSKN
export const PROTOCOL_VERSION = 1;
export const HEADER_LEN = 32;

export const PacketKind = Object.freeze({
  INIT: 1,
  RESIZE: 2,
  RESOURCE_BATCH: 3,
  FRAME: 4,
  DESTROY: 5,
  ACK: 6,
  ERROR: 7,
});

export const BackendPreference = Object.freeze({
  AUTO: 0,
  WEB_GL: 1,
  GRAPHITE: 2,
  SOFTWARE: 3,
});

export const ResourceKind = Object.freeze({
  IMAGE: 1,
  SVG: 2,
  FONT: 3,
  TEXT: 4,
  BINARY: 5,
});

export const ResourceOperation = Object.freeze({
  UPSERT: 1,
  RELEASE: 2,
});

export const ErrorCode = Object.freeze({
  INVALID_PACKET: 1,
  UNSUPPORTED_VERSION: 2,
  INVALID_STATE: 3,
  RESOURCE_FAILURE: 4,
  SURFACE_LOST: 5,
  INTERNAL: 6,
});

export const DEFAULT_LIMITS = Object.freeze({
  maxPacketBytes: 64 * 1024 * 1024,
  maxResourceUpdates: 4096,
  maxResourceBytes: 32 * 1024 * 1024,
  maxResourceSlots: 65536,
  maxFrameCommands: 32 * 1024 * 1024,
  maxDamageRects: 1024,
  maxErrorMessageBytes: 4 * 1024,
  maxSurfaceDimension: 32768,
  maxScaleFactor: 16,
});

const RESOURCE_KIND = new Set(Object.values(ResourceKind));
const BACKEND_PREFERENCE = new Set(Object.values(BackendPreference));
const COLOR_SPACE = new Set([1, 2]);
const ALPHA_MODE = new Set([1, 2]);
const DESTROY_REASON = new Set([0, 1, 2, 3]);
const ERROR_CODE = new Set(Object.values(ErrorCode));
const utf8Decoder = new TextDecoder("utf-8", { fatal: true });
const utf8Encoder = new TextEncoder();

export class ProtocolError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "ProtocolError";
    this.code = code;
  }
}

function reject(code, message) {
  throw new ProtocolError(code, message);
}

function toBytes(input) {
  if (input instanceof Uint8Array) {
    return input;
  }
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  if (input instanceof ArrayBuffer) {
    return new Uint8Array(input);
  }
  reject("invalid-buffer", "packet must be an ArrayBuffer or typed-array view");
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
    if (!Number.isSafeInteger(length) || length < 0 || this.remaining() < length) {
      reject(
        "truncated",
        `packet needs ${length} bytes but only ${this.remaining()} remain`,
      );
    }
  }

  exact(length) {
    if (this.remaining() !== length) {
      reject(
        "length-mismatch",
        `payload declares ${length} remaining bytes but has ${this.remaining()}`,
      );
    }
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

  take(length) {
    this.require(length);
    const value = this.bytes.subarray(this.position, this.position + length);
    this.position += length;
    return value;
  }

  finish() {
    if (this.remaining() !== 0) {
      reject("trailing-bytes", `packet has ${this.remaining()} trailing bytes`);
    }
  }
}

function checkedProduct(left, right, field) {
  const value = left * right;
  if (!Number.isSafeInteger(value)) {
    reject("invalid-length", `${field} byte length overflowed`);
  }
  return value;
}

function checkedSum(left, right, field) {
  const value = left + right;
  if (!Number.isSafeInteger(value)) {
    reject("invalid-length", `${field} byte length overflowed`);
  }
  return value;
}

function requireLimit(field, actual, maximum) {
  if (actual > maximum) {
    reject("limit-exceeded", `${field} ${actual} exceeds ${maximum}`);
  }
}

function requireZero(field, value) {
  if (value !== 0) {
    reject("nonzero-reserved", `${field} reserved bits must be zero`);
  }
}

function requireEnum(field, value, choices) {
  if (!choices.has(value)) {
    reject("invalid-enum", `${field} has unknown value ${value}`);
  }
  return value;
}

function decodeSurface(reader, limits) {
  const surface = {
    width: reader.u32(),
    height: reader.u32(),
    scaleFactor: reader.f32(),
  };
  requireLimit(
    "surface dimension",
    Math.max(surface.width, surface.height),
    limits.maxSurfaceDimension,
  );
  if (
    !Number.isFinite(surface.scaleFactor) ||
    surface.scaleFactor <= 0 ||
    surface.scaleFactor > limits.maxScaleFactor
  ) {
    reject("invalid-value", "surface scale factor is invalid");
  }
  return surface;
}

function decodeInit(reader, limits) {
  reader.exact(16);
  const surface = decodeSurface(reader, limits);
  const backend = requireEnum(
    "backend preference",
    reader.u8(),
    BACKEND_PREFERENCE,
  );
  const colorSpace = requireEnum("color space", reader.u8(), COLOR_SPACE);
  const alphaMode = requireEnum("alpha mode", reader.u8(), ALPHA_MODE);
  requireZero("init", reader.u8());
  return { type: "init", surface, backend, colorSpace, alphaMode };
}

function decodeResize(reader, limits) {
  reader.exact(12);
  return { type: "resize", surface: decodeSurface(reader, limits) };
}

function decodeResourceBatch(reader, limits) {
  reader.require(16);
  const resourceEpoch = reader.u64();
  if (resourceEpoch === 0n) {
    reject("invalid-value", "resource epoch must be non-zero");
  }
  const updateCount = reader.u32();
  requireLimit(
    "resource updates",
    updateCount,
    limits.maxResourceUpdates,
  );
  requireZero("resource batch", reader.u32());
  reader.require(checkedProduct(updateCount, 24, "resource update"));

  const updates = [];
  for (let index = 0; index < updateCount; index += 1) {
    const handle = { slot: reader.u32(), generation: reader.u32() };
    if (handle.slot === 0 || handle.generation === 0) {
      reject("invalid-value", "resource slot and generation must be non-zero");
    }
    const operation = reader.u8();
    if (operation !== 1 && operation !== 2) {
      reject("invalid-enum", `resource operation has unknown value ${operation}`);
    }
    const kind = requireEnum("resource kind", reader.u8(), RESOURCE_KIND);
    requireZero("resource update", reader.u16());
    const contentId = reader.u64();
    const byteLength = reader.u32();
    requireLimit("resource bytes", byteLength, limits.maxResourceBytes);
    const bytes = reader.take(byteLength);
    if (operation === 1 && contentId === 0n) {
      reject("invalid-value", "upsert content identity must be non-zero");
    }
    if (operation === 2 && (contentId !== 0n || byteLength !== 0)) {
      reject("invalid-value", "released resources cannot carry content");
    }
    updates.push({ handle, operation, kind, contentId, bytes });
  }
  return { type: "resource-batch", resourceEpoch, updates };
}

function decodeFrame(reader, limits) {
  reader.require(64);
  const frameId = reader.u64();
  if (frameId === 0n) {
    reject("invalid-value", "frame identity must be non-zero");
  }
  const resourceEpoch = reader.u64();
  const semanticsEpoch = reader.u64();
  const surface = decodeSurface(reader, limits);
  const damageCount = reader.u32();
  requireLimit("damage rectangles", damageCount, limits.maxDamageRects);
  const commandLength = reader.u32();
  requireLimit("frame commands", commandLength, limits.maxFrameCommands);
  const clearColor = [reader.f32(), reader.f32(), reader.f32(), reader.f32()];
  if (
    clearColor.some(
      (component) =>
        !Number.isFinite(component) || component < 0 || component > 1,
    )
  ) {
    reject("invalid-value", "clear color components must be finite and normalized");
  }
  requireZero("frame", reader.u32());

  const damageBytes = checkedProduct(damageCount, 16, "damage rectangle");
  reader.exact(checkedSum(damageBytes, commandLength, "frame payload"));
  const damage = [];
  for (let index = 0; index < damageCount; index += 1) {
    const rectangle = {
      x: reader.u32(),
      y: reader.u32(),
      width: reader.u32(),
      height: reader.u32(),
    };
    const right = rectangle.x + rectangle.width;
    const bottom = rectangle.y + rectangle.height;
    if (
      right > 0xffffffff ||
      bottom > 0xffffffff ||
      right > surface.width ||
      bottom > surface.height
    ) {
      reject("invalid-value", "damage rectangle is outside the frame surface");
    }
    damage.push(rectangle);
  }
  const commands = reader.take(commandLength);
  return {
    type: "frame",
    frameId,
    resourceEpoch,
    semanticsEpoch,
    surface,
    clearColor,
    damage,
    commands,
  };
}

function decodeDestroy(reader) {
  reader.exact(4);
  const reason = requireEnum("destroy reason", reader.u16(), DESTROY_REASON);
  requireZero("destroy", reader.u16());
  return { type: "destroy", reason };
}

function decodeAck(reader) {
  reader.exact(8);
  const acknowledgedSequence = reader.u64();
  if (acknowledgedSequence === 0n) {
    reject("invalid-value", "acknowledged sequence must be non-zero");
  }
  return { type: "ack", acknowledgedSequence };
}

function decodeError(reader, limits) {
  reader.require(16);
  const failedSequence = reader.u64();
  if (failedSequence === 0n) {
    reject("invalid-value", "failed sequence must be non-zero");
  }
  const code = requireEnum("error code", reader.u16(), ERROR_CODE);
  requireZero("error", reader.u16());
  const messageLength = reader.u32();
  requireLimit(
    "error message",
    messageLength,
    limits.maxErrorMessageBytes,
  );
  reader.exact(messageLength);
  let message;
  try {
    message = utf8Decoder.decode(reader.take(messageLength));
  } catch (_error) {
    reject("invalid-utf8", "error message is not valid UTF-8");
  }
  return { type: "error", failedSequence, code, message };
}

/**
 * Decode exactly one message. Byte payloads are borrowed views of `input` and
 * must be consumed synchronously or copied by the caller before that storage is
 * reused.
 */
export function decodeMessage(input, limits = DEFAULT_LIMITS) {
  const bytes = toBytes(input);
  requireLimit("packet bytes", bytes.byteLength, limits.maxPacketBytes);
  if (bytes.byteLength < HEADER_LEN) {
    reject("truncated", `packet is shorter than the ${HEADER_LEN}-byte header`);
  }
  const reader = new Reader(bytes);
  for (const expected of MAGIC) {
    if (reader.u8() !== expected) {
      reject("invalid-magic", "packet does not begin with FSKN");
    }
  }
  const version = reader.u16();
  if (version !== PROTOCOL_VERSION) {
    reject("unsupported-version", `unsupported protocol version ${version}`);
  }
  const kind = reader.u16();
  if (!Object.values(PacketKind).includes(kind)) {
    reject("unknown-packet-kind", `unknown packet kind ${kind}`);
  }
  const declaredLength = reader.u32();
  if (declaredLength !== bytes.byteLength) {
    reject(
      "length-mismatch",
      `packet declares ${declaredLength} bytes but received ${bytes.byteLength}`,
    );
  }
  requireZero("envelope flags", reader.u32());
  const session = reader.u64();
  if (session === 0n) {
    reject("invalid-session", "session identity must be non-zero");
  }
  const sequence = reader.u64();
  if (sequence === 0n) {
    reject("invalid-sequence", "sequence must be non-zero");
  }

  let packet;
  switch (kind) {
    case PacketKind.INIT:
      packet = decodeInit(reader, limits);
      break;
    case PacketKind.RESIZE:
      packet = decodeResize(reader, limits);
      break;
    case PacketKind.RESOURCE_BATCH:
      packet = decodeResourceBatch(reader, limits);
      break;
    case PacketKind.FRAME:
      packet = decodeFrame(reader, limits);
      break;
    case PacketKind.DESTROY:
      packet = decodeDestroy(reader);
      break;
    case PacketKind.ACK:
      packet = decodeAck(reader);
      break;
    case PacketKind.ERROR:
      packet = decodeError(reader, limits);
      break;
    default:
      reject("unknown-packet-kind", `unknown packet kind ${kind}`);
  }
  reader.finish();
  return Object.freeze({
    envelope: Object.freeze({ version, kind, session, sequence }),
    packet,
  });
}

function requireWireU64(field, value) {
  if (
    typeof value !== "bigint" ||
    value <= 0n ||
    value > 0xffffffffffffffffn
  ) {
    reject("invalid-value", `${field} must be a non-zero u64 BigInt`);
  }
  return value;
}

function encodeEnvelope(kind, payloadLength, session, sequence) {
  requireEnum("packet kind", kind, new Set(Object.values(PacketKind)));
  requireWireU64("session", session);
  requireWireU64("sequence", sequence);
  const packetLength = HEADER_LEN + payloadLength;
  requireLimit("packet bytes", packetLength, DEFAULT_LIMITS.maxPacketBytes);
  const bytes = new Uint8Array(packetLength);
  bytes.set(MAGIC, 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(4, PROTOCOL_VERSION, true);
  view.setUint16(6, kind, true);
  view.setUint32(8, packetLength, true);
  view.setUint32(12, 0, true);
  view.setBigUint64(16, session, true);
  view.setBigUint64(24, sequence, true);
  return { bytes, view };
}

function encodeBoundedUtf8(message, maximum) {
  const value = typeof message === "string" ? message : String(message);
  const destination = new Uint8Array(maximum);
  const { written } = utf8Encoder.encodeInto(value, destination);
  return destination.subarray(0, written);
}

/** Encode one canonical acknowledgement packet. */
export function encodeAck({ session, sequence, acknowledgedSequence }) {
  requireWireU64("acknowledged sequence", acknowledgedSequence);
  const { bytes, view } = encodeEnvelope(
    PacketKind.ACK,
    8,
    session,
    sequence,
  );
  view.setBigUint64(HEADER_LEN, acknowledgedSequence, true);
  return bytes;
}

/**
 * Encode one canonical bounded error packet. Messages longer than the protocol
 * limit are truncated at a complete UTF-8 scalar boundary by `encodeInto`.
 */
export function encodeError({
  session,
  sequence,
  failedSequence,
  code,
  message,
  limits = DEFAULT_LIMITS,
}) {
  requireWireU64("failed sequence", failedSequence);
  requireEnum("error code", code, ERROR_CODE);
  const messageBytes = encodeBoundedUtf8(
    message,
    limits.maxErrorMessageBytes,
  );
  const { bytes, view } = encodeEnvelope(
    PacketKind.ERROR,
    16 + messageBytes.byteLength,
    session,
    sequence,
  );
  view.setBigUint64(HEADER_LEN, failedSequence, true);
  view.setUint16(HEADER_LEN + 8, code, true);
  view.setUint16(HEADER_LEN + 10, 0, true);
  view.setUint32(HEADER_LEN + 12, messageBytes.byteLength, true);
  bytes.set(messageBytes, HEADER_LEN + 16);
  return bytes;
}

function cloneResources(resources) {
  return new Map(
    Array.from(resources, ([slot, state]) => [slot, { ...state }]),
  );
}

function applyResourceUpdate(resources, update) {
  const previous = resources.get(update.handle.slot);
  if (update.operation === 1) {
    if (previous && update.handle.generation < previous.generation) {
      reject("stale-resource", "resource generation is older than the slot");
    }
    if (
      previous &&
      update.handle.generation === previous.generation &&
      !previous.live
    ) {
      reject("released-resource", "released generation cannot be reused");
    }
    if (
      previous &&
      update.handle.generation > previous.generation &&
      previous.live
    ) {
      reject("occupied-resource", "live resource must be released before slot reuse");
    }
    resources.set(update.handle.slot, {
      generation: update.handle.generation,
      live: true,
    });
    return;
  }

  if (!previous || update.handle.generation > previous.generation) {
    reject("missing-resource", "resource release does not identify a live slot");
  }
  if (update.handle.generation < previous.generation) {
    reject("stale-resource", "resource generation is older than the slot");
  }
  if (!previous.live) {
    reject("released-resource", "resource generation is already released");
  }
  resources.set(update.handle.slot, {
    generation: previous.generation,
    live: false,
  });
}

/** Stateful gate for the ordered Rust-to-JavaScript command stream. */
export class ProtocolSession {
  constructor(limits = DEFAULT_LIMITS) {
    this.limits = limits;
    this.latestSession = 0n;
    this.active = null;
  }

  activeSession() {
    return this.active?.id ?? null;
  }

  liveResourceCount() {
    if (!this.active) return 0;
    let count = 0;
    for (const state of this.active.resources.values()) {
      if (state.live) count += 1;
    }
    return count;
  }

  /** Create an isolated transactional candidate for a prospective command. */
  fork() {
    const candidate = new ProtocolSession(this.limits);
    candidate.latestSession = this.latestSession;
    candidate.active = this.active
      ? {
          ...this.active,
          resources: cloneResources(this.active.resources),
        }
      : null;
    return candidate;
  }

  accept(message) {
    const { envelope, packet } = message;
    if (packet.type === "init") {
      if (this.active) {
        reject("session-active", "cannot initialize over an active session");
      }
      if (envelope.session <= this.latestSession) {
        reject("stale-session", "session identity is not newer than the retired session");
      }
      if (envelope.sequence !== 1n) {
        reject("unexpected-sequence", "init sequence must be one");
      }
      this.latestSession = envelope.session;
      this.active = {
        id: envelope.session,
        lastSequence: 1n,
        resourceEpoch: 0n,
        lastFrameId: 0n,
        semanticsEpoch: 0n,
        resources: new Map(),
      };
      return;
    }

    if (!this.active) {
      reject("no-active-session", "non-init packet has no active session");
    }
    if (envelope.session !== this.active.id) {
      if (envelope.session <= this.latestSession) {
        reject("stale-session", "packet belongs to a retired session");
      }
      reject("unexpected-session", "packet does not belong to the active session");
    }
    const expectedSequence = this.active.lastSequence + 1n;
    if (envelope.sequence < expectedSequence) {
      reject("stale-sequence", "packet sequence has already been consumed");
    }
    if (envelope.sequence > expectedSequence) {
      reject("unexpected-sequence", "packet sequence is not contiguous");
    }

    if (packet.type === "destroy") {
      this.active = null;
      return;
    }

    const next = {
      ...this.active,
      resources: cloneResources(this.active.resources),
    };
    if (packet.type === "resource-batch") {
      if (packet.resourceEpoch <= next.resourceEpoch) {
        reject("stale-resource-epoch", "resource epoch did not advance");
      }
      for (const update of packet.updates) {
        applyResourceUpdate(next.resources, update);
        requireLimit(
          "resource slots",
          next.resources.size,
          this.limits.maxResourceSlots,
        );
      }
      next.resourceEpoch = packet.resourceEpoch;
    } else if (packet.type === "frame") {
      if (packet.frameId <= next.lastFrameId) {
        reject("stale-frame", "frame identity did not advance");
      }
      if (packet.resourceEpoch !== next.resourceEpoch) {
        reject("resource-epoch-mismatch", "frame references an unapplied resource epoch");
      }
      if (packet.semanticsEpoch < next.semanticsEpoch) {
        reject("stale-semantics-epoch", "semantics epoch moved backwards");
      }
      next.lastFrameId = packet.frameId;
      next.semanticsEpoch = packet.semanticsEpoch;
    }
    next.lastSequence = envelope.sequence;
    this.active = next;
  }
}
