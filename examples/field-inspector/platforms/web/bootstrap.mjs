import CanvasKitInit from "./canvaskit/web/canvaskit.js";
import { createCanvasKitExecutor } from "./canvaskit/web/fission_skia_executor.js";
import init from "./pkg/field_inspector.js";

const CanvasKit = await CanvasKitInit({
  locateFile: (file) => new URL(`./canvaskit/web/${file}`, import.meta.url).href,
});

globalThis.__FISSION_CANVASKIT_CREATE_EXECUTOR = (canvas, eventSink) =>
  createCanvasKitExecutor({ CanvasKit, canvas, eventSink });

try {
  await init();
} catch (error) {
  delete globalThis.__FISSION_CANVASKIT_CREATE_EXECUTOR;
  throw error;
}
