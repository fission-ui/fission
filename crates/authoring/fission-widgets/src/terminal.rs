use anyhow::{Context, Result};
use arboard::Clipboard;
use fission_core::event::{ImeEvent, InputEvent, KeyCode, KeyEvent, PointerEvent};
use fission_core::op::Color;
use fission_core::ui::custom_render::{CustomEventResult, CustomHitResult, CustomRenderObject};
use fission_core::ui::{CustomNode, Node};
use fission_core::{
    AppState, BuildCtx, FlexDirection, LowerDyn, LoweringContext, NodeBuilder, View, Widget,
};
use fission_ir::op::{AlignItems, Fill, LayoutOp, PaintOp, TextRun, TextStyle};
use fission_ir::{NodeId, Op};
use fission_layout::{LayoutPoint, LayoutRect};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::fmt::{Debug, Formatter};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use wezterm_surface::{CursorShape, CursorVisibility};
use wezterm_term::color::{ColorAttribute, ColorPalette};
use wezterm_term::config::TerminalConfiguration;
use wezterm_term::input::{
    KeyCode as TermKeyCode, KeyModifiers, MouseButton as TermMouseButton, MouseEvent,
    MouseEventKind,
};
use wezterm_term::{Line, Terminal as WezTerminal, TerminalSize};

const DEFAULT_FONT_SIZE: f32 = 13.0;
const DEFAULT_LINE_HEIGHT: f32 = 18.0;
const DEFAULT_PADDING_X: f32 = 10.0;
const DEFAULT_PADDING_Y: f32 = 8.0;
const MIN_COLS: usize = 20;
const MIN_ROWS: usize = 4;
const READ_BUF_SIZE: usize = 4096;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Default)]
pub struct TerminalLaunchConfig {
    pub cwd: Option<PathBuf>,
    pub program: Option<String>,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct TerminalSession {
    inner: Arc<TerminalSessionInner>,
}

impl Debug for TerminalSession {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalSession")
            .field("id", &self.inner.id)
            .field("focused", &self.inner.focused.load(Ordering::Relaxed))
            .field(
                "scrollback_offset",
                &self.inner.scrollback_offset.load(Ordering::Relaxed),
            )
            .finish()
    }
}

struct TerminalSessionInner {
    id: u64,
    terminal: Mutex<WezTerminal>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    dirty: AtomicBool,
    focused: AtomicBool,
    scrollback_offset: AtomicUsize,
    cols: AtomicUsize,
    rows: AtomicUsize,
    exited: AtomicBool,
    selection: Mutex<Option<TerminalSelection>>,
    selection_drag_active: AtomicBool,
}

impl Drop for TerminalSessionInner {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

#[derive(Debug)]
struct FissionTerminalConfig {
    palette: ColorPalette,
    scrollback: usize,
}

impl Default for FissionTerminalConfig {
    fn default() -> Self {
        Self {
            palette: ColorPalette::default(),
            scrollback: 10_000,
        }
    }
}

impl TerminalConfiguration for FissionTerminalConfig {
    fn scrollback_size(&self) -> usize {
        self.scrollback
    }

    fn color_palette(&self) -> ColorPalette {
        self.palette.clone()
    }
}

#[derive(Clone)]
struct SharedWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner
            .lock()
            .map_err(|_| std::io::Error::other("terminal writer poisoned"))?
            .write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner
            .lock()
            .map_err(|_| std::io::Error::other("terminal writer poisoned"))?
            .flush()
    }
}

#[derive(Debug, Clone)]
pub struct TerminalView {
    pub session: Arc<TerminalSession>,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
}

impl TerminalView {
    pub fn new(session: Arc<TerminalSession>, viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            session,
            viewport_width,
            viewport_height,
            font_size: DEFAULT_FONT_SIZE,
            line_height: DEFAULT_LINE_HEIGHT,
            padding_x: DEFAULT_PADDING_X,
            padding_y: DEFAULT_PADDING_Y,
        }
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    pub fn padding(mut self, x: f32, y: f32) -> Self {
        self.padding_x = x;
        self.padding_y = y;
        self
    }
}

impl<S: AppState> Widget<S> for TerminalView {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        self.session.resize_for_viewport(
            self.viewport_width,
            self.viewport_height,
            self.font_size,
            self.line_height,
        );
        let render_node = Arc::new(TerminalRenderNode::new(
            self.session.clone(),
            self.session.snapshot(),
            self.viewport_width,
            self.viewport_height,
            self.font_size,
            self.line_height,
            self.padding_x,
            self.padding_y,
        ));
        let lowerer: Arc<dyn LowerDyn> = render_node.clone();
        let render_object: Arc<dyn CustomRenderObject> = render_node;
        Node::Custom(CustomNode {
            debug_tag: format!("TerminalView({})", self.session.id()),
            lowerer: Some(lowerer),
            render_object: Some(render_object),
        })
    }
}

