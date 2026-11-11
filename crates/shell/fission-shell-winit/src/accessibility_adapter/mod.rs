// Adapted from AccessKit's winit adapter, Apache-2.0, Copyright 2022 The AccessKit Authors.
// Kept inside this package so its adapter and shell use the same Fission winit dependency.

use accesskit::{ActionHandler, ActivationHandler, DeactivationHandler, TreeUpdate};
use winit::{event::WindowEvent, event_loop::ActiveEventLoop, window::Window};

mod platform_impl;

pub(crate) struct Adapter {
    inner: platform_impl::Adapter,
}

impl Adapter {
    pub(crate) fn with_direct_handlers(
        event_loop: &ActiveEventLoop,
        window: &Window,
        activation_handler: impl 'static + ActivationHandler + Send,
        action_handler: impl 'static + ActionHandler + Send,
        deactivation_handler: impl 'static + DeactivationHandler + Send,
    ) -> Self {
        if window.is_visible() == Some(true) {
            panic!(
                "The AccessKit winit adapter must be created before the window is shown (made visible) for the first time."
            );
        }

        Self {
            inner: platform_impl::Adapter::new(
                event_loop,
                window,
                activation_handler,
                action_handler,
                deactivation_handler,
            ),
        }
    }

    pub(crate) fn process_event(&mut self, window: &Window, event: &WindowEvent) {
        self.inner.process_event(window, event);
    }

    pub(crate) fn update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate) {
        self.inner.update_if_active(updater);
    }
}
