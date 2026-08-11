use fission_shell::{
    NativeSurfaceFrame, NativeSurfaceHandler, NativeSurfaceHost, PlatformSurfaceCapabilities,
};

/// Delivers generic custom embed frames to registered extensions.
#[derive(Default)]
pub(crate) struct NativeSurfaceRegistry {
    handlers: Vec<Box<dyn NativeSurfaceHandler>>,
    host_attached: bool,
}

impl NativeSurfaceRegistry {
    pub(crate) fn register<H>(&mut self, handler: H)
    where
        H: NativeSurfaceHandler + 'static,
    {
        self.handlers.push(Box::new(handler));
    }

    pub(crate) fn attach_host(&mut self, host: NativeSurfaceHost<'_>) {
        for handler in &mut self.handlers {
            handler.attach_host(host);
        }
        self.host_attached = true;
    }

    pub(crate) fn detach_host(&mut self) {
        if !self.host_attached {
            return;
        }
        for handler in &mut self.handlers {
            handler.detach_host();
        }
        self.host_attached = false;
    }

    /// Returns whether a registered native-view handler claims this payload.
    ///
    /// Registration is the shell's only proof that an opaque custom embed is
    /// backed by the `NativeSurfaceHandler` platform-view contract. Unknown
    /// payloads must not be advertised as `NativeView` frame bindings.
    pub(crate) fn claims_native_view_payload(&self, payload: &[u8]) -> bool {
        self.presenter_index(payload).is_some()
    }

    /// Identifies the handler that owns a payload in the registry's stable
    /// first-match dispatch order.
    pub(crate) fn presenter_index(&self, payload: &[u8]) -> Option<usize> {
        self.handlers
            .iter()
            .position(|handler| handler.handles_payload(payload))
    }

    /// Returns whether a claimed native view has a live host to present into.
    pub(crate) fn native_view_ready(&self, payload: &[u8]) -> bool {
        self.native_view_capabilities(payload).available
    }

    pub(crate) fn has_attached_host(&self) -> bool {
        self.host_attached
    }

    pub(crate) fn native_view_capabilities(&self, payload: &[u8]) -> PlatformSurfaceCapabilities {
        if !self.host_attached {
            return PlatformSurfaceCapabilities::UNAVAILABLE;
        }
        self.handlers
            .iter()
            .find(|handler| handler.handles_payload(payload))
            .map(|handler| handler.surface_capabilities(payload))
            .unwrap_or(PlatformSurfaceCapabilities::UNAVAILABLE)
    }

