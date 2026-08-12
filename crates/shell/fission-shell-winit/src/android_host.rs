use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use base64::Engine as _;
use jni::objects::{
    GlobalRef, JClass, JFloatArray, JIntArray, JObject, JObjectArray, JString, JValue,
};
use jni::sys::{jint, jlong, jobject, JNI_TRUE};
use jni::{JNIEnv, JavaVM, NativeMethod};
use winit::event_loop::EventLoopProxy;
use winit::platform::android::activity::AndroidApp;

use fission_test_driver::TestEvent;

pub(crate) const HOST_CONTRACT_VERSION: i32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AndroidSemanticsNode {
    pub id: i32,
    pub parent_id: i32,
    pub role: i32,
    pub flags: i32,
    pub actions: i32,
    pub bounds: [i32; 4],
    pub label: Option<String>,
    pub value: Option<String>,
    pub selection_utf16: [i32; 2],
    pub numeric: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AndroidImeState {
    pub active: bool,
    pub value: String,
    pub selection_utf16: [i32; 2],
    pub input_kind: i32,
    pub action: i32,
    pub flags: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AndroidHostEvent {
    HostError(String),
    Click(i32),
    Focus(i32),
    Blur(i32),
    SetText {
        id: i32,
        value: String,
    },
    SetSelection {
        id: i32,
        start_utf16: usize,
        end_utf16: usize,
    },
    Scroll {
        id: i32,
        direction: i32,
    },
    Increment {
        id: i32,
        direction: i32,
    },
    SetNumeric {
        id: i32,
        value: f32,
    },
    ImeCommit(String),
    ImePreedit {
        text: String,
        cursor_utf16: Option<(usize, usize)>,
    },
    ImeCancel,
    ImeReplace {
        value: String,
        selection_utf16: (usize, usize),
    },
    ImeSelection {
        start_utf16: usize,
        end_utf16: usize,
    },
    ImeAction,
}

struct WakeTarget {
    proxy: EventLoopProxy<TestEvent>,
}

fn wake_targets() -> &'static Mutex<HashMap<i64, Weak<WakeTarget>>> {
    static TARGETS: OnceLock<Mutex<HashMap<i64, Weak<WakeTarget>>>> = OnceLock::new();
    TARGETS.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT_TOKEN: AtomicI64 = AtomicI64::new(1);

pub(crate) struct AndroidHostBridge {
    vm: Arc<JavaVM>,
    activity: GlobalRef,
    token: i64,
    _wake_target: Arc<WakeTarget>,
}

impl AndroidHostBridge {
    pub(crate) fn install(
        app: &AndroidApp,
        proxy: EventLoopProxy<TestEvent>,
    ) -> Result<Arc<Self>, String> {
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }
            .map_err(|error| format!("failed to access Android JavaVM: {error}"))?;
        let vm = Arc::new(vm);
        let mut env = vm
            .attach_current_thread()
            .map_err(|error| format!("failed to attach Android host JNI thread: {error}"))?;
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jobject) };
        let activity = env
            .new_global_ref(&activity)
            .map_err(|error| format!("failed to retain FissionActivity: {error}"))?;

        let version = env
            .call_method(activity.as_obj(), "fissionHostContractVersion", "()I", &[])
            .and_then(|value| value.i())
            .map_err(|error| {
                format!(
                    "{}; regenerate the Android platform files",
                    jni_error(&mut env, "query Android host contract", error)
                )
            })?;
        if version != HOST_CONTRACT_VERSION {
            return Err(format!(
                "FissionActivity host contract version {version} is incompatible with runtime version {HOST_CONTRACT_VERSION}; regenerate the Android platform files"
            ));
        }
        register_wake_callback(&mut env, activity.as_obj())?;

        let token = NEXT_TOKEN
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1).filter(|next| *next > 0)
            })
            .map_err(|_| "Android host wake token space exhausted".to_string())?;
        let wake_target = Arc::new(WakeTarget { proxy });
        wake_targets()
            .lock()
            .map_err(|_| "Android host wake registry lock poisoned".to_string())?
            .insert(token, Arc::downgrade(&wake_target));

        let installed = env
            .call_method(
                activity.as_obj(),
                "fissionInstallHost",
                "(J)Z",
                &[JValue::Long(token as jlong)],
            )
            .and_then(|value| value.z())
            .map_err(|error| jni_error(&mut env, "install Android host", error));
        let installed = match installed {
            Ok(installed) => installed,
            Err(error) => {
                remove_wake_target(token);
                return Err(error);
            }
        };
        if !installed {
            let detail = host_error(&mut env, activity.as_obj())
                .unwrap_or_else(|error| format!("could not read Java host error: {error}"));
            remove_wake_target(token);
            return Err(format!(
                "FissionActivity rejected Android host installation: {detail}"
            ));
        }

        drop(env);
        Ok(Arc::new(Self {
            vm,
            activity,
            token,
            _wake_target: wake_target,
        }))
    }

    pub(crate) fn set_active(&self, active: bool) -> Result<(), String> {
        self.call_void(
            "fissionSetHostActive",
            "(Z)V",
            &[JValue::Bool(if active { JNI_TRUE } else { 0 })],
        )
    }

    pub(crate) fn update_semantics(
        &self,
        nodes: &[AndroidSemanticsNode],
        focused_id: i32,
    ) -> Result<(), String> {
        if nodes.len() > i32::MAX as usize / 4 {
            return Err("Android semantics tree exceeds the JNI array limit".to_string());
        }
        self.with_env(|env, activity| {
            let ids = int_array(env, nodes.iter().map(|node| node.id))?;
            let parents = int_array(env, nodes.iter().map(|node| node.parent_id))?;
            let roles = int_array(env, nodes.iter().map(|node| node.role))?;
            let flags = int_array(env, nodes.iter().map(|node| node.flags))?;
            let actions = int_array(env, nodes.iter().map(|node| node.actions))?;
            let bounds = int_array(env, nodes.iter().flat_map(|node| node.bounds))?;
            let labels = string_array(env, nodes.iter().map(|node| node.label.as_deref()))?;
            let values = string_array(env, nodes.iter().map(|node| node.value.as_deref()))?;
            let selections = int_array(env, nodes.iter().flat_map(|node| node.selection_utf16))?;
            let numerics = float_array(env, nodes.iter().flat_map(|node| node.numeric))?;

            env.call_method(
                activity,
                "fissionUpdateSemantics",
                "([I[I[I[I[I[I[Ljava/lang/String;[Ljava/lang/String;[I[FI)V",
                &[
                    JValue::Object(ids.as_ref()),
                    JValue::Object(parents.as_ref()),
                    JValue::Object(roles.as_ref()),
                    JValue::Object(flags.as_ref()),
                    JValue::Object(actions.as_ref()),
                    JValue::Object(bounds.as_ref()),
                    JValue::Object(labels.as_ref()),
                    JValue::Object(values.as_ref()),
                    JValue::Object(selections.as_ref()),
                    JValue::Object(numerics.as_ref()),
                    JValue::Int(focused_id),
                ],
            )?;
            Ok(())
        })
    }

    pub(crate) fn update_ime(&self, state: &AndroidImeState) -> Result<(), String> {
        self.with_env(|env, activity| {
            let value = env.new_string(&state.value)?;
            env.call_method(
                activity,
                "fissionUpdateIme",
                "(ZLjava/lang/String;IIIII)V",
                &[
                    JValue::Bool(if state.active { JNI_TRUE } else { 0 }),
                    JValue::Object(value.as_ref()),
                    JValue::Int(state.selection_utf16[0]),
                    JValue::Int(state.selection_utf16[1]),
                    JValue::Int(state.input_kind),
                    JValue::Int(state.action),
                    JValue::Int(state.flags),
                ],
            )?;
            Ok(())
        })
    }

    pub(crate) fn set_ime_caret(&self, rect: [f32; 4]) -> Result<(), String> {
        if rect.iter().any(|value| !value.is_finite()) || rect[2] < 0.0 || rect[3] < 0.0 {
            return Err("Android IME caret rectangle must be finite and non-negative".to_string());
        }
        self.call_void(
            "fissionSetImeCaret",
            "(FFFF)V",
            &[
                JValue::Float(rect[0]),
                JValue::Float(rect[1]),
                JValue::Float(rect[2]),
                JValue::Float(rect[3]),
            ],
        )
    }

    pub(crate) fn drain_events(&self) -> Result<Vec<AndroidHostEvent>, String> {
        let raw = self.with_env(|env, activity| {
            let events = env
                .call_method(
                    activity,
                    "fissionDrainHostEvents",
                    "()[Ljava/lang/String;",
                    &[],
                )?
                .l()?;
            if events.is_null() {
                return Ok(Vec::new());
            }
            let events = JObjectArray::from(events);
            let len = env.get_array_length(&events)?;
            let mut raw = Vec::with_capacity(len as usize);
            for index in 0..len {
                let value = JString::from(env.get_object_array_element(&events, index)?);
                let decoded = String::from(env.get_string(&value)?);
                env.delete_local_ref(value)?;
                raw.push(decoded);
            }
            Ok(raw)
        })?;
        raw.into_iter()
            .map(|event| {
                parse_host_event(&event).map_err(|error| {
                    let kind = event.split('|').next().unwrap_or("unknown");
                    format!("malformed Android host {kind:?} event: {error}")
                })
            })
            .collect()
    }

    fn call_void(
        &self,
        method: &str,
        signature: &str,
        args: &[JValue<'_, '_>],
    ) -> Result<(), String> {
        self.with_env(|env, activity| {
            env.call_method(activity, method, signature, args)?;
            Ok(())
        })
    }

    fn with_env<R>(
        &self,
        f: impl for<'env> FnOnce(&mut JNIEnv<'env>, &JObject<'_>) -> jni::errors::Result<R>,
    ) -> Result<R, String> {
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|error| format!("failed to attach Android host JNI thread: {error}"))?;
        env.with_local_frame(32, |env| f(env, self.activity.as_obj()))
            .map_err(|error| jni_error(&mut env, "invoke Android host", error))
    }
}

