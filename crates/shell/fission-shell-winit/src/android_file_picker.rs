use crate::android_capabilities::{app_class, AndroidHostContext};
use fission_core::{
    Bytes, CapabilityCtx, FileSystemError, FileWriteSource, FissionDataStreamError,
    FissionDataStreamErrorKind, PickOpenFilesError, PickOpenFilesRequest, PickOpenFilesResult,
    PickSaveFileRequest, PickSaveFileResult, PickedFile, SaveFileDestination, WritableFileHandle,
    WritableFileHandleId, WritePickedFileRequest, WritePickedFileResult, PICK_OPEN_FILES,
    PICK_SAVE_FILE, WRITE_PICKED_FILE,
};
use fission_shell::async_host::AsyncRegistry;
use futures_core::Stream;
use jni::objects::{JClass, JObject, JObjectArray, JString, JValue};
use jni::sys::{jint, jlong};
use jni::JNIEnv;
use std::collections::HashMap;
use std::fs::File;
use std::future::Future;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};

const JAVA_HELPER: &str = "rs.fission.runtime.FissionAndroidCapabilities";
const FILE_STREAM_CHUNK_SIZE: usize = 64 * 1024;

#[derive(Clone, Debug)]
struct PickedDocument {
    uri: String,
    name: String,
    content_type: Option<String>,
    byte_len: Option<u64>,
}

struct PickState {
    result: Mutex<Option<Result<Vec<PickedDocument>, FileSystemError>>>,
    waker: Mutex<Option<Waker>>,
}

struct PickFuture(Arc<PickState>);

impl Future for PickFuture {
    type Output = Result<Vec<PickedDocument>, FileSystemError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.0.result.lock().unwrap().take() {
            return Poll::Ready(result);
        }
        *self.0.waker.lock().unwrap() = Some(cx.waker().clone());
        self.0
            .result
            .lock()
            .unwrap()
            .take()
            .map_or(Poll::Pending, Poll::Ready)
    }
}

fn pending_picks() -> &'static Mutex<HashMap<u64, Arc<PickState>>> {
    static PICKS: OnceLock<Mutex<HashMap<u64, Arc<PickState>>>> = OnceLock::new();
    PICKS.get_or_init(Default::default)
}

fn saved_documents() -> &'static Mutex<HashMap<WritableFileHandleId, String>> {
    static FILES: OnceLock<Mutex<HashMap<WritableFileHandleId, String>>> = OnceLock::new();
    FILES.get_or_init(Default::default)
}

fn next_pick_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn next_file_id() -> WritableFileHandleId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    WritableFileHandleId(NEXT.fetch_add(1, Ordering::Relaxed))
}

#[no_mangle]
pub extern "system" fn Java_rs_fission_runtime_FissionAndroidCapabilities_nativeFilePickerResult(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    request_id: jlong,
    uris: JObjectArray<'_>,
    names: JObjectArray<'_>,
    mime_types: JObjectArray<'_>,
    sizes: JObjectArray<'_>,
    error_code: JString<'_>,
    error_message: JString<'_>,
) {
    let Some(state) = pending_picks().lock().unwrap().remove(&(request_id as u64)) else {
        return;
    };

    let result = if !error_code.is_null() {
        let code = jstring(&mut env, &error_code).unwrap_or_else(|_| "picker_failed".into());
        let message = jstring(&mut env, &error_message)
            .unwrap_or_else(|_| "Android file picker failed".into());
        Err(FileSystemError::new(code, message))
    } else {
        parse_documents(&mut env, &uris, &names, &mime_types, &sizes)
    };
    *state.result.lock().unwrap() = Some(result);
    if let Some(waker) = state.waker.lock().unwrap().take() {
        waker.wake();
    };
}

fn parse_documents(
    env: &mut JNIEnv<'_>,
    uris: &JObjectArray<'_>,
    names: &JObjectArray<'_>,
    mime_types: &JObjectArray<'_>,
    sizes: &JObjectArray<'_>,
) -> Result<Vec<PickedDocument>, FileSystemError> {
    let count = env
        .get_array_length(uris)
        .map_err(|error| picker_error(error.to_string()))?;
    for values in [names, mime_types, sizes] {
        if env
            .get_array_length(values)
            .map_err(|error| picker_error(error.to_string()))?
            != count
        {
            return Err(picker_error("Android returned mismatched file metadata"));
        }
    }

    let mut documents = Vec::with_capacity(count as usize);
    for index in 0..count {
        let uri = array_string(env, uris, index)?
            .filter(|value| !value.is_empty())
            .ok_or_else(|| picker_error("Android returned a selected file without a URI"))?;
        let name = array_string(env, names, index)?
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "selected-file".into());
        let content_type = array_string(env, mime_types, index)?.filter(|value| !value.is_empty());
        let byte_len = array_string(env, sizes, index)?.and_then(|value| value.parse::<u64>().ok());
        documents.push(PickedDocument {
            uri,
            name,
            content_type,
            byte_len,
        });
    }
    Ok(documents)
}

