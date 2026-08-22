use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use fission_test_driver::{TestCommand, TestEvent, TestResponse};
use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;
use winit::event_loop::EventLoopProxy;

enum PendingResponse {
    Ready(TestResponse),
    Waiting(Receiver<TestResponse>),
}

thread_local! {
    static NEXT_REQUEST_ID: Cell<u32> = const { Cell::new(1) };
    static RESPONSES: RefCell<HashMap<u32, PendingResponse>> = RefCell::new(HashMap::new());
}

pub(crate) fn install(proxy: EventLoopProxy<TestEvent>) -> bool {
    if option_env!("FISSION_WEB_TEST_CONTROL").is_none() {
        return false;
    }

    let submit_proxy = proxy.clone();
    let submit = Closure::wrap(Box::new(move |command_json: String| -> u32 {
        let request_id = NEXT_REQUEST_ID.with(|next| {
            let request_id = next.get();
            next.set(request_id.wrapping_add(1).max(1));
            request_id
        });
        let response = match serde_json::from_str::<TestCommand>(&command_json) {
            Ok(command) => dispatch(command, &submit_proxy),
            Err(error) => PendingResponse::Ready(TestResponse::Error {
                message: format!("parse error: {error}"),
            }),
        };
        RESPONSES.with(|responses| {
            responses.borrow_mut().insert(request_id, response);
        });
        request_id
    }) as Box<dyn FnMut(String) -> u32>);

    let poll = Closure::wrap(Box::new(move |request_id: u32| -> JsValue {
        let response = RESPONSES.with(|responses| {
            let mut responses = responses.borrow_mut();
            let completed = match responses.get(&request_id) {
                Some(PendingResponse::Ready(response)) => Some(response.clone()),
                Some(PendingResponse::Waiting(receiver)) => match receiver.try_recv() {
                    Ok(response) => Some(response),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => Some(TestResponse::Error {
                        message: "browser test response channel closed".into(),
                    }),
                },
                None => Some(TestResponse::Error {
                    message: format!("unknown browser test request {request_id}"),
                }),
            };
            if completed.is_some() {
                responses.remove(&request_id);
            }
            completed
        });

        match response {
            Some(response) => serde_json::to_string(&response)
                .map(|response| JsValue::from_str(&response))
                .unwrap_or_else(|error| {
                    JsValue::from_str(
                        &serde_json::json!({
                            "status": "Error",
                            "message": format!("response serialization failed: {error}")
                        })
                        .to_string(),
                    )
                }),
            None => JsValue::NULL,
        }
    }) as Box<dyn FnMut(u32) -> JsValue>);

    let bridge = Object::new();
    if Reflect::set(&bridge, &JsValue::from_str("submit"), submit.as_ref()).is_err()
        || Reflect::set(&bridge, &JsValue::from_str("poll"), poll.as_ref()).is_err()
        || Reflect::set(
            &js_sys::global(),
            &JsValue::from_str("__FISSION_TEST__"),
            &bridge,
        )
        .is_err()
    {
        return false;
    }

    submit.forget();
    poll.forget();
    true
}