impl Drop for AndroidHostBridge {
    fn drop(&mut self) {
        remove_wake_target(self.token);
        if let Ok(mut env) = self.vm.attach_current_thread() {
            if let Err(error) = env.call_method(
                self.activity.as_obj(),
                "fissionUninstallHost",
                "(J)V",
                &[JValue::Long(self.token)],
            ) {
                eprintln!(
                    "fission-shell-winit: failed to uninstall Android host: {}",
                    jni_error(&mut env, "uninstall Android host", error)
                );
            }
        }
    }
}

fn register_wake_callback(env: &mut JNIEnv<'_>, activity: &JObject<'_>) -> Result<(), String> {
    let class = env
        .get_object_class(activity)
        .map_err(|error| jni_error(env, "resolve FissionActivity class", error))?;
    let method = NativeMethod {
        name: "fissionNativeWake".into(),
        sig: "(J)V".into(),
        fn_ptr: fission_native_wake as *mut c_void,
    };
    env.register_native_methods(class, &[method])
        .map_err(|error| jni_error(env, "register Android host wake callback", error))
}

extern "system" fn fission_native_wake(_env: JNIEnv<'_>, _class: JClass<'_>, token: jlong) {
    let target = wake_targets()
        .lock()
        .ok()
        .and_then(|targets| targets.get(&(token as i64)).and_then(Weak::upgrade));
    if let Some(target) = target {
        let _ = target.proxy.send_event(TestEvent::Wake);
    }
}

