use fission_core::env::Clipboard;
use fission_core::{
    collect_data_stream, single_chunk_data_stream, Bytes, ClipboardContent, ClipboardError,
    ClipboardText, ClipboardWriteTextRequest, CLEAR_CLIPBOARD, READ_CLIPBOARD_CONTENT,
    READ_CLIPBOARD_TEXT, WRITE_CLIPBOARD_CONTENT, WRITE_CLIPBOARD_TEXT,
};
use fission_shell::async_host::AsyncRegistry;
#[cfg(target_os = "ios")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "ios")]
use std::ffi::CStr;
#[cfg(target_os = "ios")]
use std::os::raw::{c_char, c_void};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct ClipboardHostItem {
    pub content_type: String,
    pub bytes: Bytes,
    pub suggested_name: Option<String>,
}

#[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
use arboard::Clipboard as Arboard;

#[cfg(target_os = "ios")]
#[link(name = "UIKit", kind = "framework")]
extern "C" {}

pub struct DesktopClipboard {
    #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
    system: Arc<Mutex<Option<Arboard>>>,
    memory: Arc<Mutex<String>>,
}

impl DesktopClipboard {
    pub fn new() -> Self {
        Self {
            #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
            system: Arc::new(Mutex::new(Arboard::new().ok())),
            memory: Arc::new(Mutex::new(String::new())),
        }
    }
}

impl Clipboard for DesktopClipboard {
    fn get_text(&self) -> Option<String> {
        #[cfg(target_os = "ios")]
        if let Some(text) = ios_clipboard_text() {
            return Some(text);
        }

        #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
        if let Ok(mut lock) = self.system.lock() {
            if let Some(cb) = lock.as_mut() {
                if let Ok(text) = cb.get_text() {
                    return Some(text);
                }
            }
        }
        self.memory.lock().ok().map(|text| text.clone())
    }

    fn set_text(&self, text: &str) {
        if let Ok(mut memory) = self.memory.lock() {
            *memory = text.to_string();
        }
        #[cfg(target_os = "ios")]
        ios_set_clipboard_text(text);

        #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
        if let Ok(mut lock) = self.system.lock() {
            if let Some(cb) = lock.as_mut() {
                let _ = cb.set_text(text);
            }
        }
    }
}

#[cfg(target_os = "ios")]
fn ios_clipboard_text() -> Option<String> {
    unsafe {
        let pasteboard: *mut objc::runtime::Object =
            msg_send![class!(UIPasteboard), generalPasteboard];
        if pasteboard.is_null() {
            return None;
        }
        let string: *mut objc::runtime::Object = msg_send![pasteboard, string];
        if string.is_null() {
            return None;
        }
        let c_string: *const c_char = msg_send![string, UTF8String];
        if c_string.is_null() {
            return None;
        }
        CStr::from_ptr(c_string)
            .to_str()
            .ok()
            .map(ToOwned::to_owned)
    }
}

#[cfg(target_os = "ios")]
fn ios_set_clipboard_text(text: &str) {
    unsafe {
        let pasteboard: *mut objc::runtime::Object =
            msg_send![class!(UIPasteboard), generalPasteboard];
        if pasteboard.is_null() {
            return;
        }
        let string: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
        let string: *mut objc::runtime::Object = msg_send![
            string,
            initWithBytes: text.as_ptr() as *const c_void
            length: text.len()
            encoding: 4usize
        ];
        let _: () = msg_send![pasteboard, setString: string];
    }
}

/// Host-side clipboard provider used by shell capability registration.
pub trait ClipboardHost: Send + Sync + 'static {
    /// Reads plain text from the host clipboard.
    fn read_text(&self) -> Result<ClipboardText, ClipboardError>;
    /// Writes plain text to the host clipboard.
    fn write_text(&self, request: ClipboardWriteTextRequest) -> Result<(), ClipboardError>;
    /// Reads typed clipboard items from the host clipboard.
    fn read_content(&self) -> Result<Vec<ClipboardHostItem>, ClipboardError>;
    /// Writes typed clipboard items to the host clipboard.
    fn write_content(&self, request: Vec<ClipboardHostItem>) -> Result<(), ClipboardError>;
    /// Clears clipboard content when the host allows apps to do that.
    fn clear(&self) -> Result<(), ClipboardError>;
}

impl ClipboardHost for DesktopClipboard {
    fn read_text(&self) -> Result<ClipboardText, ClipboardError> {
        Ok(ClipboardText {
            text: self.get_text(),
        })
    }

    fn write_text(&self, request: ClipboardWriteTextRequest) -> Result<(), ClipboardError> {
        self.set_text(&request.text);
        Ok(())
    }

    fn read_content(&self) -> Result<Vec<ClipboardHostItem>, ClipboardError> {
        let text = self.get_text().unwrap_or_default();
        Ok(if text.is_empty() {
            Vec::new()
        } else {
            vec![ClipboardHostItem {
                content_type: "text/plain".into(),
                bytes: Bytes::from(text),
                suggested_name: None,
            }]
        })
    }