#[derive(Debug, Clone)]
struct TerminalSnapshot {
    lines: Vec<Line>,
    cols: usize,
    rows: usize,
    cursor_x: usize,
    cursor_y: usize,
    cursor_shape: CursorShape,
    cursor_visible: bool,
    palette: ColorPalette,
    title: String,
    scrollback_offset: usize,
    selection: Option<TerminalSelection>,
    mouse_grabbed: bool,
    alt_screen_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalSelection {
    anchor_row: usize,
    anchor_col: usize,
    focus_row: usize,
    focus_col: usize,
}

#[derive(Debug)]
struct TerminalRenderNode {
    session: Arc<TerminalSession>,
    snapshot: TerminalSnapshot,
    viewport_width: f32,
    viewport_height: f32,
    font_size: f32,
    line_height: f32,
    char_width: f32,
    padding_x: f32,
    padding_y: f32,
}

#[derive(Debug, Clone, PartialEq, Hash)]
struct TerminalRunStyle {
    fg: Color,
    bg: Option<Color>,
    underline: bool,
}

impl TerminalSelection {
    fn normalized(self) -> Self {
        if (self.anchor_row, self.anchor_col) <= (self.focus_row, self.focus_col) {
            self
        } else {
            Self {
                anchor_row: self.focus_row,
                anchor_col: self.focus_col,
                focus_row: self.anchor_row,
                focus_col: self.anchor_col,
            }
        }
    }

    fn is_empty(self) -> bool {
        self.anchor_row == self.focus_row && self.anchor_col == self.focus_col
    }

    fn range_for_row(self, row: usize, cols: usize) -> Option<(usize, usize)> {
        let normalized = self.normalized();
        if row < normalized.anchor_row || row > normalized.focus_row {
            return None;
        }
        let start = if row == normalized.anchor_row {
            normalized.anchor_col.min(cols)
        } else {
            0
        };
        let end = if row == normalized.focus_row {
            normalized.focus_col.min(cols)
        } else {
            cols
        };
        (start < end).then_some((start, end))
    }
}

impl TerminalRenderNode {
    fn new(
        session: Arc<TerminalSession>,
        snapshot: TerminalSnapshot,
        viewport_width: f32,
        viewport_height: f32,
        font_size: f32,
        line_height: f32,
        padding_x: f32,
        padding_y: f32,
    ) -> Self {
        Self {
            session,
            snapshot,
            viewport_width,
            viewport_height,
            font_size,
            line_height,
            char_width: font_size * 0.6,
            padding_x,
            padding_y,
        }
    }

    fn cursor_rect(&self, node_rect: LayoutRect) -> Option<LayoutRect> {
        if !self.snapshot.cursor_visible || self.snapshot.scrollback_offset > 0 {
            return None;
        }
        if self.snapshot.cursor_y >= self.snapshot.rows {
            return None;
        }
        let x =
            node_rect.origin.x + self.padding_x + self.snapshot.cursor_x as f32 * self.char_width;
        let y =
            node_rect.origin.y + self.padding_y + self.snapshot.cursor_y as f32 * self.line_height;
        let (width, height, y_offset) = match self.snapshot.cursor_shape {
            CursorShape::BlinkingUnderline | CursorShape::SteadyUnderline => {
                (self.char_width.max(4.0), 2.0, self.line_height - 2.0)
            }
            CursorShape::BlinkingBlock | CursorShape::SteadyBlock => {
                (self.char_width.max(6.0), self.line_height, 0.0)
            }
            CursorShape::Default | CursorShape::BlinkingBar | CursorShape::SteadyBar => {
                (2.0, self.line_height, 0.0)
            }
        };
        Some(LayoutRect::new(x, y + y_offset, width, height))
    }

    fn point_to_cell(
        &self,
        point: LayoutPoint,
        node_rect: LayoutRect,
    ) -> (usize, usize, isize, isize) {
        let local_x = (point.x - node_rect.origin.x - self.padding_x).max(0.0);
        let local_y = (point.y - node_rect.origin.y - self.padding_y).max(0.0);
        let col = (local_x / self.char_width).floor().max(0.0) as usize;
        let row = (local_y / self.line_height).floor().max(0.0) as usize;
        let col = col.min(self.snapshot.cols.saturating_sub(1));
        let row = row.min(self.snapshot.rows.saturating_sub(1));
        let x_pixel_offset = (local_x - col as f32 * self.char_width).round() as isize;
        let y_pixel_offset = (local_y - row as f32 * self.line_height).round() as isize;
        (row, col, x_pixel_offset, y_pixel_offset)
    }

    fn selection_range_for_row(&self, row: usize) -> Option<(usize, usize)> {
        self.snapshot
            .selection
            .and_then(|selection| selection.range_for_row(row, self.snapshot.cols))
    }