fn remove_wake_target(token: i64) {
    if let Ok(mut targets) = wake_targets().lock() {
        targets.remove(&token);
    }
}

fn int_array<'env>(
    env: &mut JNIEnv<'env>,
    values: impl IntoIterator<Item = i32>,
) -> jni::errors::Result<JIntArray<'env>> {
    let values = values.into_iter().collect::<Vec<jint>>();
    let array = env.new_int_array(values.len() as jint)?;
    env.set_int_array_region(&array, 0, &values)?;
    Ok(array)
}

fn string_array<'env, 'value>(
    env: &mut JNIEnv<'env>,
    values: impl IntoIterator<Item = Option<&'value str>>,
) -> jni::errors::Result<JObjectArray<'env>> {
    let values = values.into_iter().collect::<Vec<_>>();
    let class = env.find_class("java/lang/String")?;
    let array = env.new_object_array(values.len() as jint, class, JObject::null())?;
    for (index, value) in values.into_iter().enumerate() {
        if let Some(value) = value {
            let value = env.new_string(value)?;
            env.set_object_array_element(&array, index as jint, &value)?;
            env.delete_local_ref(value)?;
        }
    }
    Ok(array)
}

fn float_array<'env>(
    env: &mut JNIEnv<'env>,
    values: impl IntoIterator<Item = f32>,
) -> jni::errors::Result<JFloatArray<'env>> {
    let values = values.into_iter().collect::<Vec<_>>();
    let array = env.new_float_array(values.len() as jint)?;
    env.set_float_array_region(&array, 0, &values)?;
    Ok(array)
}

fn host_error(env: &mut JNIEnv<'_>, activity: &JObject<'_>) -> Result<String, String> {
    let value = env
        .call_method(activity, "fissionHostError", "()Ljava/lang/String;", &[])
        .and_then(|value| value.l())
        .map_err(|error| jni_error(env, "read Android host error", error))?;
    if value.is_null() {
        return Ok("unknown Java host error".to_string());
    }
    env.get_string(&JString::from(value))
        .map(String::from)
        .map_err(|error| jni_error(env, "decode Android host error", error))
}

fn jni_error(env: &mut JNIEnv<'_>, operation: &str, error: jni::errors::Error) -> String {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    format!("failed to {operation}: {error}")
}

