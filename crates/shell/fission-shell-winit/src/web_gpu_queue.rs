#[cfg(target_arch = "wasm32")]
use super::record_web_test_diagnostic;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

pub(super) fn should_defer_webgpu_redraw(
    webgpu_renderer_active: bool,
    queue_work_in_flight: bool,
) -> bool {
    webgpu_renderer_active && queue_work_in_flight
}

#[derive(Clone, Default)]
pub(super) struct WebGpuQueueBackpressure {
    work_in_flight: Arc<AtomicBool>,
    redraw_deferred: Arc<AtomicBool>,
    device_lost: Arc<AtomicBool>,
    submission_generation: Arc<AtomicU32>,
}

impl WebGpuQueueBackpressure {
    pub(super) fn is_work_in_flight(&self) -> bool {
        self.work_in_flight.load(Ordering::Acquire)
    }

    pub(super) fn mark_submitted(&self) -> u32 {
        self.work_in_flight.store(true, Ordering::Release);
        self.submission_generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1)
    }

    pub(super) fn is_submission_in_flight(&self, generation: u32) -> bool {
        self.submission_generation.load(Ordering::Acquire) == generation && self.is_work_in_flight()
    }

    pub(super) fn defer_redraw(&self) {
        self.redraw_deferred.store(true, Ordering::Release);
    }

    pub(super) fn complete_work(&self) -> bool {
        self.work_in_flight.store(false, Ordering::Release);
        self.redraw_deferred.swap(false, Ordering::AcqRel)
    }

    pub(super) fn mark_device_lost(&self) {
        self.work_in_flight.store(false, Ordering::Release);
        self.device_lost.store(true, Ordering::Release);
        self.redraw_deferred.store(true, Ordering::Release);
    }

    pub(super) fn take_device_lost(&self) -> bool {
        let lost = self.device_lost.swap(false, Ordering::AcqRel);
        if lost {
            self.work_in_flight.store(false, Ordering::Release);
            self.redraw_deferred.store(false, Ordering::Release);
        }
        lost
    }

    pub(super) fn reset(&self) {
        self.work_in_flight.store(false, Ordering::Release);
        self.redraw_deferred.store(false, Ordering::Release);
        self.device_lost.store(false, Ordering::Release);
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn arm_webgpu_completion_timeout_diagnostic(
    queue_backpressure: WebGpuQueueBackpressure,
    submission_generation: u32,
    submitted_frame: u32,
) {
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };
    let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
        if queue_backpressure.is_submission_in_flight(submission_generation) {
            record_web_test_diagnostic(559, submitted_frame);
        }
    });
    let _ = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), 30_000);
}
