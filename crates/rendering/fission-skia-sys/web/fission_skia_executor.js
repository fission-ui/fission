import {
  BackendPreference,
  ErrorCode,
  PacketKind,
  ProtocolError,
  ProtocolSession,
  ResourceKind,
  ResourceOperation,
  decodeMessage,
  encodeAck,
  encodeError,
} from "./fission_skia_web.js";
import {
  decodeCommandStream,
  executeCommandStream,
} from "./fission_skia_commands.js";
import { createCanvasKitParagraphHost } from "./fission_skia_paragraph.js";

const MAX_U64 = 0xffffffffffffffffn;

class ExecutorFault extends Error {
  constructor(code, message, { softwareFallbackAllowed = false } = {}) {
    super(message);
    this.name = "ExecutorFault";
    this.protocolCode = code;
    this.softwareFallbackAllowed = softwareFallbackAllowed;
  }
}

function fault(code, message, options) {
  throw new ExecutorFault(code, message, options);
}

function copySurface(surface) {
  return {
    width: surface.width,
    height: surface.height,
    scaleFactor: surface.scaleFactor,
  };
}

function sameSurface(left, right) {
  return (
    left.width === right.width &&
    left.height === right.height &&
    left.scaleFactor === right.scaleFactor
  );
}

function normalizeBackendPreference(value) {
  if (value === undefined || value === null) return null;
  if (typeof value === "number" && Object.values(BackendPreference).includes(value)) {
    return value;
  }
  switch (value) {
    case "auto":
      return BackendPreference.AUTO;
    case "webgl":
    case "web-gl":
      return BackendPreference.WEB_GL;
    case "graphite":
      return BackendPreference.GRAPHITE;
    case "software":
      return BackendPreference.SOFTWARE;
    default:
      throw new TypeError(`unknown CanvasKit backend preference: ${value}`);
  }
}

function requireFactoryInputs(CanvasKit, canvas, eventSink) {
  if (!CanvasKit || typeof CanvasKit !== "object") {
    throw new TypeError("CanvasKit must be an initialized CanvasKit module");
  }
  if (!canvas || typeof canvas !== "object") {
    throw new TypeError("canvas must be an HTMLCanvasElement or OffscreenCanvas");
  }
  if (eventSink !== undefined && eventSink !== null && typeof eventSink !== "function") {
    throw new TypeError("eventSink must be a function when provided");
  }
}

function colorSpaceFor(CanvasKit, value) {
  const colorSpace = value === 2 ? CanvasKit.ColorSpace?.DISPLAY_P3 : CanvasKit.ColorSpace?.SRGB;
  if (colorSpace === undefined || colorSpace === null) {
    fault(ErrorCode.INVALID_STATE, "CanvasKit does not expose the requested color space");
  }
  return colorSpace;
}

function safeDelete(value) {
  if (!value || typeof value.delete !== "function") return;
  try {
    if (typeof value.isDeleted !== "function" || !value.isDeleted()) value.delete();
  } catch (_error) {
    // Teardown must continue so one failed Embind destructor cannot leak the rest.
  }
}

function safeDisposeSurface(value) {
  if (!value) return;
  try {
    if (typeof value.isDeleted === "function" && value.isDeleted()) return;
    if (typeof value.dispose === "function") {
      value.dispose();
    } else if (typeof value.delete === "function") {
      value.delete();
    }
  } catch (_error) {
    // Surface teardown is best-effort so the remaining owned objects are freed.
  }
}

function requireOwnedObject(value, label, code = ErrorCode.RESOURCE_FAILURE) {
  if (
    !value ||
    typeof value.delete !== "function" ||
    (typeof value.isDeleted === "function" && value.isDeleted())
  ) {
    fault(code, `CanvasKit failed to create ${label}`);
  }
  return value;
}

function setCanvasSize(canvas, surface) {
  canvas.width = surface.width;
  canvas.height = surface.height;
}

function isZeroSurface(surface) {
  return surface.width === 0 || surface.height === 0;
}