fn parse_host_event(raw: &str) -> Result<AndroidHostEvent, String> {
    let mut parts = raw.split('|');
    let kind = parts.next().unwrap_or_default();
    let parse_i32 = |value: Option<&str>, field: &str| {
        value
            .ok_or_else(|| format!("missing {field}"))?
            .parse::<i32>()
            .map_err(|error| format!("invalid {field}: {error}"))
    };
    let parse_usize = |value: Option<&str>, field: &str| {
        value
            .ok_or_else(|| format!("missing {field}"))?
            .parse::<usize>()
            .map_err(|error| format!("invalid {field}: {error}"))
    };
    let decode = |value: Option<&str>| {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value.ok_or_else(|| "missing encoded text".to_string())?)
            .map_err(|error| format!("invalid encoded text: {error}"))?;
        String::from_utf8(bytes).map_err(|error| format!("invalid UTF-8 text: {error}"))
    };
    let event = match kind {
        "host_error" => Ok(AndroidHostEvent::HostError(decode(parts.next())?)),
        "a_click" => Ok(AndroidHostEvent::Click(parse_i32(parts.next(), "node id")?)),
        "a_focus" => Ok(AndroidHostEvent::Focus(parse_i32(parts.next(), "node id")?)),
        "a_blur" => Ok(AndroidHostEvent::Blur(parse_i32(parts.next(), "node id")?)),
        "a_set_text" => Ok(AndroidHostEvent::SetText {
            id: parse_i32(parts.next(), "node id")?,
            value: decode(parts.next())?,
        }),
        "a_set_selection" => Ok(AndroidHostEvent::SetSelection {
            id: parse_i32(parts.next(), "node id")?,
            start_utf16: parse_usize(parts.next(), "selection start")?,
            end_utf16: parse_usize(parts.next(), "selection end")?,
        }),
        "a_scroll" => Ok(AndroidHostEvent::Scroll {
            id: parse_i32(parts.next(), "node id")?,
            direction: parse_i32(parts.next(), "scroll direction")?,
        }),
        "a_increment" => Ok(AndroidHostEvent::Increment {
            id: parse_i32(parts.next(), "node id")?,
            direction: parse_i32(parts.next(), "increment direction")?,
        }),
        "a_set_numeric" => {
            let id = parse_i32(parts.next(), "node id")?;
            let value = parts
                .next()
                .ok_or_else(|| "missing numeric value".to_string())?
                .parse::<f32>()
                .map_err(|error| format!("invalid numeric value: {error}"))?;
            if !value.is_finite() {
                return Err("numeric value must be finite".to_string());
            }
            Ok(AndroidHostEvent::SetNumeric { id, value })
        }
        "i_commit" => Ok(AndroidHostEvent::ImeCommit(decode(parts.next())?)),
        "i_preedit" => {
            let start = parse_i32(parts.next(), "preedit cursor start")?;
            let end = parse_i32(parts.next(), "preedit cursor end")?;
            let cursor_utf16 = match (start, end) {
                (start, end) if start >= 0 && end >= start => Some((start as usize, end as usize)),
                (start, end) if start < 0 && end < 0 => None,
                _ => return Err("preedit cursor range is inconsistent".to_string()),
            };
            Ok(AndroidHostEvent::ImePreedit {
                text: decode(parts.next())?,
                cursor_utf16,
            })
        }
        "i_cancel" => Ok(AndroidHostEvent::ImeCancel),
        "i_replace" => Ok(AndroidHostEvent::ImeReplace {
            selection_utf16: (
                parse_usize(parts.next(), "selection start")?,
                parse_usize(parts.next(), "selection end")?,
            ),
            value: decode(parts.next())?,
        }),
        "i_selection" => Ok(AndroidHostEvent::ImeSelection {
            start_utf16: parse_usize(parts.next(), "selection start")?,
            end_utf16: parse_usize(parts.next(), "selection end")?,
        }),
        "i_action" => Ok(AndroidHostEvent::ImeAction),
        _ => Err(format!("unknown Android host event {kind:?}")),
    }?;
    if parts.next().is_some() {
        return Err("Android host event has trailing fields".to_string());
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unicode_ime_event() {
        assert_eq!(
            parse_host_event("i_commit|8J+MjQ==").unwrap(),
            AndroidHostEvent::ImeCommit("🌍".to_string())
        );
    }

    #[test]
    fn rejects_malformed_event() {
        assert!(parse_host_event("a_set_selection|4|nope|3").is_err());
        assert!(parse_host_event("i_cancel|unexpected").is_err());
        assert!(parse_host_event("i_preedit|-1|0|").is_err());
        assert!(parse_host_event("a_set_numeric|4|NaN").is_err());
    }
}
