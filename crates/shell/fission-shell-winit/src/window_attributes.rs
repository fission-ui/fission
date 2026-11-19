//! Native window construction and decoration configuration.

use super::*;

#[cfg(target_os = "android")]
pub(super) fn build_window(
    title: &str,
    initial_maximized: bool,
    decorations: bool,
    background_test_mode: bool,
    target: &EventLoopWindowTarget,
    _web_mount_selector: Option<&str>,
    _browser_defaults: BrowserDefaults,
) -> anyhow::Result<Arc<Window>> {
    let reported_scale_factor = target
        .primary_monitor()
        .map(|monitor| monitor.scale_factor());
    let window_attributes = build_window_attributes(
        title,
        initial_maximized,
        decorations,
        background_test_mode,
        false,
        _web_mount_selector,
        _browser_defaults,
        reported_scale_factor,
    )?;
    Ok(Arc::new(target.create_window(window_attributes).map_err(
        |e| anyhow::anyhow!("Window build error: {}", e),
    )?))
}

#[cfg(not(target_os = "android"))]
pub(super) fn build_window_before_run(
    title: &str,
    initial_maximized: bool,
    decorations: bool,
    background_test_mode: bool,
    tray_skip_taskbar: bool,
    event_loop: &EventLoop<TestEvent>,
    _web_mount_selector: Option<&str>,
    _browser_defaults: BrowserDefaults,
) -> anyhow::Result<Arc<Window>> {
    let window_attributes = build_window_attributes(
        title,
        initial_maximized,
        decorations,
        background_test_mode,
        tray_skip_taskbar,
        _web_mount_selector,
        _browser_defaults,
        None,
    )?;
    #[allow(deprecated)]
    Ok(Arc::new(
        event_loop
            .create_window(window_attributes)
            .map_err(|e| anyhow::anyhow!("Window build error: {}", e))?,
    ))
}

pub(super) fn native_surface_host(window: &Window) -> Option<NativeSurfaceHost<'_>> {
    let handle = window.window_handle().ok()?;
    Some(NativeSurfaceHost::from_window_handle(handle))
}

pub(super) fn build_window_attributes(
    title: &str,
    initial_maximized: bool,
    decorations: bool,
    background_test_mode: bool,
    tray_skip_taskbar: bool,
    _web_mount_selector: Option<&str>,
    _browser_defaults: BrowserDefaults,
    _reported_scale_factor: Option<f64>,
) -> anyhow::Result<WindowAttributes> {
    let mut window_attributes = WindowAttributes::default()
        .with_title(title)
        .with_maximized(initial_maximized)
        .with_decorations(decorations);
    #[cfg(target_os = "ios")]
    {
        // Winit leaves UIView.contentScaleFactor at UIKit's default unless the
        // app explicitly opts into the device scale. Without this, iOS presents
        // a 1x render target scaled up by the simulator/device, which makes the
        // shell look visibly soft compared with web and Android.
        let reported_scale_factor = _reported_scale_factor.unwrap_or(1.0);
        window_attributes = window_attributes.with_scale_factor(ios_effective_scale_factor(
            normalize_scale_factor(reported_scale_factor),
        ));
    }
    #[cfg(target_os = "windows")]
    {
        window_attributes = window_attributes.with_skip_taskbar(tray_skip_taskbar);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tray_skip_taskbar;
    }
    #[cfg(target_arch = "wasm32")]
    {
        window_attributes = window_attributes
            .with_prevent_default(true)
            .with_browser_defaults(web_input::to_winit(_browser_defaults));
        window_attributes = if let Some(selector) = _web_mount_selector {
            window_attributes.with_canvas(Some(canvas_for_mount_selector(selector)?))
        } else {
            window_attributes.with_append(true)
        };
    }
    if background_test_mode {
        window_attributes = window_attributes.with_active(false).with_visible(false);
    } else if accessibility::window_must_start_hidden() {
        // AccessKit's winit adapter has to be installed before the native
        // window is ever shown. The Resumed handler creates the adapter and
        // then makes the window visible.
        window_attributes = window_attributes.with_visible(false);
    }
    Ok(window_attributes)
}
