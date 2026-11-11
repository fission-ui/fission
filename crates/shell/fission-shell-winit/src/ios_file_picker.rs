use block::ConcreteBlock;
use fission_core::{
    Bytes, CapabilityCtx, FileSystemError, FileWriteSource, FissionDataStreamError,
    FissionDataStreamErrorKind, PickOpenFilesError, PickOpenFilesRequest, PickOpenFilesResult,
    PickSaveFileRequest, PickSaveFileResult, PickedFile, SaveFileDestination, WritableFileHandle,
    WritableFileHandleId, WritePickedFileRequest, WritePickedFileResult, PICK_OPEN_FILES,
    PICK_SAVE_FILE, WRITE_PICKED_FILE,
};
use fission_shell::async_host::AsyncRegistry;
use futures_core::Stream;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Protocol, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::ffi::CStr;
use std::future::Future;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};

#[link(name = "Foundation", kind = "framework")]
extern "C" {}
#[link(name = "UIKit", kind = "framework")]
extern "C" {}

const FILE_STREAM_CHUNK_SIZE: usize = 64 * 1024;

struct IosSecurityScope {
    url: usize,
}

unsafe impl Send for IosSecurityScope {}
unsafe impl Sync for IosSecurityScope {}

impl Drop for IosSecurityScope {
    fn drop(&mut self) {
        unsafe {
            let url = self.url as *mut Object;
            let _: () = msg_send![url, stopAccessingSecurityScopedResource];
            let _: () = msg_send![url, release];
        }
    }
}

struct IosPickedFile {
    path: PathBuf,
    name: String,
    content_type: Option<String>,
    byte_len: Option<u64>,
    lease: Arc<IosSecurityScope>,
}

enum PickerMode {
    Open,
    Save { temporary_file: PathBuf },
}

struct PickerState {
    result: Mutex<Option<Result<Vec<IosPickedFile>, FileSystemError>>>,
    waker: Mutex<Option<Waker>>,
    mode: PickerMode,
    delegate: Mutex<Option<usize>>,
}

struct PickerFuture(Arc<PickerState>);

impl Future for PickerFuture {
    type Output = Result<Vec<IosPickedFile>, FileSystemError>;

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

fn picker_states() -> &'static Mutex<HashMap<u64, Arc<PickerState>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<PickerState>>>> = OnceLock::new();
    STATES.get_or_init(Default::default)
}

fn writable_files() -> &'static Mutex<HashMap<WritableFileHandleId, IosPickedFile>> {
    static FILES: OnceLock<Mutex<HashMap<WritableFileHandleId, IosPickedFile>>> = OnceLock::new();
    FILES.get_or_init(Default::default)
}

fn next_picker_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn next_file_id() -> WritableFileHandleId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    WritableFileHandleId(NEXT.fetch_add(1, Ordering::Relaxed))
}

pub(crate) fn register_ios_file_picker_capabilities(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PICK_OPEN_FILES,
        |request: PickOpenFilesRequest, ctx: CapabilityCtx| async move {
            let file_types = document_types(&request.mime_types, &request.extensions);
            let files = open_document_picker(request.allow_multiple, file_types)
                .await
                .map_err(|error| PickOpenFilesError::new(error.code, error.message))?;
            let files = files
                .into_iter()
                .map(|file| {
                    let stream = open_coordinated_file(&file.path)
                        .map_err(|error| PickOpenFilesError::new(error.code, error.message))?;
                    Ok(PickedFile {
                        name: file.name,
                        content_type: file.content_type,
                        byte_len: file.byte_len,
                        stream: ctx.register_data_stream(Box::pin(IosFileStream {
                            file: stream,
                            _lease: file.lease,
                            finished: false,
                        })),
                    })
                })
                .collect::<Result<Vec<_>, PickOpenFilesError>>()?;
            Ok(PickOpenFilesResult { files })
        },
    );

    async_registry.register_operation_capability(
        PICK_SAVE_FILE,
        |request: PickSaveFileRequest, _| async move {
            let temporary_file = temporary_export_file(
                &request.suggested_name,
                &request.extensions,
                &request.mime_types,
            )?;
            let files = save_document_picker(&temporary_file).await?;
            let Some(file) = files.into_iter().next() else {
                return Ok(PickSaveFileResult { destination: None });
            };
            let id = next_file_id();
            let name = file.name.clone();
            writable_files().lock().unwrap().insert(id, file);
            Ok(PickSaveFileResult {
                destination: Some(SaveFileDestination::WritableFile(WritableFileHandle {
                    id,
                    name,
                })),
            })
        },
    );

    async_registry.register_operation_capability(
        WRITE_PICKED_FILE,
        |request: WritePickedFileRequest, ctx| async move {
            let file = writable_files()
                .lock()
                .unwrap()
                .get(&request.file.id)
                .map(|file| IosPickedFile {
                    path: file.path.clone(),
                    name: file.name.clone(),
                    content_type: file.content_type.clone(),
                    byte_len: file.byte_len,
                    lease: file.lease.clone(),
                })
                .ok_or_else(|| {
                    FileSystemError::new(
                        "invalid_handle",
                        "the iOS save destination is no longer active",
                    )
                })?;
            let staged = stage_write_source(request.source, &ctx).await?;
            let result = write_coordinated(&file.path, &staged.0);
            let _ = std::fs::remove_file(&staged.0);
            result?;
            Ok(WritePickedFileResult { byte_len: staged.1 })
        },
    );
}