    pub(crate) fn present_surfaces(&mut self, frames: &[NativeSurfaceFrame]) {
        let mut claimed = vec![false; frames.len()];
        for handler in &mut self.handlers {
            let claimed = frames
                .iter()
                .enumerate()
                .filter_map(|(index, frame)| {
                    (!claimed[index] && handler.handles_payload(&frame.payload)).then(|| {
                        claimed[index] = true;
                        frame.clone()
                    })
                })
                .collect::<Vec<_>>();
            handler.present_surfaces(&claimed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NativeSurfaceRegistry;
    use fission_ir::WidgetId;
    use fission_render::LayoutRect;
    use fission_shell::{
        NativeSurfaceFrame, NativeSurfaceHandler, NativeSurfaceHost, PlatformSurfaceCapabilities,
    };
    use raw_window_handle::{RawWindowHandle, WindowHandle};
    use std::sync::{Arc, Mutex};

    struct RecordingHandler {
        prefix: &'static [u8],
        frames: Arc<Mutex<Vec<NativeSurfaceFrame>>>,
        detach_count: Arc<Mutex<u32>>,
    }

    impl NativeSurfaceHandler for RecordingHandler {
        fn handles_payload(&self, payload: &[u8]) -> bool {
            payload.starts_with(self.prefix)
        }

        fn surface_capabilities(&self, _payload: &[u8]) -> PlatformSurfaceCapabilities {
            PlatformSurfaceCapabilities::FULL
        }

        fn attach_host(&mut self, _host: NativeSurfaceHost<'_>) {}

        fn detach_host(&mut self) {
            *self.detach_count.lock().unwrap() += 1;
        }

        fn present_surfaces(&mut self, frames: &[NativeSurfaceFrame]) {
            *self.frames.lock().unwrap() = frames.to_vec();
        }
    }

    #[test]
    fn routes_only_claimed_surfaces_to_each_handler() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let mut registry = NativeSurfaceRegistry::default();
        registry.register(RecordingHandler {
            prefix: b"maps:",
            frames: received.clone(),
            detach_count: Arc::new(Mutex::new(0)),
        });

        registry.present_surfaces(&[
            NativeSurfaceFrame {
                widget_id: WidgetId::from_u128(1),
                rect: LayoutRect::new(0.0, 0.0, 1.0, 1.0),
                payload: b"maps:payload".to_vec(),
                visible_rect: LayoutRect::new(0.0, 0.0, 1.0, 1.0),
                transform: None,
                opacity: 1.0,
                paint_order: 0,
            },
            NativeSurfaceFrame {
                widget_id: WidgetId::from_u128(2),
                rect: LayoutRect::new(0.0, 0.0, 1.0, 1.0),
                payload: b"other:payload".to_vec(),
                visible_rect: LayoutRect::new(0.0, 0.0, 1.0, 1.0),
                transform: None,
                opacity: 1.0,
                paint_order: 1,
            },
        ]);

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].widget_id, WidgetId::from_u128(1));
    }

    #[test]
    fn only_registered_handler_payloads_are_proven_native_views() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let mut registry = NativeSurfaceRegistry::default();
        assert!(!registry.claims_native_view_payload(b"maps:payload"));

        registry.register(RecordingHandler {
            prefix: b"maps:",
            frames: received,
            detach_count: Arc::new(Mutex::new(0)),
        });

        assert!(registry.claims_native_view_payload(b"maps:payload"));
        assert!(!registry.claims_native_view_payload(b"other:payload"));
        assert!(!registry.native_view_ready(b"maps:payload"));
        assert!(!registry.has_attached_host());

        let raw = RawWindowHandle::Web(raw_window_handle::WebWindowHandle::new(0));
        let handle = unsafe { WindowHandle::borrow_raw(raw) };
        registry.attach_host(NativeSurfaceHost::from_window_handle(handle));
        assert!(registry.has_attached_host());
        assert!(registry.native_view_ready(b"maps:payload"));
    }

    #[test]
    fn accepts_a_host_handle_without_retaining_it() {
        let mut registry = NativeSurfaceRegistry::default();
        registry.register(RecordingHandler {
            prefix: b"maps:",
            frames: Arc::new(Mutex::new(Vec::new())),
            detach_count: Arc::new(Mutex::new(0)),
        });
        let raw = RawWindowHandle::Web(raw_window_handle::WebWindowHandle::new(0));
        let handle = unsafe { WindowHandle::borrow_raw(raw) };
        registry.attach_host(NativeSurfaceHost::from_window_handle(handle));
    }

    #[test]
    fn gives_overlapping_payloads_to_the_first_registered_handler() {
        let first = Arc::new(Mutex::new(Vec::new()));
        let second = Arc::new(Mutex::new(Vec::new()));
        let mut registry = NativeSurfaceRegistry::default();
        registry.register(RecordingHandler {
            prefix: b"maps:",
            frames: first.clone(),
            detach_count: Arc::new(Mutex::new(0)),
        });
        registry.register(RecordingHandler {
            prefix: b"maps:",
            frames: second.clone(),
            detach_count: Arc::new(Mutex::new(0)),
        });

        registry.present_surfaces(&[NativeSurfaceFrame {
            widget_id: WidgetId::from_u128(1),
            rect: LayoutRect::new(0.0, 0.0, 1.0, 1.0),
            payload: b"maps:payload".to_vec(),
            visible_rect: LayoutRect::new(0.0, 0.0, 1.0, 1.0),
            transform: None,
            opacity: 1.0,
            paint_order: 0,
        }]);

        assert_eq!(first.lock().unwrap().len(), 1);
        assert!(second.lock().unwrap().is_empty());
    }

    #[test]
    fn detach_host_notifies_all_handlers() {
        let detach_count = Arc::new(Mutex::new(0u32));
        let mut registry = NativeSurfaceRegistry::default();
        registry.register(RecordingHandler {
            prefix: b"maps:",
            frames: Arc::new(Mutex::new(Vec::new())),
            detach_count: detach_count.clone(),
        });

        let raw = RawWindowHandle::Web(raw_window_handle::WebWindowHandle::new(0));
        let handle = unsafe { WindowHandle::borrow_raw(raw) };
        registry.attach_host(NativeSurfaceHost::from_window_handle(handle));
        registry.detach_host();

        assert_eq!(*detach_count.lock().unwrap(), 1);
    }
}
