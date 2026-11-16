use crate::web_data_stream::{browser_data_stream, BrowserDataStream, BrowserDataStreamSink};
use fission_core::{
    Bytes, FileSystemError, FileSystemPermission, FileWriteSource, FissionDataStreamError,
    FissionDataStreamErrorKind, ForgetSaveFileRequest, PickOpenFilesError, PickOpenFilesRequest,
    PickOpenFilesResult, PickSaveFileRequest, PickSaveFileResult, PickedFile,
    RestoreSaveFileRequest, RestoreSaveFileResult, SaveFileDestination, WritableFileHandle,
    WritableFileHandleId, WritableFilePermissionRequest, WritableFilePermissionResult,
    WritePickedFileRequest, WritePickedFileResult, FORGET_SAVE_FILE, PICK_OPEN_FILES,
    PICK_SAVE_FILE, RESTORE_SAVE_FILE, WRITABLE_FILE_PERMISSION, WRITE_PICKED_FILE,
};
use fission_shell::async_host::AsyncRegistry;
use futures_core::Stream;
use js_sys::{Array, Promise, Reflect, Uint8Array};
use std::pin::Pin;
use std::task::{Context, Poll};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

#[wasm_bindgen(module = "/src/web_file_picker.mjs")]
extern "C" {
    #[wasm_bindgen(catch)]
    fn fissionPickOpenFiles(allow_multiple: bool, accept: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionPickSaveFile(
        suggested_name: &str,
        types: &str,
        persistence_key: &str,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionRestoreSaveFile(persistence_key: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionForgetSaveFile(persistence_key: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionWritableFilePermission(file_id: u32, request: bool) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionOpenPickedFileReader(
        file_id: u32,
        preferred_chunk_bytes: u32,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionReadPickedFileChunk(reader: &JsValue) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionCancelPickedFileReader(reader: &JsValue) -> Result<Promise, JsValue>;
    fn fissionReleasePickedFile(file_id: u32);
    #[wasm_bindgen(catch)]
    fn fissionOpenPickedFileWriter(file_id: u32) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionWritePickedFileChunk(
        writable: &JsValue,
        bytes: &Uint8Array,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionClosePickedFileWriter(writable: &JsValue) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionAbortPickedFileWriter(writable: &JsValue) -> Result<Promise, JsValue>;
}

pub(crate) fn register_web_file_picker(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PICK_SAVE_FILE,
        |request: PickSaveFileRequest, _| async move {
            if let Some(key) = request.persistence_key.as_deref() {
                require_save_file_key(key)?;
            }
            let types = serde_json::to_string(&browser_save_file_types(&request))
                .map_err(|error| FileSystemError::new("invalid_request", error.to_string()))?;
            let value = await_promise(fissionPickSaveFile(
                &request.suggested_name,
                &types,
                request.persistence_key.as_deref().unwrap_or(""),
            ))
            .await
            .map_err(file_system_error)?;
            if value.is_null() || value.is_undefined() {
                return Ok(PickSaveFileResult { destination: None });
            }
            Ok(PickSaveFileResult {
                destination: Some(SaveFileDestination::WritableFile(writable_file_handle(
                    &value,
                    &request.suggested_name,
                )?)),
            })
        },
    );
    async_registry.register_operation_capability(
        RESTORE_SAVE_FILE,
        |request: RestoreSaveFileRequest, _| async move {
            require_save_file_key(&request.persistence_key)?;
            let value = await_promise(fissionRestoreSaveFile(&request.persistence_key))
                .await
                .map_err(file_system_error)?;
            let file = if value.is_null() || value.is_undefined() {
                None
            } else {
                Some(writable_file_handle(&value, "restored-file")?)
            };
            Ok(RestoreSaveFileResult { file })
        },
    );
    async_registry.register_operation_capability(
        FORGET_SAVE_FILE,
        |request: ForgetSaveFileRequest, _| async move {
            require_save_file_key(&request.persistence_key)?;
            await_promise(fissionForgetSaveFile(&request.persistence_key))
                .await
                .map_err(file_system_error)?;
            Ok(())
        },
    );
    async_registry.register_operation_capability(
        WRITABLE_FILE_PERMISSION,
        |request: WritableFilePermissionRequest, _| async move {
            let id = save_file_id(request.file.id)?;
            let value = await_promise(fissionWritableFilePermission(id, request.request))
                .await
                .map_err(file_system_error)?;
            Ok(WritableFilePermissionResult {
                permission: permission_value(value.as_string().as_deref()),
            })
        },
    );
    async_registry.register_operation_capability(
        WRITE_PICKED_FILE,
        |request: WritePickedFileRequest, ctx| async move {
            let id = u32::try_from(request.file.id.0).map_err(|_| {
                FileSystemError::new("invalid_handle", "save destination handle is invalid")
            })?;
            let mut source_stream = match &request.source {
                FileWriteSource::Bytes(_) => None,
                FileWriteSource::Stream(id) => {
                    Some(ctx.open_data_stream(*id).map_err(|error| {
                        FileSystemError::new("stream_open_failed", error.to_string())
                    })?)
                }
            };
            let writable = await_promise(fissionOpenPickedFileWriter(id))
                .await
                .map_err(file_system_error)?;
            let result = async {
                let mut byte_len = 0_u64;
                match request.source {
                    FileWriteSource::Bytes(bytes) => {
                        write_browser_chunk(&writable, &bytes).await?;
                        byte_len = bytes.len() as u64;
                    }
                    FileWriteSource::Stream(_) => {
                        let stream = source_stream.as_mut().expect("stream was opened above");
                        while let Some(chunk) =
                            std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await
                        {
                            let chunk = chunk.map_err(|error| {
                                FileSystemError::new("stream_read_failed", error.to_string())
                            })?;
                            write_browser_chunk(&writable, &chunk).await?;
                            byte_len = byte_len.saturating_add(chunk.len() as u64);
                        }
                    }
                }
                await_promise(fissionClosePickedFileWriter(&writable))
                    .await
                    .map_err(file_system_error)?;
                Ok::<u64, FileSystemError>(byte_len)
            }
            .await;
            match result {
                Ok(byte_len) => Ok(WritePickedFileResult { byte_len }),
                Err(error) => {
                    let _ = await_promise(fissionAbortPickedFileWriter(&writable)).await;
                    Err(error)
                }
            }
        },
    );
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
                    stream: ctx.register_data_stream(Box::pin(BrowserPickedFileStream::new(
                        file_id,
                        request.preferred_chunk_bytes,
                    ))),
                });
            }
            Ok(PickOpenFilesResult { files })
        },
    );
}