fn open_document_picker(allow_multiple: bool, types: Vec<String>) -> PickerFuture {
    begin_picker(PickerMode::Open, types, allow_multiple)
}

fn save_document_picker(temporary_file: &std::path::Path) -> PickerFuture {
    begin_picker(
        PickerMode::Save {
            temporary_file: temporary_file.to_path_buf(),
        },
        Vec::new(),
        false,
    )
}

fn begin_picker(mode: PickerMode, types: Vec<String>, allow_multiple: bool) -> PickerFuture {
    let id = next_picker_id();
    let state = Arc::new(PickerState {
        result: Mutex::new(None),
        waker: Mutex::new(None),
        mode,
        delegate: Mutex::new(None),
    });
    picker_states().lock().unwrap().insert(id, state.clone());
    let state_for_ui = state.clone();
    dispatch::Queue::main().exec_async(move || unsafe {
        if let Err(error) = present_picker(id, &state_for_ui, &types, allow_multiple) {
            picker_states().lock().unwrap().remove(&id);
            if let PickerMode::Save { temporary_file } = &state_for_ui.mode {
                remove_export_temp(temporary_file);
            }
            complete_picker(&state_for_ui, Err(error));
        }
    });
    PickerFuture(state)
}

unsafe fn present_picker(
    id: u64,
    state: &Arc<PickerState>,
    types: &[String],
    allow_multiple: bool,
) -> Result<(), FileSystemError> {
    let picker: *mut Object = match &state.mode {
        PickerMode::Open => {
            let values = ns_array(types);
            let picker: *mut Object = msg_send![class!(UIDocumentPickerViewController), alloc];
            msg_send![picker, initWithDocumentTypes: values inMode: 1i64]
        }
        PickerMode::Save { temporary_file } => {
            let path = temporary_file.to_string_lossy();
            let url: *mut Object =
                msg_send![class!(NSURL), fileURLWithPath: ns_owned_string(&path)];
            let picker: *mut Object = msg_send![class!(UIDocumentPickerViewController), alloc];
            msg_send![picker, initWithURL: url inMode: 2i64]
        }
    };
    if picker.is_null() {
        return Err(fs_error(
            "picker_unavailable",
            "iOS could not create a document picker",
        ));
    }
    let delegate: *mut Object = msg_send![picker_delegate_class(), new];
    (*delegate).set_ivar("_requestId", id as usize);
    *state.delegate.lock().unwrap() = Some(delegate as usize);
    let _: () = msg_send![picker, setDelegate: delegate];
    let _: () = msg_send![picker, setAllowsMultipleSelection: allow_multiple];
    let controller = active_view_controller();
    if controller.is_null() {
        let _: () = msg_send![picker, release];
        let _: () = msg_send![delegate, release];
        *state.delegate.lock().unwrap() = None;
        return Err(fs_error(
            "picker_unavailable",
            "iOS has no active view controller",
        ));
    }
    let _: () = msg_send![controller, presentViewController: picker animated: true completion: ptr::null::<Object>()];
    let _: () = msg_send![picker, release];
    Ok(())
}

