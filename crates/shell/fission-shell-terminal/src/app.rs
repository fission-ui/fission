use crate::frame::{TerminalColor, TerminalFrame};
use crate::render::TerminalRenderer;
use crate::screenshot::{write_frame_png, ScreenshotOptions};
use crate::text::TerminalTextMeasurer;
use crate::verify::verify_terminal_ir;
use anyhow::{Context, Result};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode as CtKeyCode, KeyEventKind,
    KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::style::{
    Color as CtColor, Print, ResetColor, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use fission_core::event::ImeEvent;
use fission_core::internal::build_layout_tree;
use fission_core::internal::BuildCtx;
use fission_core::internal::InternalLoweringCx;
use fission_core::ui::{Container, Overlay, Widget, ZStack};
use fission_core::{
    Env, GlobalState, InputEvent, KeyCode, KeyEvent, LayoutEngine, LayoutPoint, LayoutSize,
    LayoutSnapshot, PointerButton, PointerEvent, Runtime, RuntimeState, View, WidgetIdExt,
    WindowTitle,
};
use fission_ir::CoreIR;
use fission_layout::TextMeasurer;
use std::io::{stdout, Stdout, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct TerminalRunOptions {
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub screenshot: Option<PathBuf>,
    pub exit_after_render: bool,
    pub poll_interval: Duration,
}

impl Default for TerminalRunOptions {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            screenshot: None,
            exit_after_render: false,
            poll_interval: Duration::from_millis(33),
        }
    }
}

pub struct TerminalApp<S, W>
where
    S: GlobalState + 'static,
    W: Clone + Into<Widget>,
{
    root: W,
    runtime: Runtime,
    layout_engine: LayoutEngine,
    env: Env,
    sync_env: Option<Box<dyn Fn(&S, &mut Env)>>,
    state_update: Option<Box<dyn FnMut(&mut S, &mut RuntimeState, &Env) -> bool>>,
    exit_request: Option<Box<dyn FnMut(&mut S, &mut RuntimeState, &Env) -> bool>>,
    should_exit: Option<Box<dyn Fn(&S, &RuntimeState, &Env) -> bool>>,
    measurer: Arc<dyn TextMeasurer>,
    last_ir: Option<CoreIR>,
    last_snapshot: Option<LayoutSnapshot>,
    _state: std::marker::PhantomData<S>,
}

impl<S, W> TerminalApp<S, W>
where
    S: GlobalState + Default + 'static,
    W: Clone + Into<Widget>,
{
    pub fn new(root: W) -> Self {
        Self::new_with_global_state(root, S::default())
    }
}

