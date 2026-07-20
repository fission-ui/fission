#![allow(unexpected_cfgs)]

use fission_shell::{MapBackend, MapController};
use std::sync::Arc;
use winit::window::Window;

pub fn create_map_backend(window: Option<&Window>) -> Arc<dyn MapBackend> {
    #[cfg(target_os = "macos")]
    if let Some(window) = window {
        if let Some(backend) = mac::MacMapBackend::try_new(window) {
            return Arc::new(backend);
        }
    }

    #[cfg(target_os = "ios")]
    if let Some(window) = window {
        if let Some(backend) = ios::IosMapBackend::try_new(window) {
            return Arc::new(backend);
        }
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    panic!("Fission Map requires a native window on this target");

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let _ = window;
        Arc::new(NoopMapBackend)
    }
}

/// Fallback backend for platforms without native map support.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
struct NoopMapBackend;

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
impl MapBackend for NoopMapBackend {
    fn create_controller(
        &self,
        _widget_id: fission_ir::WidgetId,
        _center: (f64, f64),
        _zoom: f32,
    ) -> Box<dyn MapController> {
        panic!("Map is not supported on this platform");
    }
    fn present_surfaces(&self, _frames: &[fission_shell::MapSurfaceFrame]) {}
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
mod mac {
    use super::{MapBackend, MapController};
    use cocoa::appkit::NSWindowOrderingMode;
    use cocoa::base::{id, nil, NO, YES};
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use fission_ir::WidgetId;
    use fission_render::LayoutRect;
    use fission_shell::MapSurfaceFrame;
    use objc::rc::StrongPtr;
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};
    use winit::window::Window;

    // Link MapKit framework
    #[link(name = "MapKit", kind = "framework")]
    extern "C" {}

    // CLLocationCoordinate2D
    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct CLLocationCoordinate2D {
        latitude: f64,
        longitude: f64,
    }

    // MKCoordinateSpan
    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct MKCoordinateSpan {
        latitude_delta: f64,
        longitude_delta: f64,
    }

    // MKCoordinateRegion
    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct MKCoordinateRegion {
        center: CLLocationCoordinate2D,
        span: MKCoordinateSpan,
    }

    /// Converts a zoom level (0-20ish) to an MKCoordinateSpan delta.
    /// Zoom 0 = 360° (whole world), zoom 20 ≈ ~0.0003° (building level).
    fn zoom_to_span_delta(zoom: f32) -> f64 {
        360.0 / (2.0_f64).powf(zoom as f64)
    }

    #[derive(Clone)]
    struct RetainedId(StrongPtr);

    unsafe impl Send for RetainedId {}
    unsafe impl Sync for RetainedId {}

    impl RetainedId {
        unsafe fn new(ptr: id) -> Self {
            Self(StrongPtr::retain(ptr))
        }

        unsafe fn owned(ptr: id) -> Self {
            Self(StrongPtr::new(ptr))
        }

        fn as_id(&self) -> id {
            *self.0
        }
    }

    struct LayerContext {
        parent_view: id,
        bounds_height: f64,
    }

    pub struct MacMapBackend {
        view: Option<RetainedId>,
        layers: Mutex<HashMap<WidgetId, MapLayer>>,
        registry: Arc<MapRegistry>,
    }

    impl MacMapBackend {
        pub fn try_new(window: &Window) -> Option<Self> {
            let ns_view = ns_view_from_window(window)?;
            Some(Self {
                view: Some(unsafe { RetainedId::new(ns_view) }),
                layers: Mutex::new(HashMap::new()),
                registry: Arc::new(MapRegistry::default()),
            })
        }

        fn ensure_layer_backing(&self) -> Option<LayerContext> {
            unsafe {
                let view = self.view.as_ref()?.as_id();
                let wants_layer: bool = msg_send![view, wantsLayer];
                if !wants_layer {
                    let () = msg_send![view, setWantsLayer: YES];
                }
                let mut layer: id = msg_send![view, layer];
                if layer == nil {
                    layer = msg_send![class!(CALayer), layer];
                    let () = msg_send![view, setLayer: layer];
                }

                let window: id = msg_send![view, window];
                let scale: f64 = if window != nil {
                    msg_send![window, backingScaleFactor]
                } else {
                    1.0
                };
                let () = msg_send![layer, setContentsScale: scale];

                let bounds: CGRect = msg_send![view, bounds];

                Some(LayerContext {
                    parent_view: view,
                    bounds_height: bounds.size.height,
                })
            }
        }