fn picker_delegate_class() -> &'static Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    let value = *CLASS.get_or_init(|| {
        let mut decl = ClassDecl::new("FissionFilePickerDelegate", class!(NSObject))
            .expect("register FissionFilePickerDelegate");
        decl.add_ivar::<usize>("_requestId");
        if let Some(protocol) = Protocol::get("UIDocumentPickerDelegate") {
            decl.add_protocol(protocol);
        }
        unsafe {
            decl.add_method(
                sel!(documentPicker:didPickDocumentsAtURLs:),
                picker_did_select as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );
            decl.add_method(
                sel!(documentPickerWasCancelled:),
                picker_did_cancel as extern "C" fn(&mut Object, Sel, *mut Object),
            );
        }
        decl.register() as *const Class as usize
    });
    unsafe { &*(value as *const Class) }
}

extern "C" fn picker_did_select(
    this: &mut Object,
    _cmd: Sel,
    _picker: *mut Object,
    urls: *mut Object,
) {
    unsafe {
        let id = *this.get_ivar::<usize>("_requestId") as u64;
        let state = picker_states().lock().unwrap().remove(&id);
        let Some(state) = state else {
            return;
        };
        let result = selected_files(urls, &state.mode);
        complete_picker(&state, result);
        release_delegate_after_callback(&state);
    }
}

extern "C" fn picker_did_cancel(this: &mut Object, _cmd: Sel, _picker: *mut Object) {
    unsafe {
        let id = *this.get_ivar::<usize>("_requestId") as u64;
        let state = picker_states().lock().unwrap().remove(&id);
        if let Some(state) = state {
            if let PickerMode::Save { temporary_file } = &state.mode {
                remove_export_temp(temporary_file);
            }
            complete_picker(&state, Ok(Vec::new()));
            release_delegate_after_callback(&state);
        }
    }
}

unsafe fn selected_files(
    urls: *mut Object,
    mode: &PickerMode,
) -> Result<Vec<IosPickedFile>, FileSystemError> {
    let result = (|| {
        let count: usize = msg_send![urls, count];
        if count == 0 {
            return Ok(Vec::new());
        }
        let count = if matches!(mode, PickerMode::Save { .. }) {
            1
        } else {
            count
        };
        let mut files = Vec::with_capacity(count);
        for index in 0..count {
            let url: *mut Object = msg_send![urls, objectAtIndex: index];
            files.push(selected_file(url)?);
        }
        Ok(files)
    })();
    if let PickerMode::Save { temporary_file } = mode {
        remove_export_temp(temporary_file);
    }
    result
}

unsafe fn selected_file(url: *mut Object) -> Result<IosPickedFile, FileSystemError> {
    if url.is_null() {
        return Err(fs_error(
            "picker_failed",
            "iOS returned an empty document URL",
        ));
    }
    let accessed: bool = msg_send![url, startAccessingSecurityScopedResource];
    if !accessed {
        return Err(fs_error(
            "permission_denied",
            "iOS did not grant access to the selected document",
        ));
    }
    let retained: *mut Object = msg_send![url, retain];
    let lease = Arc::new(IosSecurityScope {
        url: retained as usize,
    });
    let path_value: *mut Object = msg_send![url, path];
    let name_value: *mut Object = msg_send![url, lastPathComponent];
    let path = ns_string(path_value).ok_or_else(|| {
        fs_error(
            "invalid_url",
            "iOS returned a document without a local path",
        )
    })?;
    let name = ns_string(name_value).unwrap_or_else(|| "selected-file".into());
    Ok(IosPickedFile {
        path: PathBuf::from(path),
        name: name.clone(),
        content_type: content_type_for_name(&name),
        byte_len: None,
        lease,
    })
}

fn complete_picker(state: &PickerState, result: Result<Vec<IosPickedFile>, FileSystemError>) {
    *state.result.lock().unwrap() = Some(result);
    if let Some(waker) = state.waker.lock().unwrap().take() {
        waker.wake();
    }
}

unsafe fn release_delegate_after_callback(state: &PickerState) {
    if let Some(delegate) = state.delegate.lock().unwrap().take() {
        dispatch::Queue::main().exec_async(move || {
            let delegate = delegate as *mut Object;
            let _: () = unsafe { msg_send![delegate, release] };
        });
    }
}

