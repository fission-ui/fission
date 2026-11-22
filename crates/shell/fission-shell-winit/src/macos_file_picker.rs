use fission_core::{
    Bytes, CapabilityCtx, FileSystemError, FileSystemPermission, FileWriteSource,
    PickSaveFileRequest, PickSaveFileResult, ReleaseWritableFileRequest, SaveFileDestination,
    WritableFileHandle, WritableFileHandleId, WritePickedFileRequest, WritePickedFileResult,
    PICK_SAVE_FILE, RELEASE_WRITABLE_FILE, WRITE_PICKED_FILE,
};
use fission_shell::async_host::AsyncRegistry;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::ffi::CStr;
use std::fs::File;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[path = "native_document_write.rs"]
mod document_write;

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

pub(crate) fn register_macos_file_picker_capabilities(async_registry: &mut AsyncRegistry) {
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
        RELEASE_WRITABLE_FILE,
        |request: ReleaseWritableFileRequest, _| async move {
            picked_files()
                .lock()
                .map_err(|_| {
                    FileSystemError::new("registry_failed", "save file registry is unavailable")
                })?
                .remove(&request.file);
            Ok(())
        },
    );
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
            "macOS returned an empty save destination",
        ));
    }
    // URLs returned by NSSavePanel carry the sandbox extension for the chosen
    // destination. Keep the URL with the opaque handle and start its scope
    // around each write; reconstructing a URL later from its path loses access.
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
    fn registers_save_picker_writer_and_release_capabilities() {
        let mut registry = AsyncRegistry::new();
        register_macos_file_picker_capabilities(&mut registry);
        assert!(registry.has_operation_capability(PICK_SAVE_FILE));
        assert!(registry.has_operation_capability(WRITE_PICKED_FILE));
        assert!(registry.has_operation_capability(RELEASE_WRITABLE_FILE));
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
}