impl<S, W> TerminalApp<S, W>
where
    S: GlobalState + 'static,
    W: Clone + Into<Widget>,
{
    pub fn new_with_global_state(root: W, state: S) -> Self {
        let measurer: Arc<dyn TextMeasurer> = Arc::new(TerminalTextMeasurer);
        let mut runtime = Runtime::default().with_measurer(measurer.clone());
        runtime
            .add_global_state(Box::new(state))
            .expect("failed to register terminal global state");
        let mut env = Env::new(measurer.clone());
        env.viewport_size = LayoutSize::new(100.0, 32.0);
        Self {
            root,
            runtime,
            layout_engine: LayoutEngine::new().with_measurer(measurer.clone()),
            env,
            sync_env: None,
            state_update: None,
            exit_request: None,
            should_exit: None,
            measurer,
            last_ir: None,
            last_snapshot: None,
            _state: std::marker::PhantomData,
        }
    }

    #[doc(hidden)]
    pub fn with_state(root: W, state: S) -> Self {
        Self::new_with_global_state(root, state)
    }

    pub fn with_global_state(mut self, global_state: S) -> Self {
        *self.runtime.get_global_state_mut::<S>().expect(
            "Fission global state must be registered before TerminalApp::with_global_state is called",
        ) = global_state;
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.env.window.title = WindowTitle::plain(title);
        self
    }

    pub fn with_env(mut self, configure: impl FnOnce(&mut Env)) -> Self {
        configure(&mut self.env);
        self
    }

    pub fn with_sync_env<F>(mut self, sync: F) -> Self
    where
        F: Fn(&S, &mut Env) + 'static,
    {
        self.sync_env = Some(Box::new(sync));
        self
    }

    pub fn with_state_update<F>(mut self, update: F) -> Self
    where
        F: FnMut(&mut S, &mut RuntimeState, &Env) -> bool + 'static,
    {
        self.state_update = Some(Box::new(update));
        self
    }

    pub fn with_exit_request<F>(mut self, request: F) -> Self
    where
        F: FnMut(&mut S, &mut RuntimeState, &Env) -> bool + 'static,
    {
        self.exit_request = Some(Box::new(request));
        self
    }

    pub fn with_should_exit<F>(mut self, should_exit: F) -> Self
    where
        F: Fn(&S, &RuntimeState, &Env) -> bool + 'static,
    {
        self.should_exit = Some(Box::new(should_exit));
        self
    }

    pub fn render_frame(&mut self, width: u16, height: u16) -> Result<TerminalFrame> {
        let width = width.max(1);
        let height = height.max(1);
        let viewport = LayoutSize::new(f32::from(width), f32::from(height));
        self.env.viewport_size = viewport;
        self.env.measurer = Some(self.measurer.clone());
        if let Some(sync_env) = &self.sync_env {
            let state = self
                .runtime
                .get_global_state::<S>()
                .context("terminal app state is missing")?;
            sync_env(state, &mut self.env);
            self.env.viewport_size = viewport;
            self.env.measurer = Some(self.measurer.clone());
        }

        let node_tree = self.build_widget_tree(viewport)?;
        let mut cx = InternalLoweringCx::new(
            &self.env,
            &self.runtime.runtime_state,
            Some(&self.measurer),
            self.last_snapshot.as_ref(),
        );
        let root_id = fission_core::internal::lower_widget(&node_tree, &mut cx);
        cx.ir.root = Some(root_id);
        verify_terminal_ir(&cx.ir).context("terminal shell support check failed")?;

        let layout_input_nodes = build_layout_tree(&cx.ir, &self.env);
        self.layout_engine.update(&layout_input_nodes);
        self.layout_engine
            .verify_post_update(&layout_input_nodes, root_id)?;
        let snapshot =
            self.layout_engine
                .compute_layout(&layout_input_nodes, root_id, viewport, &|id| {
                    self.runtime.runtime_state.scroll.get_offset(id)
                })?;

        let renderer = TerminalRenderer::from_theme(&self.env.theme);
        let frame = renderer.render(
            &cx.ir,
            &snapshot,
            &self.runtime.runtime_state.scroll,
            width,
            height,
        )?;
        self.last_ir = Some(cx.ir);
        self.last_snapshot = Some(snapshot);
        Ok(frame)
    }

    pub fn send_event(&mut self, event: InputEvent) -> Result<()> {
        let (Some(ir), Some(snapshot)) = (&self.last_ir, &self.last_snapshot) else {
            return Ok(());
        };
        self.runtime.handle_input(event, ir, snapshot)
    }

    pub fn run(self) -> Result<()> {
        self.run_with_options(TerminalRunOptions::default())
    }

    pub fn run_with_options(mut self, options: TerminalRunOptions) -> Result<()> {
        let (width, height) = terminal_size_or_options(&options)?;
        let mut frame = self.render_frame(width, height)?;
        if let Some(path) = &options.screenshot {
            write_frame_png(&frame, path, ScreenshotOptions::default())?;
        }
        if options.exit_after_render {
            return Ok(());
        }

        let mut stdout = stdout();
        let _guard = TerminalGuard::enter(&mut stdout)?;
        let mut current_size = (width, height);
        render_to_terminal(&mut stdout, &frame, true)?;

        loop {
            if !event::poll(options.poll_interval)? {
                if self.update_state()? {
                    let next_size = terminal_size_or_options(&options)?;
                    let clear = next_size != current_size;
                    current_size = next_size;
                    frame = self.render_frame(next_size.0, next_size.1)?;
                    render_to_terminal(&mut stdout, &frame, clear)?;
                }
                continue;
            }

            let mut dirty = self.update_state()?;
            if self.should_exit()? {
                break;
            }
            let mut clear = false;
            match event::read()? {
                Event::Key(key) if should_exit_key(key.code, key.modifiers) => {
                    if self.request_exit()? {
                        break;
                    }
                    dirty = true;
                }
                Event::Key(key) => {
                    if let Some(input) = map_key_event(key.code, key.kind, key.modifiers) {
                        self.send_event(input)?;
                        dirty = true;
                    }
                }
                Event::Paste(text) => {
                    self.send_event(InputEvent::Ime(ImeEvent::Commit { text }))?;
                    dirty = true;
                }
                Event::Mouse(mouse) => {
                    if let Some(input) =
                        map_mouse_event(mouse.kind, mouse.column, mouse.row, mouse.modifiers)
                    {
                        self.send_event(input)?;
                        dirty = true;
                    }
                }
                Event::Resize(width, height) => {
                    self.send_event(InputEvent::Lifecycle(
                        fission_core::event::LifecycleEvent::Resize {
                            size: LayoutSize::new(f32::from(width), f32::from(height)),
                        },
                    ))?;
                    dirty = true;
                    clear = true;
                }
                Event::FocusGained | Event::FocusLost => {}
            }

            if dirty {
                if self.should_exit()? {
                    break;
                }
                let next_size = terminal_size_or_options(&options)?;
                clear |= next_size != current_size;
                current_size = next_size;
                frame = self.render_frame(next_size.0, next_size.1)?;
                render_to_terminal(&mut stdout, &frame, clear)?;
            }
        }
        Ok(())
    }

    fn request_exit(&mut self) -> Result<bool> {
        let Some(request) = &mut self.exit_request else {
            return Ok(true);
        };
        let mut app_states = std::mem::take(&mut self.runtime.app_states);
        let state = app_states
            .get_mut(&std::any::TypeId::of::<S>())
            .and_then(|state| state.downcast_mut::<S>())
            .context("terminal app state is missing")?;
        let should_exit = request(state, &mut self.runtime.runtime_state, &self.env);
        self.runtime.app_states = app_states;
        Ok(should_exit)
    }

    fn should_exit(&self) -> Result<bool> {
        let Some(should_exit) = &self.should_exit else {
            return Ok(false);
        };
        let state = self
            .runtime
            .get_global_state::<S>()
            .context("terminal app state is missing")?;
        Ok(should_exit(state, &self.runtime.runtime_state, &self.env))
    }

    fn update_state(&mut self) -> Result<bool> {
        let Some(update) = &mut self.state_update else {
            return Ok(false);
        };
        let mut app_states = std::mem::take(&mut self.runtime.app_states);
        let state = app_states
            .get_mut(&std::any::TypeId::of::<S>())
            .and_then(|state| state.downcast_mut::<S>())
            .context("terminal app state is missing")?;
        let changed = update(state, &mut self.runtime.runtime_state, &self.env);
        self.runtime.app_states = app_states;
        Ok(changed)
    }

    fn build_widget_tree(&mut self, viewport: LayoutSize) -> Result<Widget> {
        let state = self
            .runtime
            .get_global_state::<S>()
            .context("terminal app state is missing")?;
        let view = View::new(
            state,
            &self.runtime.runtime_state,
            &self.env,
            self.last_snapshot.as_ref(),
        );
        let mut ctx = BuildCtx::<S>::new();
        let tree = fission_core::build::enter(&mut ctx, &view, || self.root.clone().into());

        self.runtime.clear_reducers();
        let animation_requests = ctx.take_animation_requests();
        let video_nodes = ctx.take_video_registrations();
        let web_nodes = ctx.take_web_registrations();
        let portals_with_ids = ctx.take_portals();
        self.runtime.absorb_registry(ctx.registry);
        for (target, request) in animation_requests {
            self.runtime.enqueue_animation(target, request);
        }
        self.runtime.sync_video_nodes(&video_nodes);
        self.runtime.sync_web_nodes(&web_nodes);

        let portals = portals_with_ids
            .into_iter()
            .map(|(id, node)| {
                if let Some(id) = id {
                    let wrapper_id = fission_ir::WidgetId::derived(id.as_u128(), &[0x0000_F001]);
                    Container::new(node)
                        .width(viewport.width)
                        .height(viewport.height)
                        .id(wrapper_id)
                } else {
                    node
                }
            })
            .collect::<Vec<_>>();

        if portals.is_empty() {
            Ok(tree)
        } else {
            Ok(Overlay {
                id: None,
                content: Container::new(tree)
                    .width(viewport.width)
                    .height(viewport.height)
                    .into(),
                overlay: ZStack {
                    id: None,
                    children: portals,
                }
                .into(),
            }
            .into())
        }
    }
}