fn temporary_export_file(
    suggested_name: &str,
    extensions: &[String],
    mime_types: &[String],
) -> Result<PathBuf, FileSystemError> {
    let raw_name = PathBuf::from(suggested_name.trim());
    let mut name = raw_name
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Untitled")
        .to_string();
    if PathBuf::from(&name).extension().is_none() {
        let extension = extensions
            .iter()
            .map(|extension| extension.trim().trim_start_matches('.'))
            .find(|extension| !extension.is_empty())
            .or_else(|| mime_types.iter().find_map(|mime| extension_for_mime(mime)));
        if let Some(extension) = extension {
            name.push('.');
            name.push_str(extension);
        }
    }
    let directory = std::env::temp_dir().join(format!("fission-export-{}", next_picker_id()));
    std::fs::create_dir_all(&directory).map_err(io_error)?;
    let path = directory.join(name);
    std::fs::File::create(&path).map_err(io_error)?;
    Ok(path)
}

fn extension_for_mime(mime: &str) -> Option<&'static str> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "application/pdf" => Some("pdf"),
        "application/json" => Some("json"),
        "application/msword" => Some("doc"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        "application/vnd.ms-excel" => Some("xls"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some("xlsx"),
        "application/vnd.ms-powerpoint" => Some("ppt"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
        "application/vnd.oasis.opendocument.text" => Some("odt"),
        "application/vnd.oasis.opendocument.spreadsheet" => Some("ods"),
        "application/vnd.oasis.opendocument.presentation" => Some("odp"),
        "application/rtf" => Some("rtf"),
        "application/zip" => Some("zip"),
        _ => None,
    }
}

fn remove_export_temp(path: &std::path::Path) {
    if let Some(directory) = path.parent() {
        let _ = std::fs::remove_dir_all(directory);
    }
}

async fn stage_write_source(
    source: FileWriteSource,
    ctx: &CapabilityCtx,
) -> Result<(PathBuf, u64), FileSystemError> {
    let path = std::env::temp_dir().join(format!("fission-write-{}.tmp", next_picker_id()));
    let result = async {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(io_error)?;
        match source {
            FileWriteSource::Bytes(bytes) => file.write_all(&bytes).map_err(io_error)?,
            FileWriteSource::Stream(id) => {
                let mut stream = ctx.open_data_stream(id).map_err(|error| {
                    FileSystemError::new("stream_open_failed", error.to_string())
                })?;
                while let Some(chunk) =
                    std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await
                {
                    let chunk = chunk.map_err(|error| {
                        FileSystemError::new("stream_read_failed", error.to_string())
                    })?;
                    file.write_all(&chunk).map_err(io_error)?;
                }
            }
        }
        file.flush().map_err(io_error)?;
        file.metadata()
            .map(|metadata| metadata.len())
            .map_err(io_error)
    }
    .await;
    match result {
        Ok(byte_len) => Ok((path, byte_len)),
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            Err(error)
        }
    }
}

fn open_coordinated_file(path: &std::path::Path) -> Result<std::fs::File, FileSystemError> {
    let path_string = path.to_string_lossy().into_owned();
    let opened = Arc::new(Mutex::new(None));
    let path_for_block = path_string.clone();
    let opened_for_block = opened.clone();
    let block = ConcreteBlock::new(move |_url: *mut Object| {
        *opened_for_block.lock().unwrap() =
            Some(std::fs::File::open(&path_for_block).map_err(|error| error.to_string()));
    })
    .copy();
    unsafe {
        let coordinator: *mut Object = msg_send![class!(NSFileCoordinator), alloc];
        let coordinator: *mut Object =
            msg_send![coordinator, initWithFilePresenter: ptr::null::<Object>()];
        if coordinator.is_null() {
            return Err(fs_error(
                "open_failed",
                "iOS could not create a coordinated file reader",
            ));
        }
        let url: *mut Object =
            msg_send![class!(NSURL), fileURLWithPath: ns_owned_string(&path_string)];
        let mut error: *mut Object = ptr::null_mut();
        let _: () = msg_send![coordinator,
            coordinateReadingItemAtURL: url
            options: 0usize
            error: &mut error
            byAccessor: &*block
        ];
        let _: () = msg_send![coordinator, release];
        if !error.is_null() {
            return Err(fs_error(
                "open_failed",
                ns_error(error)
                    .unwrap_or_else(|| "iOS could not open the selected document".into()),
            ));
        }
    }
    let result = opened
        .lock()
        .unwrap()
        .take()
        .unwrap_or_else(|| Err("iOS did not open the coordinated document".into()));
    result.map_err(|message| fs_error("open_failed", message))
}

