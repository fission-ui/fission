use crate::app::TerminalApp;
use crate::frame::TerminalFrame;
use crate::screenshot::{write_frame_png, ScreenshotOptions};
use anyhow::Result;
use base64::Engine;
use fission_core::event::ImeEvent;
use fission_core::{
    GlobalState, InputEvent, KeyCode, KeyEvent, LayoutPoint, PointerButton, PointerEvent,
};
use fission_test_driver::{TestCommand, TestResponse, TextItem};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Headless LiveTest adapter for the terminal shell.
///
/// It drives the same [`TerminalApp`] build, layout, render and input path used by
/// the interactive terminal shell, but keeps everything in-process so tests can
/// capture deterministic terminal screenshots without relying on a real TTY.
pub struct TerminalLiveTest<S, W>
where
    S: GlobalState + 'static,
    W: Clone + Into<fission_core::ui::Widget>,
{
    app: TerminalApp<S, W>,
    width: u16,
    height: u16,
    frame: TerminalFrame,
    quit_requested: bool,
}

impl<S, W> TerminalLiveTest<S, W>
where
    S: GlobalState + 'static,
    W: Clone + Into<fission_core::ui::Widget>,
{
    pub fn new(mut app: TerminalApp<S, W>, width: u16, height: u16) -> Result<Self> {
        let frame = app.render_frame(width, height)?;
        Ok(Self {
            app,
            width,
            height,
            frame,
            quit_requested: false,
        })
    }

    pub fn frame(&self) -> &TerminalFrame {
        &self.frame
    }

    pub fn is_quit_requested(&self) -> bool {
        self.quit_requested
    }

    pub fn dispatch(&mut self, command: TestCommand) -> TestResponse {
        match self.try_dispatch(command) {
            Ok(response) => response,
            Err(error) => TestResponse::Error {
                message: error.to_string(),
            },
        }
    }

    fn try_dispatch(&mut self, command: TestCommand) -> Result<TestResponse> {
        match command {
            TestCommand::Tap { x, y } => {
                self.click(x, y, PointerButton::Primary)?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::Drag {
                start_x,
                start_y,
                end_x,
                end_y,
                steps,
            } => {
                self.pointer(PointerEvent::Move {
                    point: LayoutPoint::new(start_x, start_y),
                    modifiers: 0,
                })?;
                self.pointer(PointerEvent::Down {
                    point: LayoutPoint::new(start_x, start_y),
                    button: PointerButton::Primary,
                    modifiers: 0,
                })?;
                let steps = steps.max(1);
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    self.pointer(PointerEvent::Move {
                        point: LayoutPoint::new(
                            start_x + (end_x - start_x) * t,
                            start_y + (end_y - start_y) * t,
                        ),
                        modifiers: 0,
                    })?;
                }
                self.pointer(PointerEvent::Up {
                    point: LayoutPoint::new(end_x, end_y),
                    button: PointerButton::Primary,
                    modifiers: 0,
                })?;
                self.pump()?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::TapText { text } => {
                let Some(item) = self.text_items().into_iter().find(|item| item.text.contains(&text)) else {
                    return Ok(TestResponse::Error {
                        message: format!("text `{text}` not found"),
                    });
                };
                self.click(item.x + item.width / 2.0, item.y + item.height / 2.0, PointerButton::Primary)?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::Scroll { x, y, dx, dy } => {
                self.pointer(PointerEvent::Scroll {
                    point: LayoutPoint::new(x, y),
                    delta: LayoutPoint::new(dx, dy),
                    modifiers: 0,
                })?;
                self.pump()?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::TypeText { text } | TestCommand::ImeCommit { text } => {
                self.app.send_event(InputEvent::Ime(ImeEvent::Commit { text }))?;
                self.pump()?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::ImePreedit {
                text,
                cursor_start,
                cursor_end,
            } => {
                let cursor = match (cursor_start, cursor_end) {
                    (Some(start), Some(end)) => Some((start, end)),
                    _ => None,
                };
                self.app
                    .send_event(InputEvent::Ime(ImeEvent::Preedit { text, cursor }))?;
                self.pump()?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::ImeCancel {} => {
                self.app.send_event(InputEvent::Ime(ImeEvent::Cancel))?;
                self.pump()?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::PressKey { key, modifiers } => {
                if let Some(key_code) = key_code_from_test_name(&key) {
                    self.app.send_event(InputEvent::Keyboard(KeyEvent::Down {
                        key_code: key_code.clone(),
                        modifiers,
                    }))?;
                    self.app.send_event(InputEvent::Keyboard(KeyEvent::Up {
                        key_code,
                        modifiers,
                    }))?;
                    self.pump()?;
                }
                Ok(TestResponse::Ok {})
            }
            TestCommand::Screenshot { path } => {
                write_frame_png(&self.frame, path, ScreenshotOptions::default())?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::CaptureScreenshot {} => {
                let path = temp_screenshot_path();
                write_frame_png(&self.frame, &path, ScreenshotOptions::default())?;
                let data = std::fs::read(&path)?;
                let _ = std::fs::remove_file(&path);
                Ok(TestResponse::Screenshot {
                    png_base64: base64::engine::general_purpose::STANDARD.encode(data),
                    width: u32::from(self.frame.width) * ScreenshotOptions::default().cell_width,
                    height: u32::from(self.frame.height) * ScreenshotOptions::default().cell_height,
                })
            }
            TestCommand::GetText {} => Ok(TestResponse::Text {
                items: self.text_items(),
            }),
            TestCommand::GetTree {} => Ok(TestResponse::Tree { nodes: Vec::new() }),
            TestCommand::Wait { ms } => {
                std::thread::sleep(std::time::Duration::from_millis(ms));
                Ok(TestResponse::Ok {})
            }
            TestCommand::Pump {} => {
                self.pump()?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::Quit {} => {
                self.quit_requested = true;
                Ok(TestResponse::Ok {})
            }
            TestCommand::SimulateMouseMove { x, y } => {
                self.pointer(PointerEvent::Move {
                    point: LayoutPoint::new(x, y),
                    modifiers: 0,
                })?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::SimulateRightClick { x, y } => {
                self.click(x, y, PointerButton::Secondary)?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::SimulateResize { width, height } => {
                self.width = width.max(1) as u16;
                self.height = height.max(1) as u16;
                self.pump()?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::ExternalFileHover { .. }
            | TestCommand::ExternalFileDrop { .. }
            | TestCommand::ExternalFileCancel {} => Ok(TestResponse::Error {
                message: "external file drag-and-drop is not supported by the terminal backend".into(),
            }),
            TestCommand::ResolveSelector { .. }
            | TestCommand::TapSelector { .. }
            | TestCommand::ActivateSelector { .. }
            | TestCommand::FocusSelector { .. }
            | TestCommand::HoverSelector { .. }
            | TestCommand::RightClickSelector { .. }
            | TestCommand::ScrollIntoView { .. }
            | TestCommand::FillText { .. }
            | TestCommand::ClearText { .. }
            | TestCommand::Toggle { .. }
            | TestCommand::SelectOption { .. }
            | TestCommand::WaitForSelector { .. }
            | TestCommand::WaitForVisible { .. }
            | TestCommand::WaitForEnabled { .. }
            | TestCommand::WaitForDisabled { .. }
            | TestCommand::WaitForValue { .. }
            | TestCommand::WaitForText { .. }
            | TestCommand::WaitForGone { .. } => Ok(TestResponse::Error {
                message: "semantic selector commands require a semantic terminal tree and are not yet exposed by the terminal backend".into(),
            }),
        }
    }

    fn click(&mut self, x: f32, y: f32, button: PointerButton) -> Result<()> {
        let point = LayoutPoint::new(x, y);
        self.pointer(PointerEvent::Move {
            point,
            modifiers: 0,
        })?;
        self.pointer(PointerEvent::Down {
            point,
            button: button.clone(),
            modifiers: 0,
        })?;
        self.pointer(PointerEvent::Up {
            point,
            button,
            modifiers: 0,
        })?;
        self.pump()
    }

    fn pointer(&mut self, event: PointerEvent) -> Result<()> {
        self.app.send_event(InputEvent::Pointer(event))
    }

    fn pump(&mut self) -> Result<()> {
        self.frame = self.app.render_frame(self.width, self.height)?;
        Ok(())
    }

    fn text_items(&self) -> Vec<TextItem> {
        let mut items = Vec::new();
        for y in 0..self.frame.height {
            let mut line = String::new();
            for x in 0..self.frame.width {
                line.push(self.frame.get(x, y).map(|cell| cell.ch).unwrap_or(' '));
            }
            let trimmed_end = line.trim_end();
            if trimmed_end.trim().is_empty() {
                continue;
            }
            let x = trimmed_end.chars().position(|ch| ch != ' ').unwrap_or(0) as f32;
            let text = trimmed_end.trim_start().to_string();
            items.push(TextItem {
                text,
                x,
                y: f32::from(y),
                width: (trimmed_end.len() as f32 - x).max(1.0),
                height: 1.0,
            });
        }
        items
    }
}

fn key_code_from_test_name(key: &str) -> Option<KeyCode> {
    Some(match key {
        "Backspace" => KeyCode::Backspace,
        "Enter" => KeyCode::Enter,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Tab" => KeyCode::Tab,
        "Delete" => KeyCode::Delete,
        "Escape" | "Esc" => KeyCode::Escape,
        "Space" => KeyCode::Space,
        value if value.chars().count() == 1 => KeyCode::Char(value.chars().next().unwrap()),
        _ => return None,
    })
}

fn temp_screenshot_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fission-terminal-capture-{}-{}.png",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