    fn style_for_cell(&self, attrs: &wezterm_term::CellAttributes) -> TerminalRunStyle {
        let mut fg = self.snapshot.palette.resolve_fg(attrs.foreground());
        let mut bg = self.snapshot.palette.resolve_bg(attrs.background());
        let underline = attrs.underline() != wezterm_term::Underline::None;

        if attrs.reverse() {
            std::mem::swap(&mut fg, &mut bg);
        }

        let mut fg_color = to_ir_color(fg);
        if attrs.intensity() == wezterm_term::Intensity::Half {
            fg_color = dim_color(fg_color, 0.72);
        }
        let bg_color = if attrs.background() == ColorAttribute::Default && !attrs.reverse() {
            None
        } else {
            Some(to_ir_color(bg))
        };

        TerminalRunStyle {
            fg: fg_color,
            bg: bg_color,
            underline,
        }
    }

    fn row_runs(&self, line: &Line) -> Vec<TextRun> {
        let mut runs = Vec::new();
        let mut current_style: Option<TerminalRunStyle> = None;
        let mut current_text = String::new();
        let mut cursor_col = 0usize;

        for cell in line.visible_cells() {
            let cell_index = cell.cell_index();
            if cell_index > cursor_col {
                let gap = " ".repeat(cell_index - cursor_col);
                append_run(
                    &mut runs,
                    &mut current_style,
                    &mut current_text,
                    TerminalRunStyle {
                        fg: to_ir_color(self.snapshot.palette.foreground),
                        bg: None,
                        underline: false,
                    },
                    gap,
                    self.font_size,
                );
            }

            let mut text = if cell.attrs().invisible() {
                " ".repeat(cell.width().max(1))
            } else {
                cell.str().to_string()
            };
            if text.is_empty() {
                text.push(' ');
            }
            let style = self.style_for_cell(cell.attrs());
            append_run(
                &mut runs,
                &mut current_style,
                &mut current_text,
                style,
                text,
                self.font_size,
            );
            cursor_col = cell_index + cell.width().max(1);
        }

        if cursor_col < self.snapshot.cols {
            append_run(
                &mut runs,
                &mut current_style,
                &mut current_text,
                TerminalRunStyle {
                    fg: to_ir_color(self.snapshot.palette.foreground),
                    bg: None,
                    underline: false,
                },
                " ".repeat(self.snapshot.cols - cursor_col),
                self.font_size,
            );
        }

        flush_run(
            &mut runs,
            &mut current_style,
            &mut current_text,
            self.font_size,
        );
        if runs.is_empty() {
            runs.push(TextRun {
                text: " ".into(),
                style: TextStyle {
                    font_size: self.font_size,
                    color: to_ir_color(self.snapshot.palette.foreground),
                    underline: false,
                    background_color: None,
                },
            });
        }
        runs
    }
}

impl LowerDyn for TerminalRenderNode {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let outer_height = self
            .viewport_height
            .max(self.line_height * self.snapshot.rows as f32 + self.padding_y * 2.0);
        let outer_width = self
            .viewport_width
            .max(self.char_width * self.snapshot.cols as f32 + self.padding_x * 2.0);