function makeSoftwareSurface(CanvasKit, canvas, configuration) {
  if (configuration.colorSpace !== 1) {
    fault(
      ErrorCode.INVALID_STATE,
      "CanvasKit software surfaces cannot honor a non-sRGB destination color space",
    );
  }
  if (typeof CanvasKit.MakeSWCanvasSurface !== "function") {
    fault(ErrorCode.INVALID_STATE, "CanvasKit software surface support is unavailable");
  }
  setCanvasSize(canvas, configuration.surface);
  if (typeof canvas.getContext === "function") {
    let context;
    try {
      context = canvas.getContext("2d", {
        alpha: configuration.alphaMode !== 1,
      });
    } catch (error) {
      fault(
        ErrorCode.SURFACE_LOST,
        `CanvasKit could not acquire a software canvas context: ${errorMessage(error)}`,
      );
    }
    if (!context) {
      fault(ErrorCode.SURFACE_LOST, "CanvasKit could not acquire a software canvas context");
    }
  }
  if (isZeroSurface(configuration.surface)) {
    return {
      backend: "software",
      surface: null,
      contextHandle: 0,
      grContext: null,
    };
  }
  let surface;
  try {
    surface = CanvasKit.MakeSWCanvasSurface(canvas);
  } catch (error) {
    fault(
      ErrorCode.SURFACE_LOST,
      `CanvasKit could not create a software canvas surface: ${errorMessage(error)}`,
    );
  }
  requireOwnedObject(surface, "software canvas surface", ErrorCode.SURFACE_LOST);
  return {
    backend: "software",
    surface,
    contextHandle: 0,
    grContext: null,
  };
}

function makeWebGlSurface(CanvasKit, canvas, configuration) {
  setCanvasSize(canvas, configuration.surface);
  if (CanvasKit.gpu === false) {
    fault(ErrorCode.SURFACE_LOST, "CanvasKit was built without GPU support", {
      softwareFallbackAllowed: true,
    });
  }
  if (
    typeof CanvasKit.GetWebGLContext !== "function" ||
    typeof CanvasKit.MakeOnScreenGLSurface !== "function" ||
    typeof CanvasKit.deleteContext !== "function" ||
    (typeof CanvasKit.MakeWebGLContext !== "function" &&
      typeof CanvasKit.MakeGrContext !== "function")
  ) {
    fault(ErrorCode.INVALID_STATE, "CanvasKit WebGL Ganesh APIs are unavailable", {
      softwareFallbackAllowed: true,
    });
  }
  if (isZeroSurface(configuration.surface)) {
    return {
      backend: "webgl",
      surface: null,
      contextHandle: 0,
      grContext: null,
    };
  }

  let contextHandle;
  try {
    contextHandle = CanvasKit.GetWebGLContext(canvas, {
      alpha: configuration.alphaMode === 2 ? 1 : 0,
      antialias: 1,
      depth: 0,
      stencil: 8,
      enableExtensionsByDefault: 1,
    });
  } catch (error) {
    fault(
      ErrorCode.SURFACE_LOST,
      `CanvasKit could not acquire a WebGL context: ${errorMessage(error)}`,
      { softwareFallbackAllowed: true },
    );
  }
  if (!Number.isSafeInteger(contextHandle) || contextHandle <= 0) {
    fault(ErrorCode.SURFACE_LOST, "CanvasKit could not acquire a WebGL context", {
      softwareFallbackAllowed: true,
    });
  }

  let grContext = null;
  let surface = null;
  try {
    const makeContext = CanvasKit.MakeWebGLContext ?? CanvasKit.MakeGrContext;
    try {
      grContext = makeContext.call(CanvasKit, contextHandle);
    } catch (error) {
      fault(
        ErrorCode.SURFACE_LOST,
        `CanvasKit could not create a Ganesh context: ${errorMessage(error)}`,
      );
    }
    if (
      !grContext ||
      typeof grContext.delete !== "function" ||
      typeof grContext.releaseResourcesAndAbandonContext !== "function"
    ) {
      fault(ErrorCode.SURFACE_LOST, "CanvasKit could not create an owned Ganesh context");
    }
    try {
      surface = CanvasKit.MakeOnScreenGLSurface(
        grContext,
        configuration.surface.width,
        configuration.surface.height,
        colorSpaceFor(CanvasKit, configuration.colorSpace),
      );
    } catch (error) {
      if (error instanceof ExecutorFault) throw error;
      fault(
        ErrorCode.SURFACE_LOST,
        `CanvasKit could not create an on-screen WebGL surface: ${errorMessage(error)}`,
      );
    }
    requireOwnedObject(surface, "on-screen WebGL surface", ErrorCode.SURFACE_LOST);
    return { backend: "webgl", surface, contextHandle, grContext };
  } catch (error) {
    safeDisposeSurface(surface);
    try {
      grContext?.releaseResourcesAndAbandonContext?.();
    } catch (_abandonError) {
      // Continue deterministic cleanup below.
    }
    safeDelete(grContext);
    try {
      CanvasKit.deleteContext?.(contextHandle);
    } catch (_deleteError) {
      // The original creation error is more useful to the caller.
    }
    throw error;
  }
}