struct BrowserPickedFileStream {
    file_id: u32,
    preferred_chunk_bytes: Option<u32>,
    started: bool,
    stream: BrowserDataStream,
    sink: BrowserDataStreamSink,
}

impl BrowserPickedFileStream {
    fn new(file_id: u32, preferred_chunk_bytes: Option<u32>) -> Self {
        let (stream, sink) = browser_data_stream();
        Self {
            file_id,
            preferred_chunk_bytes,
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
            let preferred_chunk_bytes = self.preferred_chunk_bytes;
            let sink = self.sink.clone();
            spawn_local(pump_picked_file(file_id, preferred_chunk_bytes, sink));
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

async fn pump_picked_file(
    file_id: u32,
    preferred_chunk_bytes: Option<u32>,
    sink: BrowserDataStreamSink,
) {
    let reader = match fissionOpenPickedFileReader(file_id, preferred_chunk_bytes.unwrap_or(0)) {
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

async fn write_browser_chunk(writable: &JsValue, bytes: &[u8]) -> Result<(), FileSystemError> {
    await_promise(fissionWritePickedFileChunk(
        writable,
        &Uint8Array::from(bytes),
    ))
    .await
    .map_err(file_system_error)?;
    Ok(())
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

fn writable_file_handle(
    value: &JsValue,
    default_name: &str,
) -> Result<WritableFileHandle, FileSystemError> {
    let id = f64_prop(value, "id")
        .filter(|value| value.is_finite() && *value >= 1.0 && *value <= u32::MAX as f64)
        .map(|value| value as u64)
        .ok_or_else(|| {
            FileSystemError::new(
                "invalid_result",
                "browser save picker returned an invalid file handle",
            )
        })?;
    Ok(WritableFileHandle {
        id: WritableFileHandleId(id),
        name: string_prop(value, "name").unwrap_or_else(|| default_name.into()),
        permission: permission_value(string_prop(value, "permission").as_deref()),
    })
}

fn save_file_id(id: WritableFileHandleId) -> Result<u32, FileSystemError> {
    u32::try_from(id.0)
        .map_err(|_| FileSystemError::new("invalid_handle", "save destination handle is invalid"))
}

fn permission_value(value: Option<&str>) -> FileSystemPermission {
    match value {
        Some("granted") => FileSystemPermission::Granted,
        Some("prompt") => FileSystemPermission::Prompt,
        Some("denied") => FileSystemPermission::Denied,
        _ => FileSystemPermission::Unknown,
    }
}

fn require_save_file_key(key: &str) -> Result<(), FileSystemError> {
    if key.trim().is_empty() {
        Err(FileSystemError::new(
            "invalid_persistence_key",
            "save-file persistence keys cannot be empty",
        ))
    } else {
        Ok(())
    }
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

fn file_system_error(value: JsValue) -> FileSystemError {
    let (code, message) = js_error(value);
    FileSystemError::new(code, message)
}

fn browser_save_file_types(
    request: &PickSaveFileRequest,
) -> Vec<(String, std::collections::BTreeMap<String, Vec<String>>)> {
    let mut extensions = request
        .extensions
        .iter()
        .map(|extension| {
            extension
                .trim()
                .trim_start_matches('.')
                .to_ascii_lowercase()
        })
        .filter(|extension| !extension.is_empty())
        .collect::<Vec<_>>();
    if extensions.is_empty() {
        for mime in &request.mime_types {
            extensions.extend(
                extensions_for_mime(mime)
                    .iter()
                    .map(|extension| (*extension).into()),
            );
        }
    }
    extensions.sort();
    extensions.dedup();
    if extensions.is_empty() {
        return Vec::new();
    }

    let mut accept = std::collections::BTreeMap::<String, Vec<String>>::new();
    for extension in extensions {
        let inferred = mime_for_extension(&extension);
        let requested = request.mime_types.iter().find(|mime| {
            inferred.is_some_and(|inferred| {
                mime.as_str() == inferred
                    || mime
                        .strip_suffix("/*")
                        .is_some_and(|prefix| inferred.starts_with(&format!("{prefix}/")))
            })
        });
        let mime = requested
            .cloned()
            .or_else(|| inferred.map(str::to_string))
            .or_else(|| {
                request
                    .mime_types
                    .iter()
                    .find(|mime| !mime.ends_with("/*"))
                    .cloned()
            })
            .unwrap_or_else(|| "application/octet-stream".into());
        accept
            .entry(mime)
            .or_default()
            .push(format!(".{extension}"));
    }
    vec![("Allowed files".into(), accept)]
}

fn extensions_for_mime(mime: &str) -> &'static [&'static str] {
    match mime {
        "image/*" => &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"],
        "audio/*" => &["mp3", "wav", "ogg", "flac", "m4a"],
        "video/*" => &["mp4", "mov", "webm", "mkv"],
        "text/*" => &["txt", "md", "csv", "json", "yaml", "yml", "toml"],
        "application/pdf" => &["pdf"],
        "application/json" => &["json"],
        "text/plain" => &["txt"],
        "text/csv" => &["csv"],
        "text/markdown" => &["md"],
        _ => &[],
    }
}

fn mime_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        "pdf" => Some("application/pdf"),
        "json" => Some("application/json"),
        "txt" => Some("text/plain"),
        "csv" => Some("text/csv"),
        "md" => Some("text/markdown"),
        "yaml" | "yml" => Some("application/yaml"),
        "toml" => Some("application/toml"),
        "mp3" => Some("audio/mpeg"),
        "wav" => Some("audio/wav"),
        "ogg" => Some("audio/ogg"),
        "flac" => Some("audio/flac"),
        "m4a" => Some("audio/mp4"),
        "mp4" => Some("video/mp4"),
        "mov" => Some("video/quicktime"),
        "webm" => Some("video/webm"),
        "mkv" => Some("video/x-matroska"),
        _ => None,
    }
}

#[cfg(test)]
mod save_file_type_tests {
    use super::*;

    #[test]
    fn browser_save_filter_groups_extensions_by_concrete_mime_type() {
        let types = browser_save_file_types(&PickSaveFileRequest {
            suggested_name: "report.pdf".into(),
            extensions: vec![".pdf".into(), "csv".into()],
            mime_types: vec!["application/pdf".into(), "text/csv".into()],
        });

        let expected = [(
            "Allowed files".into(),
            [
                ("application/pdf".into(), vec![".pdf".into()]),
                ("text/csv".into(), vec![".csv".into()]),
            ]
            .into_iter()
            .collect(),
        )];
        assert_eq!(types, expected);
    }

    #[test]
    fn browser_save_filter_expands_mime_patterns_and_keeps_suggested_name_unfiltered() {
        let types = browser_save_file_types(&PickSaveFileRequest {
            mime_types: vec!["image/*".into()],
            ..Default::default()
        });
        assert_eq!(types.len(), 1);
        let accepted = &types[0].1;
        assert_eq!(accepted.get("image/png"), Some(&vec![".png".into()]));
        assert_eq!(
            accepted.get("image/jpeg"),
            Some(&vec![".jpg".into(), ".jpeg".into()])
        );

        assert!(browser_save_file_types(&PickSaveFileRequest {
            suggested_name: "backup.custom".into(),
            ..Default::default()
        })
        .is_empty());
    }
}