fn write_coordinated(
    path: &std::path::Path,
    staged_path: &std::path::Path,
) -> Result<(), FileSystemError> {
    let path_string = path.to_string_lossy().into_owned();
    let staged_string = staged_path.to_string_lossy().into_owned();
    let write_result = Arc::new(Mutex::new(None));
    let path_for_block = path_string.clone();
    let staged_for_block = staged_string.clone();
    let result_for_block = write_result.clone();
    let block = ConcreteBlock::new(move |_url: *mut Object| {
        let result = std::fs::File::open(&staged_for_block).and_then(|mut source| {
            std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&path_for_block)
                .and_then(|mut destination| {
                    std::io::copy(&mut source, &mut destination).map(|_| ())
                })
        });
        *result_for_block.lock().unwrap() = Some(result.map_err(|error| error.to_string()));
    })
    .copy();
    unsafe {
        let coordinator: *mut Object = msg_send![class!(NSFileCoordinator), alloc];
        let coordinator: *mut Object =
            msg_send![coordinator, initWithFilePresenter: ptr::null::<Object>()];
        if coordinator.is_null() {
            return Err(fs_error(
                "write_failed",
                "iOS could not create a coordinated file writer",
            ));
        }
        let url: *mut Object =
            msg_send![class!(NSURL), fileURLWithPath: ns_owned_string(&path_string)];
        let mut error: *mut Object = ptr::null_mut();
        let _: () = msg_send![coordinator,
            coordinateWritingItemAtURL: url
            options: 0usize
            error: &mut error
            byAccessor: &*block
        ];
        let _: () = msg_send![coordinator, release];
        if !error.is_null() {
            return Err(fs_error(
                "write_failed",
                ns_error(error)
                    .unwrap_or_else(|| "iOS could not write the selected document".into()),
            ));
        }
    }
    let result = write_result
        .lock()
        .unwrap()
        .take()
        .unwrap_or_else(|| Err("iOS did not run the coordinated file writer".into()))
        .map_err(|message| fs_error("write_failed", message));
    result
}

fn document_types(mime_types: &[String], extensions: &[String]) -> Vec<String> {
    let mut types = mime_types
        .iter()
        .filter_map(|mime| uti_for_mime(mime))
        .map(str::to_string)
        .collect::<Vec<_>>();
    for extension in extensions {
        if let Some(uti) = uti_for_extension(extension) {
            types.push(uti.into());
        }
    }
    if types.is_empty() {
        types.push("public.item".into());
    }
    types.sort();
    types.dedup();
    types
}

fn uti_for_mime(mime: &str) -> Option<&'static str> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "application/pdf" => Some("com.adobe.pdf"),
        "application/json" => Some("public.json"),
        "application/zip" => Some("public.zip-archive"),
        "application/msword" => Some("com.microsoft.word.doc"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            Some("org.openxmlformats.wordprocessingml.document")
        }
        "application/vnd.ms-excel" => Some("com.microsoft.excel.xls"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            Some("org.openxmlformats.spreadsheetml.sheet")
        }
        "application/vnd.ms-powerpoint" => Some("com.microsoft.powerpoint.ppt"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            Some("org.openxmlformats.presentationml.presentation")
        }
        "application/vnd.oasis.opendocument.text" => Some("org.oasis-open.opendocument.text"),
        "application/vnd.oasis.opendocument.spreadsheet" => {
            Some("org.oasis-open.opendocument.spreadsheet")
        }
        "application/vnd.oasis.opendocument.presentation" => {
            Some("org.oasis-open.opendocument.presentation")
        }
        "application/rtf" => Some("public.rtf"),
        "image/*" => Some("public.image"),
        "audio/*" => Some("public.audio"),
        "video/*" => Some("public.movie"),
        "text/*" | "text/plain" => Some("public.text"),
        "text/csv" => Some("public.comma-separated-values-text"),
        "text/markdown" => Some("net.daringfireball.markdown"),
        _ => None,
    }
}