function makeInitialSurface(CanvasKit, canvas, configuration) {
  switch (configuration.backendPreference) {
    case BackendPreference.GRAPHITE:
      fault(
        ErrorCode.INVALID_STATE,
        "Graphite/Dawn is not implemented by the CanvasKit executor milestone",
      );
      break;
    case BackendPreference.SOFTWARE:
      return makeSoftwareSurface(CanvasKit, canvas, configuration);
    case BackendPreference.WEB_GL:
      return makeWebGlSurface(CanvasKit, canvas, configuration);
    case BackendPreference.AUTO:
      try {
        return makeWebGlSurface(CanvasKit, canvas, configuration);
      } catch (error) {
        if (!(error instanceof ExecutorFault) || !error.softwareFallbackAllowed) throw error;
        return makeSoftwareSurface(CanvasKit, canvas, configuration);
      }
    default:
      fault(ErrorCode.INVALID_STATE, "invalid CanvasKit backend preference");
  }
}

function deleteSurfaceState(CanvasKit, state, abandonContext) {
  if (!state) return;
  safeDisposeSurface(state.surface);
  if (state.grContext) {
    if (abandonContext) {
      try {
        state.grContext.releaseResourcesAndAbandonContext?.();
      } catch (_error) {
        // Context loss can make release fail; deletion still has to continue.
      }
    }
    safeDelete(state.grContext);
  }
  if (state.contextHandle) {
    try {
      CanvasKit.deleteContext?.(state.contextHandle);
    } catch (_error) {
      // Continue teardown; Emscripten may already have retired a lost context.
    }
  }
}

function decodeResourceObject(CanvasKit, kind, ownedBytes) {
  switch (kind) {
    case ResourceKind.IMAGE:
      if (typeof CanvasKit.MakeImageFromEncoded !== "function") {
        fault(ErrorCode.INVALID_STATE, "CanvasKit image codecs are unavailable");
      }
      try {
        return requireOwnedObject(
          CanvasKit.MakeImageFromEncoded(ownedBytes),
          "image resource",
        );
      } catch (error) {
        if (error instanceof ExecutorFault) throw error;
        fault(
          ErrorCode.RESOURCE_FAILURE,
          `CanvasKit could not decode the image resource: ${errorMessage(error)}`,
        );
      }
    case ResourceKind.SVG:
      fault(
        ErrorCode.RESOURCE_FAILURE,
        "raw SVG resources are unsupported; SVG must use Fission's neutral lowering path",
      );
      break;
    case ResourceKind.FONT: {
      const makeTypeface = CanvasKit.Typeface?.MakeTypefaceFromData;
      if (typeof makeTypeface !== "function") {
        fault(ErrorCode.INVALID_STATE, "CanvasKit typeface decoding is unavailable");
      }
      const buffer = ownedBytes.buffer.slice(
        ownedBytes.byteOffset,
        ownedBytes.byteOffset + ownedBytes.byteLength,
      );
      try {
        const typeface = requireOwnedObject(
          makeTypeface.call(CanvasKit.Typeface, buffer),
          "font resource",
        );
        // Paragraph providers decode from the resource's owned bytes. Keep no
        // second, otherwise-unused Typeface alive in the general resource map.
        safeDelete(typeface);
        return null;
      } catch (error) {
        if (error instanceof ExecutorFault) throw error;
        fault(
          ErrorCode.RESOURCE_FAILURE,
          `CanvasKit could not decode the font resource: ${errorMessage(error)}`,
        );
      }
    }
    case ResourceKind.TEXT:
    case ResourceKind.BINARY:
      return null;
    default:
      fault(ErrorCode.INVALID_PACKET, `unknown resource kind ${kind}`);
  }
}

function createResourceEntry(CanvasKit, update) {
  // This copy severs the only view into the Rust-Wasm module before submit returns.
  const bytes = Uint8Array.from(update.bytes);
  const object = decodeResourceObject(CanvasKit, update.kind, bytes);
  return {
    generation: update.handle.generation,
    kind: update.kind,
    contentId: update.contentId,
    bytes,
    object,
  };
}

function deleteResourceEntry(entry) {
  safeDelete(entry?.object);
}