        let bg_paint = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(to_ir_color(self.snapshot.palette.background))),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
        )
        .build(cx);

        let mut row_ids = Vec::with_capacity(self.snapshot.lines.len());
        let text_width = outer_width - self.padding_x * 2.0;
        for (row_idx, line) in self.snapshot.lines.iter().enumerate() {
            let row_paint = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRichText {
                    runs: self.row_runs(line),
                    wrap: false,
                    caret_index: None,
                }),
            )
            .build(cx);

            let row_layer = if let Some((sel_start, sel_end)) =
                self.selection_range_for_row(row_idx)
            {
                let mut builder = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::ZStack));
                let rect = NodeBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: Some(Fill::Solid(to_ir_color(self.snapshot.palette.selection_bg))),
                        stroke: None,
                        corner_radius: 0.0,
                        shadow: None,
                    }),
                )
                .build(cx);
                let mut positioned = NodeBuilder::new(
                    cx.next_node_id(),
                    Op::Layout(LayoutOp::Positioned {
                        left: Some(sel_start as f32 * self.char_width),
                        top: Some(0.0),
                        right: None,
                        bottom: None,
                        width: Some((sel_end.saturating_sub(sel_start)) as f32 * self.char_width),
                        height: Some(self.line_height),
                    }),
                );
                positioned.add_child(rect);
                builder.add_child(positioned.build(cx));
                builder.add_child(row_paint);
                builder.build(cx)
            } else {
                row_paint
            };

            let row_box = {
                let id = cx.next_node_id();
                let mut builder = NodeBuilder::new(
                    id,
                    Op::Layout(LayoutOp::Box {
                        width: Some(text_width),
                        height: Some(self.line_height),
                        min_width: None,
                        max_width: None,
                        min_height: None,
                        max_height: None,
                        padding: [0.0; 4],
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        aspect_ratio: None,
                    }),
                );
                builder.add_child(row_layer);
                builder.build(cx)
            };
            row_ids.push(row_box);
        }

        let content_column = {
            let id = cx.next_node_id();
            let mut builder = NodeBuilder::new(
                id,
                Op::Layout(LayoutOp::Flex {
                    direction: FlexDirection::Column,
                    wrap: fission_ir::op::FlexWrap::NoWrap,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    padding: [
                        self.padding_y,
                        self.padding_x,
                        self.padding_y,
                        self.padding_x,
                    ],
                    gap: None,
                    align_items: AlignItems::Stretch,
                    justify_content: fission_ir::op::JustifyContent::Start,
                }),
            );
            builder.add_children(row_ids);
            builder.build(cx)
        };

        let cursor_box = self
            .cursor_rect(LayoutRect::new(0.0, 0.0, outer_width, outer_height))
            .map(|rect| {
                let cursor_paint = NodeBuilder::new(
                    cx.next_node_id(),
                    Op::Paint(PaintOp::DrawRect {
                        fill: Some(Fill::Solid(to_ir_color(self.snapshot.palette.cursor_bg))),
                        stroke: None,
                        corner_radius: 0.0,
                        shadow: None,
                    }),
                )
                .build(cx);
                let id = cx.next_node_id();
                let mut builder = NodeBuilder::new(
                    id,
                    Op::Layout(LayoutOp::Positioned {
                        left: Some(rect.origin.x),
                        top: Some(rect.origin.y),
                        right: None,
                        bottom: None,
                        width: Some(rect.size.width),
                        height: Some(rect.size.height),
                    }),
                );
                builder.add_child(cursor_paint);
                builder.build(cx)
            });

        let layered = {
            let id = cx.next_node_id();
            let mut builder = NodeBuilder::new(id, Op::Layout(LayoutOp::ZStack));
            builder.add_child(bg_paint);
            builder.add_child(content_column);
            if let Some(cursor_box) = cursor_box {
                builder.add_child(cursor_box);
            }
            builder.build(cx)
        };

        let outer = {
            let id = cx.next_node_id();
            let mut builder = NodeBuilder::new(
                id,
                Op::Layout(LayoutOp::Box {
                    width: Some(outer_width),
                    height: Some(outer_height),
                    min_width: Some(outer_width),
                    max_width: None,
                    min_height: Some(outer_height),
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    aspect_ratio: None,
                }),
            );
            builder.add_child(layered);
            builder.build(cx)
        };

        outer
    }

    fn stable_key(&self) -> u64 {
        self.session.id()
    }
}

impl CustomRenderObject for TerminalRenderNode {
    fn is_runtime_dynamic(&self) -> bool {
        true
    }

    fn accepts_text_input(&self) -> bool {
        true
    }

    fn hit_test(&self, _local_point: LayoutPoint, _node_rect: LayoutRect) -> CustomHitResult {
        CustomHitResult::inside(None)
    }