fn terminal_size_or_options(options: &TerminalRunOptions) -> Result<(u16, u16)> {
    let (term_width, term_height) = terminal::size().unwrap_or((100, 32));
    Ok((
        options.width.unwrap_or(term_width).max(1),
        options.height.unwrap_or(term_height).max(1),
    ))
}

fn render_to_terminal(stdout: &mut Stdout, frame: &TerminalFrame, clear: bool) -> Result<()> {
    queue!(stdout, MoveTo(0, 0))?;
    if clear {
        queue!(stdout, Clear(ClearType::All))?;
    }
    for y in 0..frame.height {
        queue!(stdout, MoveTo(0, y))?;
        for x in 0..frame.width {
            let Some(cell) = frame.get(x, y) else {
                continue;
            };
            queue!(
                stdout,
                SetForegroundColor(to_crossterm_color(cell.style.fg)),
                SetBackgroundColor(to_crossterm_color(cell.style.bg)),
                Print(cell.ch)
            )?;
        }
    }
    queue!(stdout, ResetColor)?;
    stdout.flush()?;
    Ok(())
}

fn to_crossterm_color(color: TerminalColor) -> CtColor {
    CtColor::Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}

fn should_exit_key(code: CtKeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, CtKeyCode::Esc)
        || matches!(code, CtKeyCode::Char('q'))
        || (matches!(code, CtKeyCode::Char('c')) && modifiers.contains(KeyModifiers::CONTROL))
}

