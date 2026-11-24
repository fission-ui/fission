use fission_core::{
    Bytes, CapabilityCtx, FileSystemError, FileSystemPermission, FileWriteSource,
    FissionDataStreamError, FissionDataStreamErrorKind, PickOpenFilesError, PickOpenFilesRequest,
    PickOpenFilesResult, PickSaveFileRequest, PickSaveFileResult, PickedFile,
    ReadPickedFileRequest, ReadPickedFileResult, ReleaseWritableFileRequest, SaveFileDestination,
    WritableFileHandle, WritableFileHandleId, WritePickedFileRequest, WritePickedFileResult,
    PICK_OPEN_FILES, PICK_SAVE_FILE, READ_PICKED_FILE, RELEASE_WRITABLE_FILE, WRITE_PICKED_FILE,
};
use fission_shell::async_host::AsyncRegistry;
use futures_core::Stream;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::ffi::CStr;
use std::fs::File;
use std::io::Read;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};

#[path = "native_document_write.rs"]
mod document_write;

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

const DEFAULT_FILE_STREAM_CHUNK_SIZE: usize = 256 * 1024;
const MAX_FILE_STREAM_CHUNK_SIZE: usize = 16 * 1024 * 1024;

pub(crate) fn register_macos_file_picker_capabilities(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        RELEASE_WRITABLE_FILE,
        |request: ReleaseWritableFileRequest, _| async move { release_writable_file(request.file) },
    );
    // Override the generic path-based open picker so macOS keeps the panel's
    // security-scoped URL alive while the selected-file stream is consumed.
    async_registry.register_operation_capability(
        PICK_OPEN_FILES,
        |request: PickOpenFilesRequest, ctx: CapabilityCtx| async move {
            pick_open_files_with(request, &ctx, show_native_open_dialog)
        },
    );
    async_registry.register_operation_capability(
        PICK_SAVE_FILE,
        |request: PickSaveFileRequest, _| async move {
            pick_save_file_with(request, show_native_save_dialog)
        },
    );
    async_registry.register_operation_capability(
        WRITE_PICKED_FILE,
        |request: WritePickedFileRequest, ctx: CapabilityCtx| async move {
            let file = picked_files()
                .lock()
                .map_err(|_| {
                    FileSystemError::new("registry_failed", "save file registry is unavailable")
                })?
                .get(&request.file.id)
                .cloned()
                .ok_or_else(|| {
                    FileSystemError::new(
                        "invalid_handle",
                        "the macOS save destination is no longer active",
                    )
                })?;
            let byte_len = write_picked_file(request.source, &ctx, file).await?;
            Ok(WritePickedFileResult { byte_len })
        },
    );
    async_registry.register_operation_capability(
        READ_PICKED_FILE,
        |request: ReadPickedFileRequest, ctx: CapabilityCtx| async move {
            read_picked_file_with(request, &ctx)
        },
    );
}

