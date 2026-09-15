use crate::file_system::{
    register_native_file_system_operations, NativeDirectoryGrant, NativeDirectoryRegistry,
};
use dispatch::Queue;
use fission_core::{
    FileSystemError, ForgetDirectoryRequest, PickDirectoryRequest, PickDirectoryResult,
    RestoreDirectoryRequest, RestoreDirectoryResult, FORGET_DIRECTORY, PICK_DIRECTORY,
    RESTORE_DIRECTORY,
};
use fission_shell::async_host::AsyncRegistry;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Protocol, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::future::Future;
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
const PERSISTENCE_PREFIX: &str = "rs.fission.runtime.filesystem.";

pub(crate) fn register_ios_file_system_capabilities(async_registry: &mut AsyncRegistry) {
    let registry = Arc::new(NativeDirectoryRegistry::default());

    let grants = registry.clone();
    async_registry.register_operation_capability(
        PICK_DIRECTORY,
        move |request: PickDirectoryRequest, _| {
            let grants = grants.clone();
            async move {
                let picked = pick_directory(request.persistence_key.clone()).await?;
                let directory = picked.map(|picked| {
                    grants.insert(NativeDirectoryGrant {
                        root: picked.path,
                        name: picked.name,
                        access: request.access,
                        _lease: Some(picked.lease),
                    })
                });
                Ok(PickDirectoryResult { directory })
            }
        },
    );

    let grants = registry.clone();
    async_registry.register_operation_capability(
        RESTORE_DIRECTORY,
        move |request: RestoreDirectoryRequest, _| {
            let grants = grants.clone();
            async move {
                let Some(picked) = restore_directory(&request.persistence_key)? else {
                    return Ok(RestoreDirectoryResult { directory: None });
                };
                let directory = grants.insert(NativeDirectoryGrant {
                    root: picked.path,
                    name: picked.name,
                    access: request.access,
                    _lease: Some(picked.lease),
                });
                Ok(RestoreDirectoryResult {
                    directory: Some(directory),
                })
            }
        },
    );

    async_registry.register_operation_capability(
        FORGET_DIRECTORY,
        move |request: ForgetDirectoryRequest, _| async move {
            forget_directory(&request.persistence_key);
            Ok::<(), FileSystemError>(())
        },
    );

    register_native_file_system_operations(async_registry, registry);
}

struct IosPickedDirectory {
    path: PathBuf,
    name: String,
    lease: Arc<dyn Send + Sync>,
}

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

struct PickerState {
    result: Mutex<Option<Result<Option<IosPickedDirectory>, FileSystemError>>>,
    waker: Mutex<Option<Waker>>,
    persistence_key: Option<String>,
    delegate: Mutex<Option<usize>>,
}

struct PickerFuture(Arc<PickerState>);

impl Future for PickerFuture {
    type Output = Result<Option<IosPickedDirectory>, FileSystemError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.0.result.lock().unwrap().take() {
            Poll::Ready(result)
        } else {
            *self.0.waker.lock().unwrap() = Some(cx.waker().clone());
            // The delegate can complete between the first result check and
            // storing the waker. Recheck so that race cannot lose its wakeup.
            self.0
                .result
                .lock()
                .unwrap()
                .take()
                .map_or(Poll::Pending, Poll::Ready)
        }
    }
}

fn picker_states() -> &'static Mutex<HashMap<u64, Arc<PickerState>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<PickerState>>>> = OnceLock::new();
    STATES.get_or_init(Default::default)
}

fn pick_directory(persistence_key: Option<String>) -> PickerFuture {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let state = Arc::new(PickerState {
        result: Mutex::new(None),
        waker: Mutex::new(None),
        persistence_key,
        delegate: Mutex::new(None),
    });
    picker_states().lock().unwrap().insert(id, state.clone());
    let state_for_ui = state.clone();
    Queue::main().exec_async(move || unsafe {
        if let Err(error) = present_picker(id, &state_for_ui) {
            picker_states().lock().unwrap().remove(&id);
            complete_picker(&state_for_ui, Err(error));
        }
    });
    PickerFuture(state)
}

unsafe fn present_picker(id: u64, state: &Arc<PickerState>) -> Result<(), FileSystemError> {
    let folder = ns_owned_string("public.folder");
    let types: *mut Object = msg_send![class!(NSArray), arrayWithObject: folder];
    let picker: *mut Object = msg_send![class!(UIDocumentPickerViewController), alloc];
    // UIDocumentPickerModeOpen keeps the selected directory in place and
    // yields a security-scoped URL instead of importing a copy.
    let picker: *mut Object = msg_send![picker, initWithDocumentTypes: types inMode: 1i64];
    if picker.is_null() {
        return Err(fs_error(
            "picker_unavailable",
            "iOS could not create a folder picker",
        ));
    }
    let delegate: *mut Object = msg_send![picker_delegate_class(), new];
    (*delegate).set_ivar("_requestId", id as usize);
    *state.delegate.lock().unwrap() = Some(delegate as usize);
    let _: () = msg_send![picker, setDelegate: delegate];
    let _: () = msg_send![picker, setAllowsMultipleSelection: false];
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
        let mut decl = ClassDecl::new("FissionDirectoryPickerDelegate", class!(NSObject))
            .expect("register FissionDirectoryPickerDelegate");
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
        let url: *mut Object = msg_send![urls, firstObject];
        let result = selected_url(url, state.persistence_key.as_deref()).map(Some);
        complete_picker(&state, result);
        release_delegate_after_callback(&state);
    }
}

