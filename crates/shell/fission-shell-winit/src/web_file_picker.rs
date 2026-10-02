use crate::web_data_stream::{browser_data_stream, BrowserDataStream, BrowserDataStreamSink};
use fission_core::{
    Bytes, FissionDataStreamError, FissionDataStreamErrorKind, PickOpenFilesError,
    PickOpenFilesRequest, PickOpenFilesResult, PickedFile, PICK_OPEN_FILES,
};
use fission_shell::async_host::AsyncRegistry;
use futures_core::Stream;
use js_sys::{Array, Promise, Reflect, Uint8Array};
use std::pin::Pin;
use std::task::{Context, Poll};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

#[wasm_bindgen(inline_js = r#"
const pickedFiles = new Map();
let nextPickedFile = 1;
const fileChunkSize = 64 * 1024;

function registerPickedFile(file) {
  const id = nextPickedFile;
  nextPickedFile = id >= 0xffffffff ? 1 : id + 1;
  if (pickedFiles.has(id)) throw Object.assign(new Error("too many selected files are still active"), { name: "resource_exhausted" });
  pickedFiles.set(id, file);
  return id;
}

export function fissionPickOpenFiles(allowMultiple, accept) {
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = Boolean(allowMultiple);
    if (accept) input.accept = accept;
    input.style.position = "fixed";
    input.style.left = "-10000px";
    input.style.width = "1px";
    input.style.height = "1px";
    document.body.appendChild(input);

    let settled = false;
    const finish = (value, error) => {
      if (settled) return;
      settled = true;
      input.remove();
      if (error) reject(error); else resolve(value);
    };
    input.oncancel = () => finish([]);
    input.onchange = () => {
      try {
        const selected = [];
        for (const file of Array.from(input.files || [])) {
          selected.push({
            name: file.name || "selected-file",
            contentType: file.type || null,
            byteLen: file.size,
            fileId: registerPickedFile(file),
          });
        }
        finish(selected);
      } catch (error) {
        finish(null, error);
      }
    };
    input.click();
  });
}

export function fissionOpenPickedFileReader(fileId) {
  const id = Number(fileId);
  const file = pickedFiles.get(id);
  if (!file) throw Object.assign(new Error("the selected browser file is no longer available"), { name: "invalid_handle" });
  pickedFiles.delete(id);

  if (typeof file.stream === "function") {
    const reader = file.stream().getReader();
    let pending = null;
    let offset = 0;
    return {
      async read() {
        while (!pending || offset >= pending.byteLength) {
          const result = await reader.read();
          if (result.done) return { done: true, value: undefined };
          pending = result.value instanceof Uint8Array ? result.value : new Uint8Array(result.value);
          offset = 0;
          if (pending.byteLength === 0) continue;
        }
        const end = Math.min(offset + fileChunkSize, pending.byteLength);
        const value = pending.subarray(offset, end);
        offset = end;
        return { done: false, value };
      },
      cancel() { return reader.cancel().catch(() => undefined); },
    };
  }

  let offset = 0;
  let cancelled = false;
  return {
    async read() {
      if (cancelled || offset >= file.size) return { done: true, value: undefined };
      const end = Math.min(offset + fileChunkSize, file.size);
      const value = new Uint8Array(await file.slice(offset, end).arrayBuffer());
      offset = end;
      return { done: false, value };
    },
    async cancel() { cancelled = true; },
  };
}

export function fissionReadPickedFileChunk(reader) {
  return reader.read();
}

export function fissionCancelPickedFileReader(reader) {
  return reader.cancel().catch(() => undefined);
}

