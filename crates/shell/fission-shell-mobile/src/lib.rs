use anyhow::Result;
use fission_core::{AppState, Env, Widget};
use fission_shell_desktop::DesktopApp;

#[cfg(target_os = "android")]
pub use winit::platform::android::activity::AndroidApp;

pub struct MobileApp<S: AppState, W: Widget<S>> {
    inner: DesktopApp<S, W>,
}

impl<S: AppState + Default, W: Widget<S> + 'static> MobileApp<S, W> {
    pub fn new(root_widget: W) -> Self {
        Self {
            inner: DesktopApp::new(root_widget),
        }
    }

    pub fn with_key_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut S, &fission_core::KeyCode, u8) -> bool + Send + Sync + 'static,
    {
        self.inner = self.inner.with_key_handler(handler);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.inner = self.inner.with_title(title);
        self
    }

    pub fn with_state_init<F>(mut self, init: F) -> Self
    where
        F: FnOnce(&mut S),
    {
        self.inner = self.inner.with_state_init(init);
        self
    }

    pub fn with_sync_env<F>(mut self, sync: F) -> Self
    where
        F: Fn(&S, &mut Env) + Send + Sync + 'static,
    {
        self.inner = self.inner.with_sync_env(sync);
        self
    }

    pub fn with_frame_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&mut S) -> bool + Send + Sync + 'static,
    {
        self.inner = self.inner.with_frame_hook(hook);
        self
    }

    pub fn run(self) -> Result<()> {
        self.inner.run()
    }

    #[cfg(target_os = "android")]
    pub fn run_with_android_app(self, app: AndroidApp) -> Result<()> {
        self.inner.run_with_android_app(app)
    }
}