extern "C" fn picker_did_cancel(this: &mut Object, _cmd: Sel, _picker: *mut Object) {
    unsafe {
        let id = *this.get_ivar::<usize>("_requestId") as u64;
        let state = picker_states().lock().unwrap().remove(&id);
        if let Some(state) = state {
            complete_picker(&state, Ok(None));
            release_delegate_after_callback(&state);
        }
    }
}

fn complete_picker(
    state: &PickerState,
    result: Result<Option<IosPickedDirectory>, FileSystemError>,
) {
    *state.result.lock().unwrap() = Some(result);
    if let Some(waker) = state.waker.lock().unwrap().take() {
        waker.wake();
    }
}

unsafe fn release_delegate_after_callback(state: &PickerState) {
    if let Some(delegate) = state.delegate.lock().unwrap().take() {
        Queue::main().exec_async(move || {
            let delegate = delegate as *mut Object;
            let _: () = unsafe { msg_send![delegate, release] };
        });
    }
}

unsafe fn selected_url(
    url: *mut Object,
    persistence_key: Option<&str>,
) -> Result<IosPickedDirectory, FileSystemError> {
    if url.is_null() {
        return Err(fs_error("picker_failed", "iOS returned no selected folder"));
    }
    let accessed: bool = msg_send![url, startAccessingSecurityScopedResource];
    if !accessed {
        return Err(fs_error(
            "permission_denied",
            "iOS did not grant access to the selected folder",
        ));
    }
    let retained: *mut Object = msg_send![url, retain];
    let lease: Arc<dyn Send + Sync> = Arc::new(IosSecurityScope {
        url: retained as usize,
    });
    if let Some(key) = persistence_key {
        save_bookmark(url, key)?;
    }
    directory_from_url(url, lease)
}

unsafe fn directory_from_url(
    url: *mut Object,
    lease: Arc<dyn Send + Sync>,
) -> Result<IosPickedDirectory, FileSystemError> {
    let path_value: *mut Object = msg_send![url, path];
    let name_value: *mut Object = msg_send![url, lastPathComponent];
    let path = ns_string(path_value)
        .ok_or_else(|| fs_error("invalid_url", "iOS returned a folder without a local path"))?;
    let name = ns_string(name_value).unwrap_or_else(|| "selected-folder".into());
    Ok(IosPickedDirectory {
        path: PathBuf::from(path),
        name,
        lease,
    })
}

fn restore_directory(key: &str) -> Result<Option<IosPickedDirectory>, FileSystemError> {
    unsafe {
        let defaults: *mut Object = msg_send![class!(NSUserDefaults), standardUserDefaults];
        let data: *mut Object =
            msg_send![defaults, dataForKey: ns_owned_string(&persistence_key(key))];
        if data.is_null() {
            return Ok(None);
        }
        let mut stale = false;
        let mut error: *mut Object = ptr::null_mut();
        let url: *mut Object = msg_send![class!(NSURL), URLByResolvingBookmarkData: data options: 0usize relativeToURL: ptr::null::<Object>() bookmarkDataIsStale: &mut stale error: &mut error];
        if url.is_null() {
            return Err(fs_error(
                "restore_failed",
                ns_error(error).unwrap_or_else(|| "iOS could not restore the folder grant".into()),
            ));
        }
        if stale {
            save_bookmark(url, key)?;
        }
        selected_url(url, None).map(Some)
    }
}

unsafe fn save_bookmark(url: *mut Object, key: &str) -> Result<(), FileSystemError> {
    let mut error: *mut Object = ptr::null_mut();
    let data: *mut Object = msg_send![url, bookmarkDataWithOptions: 0usize includingResourceValuesForKeys: ptr::null::<Object>() relativeToURL: ptr::null::<Object>() error: &mut error];
    if data.is_null() {
        return Err(fs_error(
            "persistence_failed",
            ns_error(error).unwrap_or_else(|| "iOS could not persist the folder grant".into()),
        ));
    }
    let defaults: *mut Object = msg_send![class!(NSUserDefaults), standardUserDefaults];
    let _: () = msg_send![defaults, setObject: data forKey: ns_owned_string(&persistence_key(key))];
    Ok(())
}

fn forget_directory(key: &str) {
    unsafe {
        let defaults: *mut Object = msg_send![class!(NSUserDefaults), standardUserDefaults];
        let _: () = msg_send![defaults, removeObjectForKey: ns_owned_string(&persistence_key(key))];
    }
}

fn persistence_key(key: &str) -> String {
    format!("{PERSISTENCE_PREFIX}{key}")
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
        Some(
            std::ffi::CStr::from_ptr(pointer)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

unsafe fn ns_error(error: *mut Object) -> Option<String> {
    if error.is_null() {
        None
    } else {
        ns_string(msg_send![error, localizedDescription])
    }
}

fn fs_error(code: impl Into<String>, message: impl Into<String>) -> FileSystemError {
    FileSystemError::new(code, message)
}