fn uti_for_extension(extension: &str) -> Option<&'static str> {
    match extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" => Some("com.adobe.pdf"),
        "json" => Some("public.json"),
        "zip" => Some("public.zip-archive"),
        "doc" => Some("com.microsoft.word.doc"),
        "docx" => Some("org.openxmlformats.wordprocessingml.document"),
        "xls" => Some("com.microsoft.excel.xls"),
        "xlsx" => Some("org.openxmlformats.spreadsheetml.sheet"),
        "ppt" => Some("com.microsoft.powerpoint.ppt"),
        "pptx" => Some("org.openxmlformats.presentationml.presentation"),
        "odt" => Some("org.oasis-open.opendocument.text"),
        "ods" => Some("org.oasis-open.opendocument.spreadsheet"),
        "odp" => Some("org.oasis-open.opendocument.presentation"),
        "rtf" => Some("public.rtf"),
        "png" => Some("public.png"),
        "jpg" | "jpeg" => Some("public.jpeg"),
        "gif" => Some("com.compuserve.gif"),
        "webp" => Some("org.webmproject.webp"),
        "txt" => Some("public.plain-text"),
        "md" | "markdown" => Some("net.daringfireball.markdown"),
        "csv" => Some("public.comma-separated-values-text"),
        "mp3" => Some("public.mp3"),
        "wav" => Some("com.microsoft.waveform-audio"),
        "mp4" => Some("public.mpeg-4"),
        _ => None,
    }
}

fn content_type_for_name(name: &str) -> Option<String> {
    match PathBuf::from(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("pdf") => Some("application/pdf"),
        Some("json") => Some("application/json"),
        Some("doc") => Some("application/msword"),
        Some("docx") => {
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        }
        Some("xls") => Some("application/vnd.ms-excel"),
        Some("xlsx") => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        Some("ppt") => Some("application/vnd.ms-powerpoint"),
        Some("pptx") => {
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        }
        Some("odt") => Some("application/vnd.oasis.opendocument.text"),
        Some("ods") => Some("application/vnd.oasis.opendocument.spreadsheet"),
        Some("odp") => Some("application/vnd.oasis.opendocument.presentation"),
        Some("rtf") => Some("application/rtf"),
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        Some("txt") => Some("text/plain"),
        Some("md") => Some("text/markdown"),
        Some("csv") => Some("text/csv"),
        _ => None,
    }
    .map(str::to_string)
}

fn ns_array(values: &[String]) -> *mut Object {
    unsafe {
        let array: *mut Object = msg_send![class!(NSMutableArray), array];
        for value in values {
            let _: () = msg_send![array, addObject: ns_owned_string(value)];
        }
        array
    }
}

unsafe fn active_view_controller() -> *mut Object {
    let app: *mut Object = msg_send![class!(UIApplication), sharedApplication];
    let windows: *mut Object = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    let mut window: *mut Object = ptr::null_mut();
    for index in 0..count {
        let candidate: *mut Object = msg_send![windows, objectAtIndex: index];
        let is_key: bool = msg_send![candidate, isKeyWindow];
        if is_key {
            window = candidate;
            break;
        }
    }
    if window.is_null() {
        window = msg_send![windows, firstObject];
    }
    let mut controller: *mut Object = msg_send![window, rootViewController];
    loop {
        let presented: *mut Object = msg_send![controller, presentedViewController];
        if presented.is_null() {
            return controller;
        }
        controller = presented;
    }
}

unsafe fn ns_owned_string(value: &str) -> *mut Object {
    let bytes = value.as_bytes();
    msg_send![class!(NSString), stringWithBytes: bytes.as_ptr() length: bytes.len() encoding: 4usize]
}

unsafe fn ns_string(value: *mut Object) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let pointer: *const std::os::raw::c_char = msg_send![value, UTF8String];
    if pointer.is_null() {
        None
    } else {
        Some(CStr::from_ptr(pointer).to_string_lossy().into_owned())
    }
}

unsafe fn ns_error(error: *mut Object) -> Option<String> {
    if error.is_null() {
        None
    } else {
        ns_string(msg_send![error, localizedDescription])
    }
}

struct IosFileStream {
    file: std::fs::File,
    _lease: Arc<IosSecurityScope>,
    finished: bool,
}

impl Stream for IosFileStream {
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
    FileSystemError::new("file_io_failed", error.to_string())
}

fn fs_error(code: impl Into<String>, message: impl Into<String>) -> FileSystemError {
    FileSystemError::new(code, message)
}