fn read_picked_file_with(
    request: ReadPickedFileRequest,
    ctx: &CapabilityCtx,
) -> Result<ReadPickedFileResult, FileSystemError> {
    let file = picked_files()
        .lock()
        .map_err(|_| FileSystemError::new("registry_failed", "save file registry is unavailable"))?
        .get(&request.file.id)
        .cloned()
        .ok_or_else(|| {
            FileSystemError::new(
                "invalid_handle",
                "the macOS save destination is no longer active",
            )
        })?;
    let access = file.scope.as_ref().map(|scope| scope.start_accessing());
    let metadata = std::fs::metadata(&file.path).map_err(|error| {
        FileSystemError::new(
            "read_failed",
            format!("failed to inspect selected file: {error}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(FileSystemError::new(
            "not_a_file",
            "the selected save destination is not a regular file",
        ));
    }
    let byte_len = metadata.len();
    drop(access);

    let requested_chunk_size = request
        .preferred_chunk_bytes
        .filter(|bytes| *bytes > 0)
        .map(|bytes| bytes as usize)
        .unwrap_or(DEFAULT_FILE_STREAM_CHUNK_SIZE);
    let stream = ctx.register_data_stream(Box::pin(MacosPickedFileDataStream::new(
        file,
        requested_chunk_size.min(MAX_FILE_STREAM_CHUNK_SIZE),
    )));
    Ok(ReadPickedFileResult {
        stream,
        byte_len: Some(byte_len),
    })
}

fn pick_open_files_with(
    request: PickOpenFilesRequest,
    ctx: &CapabilityCtx,
    select_files: impl FnOnce(
        &PickOpenFilesRequest,
    ) -> Result<Option<Vec<MacosPickedFile>>, FileSystemError>,
) -> Result<PickOpenFilesResult, PickOpenFilesError> {
    let Some(files) = select_files(&request)
        .map_err(|error| PickOpenFilesError::new(error.code, error.message))?
    else {
        return Ok(PickOpenFilesResult::default());
    };

    let requested_chunk_size = request
        .preferred_chunk_bytes
        .filter(|bytes| *bytes > 0)
        .map(|bytes| bytes as usize)
        .unwrap_or(DEFAULT_FILE_STREAM_CHUNK_SIZE);
    let chunk_size = requested_chunk_size.min(MAX_FILE_STREAM_CHUNK_SIZE);
    let mut inspected_files = Vec::with_capacity(files.len());
    for file in files {
        // The open-panel URL carries the sandbox grant. Keep access active for
        // metadata, then let the stream reactivate it for its lifetime.
        let access = file.scope.as_ref().map(|scope| scope.start_accessing());
        let metadata = std::fs::metadata(&file.path).map_err(|error| {
            PickOpenFilesError::new(
                "metadata_failed",
                format!(
                    "failed to inspect selected file `{}`: {error}",
                    file.path.display()
                ),
            )
        })?;
        drop(access);
        if !metadata.is_file() {
            return Err(PickOpenFilesError::new(
                "not_a_file",
                format!(
                    "selected path `{}` is not a regular file",
                    file.path.display()
                ),
            ));
        }
        let name = file.name.clone();
        let content_type =
            crate::file_picker::content_type_for_path(&file.path).map(str::to_string);
        inspected_files.push((file, name, content_type, metadata.len()));
    }

    let files = inspected_files
        .into_iter()
        .map(|(file, name, content_type, byte_len)| {
            let stream = ctx.register_data_stream(Box::pin(MacosPickedFileDataStream::new(
                Arc::new(file),
                chunk_size,
            )));
            PickedFile {
                name,
                content_type,
                byte_len: Some(byte_len),
                stream,
            }
        })
        .collect();

    Ok(PickOpenFilesResult { files })
}

fn show_native_open_dialog(
    request: &PickOpenFilesRequest,
) -> Result<Option<Vec<MacosPickedFile>>, FileSystemError> {
    let request = request.clone();
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    dispatch::Queue::main().exec_async(move || {
        let result = unsafe { show_native_open_dialog_on_main(&request) };
        let _ = result_tx.send(result);
    });
    result_rx.recv().map_err(|error| {
        FileSystemError::new(
            "picker_failed",
            format!("macOS open-file picker failed: {error}"),
        )
    })?
}

unsafe fn show_native_open_dialog_on_main(
    request: &PickOpenFilesRequest,
) -> Result<Option<Vec<MacosPickedFile>>, FileSystemError> {
    let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
    let result = (|| {
        let panel: *mut Object = msg_send![class!(NSOpenPanel), openPanel];
        if panel.is_null() {
            return Err(FileSystemError::new(
                "picker_unavailable",
                "macOS could not create an open panel",
            ));
        }
        let _: () = msg_send![panel, setCanChooseFiles: true];
        let _: () = msg_send![panel, setCanChooseDirectories: false];
        let _: () = msg_send![panel, setAllowsMultipleSelection: request.allow_multiple];
        let extensions =
            crate::file_picker::normalized_extensions(&request.extensions, &request.mime_types);
        if !extensions.is_empty() {
            let allowed: *mut Object = msg_send![class!(NSMutableArray), array];
            for extension in extensions {
                let value = ns_string(&extension);
                let _: () = msg_send![allowed, addObject: value];
                let _: () = msg_send![value, release];
            }
            let _: () = msg_send![panel, setAllowedFileTypes: allowed];
        }
        let response: i64 = msg_send![panel, runModal];
        if response != 1 {
            return Ok(None);
        }

        let urls: *mut Object = msg_send![panel, URLs];
        let count: usize = msg_send![urls, count];
        let mut files = Vec::with_capacity(count);
        for index in 0..count {
            let url: *mut Object = msg_send![urls, objectAtIndex: index];
            files.push(selected_file(url)?);
        }
        Ok(Some(files))
    })();
    if !pool.is_null() {
        let _: () = msg_send![pool, drain];
    }
    result
}

struct MacosPickedFileDataStream {
    file: Arc<MacosPickedFile>,
    chunk_size: usize,
    reader: Option<File>,
    access: Option<MacosSecurityAccess>,
    done: bool,
}

impl MacosPickedFileDataStream {
    fn new(file: Arc<MacosPickedFile>, chunk_size: usize) -> Self {
        Self {
            file,
            chunk_size,
            reader: None,
            access: None,
            done: false,
        }
    }

    fn finish(&mut self) {
        self.done = true;
        self.reader.take();
        self.access.take();
    }
}

impl Drop for MacosPickedFileDataStream {
    fn drop(&mut self) {
        // Close the descriptor and stop scoped access before the retained URL
        // in `file` can be released.
        self.reader.take();
        self.access.take();
    }
}

impl Stream for MacosPickedFileDataStream {
    type Item = Result<Bytes, FissionDataStreamError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        if this.done {
            return Poll::Ready(None);
        }
        if this.reader.is_none() {
            this.access = this
                .file
                .scope
                .as_ref()
                .map(|scope| scope.start_accessing());
            match File::open(&this.file.path) {
                Ok(file) => this.reader = Some(file),
                Err(error) => {
                    let message = format!(
                        "failed to open selected file `{}`: {error}",
                        this.file.path.display()
                    );
                    this.finish();
                    return Poll::Ready(Some(Err(FissionDataStreamError::new(
                        FissionDataStreamErrorKind::Io,
                        message,
                    ))));
                }
            }
        }

        let mut buffer = vec![0; this.chunk_size];
        match this
            .reader
            .as_mut()
            .expect("reader was opened above")
            .read(&mut buffer)
        {
            Ok(0) => {
                this.finish();
                Poll::Ready(None)
            }
            Ok(read) => {
                buffer.truncate(read);
                Poll::Ready(Some(Ok(Bytes::from(buffer))))
            }
            Err(error) => {
                let message = format!(
                    "failed to read selected file `{}`: {error}",
                    this.file.path.display()
                );
                this.finish();
                Poll::Ready(Some(Err(FissionDataStreamError::new(
                    FissionDataStreamErrorKind::Io,
                    message,
                ))))
            }
        }
    }
}

