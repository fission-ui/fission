use super::{VideoBackend, VideoEvent, VideoPlayer};
use fission_core::ui::VideoAudioOptions;
use fission_shell::{PlatformSurfaceCapabilities, VideoSurfaceFrame};
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jobject;
use jni::{JNIEnv, JavaVM};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub struct AndroidVideoBackend {
    next_id: AtomicU64,
    scale_factor_bits: AtomicU64,
    registry: Arc<Mutex<HashMap<u64, AndroidPlayerState>>>,
}

impl AndroidVideoBackend {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            scale_factor_bits: AtomicU64::new(1.0_f64.to_bits()),
            registry: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl VideoBackend for AndroidVideoBackend {
    fn surface_capabilities(&self) -> PlatformSurfaceCapabilities {
        PlatformSurfaceCapabilities {
            available: true,
            ..PlatformSurfaceCapabilities::UNAVAILABLE
        }
    }

    fn set_scale_factor(&self, scale_factor: f64) {
        if scale_factor.is_finite() && scale_factor > 0.0 {
            self.scale_factor_bits
                .store(scale_factor.to_bits(), Ordering::Relaxed);
        }
    }

    fn create_player(&self, source: &str, _audio: &VideoAudioOptions) -> Box<dyn VideoPlayer> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let pending_error = call_with_env(|env, activity_class| {
            let source = env.new_string(source)?;
            env.call_static_method(
                &activity_class,
                "fissionCreateVideo",
                "(JLjava/lang/String;)V",
                &[
                    JValue::Long(id as i64),
                    JValue::Object(&JObject::from(source)),
                ],
            )?;
            Ok(())
        })
        .err();
        self.registry
            .lock()
            .unwrap()
            .insert(id, AndroidPlayerState { pending_error });
        Box::new(AndroidVideoPlayer {
            registry: Arc::clone(&self.registry),
            surface_id: id,
            ready_sent: false,
            ended_sent: false,
            error_sent: false,
        })
    }

    fn present_surfaces(&self, frames: &[VideoSurfaceFrame]) {
        let mut seen = HashSet::new();
        let scale_factor = f64::from_bits(self.scale_factor_bits.load(Ordering::Relaxed)) as f32;
        for frame in frames {
            seen.insert(frame.surface_id);
            let rect = frame.rect;
            let _ = call_with_env(|env, activity_class| {
                env.call_static_method(
                    &activity_class,
                    "fissionUpdateVideoSurface",
                    "(JIIIIZ)V",
                    &[
                        JValue::Long(frame.surface_id as i64),
                        JValue::Int((rect.origin.x * scale_factor).round() as i32),
                        JValue::Int((rect.origin.y * scale_factor).round() as i32),
                        JValue::Int((rect.size.width * scale_factor).max(1.0).round() as i32),
                        JValue::Int((rect.size.height * scale_factor).max(1.0).round() as i32),
                        JValue::Bool(1),
                    ],
                )?;
                Ok(())
            });
        }
        let ids = self
            .registry
            .lock()
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in ids {
            if !seen.contains(&id) {
                let _ = call_with_env(|env, activity_class| {
                    env.call_static_method(
                        &activity_class,
                        "fissionSetVideoVisible",
                        "(JZ)V",
                        &[JValue::Long(id as i64), JValue::Bool(0)],
                    )?;
                    Ok(())
                });
            }
        }
    }
}

struct AndroidPlayerState {
    pending_error: Option<String>,
}

pub struct AndroidVideoPlayer {
    registry: Arc<Mutex<HashMap<u64, AndroidPlayerState>>>,
    surface_id: u64,
    ready_sent: bool,
    ended_sent: bool,
    error_sent: bool,
}

impl Drop for AndroidVideoPlayer {
    fn drop(&mut self) {
        self.registry.lock().unwrap().remove(&self.surface_id);
        let id = self.surface_id;
        let _ = call_with_env(|env, activity_class| {
            env.call_static_method(
                &activity_class,
                "fissionDestroyVideo",
                "(J)V",
                &[JValue::Long(id as i64)],
            )?;
            Ok(())
        });
    }
}

impl AndroidVideoPlayer {
    fn call_void(&self, method: &str, signature: &str, args: &[JValue]) {
        let _ = call_with_env(|env, activity_class| {
            env.call_static_method(&activity_class, method, signature, args)?;
            Ok(())
        });
    }

    fn call_long(&self, method: &str) -> i64 {
        call_with_env(|env, activity_class| {
            Ok(env
                .call_static_method(
                    &activity_class,
                    method,
                    "(J)J",
                    &[JValue::Long(self.surface_id as i64)],
                )?
                .j()?)
        })
        .unwrap_or(0)
    }

