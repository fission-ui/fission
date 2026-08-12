import { createCanvasKitExecutor } from "./canvaskit/web/fission_skia_executor.js";
import init from "./pkg/field_inspector.js";

const { CanvasKitInit } = globalThis;
if (typeof CanvasKitInit !== "function") {
  throw new Error("Fission's verified CanvasKit loader did not install CanvasKitInit");
}
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