        fn update_map_layer(
            &self,
            layers: &mut HashMap<WidgetId, MapLayer>,
            frame: &MapSurfaceFrame,
            ctx: &LayerContext,
        ) -> bool {
            let Some(map_view) = self.registry.get(frame.widget_id) else {
                return false;
            };
            let layer = layers
                .entry(frame.widget_id)
                .or_insert_with(|| MapLayer::new(&map_view, ctx));
            layer.update(&map_view, ctx, frame.rect);
            true
        }
    }

    fn ns_view_from_window(window: &Window) -> Option<id> {
        let handle = window.window_handle().ok()?;
        match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => Some(handle.ns_view.as_ptr() as id),
            _ => None,
        }
    }

    fn cg_rect_from_layout(rect: LayoutRect, ctx: &LayerContext) -> CGRect {
        let width = rect.size.width as f64;
        let height = rect.size.height as f64;
        let x = rect.origin.x as f64;
        let y = rect.origin.y as f64;
        // macOS uses bottom-left origin; Fission layout uses top-left.
        let flipped_y = ctx.bounds_height - height - y;
        CGRect::new(&CGPoint::new(x, flipped_y), &CGSize::new(width, height))
    }

    impl MapBackend for MacMapBackend {
        fn create_controller(
            &self,
            widget_id: WidgetId,
            center: (f64, f64),
            zoom: f32,
        ) -> Box<dyn MapController> {
            unsafe {
                let frame = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
                let map_view_alloc: id = msg_send![class!(MKMapView), alloc];
                let map_view: id = msg_send![map_view_alloc, initWithFrame: frame];

                // Set center and region
                let coord = CLLocationCoordinate2D {
                    latitude: center.0,
                    longitude: center.1,
                };
                let delta = zoom_to_span_delta(zoom);
                let span = MKCoordinateSpan {
                    latitude_delta: delta,
                    longitude_delta: delta,
                };
                let region = MKCoordinateRegion {
                    center: coord,
                    span,
                };
                let () = msg_send![map_view, setRegion: region animated: NO];
                let map_view = RetainedId::owned(map_view);
                self.registry.register(widget_id, map_view.clone());

                Box::new(MacMapController {
                    map_view,
                    widget_id,
                    registry: Arc::clone(&self.registry),
                })
            }
        }

        fn present_surfaces(&self, frames: &[MapSurfaceFrame]) {
            let mut layers = self.layers.lock().unwrap();

            if frames.is_empty() {
                for layer in layers.values() {
                    unsafe {
                        layer.detach();
                    }
                }
                layers.clear();
                return;
            }

            let Some(ctx) = self.ensure_layer_backing() else {
                for layer in layers.values() {
                    unsafe {
                        layer.detach();
                    }
                }
                layers.clear();
                return;
            };

            let mut seen = HashSet::new();
            for frame in frames {
                if self.update_map_layer(&mut layers, frame, &ctx) {
                    seen.insert(frame.widget_id);
                }
            }

            layers.retain(|widget_id, layer| {
                if seen.contains(widget_id) {
                    true
                } else {
                    unsafe {
                        layer.detach();
                    }
                    false
                }
            });
        }
    }

    impl Drop for MacMapBackend {
        fn drop(&mut self) {
            if let Ok(mut layers) = self.layers.lock() {
                for layer in layers.values() {
                    unsafe {
                        layer.detach();
                    }
                }
                layers.clear();
            }
        }
    }

    #[derive(Default)]
    struct MapRegistry {
        views: Mutex<HashMap<WidgetId, RetainedId>>,
    }

    impl MapRegistry {
        fn register(&self, widget_id: WidgetId, map_view: RetainedId) {
            self.views.lock().unwrap().insert(widget_id, map_view);
        }

        fn unregister(&self, widget_id: WidgetId) {
            self.views.lock().unwrap().remove(&widget_id);
        }

        fn get(&self, widget_id: WidgetId) -> Option<RetainedId> {
            self.views.lock().unwrap().get(&widget_id).cloned()
        }
    }

    /// Wraps an MKMapView hosted as an AppKit subview.
    struct MapLayer {
        host_view: RetainedId,
        map_view: RetainedId,
    }

    impl MapLayer {
        fn new(map_view: &RetainedId, ctx: &LayerContext) -> Self {
            unsafe {
                let frame = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
                let view_alloc: id = msg_send![class!(NSView), alloc];
                let view: id = msg_send![view_alloc, initWithFrame: frame];
                let () = msg_send![view, setWantsLayer: YES];
                let () = msg_send![view, addSubview: map_view.as_id()];

                let () = msg_send![
                    ctx.parent_view,
                    addSubview: view
                    positioned: NSWindowOrderingMode::NSWindowAbove
                    relativeTo: nil
                ];

                Self {
                    host_view: RetainedId::owned(view),
                    map_view: map_view.clone(),
                }
            }
        }

        fn update(&mut self, map_view: &RetainedId, ctx: &LayerContext, rect: LayoutRect) {
            unsafe {
                let cg_rect = cg_rect_from_layout(rect, ctx);
                let view = self.host_view.as_id();
                let () = msg_send![view, setFrame: cg_rect];
                let bounds: CGRect = msg_send![view, bounds];
                let map_id = map_view.as_id();
                let () = msg_send![map_id, setFrame: bounds];
                let () = msg_send![view, addSubview: map_id];
                let () = msg_send![
                    ctx.parent_view,
                    addSubview: view
                    positioned: NSWindowOrderingMode::NSWindowAbove
                    relativeTo: nil
                ];
                self.map_view = map_view.clone();
            }
        }

        unsafe fn detach(&self) {
            let () = msg_send![self.map_view.as_id(), removeFromSuperview];
            let () = msg_send![self.host_view.as_id(), removeFromSuperview];
        }
    }

    pub struct MacMapController {
        map_view: RetainedId,
        widget_id: WidgetId,
        registry: Arc<MapRegistry>,
    }

    impl Drop for MacMapController {
        fn drop(&mut self) {
            self.registry.unregister(self.widget_id);
        }
    }

    impl MapController for MacMapController {
        fn set_center(&mut self, lat: f64, lng: f64) {
            unsafe {
                let coord = CLLocationCoordinate2D {
                    latitude: lat,
                    longitude: lng,
                };
                let () = msg_send![self.map_view.as_id(), setCenterCoordinate: coord animated: NO];
            }
        }

        fn set_zoom(&mut self, zoom: f32) {
            unsafe {
                let center: CLLocationCoordinate2D =
                    msg_send![self.map_view.as_id(), centerCoordinate];
                let delta = zoom_to_span_delta(zoom);
                let span = MKCoordinateSpan {
                    latitude_delta: delta,
                    longitude_delta: delta,
                };
                let region = MKCoordinateRegion { center, span };
                let () = msg_send![self.map_view.as_id(), setRegion: region animated: NO];
            }
        }

        fn set_show_user_location(&mut self, show: bool) {
            unsafe {
                let val: objc::runtime::BOOL = if show { YES as _ } else { NO as _ };
                let () = msg_send![self.map_view.as_id(), setShowsUserLocation: val];
            }
        }

        fn set_interactive(&mut self, interactive: bool) {
            unsafe {
                let val: objc::runtime::BOOL = if interactive { YES as _ } else { NO as _ };
                let () = msg_send![self.map_view.as_id(), setScrollEnabled: val];
                let () = msg_send![self.map_view.as_id(), setZoomEnabled: val];
                let () = msg_send![self.map_view.as_id(), setRotateEnabled: val];
                let () = msg_send![self.map_view.as_id(), setPitchEnabled: val];
            }
        }

        fn widget_id(&self) -> WidgetId {
            self.widget_id
        }
    }
}