    fn write_content(&self, request: Vec<ClipboardHostItem>) -> Result<(), ClipboardError> {
        if let Some(item) = request
            .iter()
            .find(|item| item.content_type.starts_with("text/plain"))
        {
            if let Ok(text) = String::from_utf8(item.bytes.to_vec()) {
                self.set_text(&text);
                return Ok(());
            }
        }
        Err(ClipboardError::unsupported("write_content_non_text"))
    }

    fn clear(&self) -> Result<(), ClipboardError> {
        self.set_text("");
        Ok(())
    }
}

/// In-process clipboard host for tests and non-OS environments.
#[derive(Debug, Default)]
pub struct MemoryClipboardHost {
    content: Arc<Mutex<Vec<ClipboardHostItem>>>,
}

impl ClipboardHost for MemoryClipboardHost {
    fn read_text(&self) -> Result<ClipboardText, ClipboardError> {
        let content = self.content.lock().map_err(|_| {
            ClipboardError::new("lock_poisoned", "memory clipboard lock was poisoned")
        })?;
        let text = content
            .iter()
            .find(|item| item.content_type.starts_with("text/plain"))
            .and_then(|item| String::from_utf8(item.bytes.to_vec()).ok());
        Ok(ClipboardText { text })
    }

    fn write_text(&self, request: ClipboardWriteTextRequest) -> Result<(), ClipboardError> {
        let mut content = self.content.lock().map_err(|_| {
            ClipboardError::new("lock_poisoned", "memory clipboard lock was poisoned")
        })?;
        *content = vec![ClipboardHostItem {
            content_type: "text/plain".into(),
            bytes: Bytes::from(request.text),
            suggested_name: None,
        }];
        Ok(())
    }

    fn read_content(&self) -> Result<Vec<ClipboardHostItem>, ClipboardError> {
        self.content
            .lock()
            .map(|content| content.clone())
            .map_err(|_| ClipboardError::new("lock_poisoned", "memory clipboard lock was poisoned"))
    }

    fn write_content(&self, request: Vec<ClipboardHostItem>) -> Result<(), ClipboardError> {
        let mut content = self.content.lock().map_err(|_| {
            ClipboardError::new("lock_poisoned", "memory clipboard lock was poisoned")
        })?;
        *content = request;
        Ok(())
    }

    fn clear(&self) -> Result<(), ClipboardError> {
        let mut content = self.content.lock().map_err(|_| {
            ClipboardError::new("lock_poisoned", "memory clipboard lock was poisoned")
        })?;
        content.clear();
        Ok(())
    }
}

pub(crate) fn register_clipboard_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn ClipboardHost>,
) {
    let read_text_host = host.clone();
    async_registry.register_operation_capability(READ_CLIPBOARD_TEXT, move |(), _| {
        let host = read_text_host.clone();
        async move { host.read_text() }
    });

    let write_text_host = host.clone();
    async_registry.register_operation_capability(WRITE_CLIPBOARD_TEXT, move |request, _| {
        let host = write_text_host.clone();
        async move { host.write_text(request) }
    });

    let read_content_host = host.clone();
    async_registry.register_operation_capability(READ_CLIPBOARD_CONTENT, move |(), ctx| {
        let host = read_content_host.clone();
        async move {
            let items = host.read_content()?;
            Ok(ClipboardContent {
                items: items
                    .into_iter()
                    .map(|item| {
                        let byte_len = item.bytes.len() as u64;
                        let stream = ctx.register_data_stream(single_chunk_data_stream(item.bytes));
                        fission_core::ClipboardItem {
                            content_type: item.content_type,
                            stream,
                            byte_len: Some(byte_len),
                            suggested_name: item.suggested_name,
                        }
                    })
                    .collect(),
            })
        }
    });

    let write_content_host = host.clone();
    async_registry.register_operation_capability(WRITE_CLIPBOARD_CONTENT, move |request, ctx| {
        let host = write_content_host.clone();
        async move {
            let mut items = Vec::with_capacity(request.items.len());
            for item in request.items {
                let stream = ctx.open_data_stream(item.stream).map_err(|error| {
                    ClipboardError::new("stream_open_failed", error.to_string())
                })?;
                let bytes = collect_data_stream(stream).await.map_err(|error| {
                    ClipboardError::new("stream_read_failed", error.to_string())
                })?;
                items.push(ClipboardHostItem {
                    content_type: item.content_type,
                    bytes,
                    suggested_name: item.suggested_name,
                });
            }
            host.write_content(items)
        }
    });

    async_registry.register_operation_capability(CLEAR_CLIPBOARD, move |(), _| {
        let host = host.clone();
        async move { host.clear() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_clipboard_reads_and_writes_text() {
        let host = MemoryClipboardHost::default();
        host.write_text(ClipboardWriteTextRequest {
            text: "copied".into(),
        })
        .unwrap();
        assert_eq!(host.read_text().unwrap().text.as_deref(), Some("copied"));
        host.clear().unwrap();
        assert_eq!(host.read_text().unwrap().text, None);
    }

    #[test]
    fn desktop_clipboard_host_supports_text_content() {
        let host = DesktopClipboard::new();
        host.write_text(ClipboardWriteTextRequest {
            text: "copied".into(),
        })
        .unwrap();
        let content = ClipboardHost::read_content(&host).unwrap();
        assert_eq!(content[0].content_type, "text/plain");
    }
}