struct MacosPickedFile {
    path: PathBuf,
    name: String,
    scope: Option<Arc<MacosSecurityScope>>,
}

struct MacosSecurityScope {
    url: usize,
}

unsafe impl Send for MacosSecurityScope {}
unsafe impl Sync for MacosSecurityScope {}

impl Drop for MacosSecurityScope {
    fn drop(&mut self) {
        if self.url == 0 {
            return;
        }
        unsafe {
            let url = self.url as *mut Object;
            let _: () = msg_send![url, release];
        }
    }
}

impl MacosSecurityScope {
    fn start_accessing(&self) -> MacosSecurityAccess {
        let started = unsafe {
            let url = self.url as *mut Object;
            msg_send![url, startAccessingSecurityScopedResource]
        };
        MacosSecurityAccess {
            url: self.url,
            started,
        }
    }
}

struct MacosSecurityAccess {
    url: usize,
    started: bool,
}

unsafe impl Send for MacosSecurityAccess {}

impl Drop for MacosSecurityAccess {
    fn drop(&mut self) {
        if self.started && self.url != 0 {
            unsafe {
                let url = self.url as *mut Object;
                let _: () = msg_send![url, stopAccessingSecurityScopedResource];
            }
        }
    }
}

fn picked_files() -> &'static Mutex<HashMap<WritableFileHandleId, Arc<MacosPickedFile>>> {
    static FILES: OnceLock<Mutex<HashMap<WritableFileHandleId, Arc<MacosPickedFile>>>> =
        OnceLock::new();
    FILES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_file_id() -> WritableFileHandleId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    WritableFileHandleId(NEXT.fetch_add(1, Ordering::Relaxed))
}