fn array_string(
    env: &mut JNIEnv<'_>,
    values: &JObjectArray<'_>,
    index: jint,
) -> Result<Option<String>, FileSystemError> {
    let value = env
        .get_object_array_element(values, index)
        .map_err(|error| picker_error(error.to_string()))?;
    if value.is_null() {
        Ok(None)
    } else {
        jstring(env, &JString::from(value)).map(Some)
    }
}

fn jstring(env: &mut JNIEnv<'_>, value: &JString<'_>) -> Result<String, FileSystemError> {
    env.get_string(value)
        .map(|value| value.to_string_lossy().into_owned())
        .map_err(|error| picker_error(error.to_string()))
}

fn picker_error(message: impl Into<String>) -> FileSystemError {
    FileSystemError::new("picker_failed", message)
}

fn open_picker(
    context: &AndroidHostContext,
    save: bool,
    allow_multiple: bool,
    suggested_name: &str,
    mime_types: &[String],
) -> Result<PickFuture, FileSystemError> {
    let request_id = next_pick_id();
    let state = Arc::new(PickState {
        result: Mutex::new(None),
        waker: Mutex::new(None),
    });
    pending_picks()
        .lock()
        .unwrap()
        .insert(request_id, state.clone());

    let result = context.with_env(|env, activity| {
        let helper = app_class(env, activity, JAVA_HELPER)?;
        let class = env.find_class("java/lang/String")?;
        let values = env.new_object_array(mime_types.len() as jint, class, JObject::null())?;
        for (index, mime_type) in mime_types.iter().enumerate() {
            let value = env.new_string(mime_type)?;
            env.set_object_array_element(&values, index as jint, value)?;
        }
        let types = JObject::from(values);
        let name = JObject::from(env.new_string(suggested_name)?);
        env.call_static_method(
            helper,
            "beginPickFile",
            "(Landroid/app/Activity;JZZLjava/lang/String;[Ljava/lang/String;)V",
            &[
                JValue::Object(activity),
                JValue::Long(request_id as jlong),
                JValue::Bool(save as u8),
                JValue::Bool(allow_multiple as u8),
                JValue::Object(&name),
                JValue::Object(&types),
            ],
        )?;
        Ok(())
    });

    if let Err(error) = result {
        pending_picks().lock().unwrap().remove(&request_id);
        return Err(picker_error(error));
    }
    Ok(PickFuture(state))
}

pub(crate) fn register_android_file_picker_capabilities(
    async_registry: &mut AsyncRegistry,
    context: AndroidHostContext,
) {
    let host = context.clone();
    async_registry.register_operation_capability(
        PICK_OPEN_FILES,
        move |request: PickOpenFilesRequest, ctx: CapabilityCtx| {
            let host = host.clone();
            async move {
                let mime_types = picker_mime_types(&request.mime_types, &request.extensions);
                let documents = open_picker(&host, false, request.allow_multiple, "", &mime_types)
                    .map_err(|error| PickOpenFilesError::new(error.code, error.message))?
                    .await
                    .map_err(|error| PickOpenFilesError::new(error.code, error.message))?;
                let mut files = Vec::with_capacity(documents.len());
                for document in documents {
                    let fd = open_picked_file(&host, &document.uri, false)
                        .map_err(|error| PickOpenFilesError::new(error.code, error.message))?;
                    let file = unsafe { File::from_raw_fd(fd) };
                    files.push(PickedFile {
                        name: document.name,
                        content_type: document.content_type,
                        byte_len: document.byte_len,
                        stream: ctx.register_data_stream(Box::pin(AndroidFileStream {
                            file,
                            finished: false,
                        })),
                    });
                }
                Ok(PickOpenFilesResult { files })
            }
        },
    );

    let host = context.clone();
    async_registry.register_operation_capability(
        PICK_SAVE_FILE,
        move |request: PickSaveFileRequest, _| {
            let host = host.clone();
            async move {
                let mime_types = picker_mime_types(&request.mime_types, &request.extensions);
                let documents =
                    open_picker(&host, true, false, &request.suggested_name, &mime_types)?.await?;
                let Some(document) = documents.into_iter().next() else {
                    return Ok(PickSaveFileResult { destination: None });
                };
                let id = next_file_id();
                saved_documents().lock().unwrap().insert(id, document.uri);
                Ok(PickSaveFileResult {
                    destination: Some(SaveFileDestination::WritableFile(WritableFileHandle {
                        id,
                        name: document.name,
                    })),
                })
            }
        },
    );

    async_registry.register_operation_capability(
        WRITE_PICKED_FILE,
        move |request: WritePickedFileRequest, ctx| {
            let host = context.clone();
            async move {
                let uri = saved_documents()
                    .lock()
                    .unwrap()
                    .get(&request.file.id)
                    .cloned()
                    .ok_or_else(|| {
                        FileSystemError::new(
                            "invalid_handle",
                            "the Android save destination is no longer active",
                        )
                    })?;
                let fd = open_picked_file(&host, &uri, true)?;
                let mut file = unsafe { File::from_raw_fd(fd) };
                let byte_len = match request.source {
                    FileWriteSource::Bytes(bytes) => {
                        file.write_all(&bytes).map_err(io_error)?;
                        bytes.len() as u64
                    }
                    FileWriteSource::Stream(id) => {
                        let mut stream = ctx.open_data_stream(id).map_err(|error| {
                            FileSystemError::new("stream_open_failed", error.to_string())
                        })?;
                        let mut total = 0_u64;
                        while let Some(chunk) =
                            std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await
                        {
                            let chunk = chunk.map_err(|error| {
                                FileSystemError::new("stream_read_failed", error.to_string())
                            })?;
                            file.write_all(&chunk).map_err(io_error)?;
                            total = total.saturating_add(chunk.len() as u64);
                        }
                        total
                    }
                };
                file.flush().map_err(io_error)?;
                Ok(WritePickedFileResult { byte_len })
            }
        },
    );
}