    fn call_bool(&self, method: &str) -> bool {
        call_with_env(|env, activity_class| {
            Ok(env
                .call_static_method(
                    &activity_class,
                    method,
                    "(J)Z",
                    &[JValue::Long(self.surface_id as i64)],
                )?
                .z()?)
        })
        .unwrap_or(false)
    }

    fn call_string(&self, method: &str) -> Option<String> {
        call_with_env(|env, activity_class| {
            let value = env
                .call_static_method(
                    &activity_class,
                    method,
                    "(J)Ljava/lang/String;",
                    &[JValue::Long(self.surface_id as i64)],
                )?
                .l()?;
            if value.is_null() {
                return Ok(None);
            }
            let value = JString::from(value);
            let value: String = env.get_string(&value)?.into();
            Ok(Some(value))
        })
        .ok()
        .flatten()
    }
}

impl VideoPlayer for AndroidVideoPlayer {
    fn play(&mut self) {
        self.call_void(
            "fissionPlayVideo",
            "(J)V",
            &[JValue::Long(self.surface_id as i64)],
        );
    }

    fn pause(&mut self) {
        self.call_void(
            "fissionPauseVideo",
            "(J)V",
            &[JValue::Long(self.surface_id as i64)],
        );
    }

    fn stop(&mut self) {
        self.call_void(
            "fissionStopVideo",
            "(J)V",
            &[JValue::Long(self.surface_id as i64)],
        );
    }

    fn position(&self) -> u64 {
        self.call_long("fissionVideoPosition").max(0) as u64
    }

    fn duration(&self) -> Option<u64> {
        let duration = self.call_long("fissionVideoDuration");
        (duration >= 0).then_some(duration as u64)
    }

    fn surface_id(&self) -> u64 {
        self.surface_id
    }

    fn poll_events(&mut self) -> Vec<VideoEvent> {
        let mut events = Vec::new();
        if !self.error_sent {
            let pending_error = self
                .registry
                .lock()
                .unwrap()
                .get_mut(&self.surface_id)
                .and_then(|state| state.pending_error.take())
                .or_else(|| self.call_string("fissionVideoError"));
            if let Some(error) = pending_error {
                self.error_sent = true;
                events.push(VideoEvent::Error(error));
            }
        }
        if !self.ready_sent && self.call_bool("fissionVideoReady") {
            self.ready_sent = true;
            events.push(VideoEvent::Ready {
                duration: self.duration().unwrap_or(0),
            });
        }
        if self.call_bool("fissionVideoEnded") && !self.ended_sent {
            self.ended_sent = true;
            events.push(VideoEvent::Ended);
        }
        if !self.call_bool("fissionVideoEnded") {
            self.ended_sent = false;
        }
        events
    }

    fn seek_to(&mut self, position_ms: u64) {
        self.call_void(
            "fissionSeekVideo",
            "(JJ)V",
            &[
                JValue::Long(self.surface_id as i64),
                JValue::Long(position_ms as i64),
            ],
        );
    }

    fn set_rate(&mut self, rate: f32) {
        self.call_void(
            "fissionSetVideoRate",
            "(JF)V",
            &[JValue::Long(self.surface_id as i64), JValue::Float(rate)],
        );
    }

    fn set_volume(&mut self, volume: f32) {
        self.call_void(
            "fissionSetVideoVolume",
            "(JF)V",
            &[
                JValue::Long(self.surface_id as i64),
                JValue::Float(volume.clamp(0.0, 1.0)),
            ],
        );
    }

    fn set_muted(&mut self, muted: bool) {
        self.call_void(
            "fissionSetVideoMuted",
            "(JZ)V",
            &[
                JValue::Long(self.surface_id as i64),
                JValue::Bool(if muted { 1 } else { 0 }),
            ],
        );
    }
}

fn call_with_env<R>(
    f: impl for<'local> FnOnce(&mut JNIEnv<'local>, JClass<'local>) -> jni::errors::Result<R>,
) -> Result<R, String> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|error| format!("failed to access Android JavaVM: {error}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|error| format!("failed to attach Android JNI thread: {error}"))?;
    let activity = unsafe { JObject::from_raw(ctx.context() as jobject) };
    let activity_class = env
        .get_object_class(&activity)
        .map_err(|error| format!("failed to resolve Android Activity class: {error}"))?;
    f(&mut env, activity_class).map_err(|error| format!("Android video JNI call failed: {error}"))
}