fn release_writable_file(id: WritableFileHandleId) -> Result<(), FileSystemError> {
    // Reads and writes clone the Arc before releasing the registry lock. Removing
    // this entry prevents new operations without invalidating an active stream.
    picked_files()
        .lock()
        .map_err(|_| FileSystemError::new("registry_failed", "save file registry is unavailable"))?
        .remove(&id);
    Ok(())
}

fn pick_save_file_with(
    request: PickSaveFileRequest,
    select_destination: impl FnOnce(
        &PickSaveFileRequest,
    ) -> Result<Option<MacosPickedFile>, FileSystemError>,
) -> Result<PickSaveFileResult, FileSystemError> {
    let Some(file) = select_destination(&request)? else {
        return Ok(PickSaveFileResult { destination: None });
    };
    let id = next_file_id();
    let handle = WritableFileHandle {
        id,
        name: file.name.clone(),
        permission: FileSystemPermission::Unknown,
    };
    picked_files()
        .lock()
        .map_err(|_| FileSystemError::new("registry_failed", "save file registry is unavailable"))?
        .insert(id, Arc::new(file));
    Ok(PickSaveFileResult {
        destination: Some(SaveFileDestination::WritableFile(handle)),
    })
}

fn show_native_save_dialog(
    request: &PickSaveFileRequest,
) -> Result<Option<MacosPickedFile>, FileSystemError> {
    let request = request.clone();
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    dispatch::Queue::main().exec_async(move || {
        let result = unsafe { show_native_save_dialog_on_main(&request) };
        let _ = result_tx.send(result);
    });
    result_rx.recv().map_err(|error| {
        FileSystemError::new(
            "picker_failed",
            format!("macOS save picker failed: {error}"),
        )
    })?
}

unsafe fn show_native_save_dialog_on_main(
    request: &PickSaveFileRequest,
) -> Result<Option<MacosPickedFile>, FileSystemError> {
    let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
    let result = (|| {
        let panel: *mut Object = msg_send![class!(NSSavePanel), savePanel];
        if panel.is_null() {
            return Err(FileSystemError::new(
                "picker_unavailable",
                "macOS could not create a save panel",
            ));
        }
        let _: () = msg_send![panel, setCanCreateDirectories: true];
        if !request.suggested_name.trim().is_empty() {
            let name = ns_string(request.suggested_name.trim());
            let _: () = msg_send![panel, setNameFieldStringValue: name];
            let _: () = msg_send![name, release];
        }
        let extensions =
            crate::file_picker::normalized_extensions(&request.extensions, &request.mime_types);
        if !extensions.is_empty() {
            let allowed: *mut Object = msg_send![class!(NSMutableArray), array];
            for extension in extensions {
                let value = ns_string(&extension);
                let _: () = msg_send![allowed, addObject: value];
                let _: () = msg_send![value, release];
            }
            let _: () = msg_send![panel, setAllowedFileTypes: allowed];
        }
        let response: i64 = msg_send![panel, runModal];
        if response != 1 {
            return Ok(None);
        }
        let url: *mut Object = msg_send![panel, URL];
        selected_file(url).map(Some)
    })();
    if !pool.is_null() {
        let _: () = msg_send![pool, drain];
    }
    result
}