    fn handle_event(
        &self,
        _node_id: NodeId,
        event: &InputEvent,
        node_rect: LayoutRect,
    ) -> CustomEventResult {
        match event {
            InputEvent::Pointer(PointerEvent::Down { point, button }) => {
                self.session.set_focused(true);
                let (row, col, x_pixel_offset, y_pixel_offset) =
                    self.point_to_cell(*point, node_rect);
                if self.snapshot.mouse_grabbed
                    || *button != fission_core::event::PointerButton::Primary
                {
                    let _ = self.session.send_mouse_event(
                        row,
                        col,
                        MouseEventKind::Press,
                        map_pointer_button(button),
                        0,
                        x_pixel_offset,
                        y_pixel_offset,
                    );
                } else {
                    self.session.begin_selection(row, col);
                }
                CustomEventResult::consumed()
            }
            InputEvent::Pointer(PointerEvent::Move { point }) => {
                let (row, col, x_pixel_offset, y_pixel_offset) =
                    self.point_to_cell(*point, node_rect);
                if self.snapshot.mouse_grabbed {
                    let _ = self.session.send_mouse_event(
                        row,
                        col,
                        MouseEventKind::Move,
                        TermMouseButton::None,
                        0,
                        x_pixel_offset,
                        y_pixel_offset,
                    );
                } else if self.session.is_selecting() {
                    self.session.update_selection(row, col);
                }
                CustomEventResult::consumed()
            }
            InputEvent::Pointer(PointerEvent::Up { point, button }) => {
                let (row, col, x_pixel_offset, y_pixel_offset) =
                    self.point_to_cell(*point, node_rect);
                if self.snapshot.mouse_grabbed
                    || *button != fission_core::event::PointerButton::Primary
                {
                    let _ = self.session.send_mouse_event(
                        row,
                        col,
                        MouseEventKind::Release,
                        map_pointer_button(button),
                        0,
                        x_pixel_offset,
                        y_pixel_offset,
                    );
                } else {
                    self.session.finish_selection(row, col);
                }
                CustomEventResult::consumed()
            }
            InputEvent::Pointer(PointerEvent::Scroll { point, delta }) => {
                let lines = if delta.y.abs() < 1.0 {
                    1
                } else {
                    delta.y.abs().round() as usize
                };
                if self.snapshot.mouse_grabbed || self.snapshot.alt_screen_active {
                    let (row, col, x_pixel_offset, y_pixel_offset) =
                        self.point_to_cell(*point, node_rect);
                    let button = if delta.y >= 0.0 {
                        TermMouseButton::WheelDown(lines)
                    } else {
                        TermMouseButton::WheelUp(lines)
                    };
                    let _ = self.session.send_mouse_event(
                        row,
                        col,
                        MouseEventKind::Press,
                        button,
                        0,
                        x_pixel_offset,
                        y_pixel_offset,
                    );
                } else {
                    let signed_lines = if delta.y.abs() < 1.0 {
                        delta.y.signum() as i32
                    } else {
                        (delta.y / self.line_height).round() as i32
                    };
                    self.session.scroll_scrollback(signed_lines);
                }
                CustomEventResult::consumed()
            }
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }) => {
                if is_primary_shortcut(*modifiers) {
                    match key_code {
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            if self
                                .session
                                .copy_selection_to_clipboard()
                                .ok()
                                .flatten()
                                .is_some()
                            {
                                return CustomEventResult::consumed();
                            }
                        }
                        KeyCode::Char('v') | KeyCode::Char('V') => {
                            if self.session.paste_clipboard().is_ok() {
                                return CustomEventResult::consumed();
                            }
                        }
                        _ => {}
                    }
                }
                if self
                    .session
                    .send_key(map_key_code(key_code), *modifiers)
                    .is_ok()
                {
                    return CustomEventResult::consumed();
                }
                CustomEventResult::ignored()
            }
            InputEvent::Ime(ImeEvent::Commit { text }) => {
                if self.session.send_text(text).is_ok() {
                    return CustomEventResult::consumed();
                }
                CustomEventResult::ignored()
            }
            InputEvent::Ime(ImeEvent::Preedit { .. }) => CustomEventResult::consumed(),
            _ => CustomEventResult::ignored(),
        }
    }

    fn ime_cursor_area(&self, node_rect: LayoutRect) -> Option<LayoutRect> {
        self.cursor_rect(node_rect)
    }

    fn blur_actions(&self, _node_id: NodeId) -> Vec<(NodeId, fission_core::ActionEnvelope)> {
        self.session.set_focused(false);
        Vec::new()
    }
}