function applyResourceBatch(CanvasKit, currentResources, batch) {
  const candidate = new Map(currentResources);
  const created = new Set();
  const deleteOnCommit = new Set();
  try {
    for (const update of batch.updates) {
      const previous = candidate.get(update.handle.slot);
      if (update.operation === ResourceOperation.RELEASE) {
        if (!previous || previous.generation !== update.handle.generation) {
          fault(ErrorCode.INVALID_STATE, "resource table diverged from protocol state");
        }
        if (previous.object) deleteOnCommit.add(previous.object);
        candidate.delete(update.handle.slot);
        continue;
      }

      const next = createResourceEntry(CanvasKit, update);
      if (next.object) created.add(next.object);
      if (previous?.object) deleteOnCommit.add(previous.object);
      candidate.set(update.handle.slot, next);
    }
  } catch (error) {
    for (const object of created) safeDelete(object);
    throw error;
  }

  for (const object of deleteOnCommit) safeDelete(object);
  return candidate;
}

function rebuildResources(CanvasKit, resources) {
  const candidate = new Map();
  const created = new Set();
  try {
    for (const [slot, entry] of resources) {
      const object = decodeResourceObject(CanvasKit, entry.kind, entry.bytes);
      if (object) created.add(object);
      candidate.set(slot, { ...entry, object });
    }
  } catch (error) {
    for (const object of created) safeDelete(object);
    throw error;
  }
  return candidate;
}

function materializeImageResources(CanvasKit, resources) {
  const candidate = new Map(resources);
  const created = new Set();
  try {
    for (const [slot, entry] of resources) {
      if (entry.kind !== ResourceKind.IMAGE || entry.object) continue;
      const object = decodeResourceObject(CanvasKit, entry.kind, entry.bytes);
      if (object) created.add(object);
      candidate.set(slot, { ...entry, object });
    }
  } catch (error) {
    for (const object of created) safeDelete(object);
    throw error;
  }
  return candidate;
}

function stripResourceObjects(resources) {
  const stripped = new Map();
  for (const [slot, entry] of resources) {
    deleteResourceEntry(entry);
    stripped.set(slot, { ...entry, object: null });
  }
  return stripped;
}

function recreateSurface(CanvasKit, canvas, state, configuration) {
  setCanvasSize(canvas, configuration.surface);
  if (isZeroSurface(configuration.surface)) {
    safeDisposeSurface(state?.surface);
    return { ...state, surface: null };
  }

  if (state?.backend === "webgl" && state.grContext && state.contextHandle) {
    let surface;
    try {
      surface = CanvasKit.MakeOnScreenGLSurface(
        state.grContext,
        configuration.surface.width,
        configuration.surface.height,
        colorSpaceFor(CanvasKit, configuration.colorSpace),
      );
    } catch (error) {
      if (error instanceof ExecutorFault) throw error;
      fault(
        ErrorCode.SURFACE_LOST,
        `CanvasKit could not resize the WebGL surface: ${errorMessage(error)}`,
      );
    }
    requireOwnedObject(surface, "resized WebGL surface", ErrorCode.SURFACE_LOST);
    safeDisposeSurface(state.surface);
    return { ...state, surface };
  }

  if (state?.backend === "software") {
    let surface;
    try {
      surface = CanvasKit.MakeSWCanvasSurface(canvas);
    } catch (error) {
      fault(
        ErrorCode.SURFACE_LOST,
        `CanvasKit could not resize the software surface: ${errorMessage(error)}`,
      );
    }
    requireOwnedObject(surface, "resized software surface", ErrorCode.SURFACE_LOST);
    safeDisposeSurface(state.surface);
    return { ...state, surface };
  }

  return state?.backend === "webgl"
    ? makeWebGlSurface(CanvasKit, canvas, configuration)
    : makeSoftwareSurface(CanvasKit, canvas, configuration);
}

function recoverIdentity(input) {
  try {
    let bytes;
    if (input instanceof Uint8Array) bytes = input;
    else if (ArrayBuffer.isView(input)) {
      bytes = new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    } else if (input instanceof ArrayBuffer) bytes = new Uint8Array(input);
    else return null;
    if (bytes.byteLength < 32) return null;
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const session = view.getBigUint64(16, true);
    const sequence = view.getBigUint64(24, true);
    return {
      session: session === 0n ? null : session,
      sequence: sequence === 0n ? null : sequence,
    };
  } catch (_error) {
    return null;
  }
}

const PROTOCOL_STATE_ERRORS = new Set([
  "session-active",
  "stale-session",
  "unexpected-session",
  "no-active-session",
  "stale-sequence",
  "unexpected-sequence",
  "stale-resource-epoch",
  "resource-epoch-mismatch",
  "stale-resource",
  "released-resource",
  "occupied-resource",
  "missing-resource",
  "stale-frame",
  "stale-semantics-epoch",
]);