unsafe fn selected_file(url: *mut Object) -> Result<MacosPickedFile, FileSystemError> {
    if url.is_null() {
        return Err(FileSystemError::new(
            "picker_failed",
            "macOS returned an empty picker destination",
        ));
    }
    // URLs returned by NSOpenPanel and NSSavePanel carry the sandbox extension
    // for the chosen file. Keep the URL and start its scope around each
    // operation; reconstructing a URL later from its path loses access.
    let retained: *mut Object = msg_send![url, retain];
    let scope = Arc::new(MacosSecurityScope {
        url: retained as usize,
    });
    let path_value: *mut Object = msg_send![url, path];
    let name_value: *mut Object = msg_send![url, lastPathComponent];
    let path = ns_string_to_string(path_value).ok_or_else(|| {
        FileSystemError::new("invalid_url", "macOS returned a save URL without a path")
    })?;
    let name = ns_string_to_string(name_value).unwrap_or_else(|| "selected-file".into());
    Ok(MacosPickedFile {
        path: PathBuf::from(path),
        name,
        scope: Some(scope),
    })
}

async fn write_picked_file(
    source: FileWriteSource,
    ctx: &CapabilityCtx,
    file: Arc<MacosPickedFile>,
) -> Result<u64, FileSystemError> {
    let _access = file.scope.as_ref().map(|scope| scope.start_accessing());
    document_write::write_source(source, ctx, || {
        File::create(&file.path)
            .map_err(|error| FileSystemError::new("write_failed", error.to_string()))
    })
    .await
}

fn ns_string(value: &str) -> *mut Object {
    unsafe {
        let string: *mut Object = msg_send![class!(NSString), alloc];
        msg_send![
            string,
            initWithBytes: value.as_ptr() as *const std::ffi::c_void
            length: value.len()
            encoding: 4usize
        ]
    }
}

