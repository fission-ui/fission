use fission_core::env::Clipboard;
use fission_core::{
    ClipboardContent, ClipboardError, ClipboardText, ClipboardWriteTextRequest, CLEAR_CLIPBOARD,
    READ_CLIPBOARD_CONTENT, READ_CLIPBOARD_TEXT, WRITE_CLIPBOARD_CONTENT, WRITE_CLIPBOARD_TEXT,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::{Arc, Mutex};

#[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
use arboard::Clipboard as Arboard;

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
        #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
        if let Ok(mut lock) = self.system.lock() {
            if let Some(cb) = lock.as_mut() {
                let _ = cb.set_text(text);
            }
        }
    }
}

/// Host-side clipboard provider used by shell capability registration.
pub trait ClipboardHost: Send + Sync + 'static {
    /// Reads plain text from the host clipboard.
    fn read_text(&self) -> Result<ClipboardText, ClipboardError>;
    /// Writes plain text to the host clipboard.
    fn write_text(&self, request: ClipboardWriteTextRequest) -> Result<(), ClipboardError>;
    /// Reads typed clipboard items from the host clipboard.
    fn read_content(&self) -> Result<ClipboardContent, ClipboardError>;
    /// Writes typed clipboard items to the host clipboard.
    fn write_content(&self, request: ClipboardContent) -> Result<(), ClipboardError>;
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

    fn read_content(&self) -> Result<ClipboardContent, ClipboardError> {
        let text = self.get_text().unwrap_or_default();
        Ok(ClipboardContent {
            items: if text.is_empty() {
                Vec::new()
            } else {
                vec![fission_core::ClipboardItem {
                    content_type: "text/plain".into(),
                    bytes: text.into_bytes(),
                    suggested_name: None,
                }]
            },
        })
    }

    fn write_content(&self, request: ClipboardContent) -> Result<(), ClipboardError> {
        if let Some(item) = request
            .items
            .iter()
            .find(|item| item.content_type.starts_with("text/plain"))
        {
            if let Ok(text) = String::from_utf8(item.bytes.clone()) {
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
    content: Arc<Mutex<ClipboardContent>>,
}

impl ClipboardHost for MemoryClipboardHost {
    fn read_text(&self) -> Result<ClipboardText, ClipboardError> {
        let content = self.content.lock().map_err(|_| {
            ClipboardError::new("lock_poisoned", "memory clipboard lock was poisoned")
        })?;
        let text = content
            .items
            .iter()
            .find(|item| item.content_type.starts_with("text/plain"))
            .and_then(|item| String::from_utf8(item.bytes.clone()).ok());
        Ok(ClipboardText { text })
    }

    fn write_text(&self, request: ClipboardWriteTextRequest) -> Result<(), ClipboardError> {
        let mut content = self.content.lock().map_err(|_| {
            ClipboardError::new("lock_poisoned", "memory clipboard lock was poisoned")
        })?;
        *content = ClipboardContent {
            items: vec![fission_core::ClipboardItem {
                content_type: "text/plain".into(),
                bytes: request.text.into_bytes(),
                suggested_name: None,
            }],
        };
        Ok(())
    }

    fn read_content(&self) -> Result<ClipboardContent, ClipboardError> {
        self.content
            .lock()
            .map(|content| content.clone())
            .map_err(|_| ClipboardError::new("lock_poisoned", "memory clipboard lock was poisoned"))
    }

    fn write_content(&self, request: ClipboardContent) -> Result<(), ClipboardError> {
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
        content.items.clear();
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
    async_registry.register_operation_capability(READ_CLIPBOARD_CONTENT, move |(), _| {
        let host = read_content_host.clone();
        async move { host.read_content() }
    });

    let write_content_host = host.clone();
    async_registry.register_operation_capability(WRITE_CLIPBOARD_CONTENT, move |request, _| {
        let host = write_content_host.clone();
        async move { host.write_content(request) }
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
        assert_eq!(content.items[0].content_type, "text/plain");
    }
}
