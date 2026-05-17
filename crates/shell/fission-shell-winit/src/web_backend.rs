#![allow(unexpected_cfgs)]

use fission_ir::WidgetNodeId;
use fission_render::LayoutRect;

#[derive(Clone, Debug)]
pub struct WebSurfaceFrame {
    pub widget_id: WidgetNodeId,
    pub url: String,
    pub user_agent: Option<String>,
    pub rect: LayoutRect,
}

#[cfg(target_os = "macos")]
pub use mac::MacWebBackend;

#[cfg(not(target_os = "macos"))]
pub use mock::MockWebBackend;

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
mod mac {
    use super::WebSurfaceFrame;
    use cocoa::appkit::NSWindowOrderingMode;
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::NSString;
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use fission_ir::WidgetNodeId;
    use fission_render::LayoutRect;
    use objc::rc::StrongPtr;
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;
    use winit::window::Window;

    #[link(name = "WebKit", kind = "framework")]
    extern "C" {}

    struct RetainedId(StrongPtr);

    unsafe impl Send for RetainedId {}
    unsafe impl Sync for RetainedId {}

    impl RetainedId {
        unsafe fn new_owned(ptr: id) -> Self {
            Self(StrongPtr::new(ptr))
        }

        unsafe fn retain(ptr: id) -> Self {
            Self(StrongPtr::retain(ptr))
        }

        fn as_id(&self) -> id {
            *self.0
        }
    }

    struct ViewContext {
        parent_view: id,
        bounds_height: f64,
    }

    pub struct MacWebBackend {
        view: RetainedId,
        views: Mutex<HashMap<WidgetNodeId, WebViewEntry>>,
    }

    impl MacWebBackend {
        pub fn new(window: &Window) -> Self {
            let ns_view = ns_view_from_window(window);
            Self {
                view: unsafe { RetainedId::retain(ns_view) },
                views: Mutex::new(HashMap::new()),
            }
        }

        pub fn present_surfaces(&self, frames: &[WebSurfaceFrame]) {
            let mut views = self.views.lock().unwrap();
            if frames.is_empty() {
                for view in views.values() {
                    view.detach();
                }
                views.clear();
                return;
            }

            let ctx = self.context();
            let mut seen = HashSet::new();
            for frame in frames {
                seen.insert(frame.widget_id);
                let entry = views
                    .entry(frame.widget_id)
                    .or_insert_with(|| WebViewEntry::new(&ctx, frame));
                entry.update(&ctx, frame);
            }

            views.retain(|widget_id, entry| {
                if seen.contains(widget_id) {
                    true
                } else {
                    entry.detach();
                    false
                }
            });
        }

        fn context(&self) -> ViewContext {
            unsafe {
                let parent_view = self.view.as_id();
                let bounds: CGRect = msg_send![parent_view, bounds];
                ViewContext {
                    parent_view,
                    bounds_height: bounds.size.height,
                }
            }
        }
    }

    impl Drop for MacWebBackend {
        fn drop(&mut self) {
            if let Ok(mut views) = self.views.lock() {
                for view in views.values() {
                    view.detach();
                }
                views.clear();
            }
        }
    }

    struct WebViewEntry {
        web_view: RetainedId,
        current_url: Option<String>,
        current_user_agent: Option<String>,
    }

    impl WebViewEntry {
        fn new(ctx: &ViewContext, frame: &WebSurfaceFrame) -> Self {
            unsafe {
                let cg_rect = cg_rect_from_layout(frame.rect, ctx.bounds_height);
                let config: id = msg_send![class!(WKWebViewConfiguration), new];
                let config = RetainedId::new_owned(config);
                let web_view_alloc: id = msg_send![class!(WKWebView), alloc];
                let web_view: id =
                    msg_send![web_view_alloc, initWithFrame: cg_rect configuration: config.as_id()];
                let web_view = RetainedId::new_owned(web_view);
                let () = msg_send![web_view.as_id(), setWantsLayer: YES];
                let web_layer: id = msg_send![web_view.as_id(), layer];
                if web_layer != nil {
                    let () = msg_send![web_layer, setZPosition: 2.0f64];
                }
                let () = msg_send![web_view.as_id(), setAllowsBackForwardNavigationGestures: YES];
                let () = msg_send![
                    ctx.parent_view,
                    addSubview: web_view.as_id()
                    positioned: NSWindowOrderingMode::NSWindowAbove
                    relativeTo: nil
                ];

                let mut entry = Self {
                    web_view,
                    current_url: None,
                    current_user_agent: None,
                };
                entry.update(ctx, frame);
                entry
            }
        }

        fn update(&mut self, ctx: &ViewContext, frame: &WebSurfaceFrame) {
            unsafe {
                let web_view = self.web_view.as_id();
                let cg_rect = cg_rect_from_layout(frame.rect, ctx.bounds_height);
                let () = msg_send![web_view, setFrame: cg_rect];
                let () = msg_send![web_view, setHidden: false];
                let () = msg_send![
                    ctx.parent_view,
                    addSubview: web_view
                    positioned: NSWindowOrderingMode::NSWindowAbove
                    relativeTo: nil
                ];

                if self.current_user_agent != frame.user_agent {
                    match frame.user_agent.as_deref() {
                        Some(agent) => {
                            let ns_agent = NSString::alloc(nil).init_str(agent);
                            let () = msg_send![web_view, setCustomUserAgent: ns_agent];
                        }
                        None => {
                            let () = msg_send![web_view, setCustomUserAgent: nil];
                        }
                    }
                    self.current_user_agent = frame.user_agent.clone();
                }

                if self.current_url.as_deref() != Some(frame.url.as_str()) {
                    load_url(web_view, &frame.url);
                    self.current_url = Some(frame.url.clone());
                }
            }
        }

        fn detach(&self) {
            unsafe {
                let () = msg_send![self.web_view.as_id(), stopLoading];
                let () = msg_send![self.web_view.as_id(), removeFromSuperview];
            }
        }
    }

    fn ns_view_from_window(window: &Window) -> id {
        let handle = window
            .window_handle()
            .expect("window handle unavailable on macOS");
        match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as id,
            other => panic!("expected AppKit window handle, got {other:?}"),
        }
    }

    unsafe fn load_url(web_view: id, url: &str) {
        let ns_url_string = NSString::alloc(nil).init_str(url);
        let ns_url: id = msg_send![class!(NSURL), URLWithString: ns_url_string];
        if ns_url == nil {
            return;
        }
        let request: id = msg_send![class!(NSURLRequest), requestWithURL: ns_url];
        let _: id = msg_send![web_view, loadRequest: request];
    }

    fn cg_rect_from_layout(rect: LayoutRect, bounds_height: f64) -> CGRect {
        let width = rect.size.width as f64;
        let height = rect.size.height as f64;
        let x = rect.origin.x as f64;
        let y = rect.origin.y as f64;
        let flipped_y = bounds_height - height - y;
        CGRect::new(&CGPoint::new(x, flipped_y), &CGSize::new(width, height))
    }
}

#[cfg(not(target_os = "macos"))]
mod mock {
    use super::WebSurfaceFrame;

    pub struct MockWebBackend;

    impl MockWebBackend {
        pub fn new() -> Self {
            Self
        }

        pub fn present_surfaces(&self, _frames: &[WebSurfaceFrame]) {}
    }
}
