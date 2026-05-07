use fission_core::env::Clipboard;
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