function errorCodeFor(error) {
  if (Number.isInteger(error?.protocolCode)) return error.protocolCode;
  if (error instanceof ExecutorFault) return error.protocolCode;
  if (error instanceof ProtocolError) {
    if (error.code === "unsupported-version") return ErrorCode.UNSUPPORTED_VERSION;
    if (PROTOCOL_STATE_ERRORS.has(error.code)) {
      return ErrorCode.INVALID_STATE;
    }
    return ErrorCode.INVALID_PACKET;
  }
  return ErrorCode.INTERNAL;
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function requireReadbackCoordinate(value, field) {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffffffff) {
    fault(ErrorCode.INVALID_PACKET, `${field} must be a non-negative u32`);
  }
  return value;
}

function pulseCacheLimit(owner, getterName, usageName, setterName, pressure) {
  const getter = owner?.[getterName];
  const usage = owner?.[usageName];
  const setter = owner?.[setterName];
  if (
    typeof getter !== "function" ||
    typeof usage !== "function" ||
    typeof setter !== "function"
  ) {
    return;
  }
  const previousLimit = getter.call(owner);
  const usedBytes = usage.call(owner);
  if (
    !Number.isSafeInteger(previousLimit) ||
    previousLimit < 0 ||
    !Number.isSafeInteger(usedBytes) ||
    usedBytes < 0
  ) {
    fault(ErrorCode.INTERNAL, `${getterName}/${usageName} returned invalid cache bytes`);
  }
  const temporaryLimit = pressure === 2 ? 0 : Math.floor(usedBytes / 2);
  setter.call(owner, temporaryLimit);
  setter.call(owner, previousLimit);
}

/**
 * Creates the synchronous CanvasKit command executor.
 *
 * `submit` returns a newly-owned Ack/Error packet. `eventSink`, when present,
 * receives `{ type, packet }` for asynchronous context-loss/restoration events.
 */