// ---------------------------------------------------------------------------
// iOS
// ---------------------------------------------------------------------------
#[cfg(target_os = "ios")]
mod ios {
    use super::{MapBackend, MapController};
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use fission_ir::WidgetId;
    use fission_render::LayoutRect;
    use fission_shell::MapSurfaceFrame;
    use objc::rc::StrongPtr;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};
    use winit::window::Window;

    type Id = *mut Object;
    const YES: i8 = 1;
    const NO: i8 = 0;

    #[link(name = "MapKit", kind = "framework")]
    extern "C" {}

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct CLLocationCoordinate2D {
        latitude: f64,
        longitude: f64,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct MKCoordinateSpan {
        latitude_delta: f64,
        longitude_delta: f64,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct MKCoordinateRegion {
        center: CLLocationCoordinate2D,
        span: MKCoordinateSpan,
    }

    fn zoom_to_span_delta(zoom: f32) -> f64 {
        360.0 / (2.0_f64).powf(zoom as f64)
    }

    #[derive(Clone)]
    struct RetainedId(StrongPtr);
    unsafe impl Send for RetainedId {}
    unsafe impl Sync for RetainedId {}

    impl RetainedId {
        unsafe fn new(ptr: Id) -> Self {
            Self(StrongPtr::retain(ptr))
        }
        unsafe fn owned(ptr: Id) -> Self {
            Self(StrongPtr::new(ptr))
        }
        fn as_id(&self) -> Id {
            *self.0
        }
    }

    pub struct IosMapBackend {
        ui_view: Option<RetainedId>,
        layers: Mutex<HashMap<WidgetId, IosMapLayer>>,
        registry: Arc<MapRegistry>,
    }

    impl IosMapBackend {
        pub fn try_new(window: &Window) -> Option<Self> {
            let ui_view = ui_view_from_window(window)?;
            Some(Self {
                ui_view: Some(unsafe { RetainedId::new(ui_view) }),
                layers: Mutex::new(HashMap::new()),
                registry: Arc::new(MapRegistry::default()),
            })
        }

        fn update_map_layer(
            &self,
            layers: &mut HashMap<WidgetId, IosMapLayer>,
            frame: &MapSurfaceFrame,
            parent: Id,
        ) -> bool {
            let Some(map_view) = self.registry.get(frame.widget_id) else {
                return false;
            };
            let layer = layers
                .entry(frame.widget_id)
                .or_insert_with(|| IosMapLayer::new(&map_view, parent));
            layer.update(&map_view, parent, frame.rect);
            true
        }
    }

    fn ui_view_from_window(window: &Window) -> Option<Id> {
        let handle = window.window_handle().ok()?;
        match handle.as_raw() {
            RawWindowHandle::UiKit(handle) => Some(handle.ui_view.as_ptr() as Id),
            _ => None,
        }
    }

    fn cg_rect_from_layout(rect: LayoutRect) -> CGRect {
        CGRect::new(
            &CGPoint::new(rect.origin.x as f64, rect.origin.y as f64),
            &CGSize::new(rect.size.width as f64, rect.size.height as f64),
        )
    }

    impl MapBackend for IosMapBackend {
        fn create_controller(
            &self,
            widget_id: WidgetId,
            center: (f64, f64),
            zoom: f32,
        ) -> Box<dyn MapController> {
            unsafe {
                let frame = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
                let map_view_alloc: Id = msg_send![class!(MKMapView), alloc];
                let map_view: Id = msg_send![map_view_alloc, initWithFrame: frame];

                let coord = CLLocationCoordinate2D {
                    latitude: center.0,
                    longitude: center.1,
                };
                let delta = zoom_to_span_delta(zoom);
                let span = MKCoordinateSpan {
                    latitude_delta: delta,
                    longitude_delta: delta,
                };
                let region = MKCoordinateRegion {
                    center: coord,
                    span,
                };
                let () = msg_send![map_view, setRegion: region animated: NO];
                let map_view = RetainedId::owned(map_view);
                self.registry.register(widget_id, map_view.clone());

                Box::new(IosMapController {
                    map_view,
                    widget_id,
                    registry: Arc::clone(&self.registry),
                })
            }
        }

        fn present_surfaces(&self, frames: &[MapSurfaceFrame]) {
            let mut layers = self.layers.lock().unwrap();

            if frames.is_empty() {
                for layer in layers.values() {
                    unsafe {
                        layer.detach();
                    }
                }
                layers.clear();
                return;
            }

            let parent = match self.ui_view.as_ref() {
                Some(v) => v.as_id(),
                None => {
                    layers.clear();
                    return;
                }
            };

            let mut seen = HashSet::new();
            for frame in frames {
                if self.update_map_layer(&mut layers, frame, parent) {
                    seen.insert(frame.widget_id);
                }
            }

            layers.retain(|wid, layer| {
                if seen.contains(wid) {
                    true
                } else {
                    unsafe {
                        layer.detach();
                    }
                    false
                }
            });
        }
    }

    impl Drop for IosMapBackend {
        fn drop(&mut self) {
            if let Ok(mut layers) = self.layers.lock() {
                for layer in layers.values() {
                    unsafe {
                        layer.detach();
                    }
                }
                layers.clear();
            }
        }
    }

    #[derive(Default)]
    struct MapRegistry {
        views: Mutex<HashMap<WidgetId, RetainedId>>,
    }

    impl MapRegistry {
        fn register(&self, widget_id: WidgetId, map_view: RetainedId) {
            self.views.lock().unwrap().insert(widget_id, map_view);
        }

        fn unregister(&self, widget_id: WidgetId) {
            self.views.lock().unwrap().remove(&widget_id);
        }

        fn get(&self, widget_id: WidgetId) -> Option<RetainedId> {
            self.views.lock().unwrap().get(&widget_id).cloned()
        }
    }

    struct IosMapLayer {
        host_view: RetainedId,
        map_view: RetainedId,
    }

    impl IosMapLayer {
        fn new(map_view: &RetainedId, parent: Id) -> Self {
            unsafe {
                let frame = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
                let view_alloc: Id = msg_send![class!(UIView), alloc];
                let view: Id = msg_send![view_alloc, initWithFrame: frame];
                let () = msg_send![view, addSubview: map_view.as_id()];
                let () = msg_send![parent, addSubview: view];
                Self {
                    host_view: RetainedId::owned(view),
                    map_view: map_view.clone(),
                }
            }
        }

        fn update(&mut self, map_view: &RetainedId, parent: Id, rect: LayoutRect) {
            unsafe {
                let cg_rect = cg_rect_from_layout(rect);
                let view = self.host_view.as_id();
                let () = msg_send![view, setFrame: cg_rect];
                let bounds: CGRect = msg_send![view, bounds];
                let map_id = map_view.as_id();
                let () = msg_send![map_id, setFrame: bounds];
                let () = msg_send![view, addSubview: map_id];
                let () = msg_send![parent, addSubview: view];
                self.map_view = map_view.clone();
            }
        }

        unsafe fn detach(&self) {
            let () = msg_send![self.map_view.as_id(), removeFromSuperview];
            let () = msg_send![self.host_view.as_id(), removeFromSuperview];
        }
    }

    pub struct IosMapController {
        map_view: RetainedId,
        widget_id: WidgetId,
        registry: Arc<MapRegistry>,
    }

    impl Drop for IosMapController {
        fn drop(&mut self) {
            self.registry.unregister(self.widget_id);
        }
    }

    impl MapController for IosMapController {
        fn set_center(&mut self, lat: f64, lng: f64) {
            unsafe {
                let coord = CLLocationCoordinate2D {
                    latitude: lat,
                    longitude: lng,
                };
                let () = msg_send![self.map_view.as_id(), setCenterCoordinate: coord animated: NO];
            }
        }

        fn set_zoom(&mut self, zoom: f32) {
            unsafe {
                let center: CLLocationCoordinate2D =
                    msg_send![self.map_view.as_id(), centerCoordinate];
                let delta = zoom_to_span_delta(zoom);
                let span = MKCoordinateSpan {
                    latitude_delta: delta,
                    longitude_delta: delta,
                };
                let region = MKCoordinateRegion { center, span };
                let () = msg_send![self.map_view.as_id(), setRegion: region animated: NO];
            }
        }

        fn set_show_user_location(&mut self, show: bool) {
            unsafe {
                let val = if show { YES } else { NO };
                let () = msg_send![self.map_view.as_id(), setShowsUserLocation: val];
            }
        }

        fn set_interactive(&mut self, interactive: bool) {
            unsafe {
                let val = if interactive { YES } else { NO };
                let () = msg_send![self.map_view.as_id(), setScrollEnabled: val];
                let () = msg_send![self.map_view.as_id(), setZoomEnabled: val];
                let () = msg_send![self.map_view.as_id(), setRotateEnabled: val];
                let () = msg_send![self.map_view.as_id(), setPitchEnabled: val];
            }
        }

        fn widget_id(&self) -> WidgetId {
            self.widget_id
        }
    }
}