fn dispatch(command: TestCommand, proxy: &EventLoopProxy<TestEvent>) -> PendingResponse {
    match command {
        TestCommand::Tap { x, y } => ready_after(
            proxy,
            [
                TestEvent::MouseMove { x, y },
                TestEvent::MouseDown { x, y, button: 0 },
                TestEvent::MouseUp { x, y, button: 0 },
            ],
        ),
        TestCommand::Drag {
            start_x,
            start_y,
            end_x,
            end_y,
            steps,
        } => {
            let steps = steps.max(1);
            let mut events = Vec::with_capacity(steps as usize + 3);
            events.push(TestEvent::MouseMove {
                x: start_x,
                y: start_y,
            });
            events.push(TestEvent::MouseDown {
                x: start_x,
                y: start_y,
                button: 0,
            });
            for step in 1..=steps {
                let progress = step as f32 / steps as f32;
                events.push(TestEvent::MouseMove {
                    x: start_x + (end_x - start_x) * progress,
                    y: start_y + (end_y - start_y) * progress,
                });
            }
            events.push(TestEvent::MouseUp {
                x: end_x,
                y: end_y,
                button: 0,
            });
            ready_after(proxy, events)
        }
        TestCommand::PointerDown {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerDown {
                pointer_id,
                kind,
                x,
                y,
                modifiers,
            }],
        ),
        TestCommand::PointerMove {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerMove {
                pointer_id,
                kind,
                x,
                y,
                modifiers,
            }],
        ),
        TestCommand::PointerUp {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerUp {
                pointer_id,
                kind,
                x,
                y,
                modifiers,
            }],
        ),
        TestCommand::PointerCancel {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerCancel {
                pointer_id,
                kind,
                x,
                y,
                modifiers,
            }],
        ),
        TestCommand::PointerScroll {
            x,
            y,
            dx,
            dy,
            delta_mode,
            phase,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerScroll {
                x,
                y,
                dx,
                dy,
                delta_mode,
                phase,
                modifiers,
            }],
        ),
        TestCommand::Magnify {
            x,
            y,
            scale_factor,
            phase,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::Magnify {
                x,
                y,
                scale_factor,
                phase,
                modifiers,
            }],
        ),
        TestCommand::TapText { text } => query(proxy, |response_tx| TestEvent::TapText {
            text,
            response_tx,
        }),
        TestCommand::ResolveSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::ResolveSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::TapSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::TapSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::ActivateSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::ActivateSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::FocusSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::FocusSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::HoverSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::HoverSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::RightClickSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::RightClickSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::ScrollIntoView { query: selector } => {
            query(proxy, |response_tx| TestEvent::ScrollIntoView {
                query: selector,
                response_tx,
            })
        }
        TestCommand::FillText {
            query: selector,
            text,
        } => query(proxy, |response_tx| TestEvent::FillText {
            query: selector,
            text,
            response_tx,
        }),
        TestCommand::ClearText { query: selector } => {
            query(proxy, |response_tx| TestEvent::ClearText {
                query: selector,
                response_tx,
            })
        }
        TestCommand::Toggle { query: selector } => query(proxy, |response_tx| TestEvent::Toggle {
            query: selector,
            response_tx,
        }),
        TestCommand::SelectOption { query: selector } => {
            query(proxy, |response_tx| TestEvent::SelectOption {
                query: selector,
                response_tx,
            })
        }
        TestCommand::Scroll { x, y, dx, dy } => {
            ready_after(proxy, [TestEvent::Scroll { x, y, dx, dy }])
        }
        TestCommand::ExternalFileHover { x, y, paths } => {
            ready_after(proxy, [TestEvent::ExternalFileHover { x, y, paths }])
        }
        TestCommand::ExternalFileDrop { x, y, paths } => {
            ready_after(proxy, [TestEvent::ExternalFileDrop { x, y, paths }])
        }
        TestCommand::ExternalFileCancel {} => ready_after(proxy, [TestEvent::ExternalFileCancel]),
        TestCommand::TypeText { text } => ready_after(proxy, [TestEvent::TextInput { text }]),
        TestCommand::ImePreedit {
            text,
            cursor_start,
            cursor_end,
        } => ready_after(
            proxy,
            [TestEvent::ImePreedit {
                text,
                cursor: cursor_start.zip(cursor_end),
            }],
        ),
        TestCommand::ImeCommit { text } => ready_after(proxy, [TestEvent::ImeCommit { text }]),
        TestCommand::ImeCancel {} => ready_after(proxy, [TestEvent::ImeCancel]),
        TestCommand::PressKey { key, modifiers } => ready_after(
            proxy,
            [
                TestEvent::KeyDown {
                    key_code: key.clone(),
                    modifiers,
                },
                TestEvent::KeyUp {
                    key_code: key,
                    modifiers,
                },
            ],
        ),
        TestCommand::PauseAnimations {} => query(proxy, |response_tx| TestEvent::PauseAnimations {
            response_tx,
        }),
        TestCommand::ResumeAnimations {} => query(proxy, |response_tx| {
            TestEvent::ResumeAnimations { response_tx }
        }),
        TestCommand::AdvanceClock { ms } => query(proxy, |response_tx| TestEvent::AdvanceClock {
            ms,
            response_tx,
        }),
        TestCommand::WaitForIdle { .. } => {
            query(proxy, |response_tx| TestEvent::MotionStatus { response_tx })
        }
        TestCommand::GetText {} => query(proxy, |response_tx| TestEvent::GetText { response_tx }),
        TestCommand::GetTree {} => query(proxy, |response_tx| TestEvent::GetTree { response_tx }),
        TestCommand::Pump {} => query(proxy, |response_tx| TestEvent::Pump { response_tx }),
        TestCommand::Quit {} => ready_after(proxy, [TestEvent::Quit]),
        TestCommand::SimulateMouseMove { x, y } => {
            ready_after(proxy, [TestEvent::MouseMove { x, y }])
        }
        TestCommand::SimulateRightClick { x, y } => ready_after(
            proxy,
            [
                TestEvent::MouseMove { x, y },
                TestEvent::MouseDown { x, y, button: 1 },
                TestEvent::MouseUp { x, y, button: 1 },
            ],
        ),
        TestCommand::SimulateResize { width, height } => {
            ready_after(proxy, [TestEvent::Resize { width, height }])
        }
        TestCommand::WaitForSelector { .. }
        | TestCommand::WaitForVisible { .. }
        | TestCommand::WaitForEnabled { .. }
        | TestCommand::WaitForDisabled { .. }
        | TestCommand::WaitForValue { .. }
        | TestCommand::WaitForText { .. }
        | TestCommand::WaitForGone { .. }
        | TestCommand::Wait { .. }
        | TestCommand::Screenshot { .. }
        | TestCommand::CaptureScreenshot {}
        | TestCommand::CaptureAt { .. } => PendingResponse::Ready(TestResponse::Error {
            message: "command is handled by the browser test host".into(),
        }),
    }
}

fn ready_after(
    proxy: &EventLoopProxy<TestEvent>,
    events: impl IntoIterator<Item = TestEvent>,
) -> PendingResponse {
    for event in events {
        if proxy.send_event(event).is_err() {
            return PendingResponse::Ready(TestResponse::Error {
                message: "Fission browser event loop is closed".into(),
            });
        }
    }
    PendingResponse::Ready(TestResponse::Ok {})
}

fn query(
    proxy: &EventLoopProxy<TestEvent>,
    make_event: impl FnOnce(mpsc::Sender<TestResponse>) -> TestEvent,
) -> PendingResponse {
    let (response_tx, response_rx) = mpsc::channel();
    if proxy.send_event(make_event(response_tx)).is_err() {
        PendingResponse::Ready(TestResponse::Error {
            message: "Fission browser event loop is closed".into(),
        })
    } else {
        PendingResponse::Waiting(response_rx)
    }
}