export function fissionReleasePickedFile(fileId) {
  pickedFiles.delete(Number(fileId));
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn fissionPickOpenFiles(allow_multiple: bool, accept: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionOpenPickedFileReader(file_id: u32) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionReadPickedFileChunk(reader: &JsValue) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionCancelPickedFileReader(reader: &JsValue) -> Result<Promise, JsValue>;
    fn fissionReleasePickedFile(file_id: u32);
}

pub(crate) fn register_web_file_picker(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PICK_OPEN_FILES,
        move |request: PickOpenFilesRequest, ctx| async move {
            let accept = request
                .mime_types
                .iter()
                .cloned()
                .chain(request.extensions.iter().filter_map(|extension| {
                    let extension = extension.trim().trim_start_matches('.');
                    (!extension.is_empty()).then(|| format!(".{extension}"))
                }))
                .collect::<Vec<_>>()
                .join(",");
            let value = await_promise(fissionPickOpenFiles(request.allow_multiple, &accept))
                .await
                .map_err(file_picker_error)?;
            let values = value.dyn_into::<Array>().map_err(|_| {
                PickOpenFilesError::new(
                    "invalid_result",
                    "browser file picker returned a non-array result",
                )
            })?;
            let mut files = Vec::with_capacity(values.length() as usize);
            for value in values.iter() {
                let file_id = f64_prop(&value, "fileId")
                    .filter(|value| value.is_finite() && *value >= 1.0 && *value <= u32::MAX as f64)
                    .map(|value| value as u32)
                    .ok_or_else(|| {
                        PickOpenFilesError::new(
                            "invalid_result",
                            "selected browser file did not contain a valid handle",
                        )
                    })?;
                let byte_len = f64_prop(&value, "byteLen")
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .map(|value| value as u64);
                files.push(PickedFile {
                    name: string_prop(&value, "name").unwrap_or_else(|| "selected-file".into()),
                    content_type: string_prop(&value, "contentType")
                        .filter(|value| !value.is_empty()),
                    byte_len,
                    stream: ctx
                        .register_data_stream(Box::pin(BrowserPickedFileStream::new(file_id))),
                });
            }
            Ok(PickOpenFilesResult { files })
        },
    );
}

struct BrowserPickedFileStream {
    file_id: u32,
    started: bool,
    stream: BrowserDataStream,
    sink: BrowserDataStreamSink,
}

impl BrowserPickedFileStream {
    fn new(file_id: u32) -> Self {
        let (stream, sink) = browser_data_stream();
        Self {
            file_id,
            started: false,
            stream,
            sink,
        }
    }
}

impl Stream for BrowserPickedFileStream {
    type Item = Result<Bytes, FissionDataStreamError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if !self.started {
            self.started = true;
            let file_id = self.file_id;
            let sink = self.sink.clone();
            spawn_local(pump_picked_file(file_id, sink));
        }
        Pin::new(&mut self.stream).poll_next(cx)
    }
}

impl Drop for BrowserPickedFileStream {
    fn drop(&mut self) {
        if !self.started {
            fissionReleasePickedFile(self.file_id);
        }
    }
}

async fn pump_picked_file(file_id: u32, sink: BrowserDataStreamSink) {
    let reader = match fissionOpenPickedFileReader(file_id) {
        Ok(reader) => reader,
        Err(error) => {
            let (_, message) = js_error(error);
            let _ = sink
                .push(Err(FissionDataStreamError::new(
                    FissionDataStreamErrorKind::Io,
                    message,
                )))
                .await;
            sink.finish();
            return;
        }
    };

    loop {
        let result = match await_promise(fissionReadPickedFileChunk(&reader)).await {
            Ok(result) => result,
            Err(error) => {
                let (_, message) = js_error(error);
                let _ = sink
                    .push(Err(FissionDataStreamError::new(
                        FissionDataStreamErrorKind::Io,
                        message,
                    )))
                    .await;
                break;
            }
        };
        if prop(&result, "done")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            break;
        }
        let Some(bytes) =
            prop(&result, "value").and_then(|value| value.dyn_into::<Uint8Array>().ok())
        else {
            let _ = sink
                .push(Err(FissionDataStreamError::new(
                    FissionDataStreamErrorKind::InvalidData,
                    "browser file stream returned an invalid chunk",
                )))
                .await;
            break;
        };
        if !sink.push(Ok(Bytes::from(bytes.to_vec()))).await {
            let _ = await_promise(fissionCancelPickedFileReader(&reader)).await;
            return;
        }
    }
    let _ = await_promise(fissionCancelPickedFileReader(&reader)).await;
    sink.finish();
}

async fn await_promise(promise: Result<Promise, JsValue>) -> Result<JsValue, JsValue> {
    JsFuture::from(promise?).await
}

fn prop(value: &JsValue, key: &str) -> Option<JsValue> {
    Reflect::get(value, &JsValue::from_str(key)).ok()
}

fn string_prop(value: &JsValue, key: &str) -> Option<String> {
    prop(value, key).and_then(|value| value.as_string())
}

fn f64_prop(value: &JsValue, key: &str) -> Option<f64> {
    prop(value, key).and_then(|value| value.as_f64())
}

fn js_error(value: JsValue) -> (String, String) {
    let code = string_prop(&value, "name")
        .unwrap_or_else(|| "host_error".into())
        .to_ascii_lowercase();
    let message = string_prop(&value, "message")
        .or_else(|| value.as_string())
        .unwrap_or_else(|| format!("{value:?}"));
    (code, message)
}

fn file_picker_error(value: JsValue) -> PickOpenFilesError {
    let (code, message) = js_error(value);
    PickOpenFilesError::new(code, message)
}
