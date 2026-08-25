//! Android host services that are not exposed by android-activity's text API.

use jni::objects::{JObject, JString, JValue};
use jni::sys::jobject;
use jni::{JNIEnv, JavaVM};
use std::sync::{Arc, OnceLock};
use winit::platform::android::activity::AndroidApp;

#[derive(Clone)]
struct AndroidTextHost {
    vm: Arc<JavaVM>,
    activity: usize,
}

static HOST: OnceLock<AndroidTextHost> = OnceLock::new();

pub(crate) fn install(app: &AndroidApp) {
    let Ok(vm) = (unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }) else {
        eprintln!("Fission could not attach Android text services to the Java VM");
        return;
    };
    let _ = HOST.set(AndroidTextHost {
        vm: Arc::new(vm),
        activity: app.activity_as_ptr() as usize,
    });
}

pub(crate) fn update_cursor_area(rect: fission_render::LayoutRect, scale_factor: f32) {
    let Some(host) = HOST.get() else { return };
    let _ = host.with_env(|env, activity| {
        let view = content_view(env, activity)?;
        let service_name = env.new_string("input_method")?;
        let manager = env
            .call_method(
                activity,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&JObject::from(service_name))],
            )?
            .l()?;
        if manager.is_null() {
            return Ok(());
        }

        let builder = env.new_object(
            "android/view/inputmethod/CursorAnchorInfo$Builder",
            "()V",
            &[],
        )?;
        env.call_method(
            &builder,
            "setInsertionMarkerLocation",
            "(FFFFI)Landroid/view/inputmethod/CursorAnchorInfo$Builder;",
            &[
                JValue::Float(rect.x() * scale_factor),
                JValue::Float((rect.y() + rect.height()) * scale_factor),
                JValue::Float(rect.y() * scale_factor),
                JValue::Float((rect.y() + rect.height()) * scale_factor),
                JValue::Int(1),
            ],
        )?;
        let anchor = env
            .call_method(
                builder,
                "build",
                "()Landroid/view/inputmethod/CursorAnchorInfo;",
                &[],
            )?
            .l()?;
        env.call_method(
            manager,
            "updateCursorAnchorInfo",
            "(Landroid/view/View;Landroid/view/inputmethod/CursorAnchorInfo;)V",
            &[JValue::Object(&view), JValue::Object(&anchor)],
        )?;
        Ok(())
    });
}

pub(crate) fn configure_autofill(hints: &[String]) {
    let Some(host) = HOST.get() else { return };
    let _ = host.with_env(|env, activity| {
        let view = content_view(env, activity)?;
        if hints.is_empty() {
            env.call_method(
                &view,
                "setAutofillHints",
                "([Ljava/lang/String;)V",
                &[JValue::Object(&JObject::null())],
            )?;
            return Ok(());
        }
        let string_class = env.find_class("java/lang/String")?;
        let array = env.new_object_array(hints.len() as i32, string_class, JObject::null())?;
        for (index, hint) in hints.iter().enumerate() {
            let hint = env.new_string(hint)?;
            env.set_object_array_element(&array, index as i32, JString::from(hint))?;
        }
        let array_object = JObject::from(array);
        env.call_method(
            &view,
            "setAutofillHints",
            "([Ljava/lang/String;)V",
            &[JValue::Object(&array_object)],
        )?;
        env.call_method(&view, "setImportantForAutofill", "(I)V", &[JValue::Int(1)])?;
        Ok(())
    });
}

fn content_view<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject<'_>,
) -> jni::errors::Result<JObject<'local>> {
    let window = env
        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
        .l()?;
    let decor = env
        .call_method(window, "getDecorView", "()Landroid/view/View;", &[])?
        .l()?;
    let content = env
        .call_method(
            decor,
            "findViewById",
            "(I)Landroid/view/View;",
            &[JValue::Int(0x0102_0002)],
        )?
        .l()?;
    let child = env
        .call_method(
            &content,
            "getChildAt",
            "(I)Landroid/view/View;",
            &[JValue::Int(0)],
        )?
        .l()?;
    if child.is_null() {
        Ok(content)
    } else {
        Ok(child)
    }
}

impl AndroidTextHost {
    fn with_env<R>(
        &self,
        operation: impl for<'local> FnOnce(
            &mut JNIEnv<'local>,
            &JObject<'static>,
        ) -> jni::errors::Result<R>,
    ) -> Result<R, String> {
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|error| error.to_string())?;
        let activity = unsafe { JObject::from_raw(self.activity as jobject) };
        operation(&mut env, &activity).map_err(|error| error.to_string())
    }
}
