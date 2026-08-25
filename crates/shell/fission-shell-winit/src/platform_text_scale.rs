use fission_core::ui::TextScaler;
use winit::event_loop::ActiveEventLoop;

pub(crate) fn current(event_loop: &ActiveEventLoop) -> TextScaler {
    let factor = configured_override().or_else(|| platform_factor(event_loop));
    factor.map(TextScaler::accessibility).unwrap_or_default()
}

fn configured_override() -> Option<f32> {
    std::env::var("FISSION_TEXT_SCALE_FACTOR")
        .ok()
        .and_then(|value| parse_factor(&value))
}

fn parse_factor(value: &str) -> Option<f32> {
    value
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

#[cfg(target_arch = "wasm32")]
fn platform_factor(_event_loop: &ActiveEventLoop) -> Option<f32> {
    let window = web_sys::window()?;
    let root = window.document()?.document_element()?;
    let value = window
        .get_computed_style(&root)
        .ok()
        .flatten()?
        .get_property_value("font-size")
        .ok()?;
    let pixels = value
        .trim()
        .strip_suffix("px")?
        .trim()
        .parse::<f32>()
        .ok()?;
    (pixels.is_finite() && pixels > 0.0).then_some(pixels / 16.0)
}

#[cfg(target_os = "android")]
fn platform_factor(event_loop: &ActiveEventLoop) -> Option<f32> {
    use jni::objects::JObject;
    use jni::sys::jobject;
    use jni::JavaVM;
    use winit::platform::android::ActiveEventLoopExtAndroid;

    let app = event_loop.android_app();
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jobject) };
    let resources = env
        .call_method(
            &activity,
            "getResources",
            "()Landroid/content/res/Resources;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;
    let configuration = env
        .call_method(
            resources,
            "getConfiguration",
            "()Landroid/content/res/Configuration;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;
    let factor = env
        .get_field(configuration, "fontScale", "F")
        .ok()?
        .f()
        .ok()?;
    (factor.is_finite() && factor > 0.0).then_some(factor)
}

#[cfg(target_os = "ios")]
fn platform_factor(_event_loop: &ActiveEventLoop) -> Option<f32> {
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::CStr;

    let category = unsafe {
        let application: *mut objc::runtime::Object =
            msg_send![class!(UIApplication), sharedApplication];
        let category: *mut objc::runtime::Object =
            msg_send![application, preferredContentSizeCategory];
        let bytes: *const std::ffi::c_char = msg_send![category, UTF8String];
        if bytes.is_null() {
            return None;
        }
        CStr::from_ptr(bytes).to_string_lossy().into_owned()
    };
    let factor = match category.as_str() {
        "UICTContentSizeCategoryXS" => 0.82,
        "UICTContentSizeCategoryS" => 0.88,
        "UICTContentSizeCategoryM" => 0.94,
        "UICTContentSizeCategoryL" => 1.0,
        "UICTContentSizeCategoryXL" => 1.12,
        "UICTContentSizeCategoryXXL" => 1.23,
        "UICTContentSizeCategoryXXXL" => 1.35,
        "UICTContentSizeCategoryAccessibilityM" => 1.64,
        "UICTContentSizeCategoryAccessibilityL" => 1.95,
        "UICTContentSizeCategoryAccessibilityXL" => 2.35,
        "UICTContentSizeCategoryAccessibilityXXL" => 2.76,
        "UICTContentSizeCategoryAccessibilityXXXL" => 3.12,
        _ => return None,
    };
    Some(factor)
}

#[cfg(target_os = "windows")]
fn platform_factor(_event_loop: &ActiveEventLoop) -> Option<f32> {
    use windows::UI::ViewManagement::UISettings;

    UISettings::GetForCurrentView()
        .and_then(|settings| settings.TextScaleFactor())
        .ok()
        .map(|percentage| percentage as f32 / 100.0)
        .filter(|factor| factor.is_finite() && *factor > 0.0)
}

#[cfg(not(any(
    target_arch = "wasm32",
    target_os = "android",
    target_os = "ios",
    target_os = "windows"
)))]
fn platform_factor(_event_loop: &ActiveEventLoop) -> Option<f32> {
    None
}

#[cfg(test)]
mod tests {
    use super::parse_factor;

    #[test]
    fn invalid_override_is_ignored() {
        assert_eq!(parse_factor("not-a-number"), None);
        assert_eq!(parse_factor("NaN"), None);
        assert_eq!(parse_factor("0"), None);
        assert_eq!(parse_factor("1.5"), Some(1.5));
    }
}