impl TerminalSession {
    pub fn spawn(config: TerminalLaunchConfig) -> Result<Arc<Self>> {
        let pty_system = native_pty_system();
        let initial_size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system
            .openpty(initial_size)
            .context("failed to create PTY")?;

        let mut command = if let Some(program) = &config.program {
            let mut command = CommandBuilder::new(program);
            command.args(config.args.iter());
            command
        } else {
            CommandBuilder::new_default_prog()
        };
        if let Some(cwd) = &config.cwd {
            command.cwd(cwd.as_os_str());
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        for (key, value) in &config.env {
            command.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn terminal child")?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;
        let writer = Arc::new(Mutex::new(
            pair.master
                .take_writer()
                .context("failed to acquire PTY writer")?,
        ));
        let terminal_config = Arc::new(FissionTerminalConfig::default());
        let terminal = WezTerminal::new(
            TerminalSize {
                rows: initial_size.rows as usize,
                cols: initial_size.cols as usize,
                pixel_width: 0,
                pixel_height: 0,
                dpi: 96,
            },
            terminal_config,
            "fission",
            env!("CARGO_PKG_VERSION"),
            Box::new(SharedWriter {
                inner: writer.clone(),
            }),
        );

        let session = Arc::new(Self {
            inner: Arc::new(TerminalSessionInner {
                id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
                terminal: Mutex::new(terminal),
                master: Mutex::new(pair.master),
                child: Mutex::new(child),
                writer: writer.clone(),
                dirty: AtomicBool::new(true),
                focused: AtomicBool::new(false),
                scrollback_offset: AtomicUsize::new(0),
                cols: AtomicUsize::new(initial_size.cols as usize),
                rows: AtomicUsize::new(initial_size.rows as usize),
                exited: AtomicBool::new(false),
                selection: Mutex::new(None),
                selection_drag_active: AtomicBool::new(false),
            }),
        });

        let session_for_thread = session.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; READ_BUF_SIZE];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        session_for_thread
                            .inner
                            .exited
                            .store(true, Ordering::Relaxed);
                        session_for_thread.mark_dirty();
                        break;
                    }
                    Ok(n) => {
                        if let Ok(mut terminal) = session_for_thread.inner.terminal.lock() {
                            terminal.advance_bytes(&buf[..n]);
                        }
                        session_for_thread.mark_dirty();
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        session_for_thread
                            .inner
                            .exited
                            .store(true, Ordering::Relaxed);
                        session_for_thread.mark_dirty();
                        break;
                    }
                }
            }
        });

        Ok(session)
    }

    pub fn id(&self) -> u64 {
        self.inner.id
    }

    pub fn mark_dirty(&self) {
        self.inner.dirty.store(true, Ordering::Relaxed);
    }

    pub fn take_dirty(&self) -> bool {
        self.inner.dirty.swap(false, Ordering::Relaxed)
    }

    pub fn set_focused(&self, focused: bool) {
        self.inner.focused.store(focused, Ordering::Relaxed);
        if let Ok(mut terminal) = self.inner.terminal.lock() {
            terminal.focus_changed(focused);
        }
        self.mark_dirty();
    }

    pub fn is_selecting(&self) -> bool {
        self.inner.selection_drag_active.load(Ordering::Relaxed)
    }

    pub fn begin_selection(&self, row: usize, col: usize) {
        if let Ok(mut selection) = self.inner.selection.lock() {
            *selection = Some(TerminalSelection {
                anchor_row: row,
                anchor_col: col,
                focus_row: row,
                focus_col: col,
            });
        }
        self.inner
            .selection_drag_active
            .store(true, Ordering::Relaxed);
        self.mark_dirty();
    }

    pub fn update_selection(&self, row: usize, col: usize) {
        if let Ok(mut selection) = self.inner.selection.lock() {
            if let Some(selection) = selection.as_mut() {
                selection.focus_row = row;
                selection.focus_col = col;
                self.mark_dirty();
            }
        }
    }

    pub fn finish_selection(&self, row: usize, col: usize) {
        self.inner
            .selection_drag_active
            .store(false, Ordering::Relaxed);
        self.update_selection(row, col);
    }

    pub fn clear_selection(&self) {
        if let Ok(mut selection) = self.inner.selection.lock() {
            *selection = None;
        }
        self.inner
            .selection_drag_active
            .store(false, Ordering::Relaxed);
        self.mark_dirty();
    }

    pub fn send_text(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        self.clear_selection();
        self.inner.scrollback_offset.store(0, Ordering::Relaxed);
        let mut writer = self.writer_lock().context("terminal writer unavailable")?;
        writer.write_all(text.as_bytes())?;
        writer.flush()?;
        Ok(())
    }

    pub fn send_key(&self, key: TermKeyCode, modifiers: u8) -> Result<()> {
        self.inner.scrollback_offset.store(0, Ordering::Relaxed);
        let mut terminal = self
            .inner
            .terminal
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal state poisoned"))?;
        terminal.key_down(key, map_modifiers(modifiers))?;
        Ok(())
    }

    pub fn paste_clipboard(&self) -> Result<()> {
        let mut clipboard = Clipboard::new().context("terminal clipboard unavailable")?;
        let text = clipboard.get_text().unwrap_or_default();
        if text.is_empty() {
            return Ok(());
        }
        self.clear_selection();
        self.inner.scrollback_offset.store(0, Ordering::Relaxed);
        let mut terminal = self
            .inner
            .terminal
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal state poisoned"))?;
        terminal.send_paste(&text)?;
        self.mark_dirty();
        Ok(())
    }

    pub fn copy_selection_to_clipboard(&self) -> Result<Option<String>> {
        let snapshot = self.snapshot();
        let Some(selection) = snapshot.selection else {
            return Ok(None);
        };
        if selection.is_empty() {
            return Ok(None);
        }
        let text = extract_selection_text(&snapshot, selection);
        if text.is_empty() {
            return Ok(None);
        }
        let mut clipboard = Clipboard::new().context("terminal clipboard unavailable")?;
        clipboard.set_text(text.clone())?;
        Ok(Some(text))
    }

    pub fn send_mouse_event(
        &self,
        row: usize,
        col: usize,
        kind: MouseEventKind,
        button: TermMouseButton,
        modifiers: u8,
        x_pixel_offset: isize,
        y_pixel_offset: isize,
    ) -> Result<()> {
        self.inner.scrollback_offset.store(0, Ordering::Relaxed);
        let mut terminal = self
            .inner
            .terminal
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal state poisoned"))?;
        terminal.mouse_event(MouseEvent {
            kind,
            x: col,
            y: row as i64,
            x_pixel_offset,
            y_pixel_offset,
            button,
            modifiers: map_modifiers(modifiers),
        })?;
        self.mark_dirty();
        Ok(())
    }

    pub fn scroll_scrollback(&self, delta_lines: i32) {
        let (max_scrollback, current) = {
            let terminal = match self.inner.terminal.lock() {
                Ok(terminal) => terminal,
                Err(_) => return,
            };
            let screen = terminal.screen();
            let max = screen
                .scrollback_rows()
                .saturating_sub(screen.physical_rows);
            (max, self.inner.scrollback_offset.load(Ordering::Relaxed))
        };

        let next = if delta_lines < 0 {
            current.saturating_add(delta_lines.unsigned_abs() as usize)
        } else {
            current.saturating_sub(delta_lines as usize)
        }
        .min(max_scrollback);

        if next != current {
            self.inner.scrollback_offset.store(next, Ordering::Relaxed);
            self.mark_dirty();
        }
    }

    pub fn resize_for_viewport(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        font_size: f32,
        line_height: f32,
    ) {
        let char_width = (font_size * 0.6).max(1.0);
        let cols = ((viewport_width - DEFAULT_PADDING_X * 2.0) / char_width)
            .floor()
            .max(MIN_COLS as f32) as usize;
        let rows = ((viewport_height - DEFAULT_PADDING_Y * 2.0) / line_height)
            .floor()
            .max(MIN_ROWS as f32) as usize;

        let prev_cols = self.inner.cols.load(Ordering::Relaxed);
        let prev_rows = self.inner.rows.load(Ordering::Relaxed);
        if prev_cols == cols && prev_rows == rows {
            return;
        }

        self.inner.cols.store(cols, Ordering::Relaxed);
        self.inner.rows.store(rows, Ordering::Relaxed);

        if let Ok(master) = self.inner.master.lock() {
            let _ = master.resize(PtySize {
                rows: rows.min(u16::MAX as usize) as u16,
                cols: cols.min(u16::MAX as usize) as u16,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
        if let Ok(mut terminal) = self.inner.terminal.lock() {
            terminal.resize(TerminalSize {
                rows,
                cols,
                pixel_width: viewport_width.max(0.0) as usize,
                pixel_height: viewport_height.max(0.0) as usize,
                dpi: 96,
            });
        }
        self.mark_dirty();
    }

    fn snapshot(&self) -> TerminalSnapshot {
        let terminal = self.inner.terminal.lock().expect("terminal state poisoned");
        let screen = terminal.screen();
        let total_rows = screen.scrollback_rows();
        let rows = screen.physical_rows.max(1);
        let cols = screen.physical_cols.max(1);
        let scrollback_offset = self
            .inner
            .scrollback_offset
            .load(Ordering::Relaxed)
            .min(total_rows.saturating_sub(rows));
        let end = total_rows.saturating_sub(scrollback_offset);
        let start = end.saturating_sub(rows);
        let lines = screen.lines_in_phys_range(start..end.min(total_rows));
        let cursor = terminal.cursor_pos();
        TerminalSnapshot {
            lines,
            cols,
            rows,
            cursor_x: cursor.x.min(cols.saturating_sub(1)),
            cursor_y: (cursor.y.max(0) as usize).min(rows.saturating_sub(1)),
            cursor_shape: cursor.shape,
            cursor_visible: self.inner.focused.load(Ordering::Relaxed)
                && matches!(cursor.visibility, CursorVisibility::Visible),
            palette: terminal.get_config().color_palette(),
            title: terminal.get_title().to_string(),
            scrollback_offset,
            selection: self
                .inner
                .selection
                .lock()
                .ok()
                .and_then(|selection| *selection),
            mouse_grabbed: terminal.is_mouse_grabbed(),
            alt_screen_active: terminal.is_alt_screen_active(),
        }
    }

    pub fn title(&self) -> String {
        self.snapshot().title
    }

    fn writer_lock(&self) -> Result<std::sync::MutexGuard<'_, Box<dyn Write + Send>>> {
        self.inner
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal writer poisoned"))
    }
}

fn map_modifiers(modifiers: u8) -> KeyModifiers {
    let mut mapped = KeyModifiers::NONE;
    if (modifiers & 1) != 0 {
        mapped |= KeyModifiers::SHIFT;
    }
    if (modifiers & 2) != 0 {
        mapped |= KeyModifiers::ALT;
    }
    if (modifiers & 4) != 0 {
        mapped |= KeyModifiers::CTRL;
    }
    if (modifiers & 8) != 0 {
        mapped |= KeyModifiers::SUPER;
    }
    mapped
}

fn is_primary_shortcut(modifiers: u8) -> bool {
    (modifiers & 4) != 0 || (modifiers & 8) != 0
}

fn map_pointer_button(button: &fission_core::event::PointerButton) -> TermMouseButton {
    match button {
        fission_core::event::PointerButton::Primary => TermMouseButton::Left,
        fission_core::event::PointerButton::Secondary => TermMouseButton::Right,
        fission_core::event::PointerButton::Middle => TermMouseButton::Middle,
        fission_core::event::PointerButton::Other(_) => TermMouseButton::None,
    }
}

fn map_key_code(key: &KeyCode) -> TermKeyCode {
    match key {
        KeyCode::Space => TermKeyCode::Char(' '),
        KeyCode::Enter => TermKeyCode::Enter,
        KeyCode::Escape => TermKeyCode::Escape,
        KeyCode::Backspace => TermKeyCode::Backspace,
        KeyCode::Tab => TermKeyCode::Tab,
        KeyCode::Left => TermKeyCode::LeftArrow,
        KeyCode::Right => TermKeyCode::RightArrow,
        KeyCode::Up => TermKeyCode::UpArrow,
        KeyCode::Down => TermKeyCode::DownArrow,
        KeyCode::Home => TermKeyCode::Home,
        KeyCode::End => TermKeyCode::End,
        KeyCode::Char(ch) => TermKeyCode::Char(*ch),
    }
}

fn extract_selection_text(snapshot: &TerminalSnapshot, selection: TerminalSelection) -> String {
    let selection = selection.normalized();
    let mut parts = Vec::new();
    for row in selection.anchor_row..=selection.focus_row {
        let Some(line) = snapshot.lines.get(row) else {
            continue;
        };
        let Some((start_col, end_col)) = selection.range_for_row(row, snapshot.cols) else {
            continue;
        };
        let plain = plain_line_text(line, snapshot.cols);
        let chars: Vec<char> = plain.chars().collect();
        if start_col >= chars.len() {
            parts.push(String::new());
        } else {
            parts.push(chars[start_col..end_col.min(chars.len())].iter().collect());
        }
    }
    parts.join("\n")
}

fn plain_line_text(line: &Line, cols: usize) -> String {
    let mut text = String::with_capacity(cols);
    let mut cursor_col = 0usize;
    for cell in line.visible_cells() {
        let cell_index = cell.cell_index();
        while cursor_col < cell_index {
            text.push(' ');
            cursor_col += 1;
        }
        if cell.attrs().invisible() {
            for _ in 0..cell.width().max(1) {
                text.push(' ');
                cursor_col += 1;
            }
            continue;
        }
        let cell_text = cell.str();
        if cell_text.is_empty() {
            text.push(' ');
            cursor_col += 1;
        } else {
            text.push_str(cell_text);
            cursor_col += cell.width().max(1);
            for _ in 1..cell.width().max(1) {
                text.push(' ');
            }
        }
    }
    while text.chars().count() < cols {
        text.push(' ');
    }
    text
}

fn to_ir_color(color: wezterm_term::color::SrgbaTuple) -> Color {
    let (r, g, b, a) = color.to_srgb_u8();
    Color { r, g, b, a }
}

fn dim_color(color: Color, factor: f32) -> Color {
    Color {
        r: ((color.r as f32) * factor).round().clamp(0.0, 255.0) as u8,
        g: ((color.g as f32) * factor).round().clamp(0.0, 255.0) as u8,
        b: ((color.b as f32) * factor).round().clamp(0.0, 255.0) as u8,
        a: color.a,
    }
}

fn append_run(
    runs: &mut Vec<TextRun>,
    current_style: &mut Option<TerminalRunStyle>,
    current_text: &mut String,
    style: TerminalRunStyle,
    text: String,
    font_size: f32,
) {
    if current_style.as_ref() == Some(&style) {
        current_text.push_str(&text);
        return;
    }
    flush_run(runs, current_style, current_text, font_size);
    *current_style = Some(style);
    current_text.push_str(&text);
}

fn flush_run(
    runs: &mut Vec<TextRun>,
    current_style: &mut Option<TerminalRunStyle>,
    current_text: &mut String,
    font_size: f32,
) {
    let Some(style) = current_style.take() else {
        current_text.clear();
        return;
    };
    if current_text.is_empty() {
        return;
    }
    runs.push(TextRun {
        text: std::mem::take(current_text),
        style: TextStyle {
            font_size,
            color: style.fg,
            underline: style.underline,
            background_color: style.bg,
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use wezterm_term::{Cell, Line};

    #[test]
    fn selection_range_is_normalized_per_row() {
        let selection = TerminalSelection {
            anchor_row: 2,
            anchor_col: 8,
            focus_row: 0,
            focus_col: 3,
        };
        assert_eq!(selection.range_for_row(0, 20), Some((3, 20)));
        assert_eq!(selection.range_for_row(1, 20), Some((0, 20)));
        assert_eq!(selection.range_for_row(2, 20), Some((0, 8)));
    }

    #[test]
    fn extract_selection_text_spans_multiple_rows() {
        let mut line0 = Line::with_width(24, 0);
        line0.set_cell(
            0,
            Cell::new_grapheme("alpha".into(), Default::default(), None),
            0,
        );
        let mut line1 = Line::with_width(24, 0);
        line1.set_cell(
            0,
            Cell::new_grapheme("beta".into(), Default::default(), None),
            0,
        );
        let snapshot = TerminalSnapshot {
            lines: vec![line0, line1],
            cols: 24,
            rows: 2,
            cursor_x: 0,
            cursor_y: 0,
            cursor_shape: CursorShape::SteadyBar,
            cursor_visible: false,
            palette: ColorPalette::default(),
            title: String::new(),
            scrollback_offset: 0,
            selection: None,
            mouse_grabbed: false,
            alt_screen_active: false,
        };
        let text = extract_selection_text(
            &snapshot,
            TerminalSelection {
                anchor_row: 0,
                anchor_col: 1,
                focus_row: 1,
                focus_col: 2,
            },
        );
        assert!(text.starts_with("lpha"));
        assert!(text.ends_with("be"));
    }
}