unsafe fn ns_string_to_string(value: *mut Object) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let pointer: *const c_char = msg_send![value, UTF8String];
    if pointer.is_null() {
        None
    } else {
        Some(CStr::from_ptr(pointer).to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::{single_chunk_data_stream, DataStreamRegistry};
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    fn temp_path() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        std::env::temp_dir().join(format!(
            "fission-macos-picked-file-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn scoped_test_file(path: &std::path::Path) -> MacosPickedFile {
        // Objective-C message sending requires a real object, even for a test
        // running outside the sandbox. A null URL is not a valid scoped grant.
        unsafe {
            let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
            let native_path = ns_string(&path.to_string_lossy());
            let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: native_path];
            let _: () = msg_send![native_path, release];
            let file = selected_file(url);
            let _: () = msg_send![pool, drain];
            file.expect("create a retained file URL for the scoped read fixture")
        }
    }

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match Box::pin(future)
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test data stream unexpectedly blocked"),
        }
    }

    #[test]
    fn registers_save_picker_and_picked_file_writer() {
        let mut registry = AsyncRegistry::new();
        register_macos_file_picker_capabilities(&mut registry);
        assert!(registry.has_operation_capability(PICK_OPEN_FILES));
        assert!(registry.has_operation_capability(PICK_SAVE_FILE));
        assert!(registry.has_operation_capability(READ_PICKED_FILE));
        assert!(registry.has_operation_capability(WRITE_PICKED_FILE));
        assert!(registry.has_operation_capability(RELEASE_WRITABLE_FILE));
    }

    #[test]
    fn open_picker_returns_a_chunked_stream_for_the_selected_file() {
        let path = temp_path();
        std::fs::write(&path, b"streamed import").unwrap();
        let ctx = CapabilityCtx::new_runtime(1, DataStreamRegistry::default());
        let result = pick_open_files_with(
            PickOpenFilesRequest {
                preferred_chunk_bytes: Some(4),
                ..Default::default()
            },
            &ctx,
            |_| {
                Ok(Some(vec![MacosPickedFile {
                    path: path.clone(),
                    name: "workspace.sqlite".into(),
                    scope: None,
                }]))
            },
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "workspace.sqlite");
        assert_eq!(result.files[0].byte_len, Some(15));

        let mut stream = ctx.open_data_stream(result.files[0].stream).unwrap();
        let mut chunks = Vec::new();
        let mut context = Context::from_waker(Waker::noop());
        loop {
            match stream.as_mut().poll_next(&mut context) {
                Poll::Ready(Some(Ok(chunk))) => chunks.push(chunk),
                Poll::Ready(Some(Err(error))) => panic!("file stream failed: {error:?}"),
                Poll::Ready(None) => break,
                Poll::Pending => panic!("native file reads complete synchronously"),
            }
        }
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            chunks.iter().map(Bytes::len).collect::<Vec<_>>(),
            [4, 4, 4, 3]
        );
        assert_eq!(
            chunks
                .iter()
                .flat_map(|chunk| chunk.iter().copied())
                .collect::<Vec<_>>(),
            b"streamed import"
        );
    }

    #[test]
    fn open_stream_holds_security_access_until_reading_finishes() {
        let path = temp_path();
        std::fs::write(&path, b"scoped read").unwrap();
        let file = Arc::new(scoped_test_file(&path));
        let mut stream = Box::pin(MacosPickedFileDataStream::new(file, 4));
        let mut context = Context::from_waker(Waker::noop());
        let first = stream.as_mut().poll_next(&mut context);
        assert!(matches!(first, Poll::Ready(Some(Ok(_)))));
        assert!(stream.as_ref().get_ref().access.is_some());

        loop {
            match stream.as_mut().poll_next(&mut context) {
                Poll::Ready(Some(Ok(_))) => {}
                Poll::Ready(Some(Err(error))) => panic!("file stream failed: {error:?}"),
                Poll::Ready(None) => break,
                Poll::Pending => panic!("native file reads complete synchronously"),
            }
        }
        assert!(stream.as_ref().get_ref().access.is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn save_picker_returns_an_opaque_writable_handle() {
        let path = temp_path();
        let request = PickSaveFileRequest {
            suggested_name: "report.pdf".into(),
            ..Default::default()
        };
        let result = pick_save_file_with(request, |_| {
            Ok(Some(MacosPickedFile {
                path: path.clone(),
                name: "report.pdf".into(),
                scope: None,
            }))
        })
        .unwrap();
        let Some(SaveFileDestination::WritableFile(handle)) = result.destination else {
            panic!("expected a writable file handle");
        };
        assert_eq!(handle.name, "report.pdf");
        let picked = picked_files().lock().unwrap().remove(&handle.id).unwrap();
        assert_eq!(picked.path, path);
    }

    #[test]
    fn save_picker_cancellation_returns_no_destination() {
        let result = pick_save_file_with(PickSaveFileRequest::default(), |_| Ok(None)).unwrap();
        assert_eq!(result.destination, None);
    }

    #[test]
    fn releasing_a_save_handle_keeps_an_open_read_alive_until_it_is_dropped() {
        let path = temp_path();
        std::fs::write(&path, b"still reading").unwrap();
        let ctx = CapabilityCtx::new_runtime(1, DataStreamRegistry::default());
        let selected = pick_save_file_with(PickSaveFileRequest::default(), |_| {
            Ok(Some(scoped_test_file(&path)))
        })
        .unwrap();
        let Some(SaveFileDestination::WritableFile(handle)) = selected.destination else {
            panic!("expected an opaque save handle");
        };
        let retained = Arc::downgrade(picked_files().lock().unwrap().get(&handle.id).unwrap());
        let request = ReadPickedFileRequest {
            file: handle.clone(),
            preferred_chunk_bytes: Some(4),
        };
        let result = read_picked_file_with(request.clone(), &ctx).unwrap();
        let mut stream = ctx.open_data_stream(result.stream).unwrap();

        release_writable_file(handle.id).unwrap();
        release_writable_file(handle.id).unwrap();
        assert!(!picked_files().lock().unwrap().contains_key(&handle.id));
        assert_eq!(
            read_picked_file_with(request, &ctx).unwrap_err().code,
            "invalid_handle"
        );
        assert!(retained.upgrade().is_some());

        let mut actual = Vec::new();
        let mut context = Context::from_waker(Waker::noop());
        loop {
            match stream.as_mut().poll_next(&mut context) {
                Poll::Ready(Some(Ok(chunk))) => actual.extend_from_slice(&chunk),
                Poll::Ready(None) => break,
                other => panic!("unexpected read result after release: {other:?}"),
            }
        }
        assert_eq!(actual, b"still reading");
        drop(stream);
        assert!(retained.upgrade().is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn picked_file_capability_streams_directly_to_the_selected_path() {
        let path = temp_path();
        let ctx = CapabilityCtx::new_runtime(1, DataStreamRegistry::default());
        let stream = ctx.register_data_stream(single_chunk_data_stream(Bytes::from_static(
            b"streamed export",
        )));
        let file = Arc::new(MacosPickedFile {
            path: path.clone(),
            name: "report.pdf".into(),
            scope: None,
        });
        let written = ready(write_picked_file(
            FileWriteSource::Stream(stream),
            &ctx,
            file,
        ))
        .unwrap();
        assert_eq!(written, 15);
        assert_eq!(std::fs::read(&path).unwrap(), b"streamed export");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn picked_save_file_round_trips_through_opaque_stream_capabilities() {
        let path = temp_path();
        let ctx = CapabilityCtx::new_runtime(1, DataStreamRegistry::default());
        let result = pick_save_file_with(
            PickSaveFileRequest {
                suggested_name: "workspace.sqlite".into(),
                ..Default::default()
            },
            |_| Ok(Some(scoped_test_file(&path))),
        )
        .unwrap();
        let Some(SaveFileDestination::WritableFile(handle)) = result.destination else {
            panic!("expected opaque save handle");
        };
        assert!(!serde_json::to_string(&handle)
            .unwrap()
            .contains(path.to_string_lossy().as_ref()));
        let input = ctx.register_data_stream(single_chunk_data_stream(Bytes::from_static(
            b"sqlite snapshot bytes",
        )));
        assert_eq!(
            ready(write_picked_file(
                FileWriteSource::Stream(input),
                &ctx,
                picked_files()
                    .lock()
                    .unwrap()
                    .get(&handle.id)
                    .unwrap()
                    .clone(),
            ))
            .unwrap(),
            21
        );

        let read = read_picked_file_with(
            ReadPickedFileRequest {
                file: handle.clone(),
                preferred_chunk_bytes: Some(5),
            },
            &ctx,
        )
        .unwrap();
        assert_eq!(read.byte_len, Some(21));
        let mut stream = ctx.open_data_stream(read.stream).unwrap();
        let mut actual = Vec::new();
        let mut context = Context::from_waker(Waker::noop());
        match stream.as_mut().poll_next(&mut context) {
            Poll::Ready(Some(Ok(chunk))) => actual.extend_from_slice(&chunk),
            Poll::Ready(Some(Err(error))) => panic!("save destination read failed: {error:?}"),
            other => panic!("unexpected first read result: {other:?}"),
        }
        loop {
            match stream.as_mut().poll_next(&mut context) {
                Poll::Ready(Some(Ok(chunk))) => actual.extend_from_slice(&chunk),
                Poll::Ready(Some(Err(error))) => panic!("save destination read failed: {error:?}"),
                Poll::Ready(None) => break,
                Poll::Pending => panic!("native file reads complete synchronously"),
            }
        }
        assert_eq!(actual, b"sqlite snapshot bytes");
        picked_files().lock().unwrap().remove(&handle.id);
        std::fs::remove_file(path).unwrap();
    }
}