fn picker_mime_types(mime_types: &[String], extensions: &[String]) -> Vec<String> {
    let mut values = mime_types
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| value.contains('/'))
        .collect::<Vec<_>>();
    for extension in extensions {
        let mime = match extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "pdf" => Some("application/pdf"),
            "json" => Some("application/json"),
            "doc" => Some("application/msword"),
            "docx" => {
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
            }
            "xls" => Some("application/vnd.ms-excel"),
            "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
            "ppt" => Some("application/vnd.ms-powerpoint"),
            "pptx" => {
                Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
            }
            "odt" => Some("application/vnd.oasis.opendocument.text"),
            "ods" => Some("application/vnd.oasis.opendocument.spreadsheet"),
            "odp" => Some("application/vnd.oasis.opendocument.presentation"),
            "rtf" => Some("application/rtf"),
            "zip" => Some("application/zip"),
            "txt" | "md" | "csv" | "toml" | "yaml" | "yml" => Some("text/*"),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" => Some("image/*"),
            "mp3" | "wav" | "ogg" | "flac" | "m4a" => Some("audio/*"),
            "mp4" | "mov" | "webm" | "mkv" => Some("video/*"),
            _ => None,
        };
        if let Some(mime) = mime {
            values.push(mime.into());
        }
    }
    values.sort();
    values.dedup();
    values
}

fn open_picked_file(
    context: &AndroidHostContext,
    uri: &str,
    write: bool,
) -> Result<i32, FileSystemError> {
    context
        .with_env(|env, activity| {
            let helper = app_class(env, activity, JAVA_HELPER)?;
            let uri = JObject::from(env.new_string(uri)?);
            env.call_static_method(
                helper,
                if write {
                    "openPickedFileWrite"
                } else {
                    "openPickedFileRead"
                },
                "(Landroid/app/Activity;Ljava/lang/String;)I",
                &[JValue::Object(activity), JValue::Object(&uri)],
            )?
            .i()
        })
        .map_err(|message| {
            FileSystemError::new(if write { "write_failed" } else { "open_failed" }, message)
        })
}

struct AndroidFileStream {
    file: File,
    finished: bool,
}

impl Stream for AndroidFileStream {
    type Item = Result<Bytes, FissionDataStreamError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }
        let mut chunk = vec![0; FILE_STREAM_CHUNK_SIZE];
        match self.file.read(&mut chunk) {
            Ok(0) => {
                self.finished = true;
                Poll::Ready(None)
            }
            Ok(read) => {
                chunk.truncate(read);
                Poll::Ready(Some(Ok(Bytes::from(chunk))))
            }
            Err(error) => {
                self.finished = true;
                Poll::Ready(Some(Err(FissionDataStreamError::new(
                    FissionDataStreamErrorKind::Io,
                    error.to_string(),
                ))))
            }
        }
    }
}

fn io_error(error: std::io::Error) -> FileSystemError {
    FileSystemError::new("write_failed", error.to_string())
}
