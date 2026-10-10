//! Window commands cross back to the owned event loop before touching native UI.

use crate::Pipeline;
use fission_core::{Runtime, WindowCommand, WindowControlError, WINDOW_CONTROL};
use fission_ir::{CoreIR, Op, WidgetId};
use fission_render::LayoutPoint;
use fission_shell::async_host::AsyncRegistry;
use fission_test_driver::TestEvent;
use futures_channel::oneshot;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::{
    event_loop::EventLoopProxy,
    window::{ResizeDirection, Window},
};

struct PendingCommand {
    command: WindowCommand,
    completion: oneshot::Sender<Result<(), WindowControlError>>,
}

pub(crate) struct WindowControls {
    pending: Arc<Mutex<Vec<PendingCommand>>>,
}

#[derive(Default)]
pub(crate) struct CommandOutcome {
    pub changed: bool,
    pub close: bool,
}

impl WindowControls {
    pub(crate) fn new(registry: &mut AsyncRegistry, proxy: EventLoopProxy<TestEvent>) -> Self {
        let pending = Arc::new(Mutex::new(Vec::new()));
        let weak = Arc::downgrade(&pending);
        registry.register_operation_capability(WINDOW_CONTROL, move |command, _| {
            let pending = weak.upgrade();
            let proxy = proxy.clone();
            async move {
                let (completion, response) = oneshot::channel();
                let queue = pending.ok_or(WindowControlError::Unavailable)?;
                queue
                    .lock()
                    .map_err(|_| WindowControlError::Unavailable)?
                    .push(PendingCommand {
                        command,
                        completion,
                    });
                // A waiting operation must not keep the host queue alive after shutdown.
                drop(queue);
                proxy
                    .send_event(TestEvent::Wake)
                    .map_err(|_| WindowControlError::Unavailable)?;
                response
                    .await
                    .unwrap_or(Err(WindowControlError::Unavailable))
            }
        });
        Self { pending }
    }

    /// Called only by the native event loop. No background worker invokes winit UI.
    pub(crate) fn apply_pending(&self, window: &Window) -> CommandOutcome {
        let commands = self
            .pending
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default();
        let mut outcome = CommandOutcome::default();
        for pending in commands {
            if outcome.close {
                let _ = pending
                    .completion
                    .send(Err(WindowControlError::Unavailable));
                continue;
            }
            match pending.command {
                WindowCommand::Minimize => window.set_minimized(true),
                WindowCommand::Maximize => window.set_maximized(true),
                WindowCommand::Restore => window.set_maximized(false),
                WindowCommand::Close => outcome.close = true,
            }
            outcome.changed = true;
            let _ = pending.completion.send(Ok(()));
        }
        outcome
    }
}

/// Finds a drag surface using the same retained hit-test path as ordinary input.
/// Paint/layout children may hit first; interactive semantics stop the ancestor walk.
fn is_drag_target(ir: &CoreIR, mut target: WidgetId) -> bool {
    loop {
        let Some(node) = ir.nodes.get(&target) else {
            return false;
        };
        if let Op::Semantics(semantics) = &node.op {
            if semantics.window_drag_region {
                return !semantics.disabled;
            }
            if semantics.focusable
                || !semantics.actions.entries.is_empty()
                || !semantics.key_actions.is_empty()
                || semantics.hyperlink.is_some()
                || semantics.text_editable
                || semantics.selectable_text
                || semantics.draggable
                || semantics.scrollable_x
                || semantics.scrollable_y
                || semantics.context_menu
            {
                return false;
            }
        }
        let Some(parent) = node.parent else {
            return false;
        };
        target = parent;
    }
}

fn resize_direction(point: LayoutPoint, width: f32, height: f32) -> Option<ResizeDirection> {
    // Reserve a narrow native resize border; title-bar controls belong inside it.
    let edge = 5.0;
    if point.x < 0.0 || point.y < 0.0 || point.x >= width || point.y >= height {
        return None;
    }
    let horizontal = if point.x < edge {
        -1
    } else if point.x >= width - edge {
        1
    } else {
        0
    };
    let vertical = if point.y < edge {
        -1
    } else if point.y >= height - edge {
        1
    } else {
        0
    };
    match (horizontal, vertical) {
        (-1, -1) => Some(ResizeDirection::NorthWest),
        (0, -1) => Some(ResizeDirection::North),
        (1, -1) => Some(ResizeDirection::NorthEast),
        (-1, 0) => Some(ResizeDirection::West),
        (1, 0) => Some(ResizeDirection::East),
        (-1, 1) => Some(ResizeDirection::SouthWest),
        (0, 1) => Some(ResizeDirection::South),
        (1, 1) => Some(ResizeDirection::SouthEast),
        _ => None,
    }
}

#[derive(Default)]
pub(crate) struct WindowDragState {
    last_press: Option<(Instant, LayoutPoint)>,
}

impl WindowDragState {
    fn double_press(&mut self, now: Instant, point: LayoutPoint) -> bool {
        let previous = self.last_press.replace((now, point));
        let double = previous.is_some_and(|(at, position)| {
            now.saturating_duration_since(at) <= Duration::from_millis(400)
                && (point.x - position.x).powi(2) + (point.y - position.y).powi(2) <= 16.0
        });
        if double {
            self.last_press = None;
        }
        double
    }

    pub(crate) fn handle_press(
        &mut self,
        window: &Window,
        runtime: &Runtime,
        pipeline: &Pipeline,
        point: LayoutPoint,
    ) -> bool {
        if window.is_decorated() {
            self.last_press = None;
            return false;
        }
        if window.is_resizable() && !window.is_maximized() {
            let size = window.inner_size().to_logical::<f32>(window.scale_factor());
            if let Some(direction) = resize_direction(point, size.width, size.height) {
                self.last_press = None;
                if window.drag_resize_window(direction).is_ok() {
                    return true;
                }
            }
        }
        let target = pipeline
            .prev_ir
            .as_ref()
            .zip(pipeline.last_snapshot.as_ref())
            .and_then(|(ir, layout)| {
                runtime
                    .hit_test(point, ir, layout)
                    .map(|target| is_drag_target(ir, target))
            })
            .unwrap_or(false);
        if !target {
            self.last_press = None;
            return false;
        }
        if self.double_press(Instant::now(), point) {
            window.set_maximized(!window.is_maximized());
            true
        } else {
            match window.drag_window() {
                Ok(()) => true,
                Err(error) => {
                    self.last_press = None;
                    log::debug!("native window drag unavailable: {error}");
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