export function createCanvasKitExecutor({
  CanvasKit,
  canvas,
  eventSink,
  backendPreference,
}) {
  requireFactoryInputs(CanvasKit, canvas, eventSink);
  const backendOverride = normalizeBackendPreference(backendPreference);

  let protocol = new ProtocolSession();
  let resources = new Map();
  const paragraphHost = createCanvasKitParagraphHost({
    CanvasKit,
    resolveResource: (slot) => resources.get(slot),
  });
  let configuration = null;
  let surfaceState = null;
  let contextLost = false;
  let permanentlyDestroyed = false;
  let responseSession = 0n;
  let responseSequence = 0n;
  let lastCommandSequence = 0n;

  function nextResponseSequence(session) {
    if (responseSession !== session) {
      responseSession = session;
      responseSequence = 0n;
    }
    if (responseSequence === MAX_U64) {
      throw new Error("CanvasKit response sequence exhausted");
    }
    responseSequence += 1n;
    return responseSequence;
  }

  function ackFor(session, acknowledgedSequence) {
    return encodeAck({
      session,
      sequence: nextResponseSequence(session),
      acknowledgedSequence,
    });
  }

  function errorFor(session, failedSequence, error) {
    return encodeError({
      session,
      sequence: nextResponseSequence(session),
      failedSequence,
      code: errorCodeFor(error),
      message: errorMessage(error),
    });
  }

  function emitLifecycle(type, packet) {
    if (!eventSink) return;
    try {
      eventSink(Object.freeze({ type, packet }));
    } catch (_error) {
      // Host event handling must not interrupt CanvasKit teardown or recovery.
    }
  }

  function deleteAllResources() {
    for (const entry of resources.values()) deleteResourceEntry(entry);
    resources = new Map();
  }

  function closeSession(abandonContext) {
    paragraphHost.clear();
    deleteAllResources();
    deleteSurfaceState(CanvasKit, surfaceState, abandonContext);
    surfaceState = null;
    configuration = null;
    contextLost = false;
  }

  function handleInit(packet) {
    const selectedBackend = backendOverride ?? packet.backend;
    const nextConfiguration = {
      surface: copySurface(packet.surface),
      backendPreference: selectedBackend,
      colorSpace: packet.colorSpace,
      alphaMode: packet.alphaMode,
    };
    const previousWidth = canvas.width;
    const previousHeight = canvas.height;
    let nextSurface;
    try {
      nextSurface = makeInitialSurface(CanvasKit, canvas, nextConfiguration);
    } catch (error) {
      canvas.width = previousWidth;
      canvas.height = previousHeight;
      throw error;
    }
    configuration = nextConfiguration;
    surfaceState = nextSurface;
    contextLost = false;
  }

  function handleResize(packet) {
    if (!configuration || !surfaceState) {
      fault(ErrorCode.INVALID_STATE, "resize arrived before CanvasKit initialization");
    }
    if (contextLost) {
      fault(ErrorCode.SURFACE_LOST, "cannot resize while the WebGL context is lost");
    }
    const nextConfiguration = {
      ...configuration,
      surface: copySurface(packet.surface),
    };
    let nextSurface;
    try {
      nextSurface = recreateSurface(
        CanvasKit,
        canvas,
        surfaceState,
        nextConfiguration,
      );
    } catch (error) {
      setCanvasSize(canvas, configuration.surface);
      throw error;
    }
    configuration = nextConfiguration;
    surfaceState = nextSurface;
  }

  function handleFrame(packet) {
    if (!configuration || !surfaceState) {
      fault(ErrorCode.INVALID_STATE, "frame arrived before CanvasKit initialization");
    }
    if (!sameSurface(packet.surface, configuration.surface)) {
      fault(ErrorCode.INVALID_STATE, "frame surface does not match the latest Init/Resize");
    }
    if (contextLost) {
      fault(ErrorCode.SURFACE_LOST, "cannot render while the WebGL context is lost");
    }
    const commands = decodeCommandStream(packet.commands);
    if (isZeroSurface(configuration.surface)) {
      if (commands.length !== 0) {
        fault(ErrorCode.SURFACE_LOST, "cannot execute paint commands on a zero-sized surface");
      }
      return;
    }
    if (!surfaceState.surface) {
      fault(ErrorCode.SURFACE_LOST, "CanvasKit surface is unavailable");
    }
    if (typeof CanvasKit.Color4f !== "function") {
      fault(ErrorCode.INVALID_STATE, "CanvasKit Color4f is unavailable");
    }
    if (
      typeof surfaceState.surface.imageInfo !== "function" ||
      typeof surfaceState.surface.makeSurface !== "function"
    ) {
      fault(ErrorCode.INVALID_STATE, "CanvasKit surface cannot create a transactional frame");
    }

    // Critical memory pressure releases decoded images while retaining their
    // immutable encoded bytes. Rebuild them transactionally on the owning
    // browser executor immediately before the next frame needs them.
    resources = materializeImageResources(CanvasKit, resources);

    let staging = null;
    let snapshot = null;
    let presentationPaint = null;
    try {
      staging = requireOwnedObject(
        surfaceState.surface.makeSurface(surfaceState.surface.imageInfo()),
        "transactional frame surface",
        ErrorCode.SURFACE_LOST,
      );
      const stagingCanvas = staging.getCanvas?.();
      if (!stagingCanvas || typeof stagingCanvas.clear !== "function") {
        fault(ErrorCode.INVALID_STATE, "transactional surface is not drawable");
      }
      stagingCanvas.clear(CanvasKit.Color4f(...packet.clearColor));
      executeCommandStream({
        CanvasKit,
        canvas: stagingCanvas,
        commands,
        resolveResource: (slot) => resources.get(slot),
        paragraphHost,
      });
      if (typeof staging.flush !== "function" || typeof staging.makeImageSnapshot !== "function") {
        fault(ErrorCode.INVALID_STATE, "transactional surface cannot be snapshotted");
      }
      staging.flush();
      snapshot = requireOwnedObject(
        staging.makeImageSnapshot(),
        "transactional frame snapshot",
        ErrorCode.SURFACE_LOST,
      );

      const liveCanvas = surfaceState.surface.getCanvas?.();
      if (
        !liveCanvas ||
        typeof liveCanvas.drawImage !== "function" ||
        typeof surfaceState.surface.flush !== "function"
      ) {
        fault(ErrorCode.INVALID_STATE, "CanvasKit surface cannot present a frame snapshot");
      }
      const Paint = CanvasKit.Paint;
      if (typeof Paint !== "function" || CanvasKit.BlendMode?.Src === undefined) {
        fault(ErrorCode.INVALID_STATE, "CanvasKit source-replacement presentation is unavailable");
      }
      presentationPaint = new Paint();
      if (!presentationPaint || typeof presentationPaint.setBlendMode !== "function") {
        fault(ErrorCode.INVALID_STATE, "CanvasKit presentation paint is unavailable");
      }
      presentationPaint.setBlendMode(CanvasKit.BlendMode.Src);
      liveCanvas.drawImage(snapshot, 0, 0, presentationPaint);
      surfaceState.surface.flush();
    } finally {
      safeDelete(presentationPaint);
      safeDelete(snapshot);
      safeDisposeSurface(staging);
    }
  }

  function dispatch(message) {
    switch (message.envelope.kind) {
      case PacketKind.INIT:
        handleInit(message.packet);
        break;
      case PacketKind.RESIZE:
        handleResize(message.packet);
        break;
      case PacketKind.RESOURCE_BATCH:
        if (contextLost) {
          fault(ErrorCode.SURFACE_LOST, "cannot update resources while the WebGL context is lost");
        }
        resources = applyResourceBatch(CanvasKit, resources, message.packet);
        break;
      case PacketKind.FRAME:
        handleFrame(message.packet);
        break;
      case PacketKind.DESTROY:
        closeSession(true);
        break;
      case PacketKind.ACK:
      case PacketKind.ERROR:
        fault(ErrorCode.INVALID_PACKET, "Ack/Error packets are not executor commands");
        break;
      default:
        fault(ErrorCode.INVALID_PACKET, "unknown executor command");
    }
  }

  function submit(input) {
    const recovered = recoverIdentity(input);
    let message;
    try {
      message = decodeMessage(input);
    } catch (error) {
      const session = protocol.activeSession() ?? recovered?.session;
      const failedSequence = (recovered?.sequence ?? lastCommandSequence) || 1n;
      if (!session) throw error;
      return errorFor(session, failedSequence, error);
    }

    const responseIdentity = protocol.activeSession() ?? message.envelope.session;
    if (permanentlyDestroyed) {
      return errorFor(
        responseIdentity,
        message.envelope.sequence,
        new ExecutorFault(ErrorCode.INVALID_STATE, "CanvasKit executor is destroyed"),
      );
    }

    const candidate = protocol.fork();
    try {
      if (
        message.envelope.kind === PacketKind.ACK ||
        message.envelope.kind === PacketKind.ERROR
      ) {
        fault(ErrorCode.INVALID_PACKET, "Ack/Error packets are not executor commands");
      }
      candidate.accept(message);
      dispatch(message);
      protocol = candidate;
      lastCommandSequence = message.envelope.sequence;
      return ackFor(message.envelope.session, message.envelope.sequence);
    } catch (error) {
      return errorFor(responseIdentity, message.envelope.sequence, error);
    }
  }

  function onContextLost(event) {
    if (permanentlyDestroyed || contextLost || surfaceState?.backend !== "webgl") return;
    event?.preventDefault?.();
    contextLost = true;
    resources = stripResourceObjects(resources);
    deleteSurfaceState(CanvasKit, surfaceState, true);
    surfaceState = {
      backend: "webgl",
      surface: null,
      contextHandle: 0,
      grContext: null,
    };
    const session = protocol.activeSession();
    if (session && eventSink) {
      const failed = lastCommandSequence || 1n;
      emitLifecycle(
        "context-lost",
        errorFor(
          session,
          failed,
          new ExecutorFault(ErrorCode.SURFACE_LOST, "browser WebGL context was lost"),
        ),
      );
    }
  }

  function onContextRestored() {
    if (
      permanentlyDestroyed ||
      !contextLost ||
      !configuration ||
      surfaceState?.backend !== "webgl"
    ) {
      return;
    }
    const session = protocol.activeSession();
    let nextSurface = null;
    let restored = false;
    try {
      nextSurface = makeWebGlSurface(CanvasKit, canvas, configuration);
      const nextResources = rebuildResources(CanvasKit, resources);
      surfaceState = nextSurface;
      resources = nextResources;
      contextLost = false;
      restored = true;
    } catch (error) {
      deleteSurfaceState(CanvasKit, nextSurface, true);
      deleteSurfaceState(CanvasKit, surfaceState, true);
      surfaceState = {
        backend: "webgl",
        surface: null,
        contextHandle: 0,
        grContext: null,
      };
      if (session && eventSink) {
        emitLifecycle(
          "context-restore-failed",
          errorFor(session, lastCommandSequence || 1n, error),
        );
      }
    }
    if (restored && session && eventSink) {
      emitLifecycle(
        "context-restored",
        ackFor(session, lastCommandSequence || 1n),
      );
    }
  }

  canvas.addEventListener?.("webglcontextlost", onContextLost, false);
  canvas.addEventListener?.("webglcontextrestored", onContextRestored, false);

  function destroy() {
    if (permanentlyDestroyed) return;
    permanentlyDestroyed = true;
    canvas.removeEventListener?.("webglcontextlost", onContextLost, false);
    canvas.removeEventListener?.("webglcontextrestored", onContextRestored, false);
    closeSession(true);
  }

  function requireParagraphSession(operation) {
    if (permanentlyDestroyed) {
      fault(ErrorCode.INVALID_STATE, `cannot ${operation}; CanvasKit executor is destroyed`);
    }
    if (!configuration || !surfaceState) {
      fault(ErrorCode.INVALID_STATE, `cannot ${operation} before CanvasKit initialization`);
    }
    if (contextLost) {
      fault(ErrorCode.SURFACE_LOST, `cannot ${operation} while the WebGL context is lost`);
    }
  }

  function layoutParagraph(packet) {
    requireParagraphSession("layout a paragraph");
    return paragraphHost.layout(packet);
  }

  function destroyParagraph(handle) {
    requireParagraphSession("destroy a paragraph");
    if (!paragraphHost.destroy(handle)) {
      fault(ErrorCode.RESOURCE_FAILURE, "paragraph handle is not live at its generation");
    }
  }

  function readPixels(x, y, width, height) {
    requireParagraphSession("read surface pixels");
    x = requireReadbackCoordinate(x, "readback x");
    y = requireReadbackCoordinate(y, "readback y");
    width = requireReadbackCoordinate(width, "readback width");
    height = requireReadbackCoordinate(height, "readback height");
    const right = x + width;
    const bottom = y + height;
    if (
      !Number.isSafeInteger(right) ||
      !Number.isSafeInteger(bottom) ||
      right > configuration.surface.width ||
      bottom > configuration.surface.height
    ) {
      fault(ErrorCode.INVALID_PACKET, "readback rectangle is outside the CanvasKit surface");
    }
    if (width === 0 || height === 0) return new Uint8Array(0);
    if (!surfaceState.surface || typeof surfaceState.surface.flush !== "function") {
      fault(ErrorCode.SURFACE_LOST, "CanvasKit readback surface is unavailable");
    }
    const liveCanvas = surfaceState.surface.getCanvas?.();
    if (!liveCanvas || typeof liveCanvas.readPixels !== "function") {
      fault(ErrorCode.INVALID_STATE, "CanvasKit canvas readPixels is unavailable");
    }
    const colorType = CanvasKit.ColorType?.RGBA_8888;
    const alphaType = CanvasKit.AlphaType?.Premul;
    const colorSpace = CanvasKit.ColorSpace?.SRGB;
    if (colorType === undefined || alphaType === undefined || colorSpace === undefined) {
      fault(ErrorCode.INVALID_STATE, "CanvasKit RGBA8888 readback enums are unavailable");
    }
    const rowBytes = width * 4;
    surfaceState.surface.flush();
    let pixels;
    try {
      pixels = liveCanvas.readPixels(
        x,
        y,
        { width, height, colorType, alphaType, colorSpace },
        undefined,
        rowBytes,
      );
    } catch (error) {
      fault(ErrorCode.INTERNAL, `CanvasKit readPixels failed: ${errorMessage(error)}`);
    }
    if (!(pixels instanceof Uint8Array) || pixels.byteLength !== rowBytes * height) {
      fault(ErrorCode.INTERNAL, "CanvasKit readPixels returned an invalid RGBA8888 buffer");
    }
    return Uint8Array.from(pixels);
  }

  function trimMemory(pressure) {
    if (permanentlyDestroyed) {
      fault(ErrorCode.INVALID_STATE, "cannot trim memory; CanvasKit executor is destroyed");
    }
    if (pressure !== 1 && pressure !== 2) {
      fault(ErrorCode.INVALID_PACKET, "memory pressure must be moderate or critical");
    }
    pulseCacheLimit(
      CanvasKit,
      "getDecodeCacheLimitBytes",
      "getDecodeCacheUsedBytes",
      "setDecodeCacheLimitBytes",
      pressure,
    );
    pulseCacheLimit(
      surfaceState?.grContext,
      "getResourceCacheLimitBytes",
      "getResourceCacheUsageBytes",
      "setResourceCacheLimitBytes",
      pressure,
    );
    if (pressure === 2) {
      resources = stripResourceObjects(resources);
    }
  }

  return Object.freeze({
    submit,
    layoutParagraph,
    destroyParagraph,
    readPixels,
    trimMemory,
    destroy,
  });
}