fn map_key_event(
    code: CtKeyCode,
    kind: KeyEventKind,
    modifiers: KeyModifiers,
) -> Option<InputEvent> {
    let key_code = match code {
        CtKeyCode::Backspace => KeyCode::Backspace,
        CtKeyCode::Enter => KeyCode::Enter,
        CtKeyCode::Left => KeyCode::Left,
        CtKeyCode::Right => KeyCode::Right,
        CtKeyCode::Up => KeyCode::Up,
        CtKeyCode::Down => KeyCode::Down,
        CtKeyCode::Home => KeyCode::Home,
        CtKeyCode::End => KeyCode::End,
        CtKeyCode::PageUp => KeyCode::PageUp,
        CtKeyCode::PageDown => KeyCode::PageDown,
        CtKeyCode::Tab | CtKeyCode::BackTab => KeyCode::Tab,
        CtKeyCode::Delete => KeyCode::Delete,
        CtKeyCode::Insert
        | CtKeyCode::F(_)
        | CtKeyCode::Null
        | CtKeyCode::CapsLock
        | CtKeyCode::ScrollLock
        | CtKeyCode::NumLock
        | CtKeyCode::PrintScreen
        | CtKeyCode::Pause
        | CtKeyCode::Menu
        | CtKeyCode::KeypadBegin
        | CtKeyCode::Media(_)
        | CtKeyCode::Modifier(_) => return None,
        CtKeyCode::Esc => KeyCode::Escape,
        CtKeyCode::Char(' ') => KeyCode::Space,
        CtKeyCode::Char(ch) => KeyCode::Char(ch),
    };
    let modifiers = modifier_bits(modifiers);
    match kind {
        KeyEventKind::Press | KeyEventKind::Repeat => Some(InputEvent::Keyboard(KeyEvent::Down {
            key_code,
            modifiers,
        })),
        KeyEventKind::Release => Some(InputEvent::Keyboard(KeyEvent::Up {
            key_code,
            modifiers,
        })),
    }
}

fn map_mouse_event(
    kind: MouseEventKind,
    column: u16,
    row: u16,
    modifiers: KeyModifiers,
) -> Option<InputEvent> {
    let point = LayoutPoint::new(f32::from(column), f32::from(row));
    let modifiers = modifier_bits(modifiers);
    match kind {
        MouseEventKind::Down(button) => Some(InputEvent::Pointer(PointerEvent::Down {
            point,
            button: map_mouse_button(button),
            modifiers,
        })),
        MouseEventKind::Up(button) => Some(InputEvent::Pointer(PointerEvent::Up {
            point,
            button: map_mouse_button(button),
            modifiers,
        })),
        MouseEventKind::Drag(_) | MouseEventKind::Moved => {
            Some(InputEvent::Pointer(PointerEvent::Move { point, modifiers }))
        }
        MouseEventKind::ScrollDown => Some(InputEvent::Pointer(PointerEvent::Scroll {
            point,
            delta: LayoutPoint::new(0.0, 3.0),
            modifiers,
        })),
        MouseEventKind::ScrollUp => Some(InputEvent::Pointer(PointerEvent::Scroll {
            point,
            delta: LayoutPoint::new(0.0, -3.0),
            modifiers,
        })),
        MouseEventKind::ScrollLeft => Some(InputEvent::Pointer(PointerEvent::Scroll {
            point,
            delta: LayoutPoint::new(-3.0, 0.0),
            modifiers,
        })),
        MouseEventKind::ScrollRight => Some(InputEvent::Pointer(PointerEvent::Scroll {
            point,
            delta: LayoutPoint::new(3.0, 0.0),
            modifiers,
        })),
    }
}

fn map_mouse_button(button: MouseButton) -> PointerButton {
    match button {
        MouseButton::Left => PointerButton::Primary,
        MouseButton::Right => PointerButton::Secondary,
        MouseButton::Middle => PointerButton::Middle,
    }
}

fn modifier_bits(modifiers: KeyModifiers) -> u8 {
    let mut bits = 0;
    if modifiers.contains(KeyModifiers::SHIFT) {
        bits |= fission_core::event::MOD_SHIFT;
    }
    if modifiers.contains(KeyModifiers::ALT) {
        bits |= fission_core::event::MOD_ALT;
    }
    if modifiers.contains(KeyModifiers::CONTROL) {
        bits |= fission_core::event::MOD_CTRL;
    }
    if modifiers.contains(KeyModifiers::SUPER) {
        bits |= fission_core::event::MOD_SUPER;
    }
    bits
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter(stdout: &mut Stdout) -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = stdout();
        let _ = execute!(
            stdout,
            Show,
            ResetColor,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}
