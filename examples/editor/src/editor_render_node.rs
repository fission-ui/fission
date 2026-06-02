//! Custom render object for the code editor surface.
//!
//! `EditorRenderNode` replaces the previous `TextInput`-based editor with a
//! single `LowerDyn` implementation that:
//!
//!   1. Renders either the full buffer or a virtualized visible window,
//!   2. Performs its own hit-testing geometry,
//!   3. Manages cursor bar placement directly via `DrawRect`.
//!   4. Renders selection highlights behind text.
//!   5. Scrolls to follow the cursor.
//!
//! The struct is built from `EditorState` via [`EditorRenderNode::from_state`]
//! and wrapped in a `Node::Custom(CustomWidget { .. })` by `EditorSurface`.

use crate::model::{
    ApplyEditorEdit, DocumentBacking, EditorState, Language, SetEditorPreedit,
    ShiftActiveFileWindow, UpdateCursorPosition, UpdateScrollY, WrapMode,
};
use crate::syntax;
use fission::core::action::ActionEnvelope;
use fission::core::event::{InputEvent, KeyCode, KeyEvent, PointerEvent};
use fission::core::lowering::{LoweringContext, NodeBuilder};
use fission::core::ui::custom_render::{CustomEventResult, CustomHitResult, CustomRenderObject};
use fission::core::ui::traits::LowerDyn;
use fission::core::{LayoutPoint, LayoutRect};
use fission::ir::op::{
    AlignItems, Color as IrColor, Fill, FlexDirection, LayoutOp, Op, PaintOp, TextRun, TextStyle,
};
use fission::ir::WidgetId;
use std::fmt;

// ---------------------------------------------------------------------------
// SyntaxSpan -- per-fragment colouring info for a single visible line
// ---------------------------------------------------------------------------

/// A span of text within a single line together with its display colour.
/// Produced by the syntax module and consumed during lowering to build
/// `TextRun` sequences for `DrawRichText`.
#[derive(Debug, Clone)]
pub struct SyntaxSpan {
    pub text: String,
    pub color: IrColor,
}

#[derive(Debug, Clone)]
struct VisualLine {
    source_line: usize,
    start_col: usize,
    end_col: usize,
    text: String,
    starts_source_line: bool,
}

// ---------------------------------------------------------------------------
// EditorRenderNode
// ---------------------------------------------------------------------------

/// Custom render object that replaces the TextInput-based code editor.
///
/// Holds a snapshot of everything needed to render the visible portion of the
/// file: content, cursor state, syntax cache, scroll position and metrics.
#[derive(Clone)]
pub struct EditorRenderNode {
    /// Full file content (String for now, will migrate to rope later).
    pub content: String,
    /// Language of the file (for syntax highlighting).
    pub language: Language,
    /// Wrap behavior for the current buffer.
    pub wrap_mode: WrapMode,
    /// Whether the underlying document is editable.
    pub editable: bool,
    /// Whether this buffer is backed by a file window.
    pub windowed_file: bool,
    pub has_more_before: bool,
    pub has_more_after: bool,
    /// Byte offset of the cursor (caret) within `content`.
    pub cursor_offset: usize,
    /// Byte offset of the selection anchor within `content`.
    pub anchor_offset: usize,
    /// Active IME preedit range in display coordinates, if any.
    pub preedit_range: Option<(usize, usize)>,
    /// Pre-computed syntax spans, one `Vec<SyntaxSpan>` per source line.
    pub syntax_cache: Vec<Vec<SyntaxSpan>>,
    /// Height of a single line in logical pixels.
    pub line_height: f32,
    /// Font size used for code text.
    pub font_size: f32,
    /// Width of the gutter (line-number column) in logical pixels.
    pub gutter_width: f32,
    /// Approximate wrap width in columns for soft-wrapped content.
    pub wrap_columns: usize,
    /// Current vertical scroll offset in logical pixels.
    pub scroll_y: f32,
    /// Stable string used to derive deterministic `WidgetId`s across rebuilds.
    pub file_path: String,
    /// Viewport height in logical pixels (from the last layout pass).
    pub viewport_height: f32,
    /// Viewport width in logical pixels (used to clip no-wrap rendering).
    pub viewport_width: f32,
}

const VISIBLE_LINE_OVERSCAN: usize = 4;
const FULL_RENDER_VISUAL_LINE_LIMIT: usize = 512;

impl fmt::Debug for EditorRenderNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EditorRenderNode")
            .field("language", &self.language)
            .field("wrap_mode", &self.wrap_mode)
            .field("editable", &self.editable)
            .field("windowed_file", &self.windowed_file)
            .field("cursor_offset", &self.cursor_offset)
            .field("anchor_offset", &self.anchor_offset)
            .field("preedit_range", &self.preedit_range)
            .field("line_height", &self.line_height)
            .field("font_size", &self.font_size)
            .field("gutter_width", &self.gutter_width)
            .field("wrap_columns", &self.wrap_columns)
            .field("scroll_y", &self.scroll_y)
            .field("content_len", &self.content.len())
            .field("syntax_lines", &self.syntax_cache.len())
            .field("viewport_width", &self.viewport_width)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

impl EditorRenderNode {
    /// Build an `EditorRenderNode` from the current `EditorState`.
    ///
    /// Returns `None` if there is no active buffer (the caller should fall back
    /// to the welcome screen).
    pub fn from_state(
        state: &EditorState,
        viewport_width: f32,
        viewport_height: f32,
        scroll_y: f32,
    ) -> Option<Self> {
        let (tab, buffer) = state.active_buffer()?;
        let content = buffer.display_content();
        let language = buffer.language;
        let wrap_mode = buffer.wrap_mode;
        let editable = buffer.is_editable();
        let (windowed_file, has_more_before, has_more_after) = match &buffer.backing {
            DocumentBacking::FileWindow { window, .. } => {
                (true, window.has_more_before, window.has_more_after)
            }
            DocumentBacking::InMemory => (false, false, false),
        };
        let file_path = tab.path.clone();
        let font_size = 13.0;
        let line_height = 20.0;

        // --- Compute cursor byte offset from current editing/preedit state ---
        let (cursor_offset, anchor_offset) = buffer.display_offsets();
        let preedit_range = buffer.preedit_range();

        // --- Build syntax cache ---
        let line_count = content.lines().count().max(1);
        let syntax_cache: Vec<Vec<SyntaxSpan>> = if line_count > SYNTAX_HIGHLIGHT_LINE_LIMIT {
            // Large file: plain unstyled text
            content
                .lines()
                .map(|l| {
                    vec![SyntaxSpan {
                        text: l.to_string(),
                        color: DEFAULT_TEXT,
                    }]
                })
                .collect()
        } else {
            let doc_spans = syntax::highlight_document(&content, language);
            doc_spans
                .into_iter()
                .map(|spans| {
                    spans
                        .into_iter()
                        .map(|s| SyntaxSpan {
                            text: s.text,
                            color: IrColor {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                        })
                        .collect()
                })
                .collect()
        };

        // --- Gutter width: enough room for the widest line number + padding ---
        let digits = format!("{}", line_count).len();
        let gutter_width = digits as f32 * 9.0 + 16.0;
        let char_width = font_size * 0.6;
        let available_text_width = (viewport_width - gutter_width - 24.0).max(char_width * 20.0);
        let wrap_columns = ((available_text_width / char_width).floor() as usize).max(20);

        Some(Self {
            content,
            language,
            wrap_mode,
            editable,
            windowed_file,
            has_more_before,
            has_more_after,
            cursor_offset,
            anchor_offset,
            preedit_range,
            syntax_cache,
            line_height,
            font_size,
            gutter_width,
            wrap_columns,
            scroll_y,
            file_path,
            viewport_height: viewport_height.max(line_height),
            viewport_width,
        })
    }
}

// ---------------------------------------------------------------------------
// LowerDyn implementation -- the core rendering logic
// ---------------------------------------------------------------------------

impl LowerDyn for EditorRenderNode {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let visual_lines = self.visual_lines();
        let total_visual_lines = visual_lines.len().max(1);
        let total_lines = self.logical_line_count();
        let (first_line, last_line) =
            if self.windowed_file || total_visual_lines <= FULL_RENDER_VISUAL_LINE_LIMIT {
                (0, total_visual_lines)
            } else {
                self.visible_visual_line_range(total_visual_lines)
            };
        let content_height = total_visual_lines as f32 * self.line_height;
        let sel_start = self.cursor_offset.min(self.anchor_offset);
        let sel_end = self.cursor_offset.max(self.anchor_offset);
        let has_selection = sel_start != sel_end;
        let (sel_start_line, sel_start_col) = offset_to_line_col(&self.content, sel_start);
        let (sel_end_line, sel_end_col) = offset_to_line_col(&self.content, sel_end);
        let mut row_ids: Vec<WidgetId> = Vec::with_capacity(last_line - first_line);
        let gutter_digits = format!("{}", total_lines).len();
        let (cursor_row, cursor_col) =
            self.visual_position_for_offset(&self.content, self.cursor_offset);
        let char_width = self.font_size * 0.6;

        for visual_idx in first_line..last_line {
            let visual_line = &visual_lines[visual_idx];
            let line_idx = visual_line.source_line;
            let mut line_children: Vec<WidgetId> = Vec::new();

            if has_selection && line_idx >= sel_start_line && line_idx <= sel_end_line {
                let highlight_start_col = if line_idx == sel_start_line {
                    sel_start_col.max(visual_line.start_col)
                } else {
                    visual_line.start_col
                };
                let highlight_end_col = if line_idx == sel_end_line {
                    sel_end_col.min(visual_line.end_col)
                } else {
                    visual_line.end_col
                };

                if highlight_end_col > highlight_start_col {
                    let local_start = highlight_start_col.saturating_sub(visual_line.start_col);
                    let local_end = highlight_end_col.saturating_sub(visual_line.start_col);
                    let sel_x = self.gutter_width + local_start as f32 * char_width;
                    let sel_w = (local_end.saturating_sub(local_start)) as f32 * char_width;

                    let sel_paint = NodeBuilder::new(
                        cx.next_node_id(),
                        Op::Paint(PaintOp::DrawRect {
                            fill: Some(Fill::Solid(SELECTION_COLOR)),
                            stroke: None,
                            corner_radius: 0.0,
                            shadow: None,
                        }),
                    )
                    .build(cx);

                    let sel_box = {
                        let id = cx.next_node_id();
                        let mut b = NodeBuilder::new(
                            id,
                            Op::Layout(LayoutOp::Positioned {
                                left: Some(sel_x),
                                top: Some(0.0),
                                right: None,
                                bottom: None,
                                width: Some(sel_w),
                                height: Some(self.line_height),
                            }),
                        );
                        b.add_child(sel_paint);
                        b.build(cx)
                    };
                    line_children.push(sel_box);
                }
            }

            let line_num_text = if visual_line.starts_source_line {
                format!("{:>width$}", line_idx + 1, width = gutter_digits)
            } else {
                " ".repeat(gutter_digits)
            };

            let gutter_paint_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText {
                    text: line_num_text,
                    size: self.font_size,
                    color: GUTTER_COLOR,
                    underline: false,
                    wrap: false,
                    caret_index: None,
                    caret_color: None,
                    caret_width: None,
                    caret_height: None,
                    caret_radius: None,
                    paragraph_style: None,
                }),
            )
            .build(cx);

            let gutter_box_id = {
                let id = cx.next_node_id();
                let mut b = NodeBuilder::new(
                    id,
                    Op::Layout(LayoutOp::Box {
                        width: Some(self.gutter_width),
                        height: Some(self.line_height),
                        min_width: None,
                        max_width: None,
                        min_height: None,
                        max_height: None,
                        padding: [0.0, 4.0, 0.0, 4.0],
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        aspect_ratio: None,
                    }),
                );
                b.add_child(gutter_paint_id);
                b.build(cx)
            };

            let runs: Vec<TextRun> = if self.wraps_softly() {
                vec![TextRun {
                    text: visual_line.text.clone(),
                    style: TextStyle {
                        font_size: self.font_size,
                        color: DEFAULT_TEXT,
                        underline: false,
                        font_family: None,
                        locale: None,
                        font_weight: 400,
                        font_style: fission::ir::op::FontStyle::Normal,
                        line_height: None,
                        letter_spacing: 0.0,
                        background_color: None,
                    },
                }]
            } else if line_idx < self.syntax_cache.len() {
                self.syntax_cache[line_idx]
                    .iter()
                    .map(|span| TextRun {
                        text: span.text.clone(),
                        style: TextStyle {
                            font_size: self.font_size,
                            color: span.color,
                            underline: false,
                            font_family: None,
                            locale: None,
                            font_weight: 400,
                            font_style: fission::ir::op::FontStyle::Normal,
                            line_height: None,
                            letter_spacing: 0.0,
                            background_color: None,
                        },
                    })
                    .collect()
            } else {
                vec![TextRun {
                    text: visual_line.text.clone(),
                    style: TextStyle {
                        font_size: self.font_size,
                        color: DEFAULT_TEXT,
                        underline: false,
                        font_family: None,
                        locale: None,
                        font_weight: 400,
                        font_style: fission::ir::op::FontStyle::Normal,
                        line_height: None,
                        letter_spacing: 0.0,
                        background_color: None,
                    },
                }]
            };

            let code_paint_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRichText {
                    runs,
                    wrap: self.wraps_softly(),
                    caret_index: None,
                    caret_color: None,
                    caret_width: None,
                    caret_height: None,
                    caret_radius: None,
                    paragraph_style: None,
                }),
            )
            .build(cx);

            let code_box_width = self.code_box_width_for_visual_line(visual_line, char_width);
            let code_box_id = {
                let id = cx.next_node_id();
                let mut b = NodeBuilder::new(
                    id,
                    Op::Layout(LayoutOp::Box {
                        width: code_box_width,
                        height: Some(self.line_height),
                        min_width: None,
                        max_width: None,
                        min_height: None,
                        max_height: None,
                        padding: [0.0; 4],
                        flex_grow: if self.wraps_softly() { 1.0 } else { 0.0 },
                        flex_shrink: if self.wraps_softly() { 1.0 } else { 0.0 },
                        aspect_ratio: None,
                    }),
                );
                b.add_child(code_paint_id);
                b.build(cx)
            };

            let text_row_id = {
                let id = cx.next_node_id();
                let mut b = NodeBuilder::new(
                    id,
                    Op::Layout(LayoutOp::Flex {
                        direction: FlexDirection::Row,
                        wrap: fission::ir::op::FlexWrap::NoWrap,
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        padding: [0.0; 4],
                        gap: None,
                        align_items: AlignItems::Center,
                        justify_content: fission::ir::op::JustifyContent::Start,
                    }),
                );
                b.add_child(gutter_box_id);
                b.add_child(code_box_id);
                b.build(cx)
            };

            let row_id = if line_children.is_empty() {
                text_row_id
            } else {
                let id = cx.next_node_id();
                let mut b = NodeBuilder::new(id, Op::Layout(LayoutOp::ZStack));
                for child_id in line_children {
                    b.add_child(child_id);
                }
                b.add_child(text_row_id);
                b.build(cx)
            };

            row_ids.push(row_id);
        }

        let cursor_bar_id = if cursor_row >= first_line && cursor_row < last_line {
            let cursor_y = cursor_row as f32 * self.line_height;
            let cursor_x = self.gutter_width + cursor_col as f32 * char_width;

            let rect_paint = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(Fill::Solid(CURSOR_COLOR)),
                    stroke: None,
                    corner_radius: 0.0,
                    shadow: None,
                }),
            )
            .build(cx);

            let cursor_box = {
                let id = cx.next_node_id();
                let mut b = NodeBuilder::new(
                    id,
                    Op::Layout(LayoutOp::Positioned {
                        left: Some(cursor_x),
                        top: Some(cursor_y),
                        right: None,
                        bottom: None,
                        width: Some(2.0),
                        height: Some(self.line_height),
                    }),
                );
                b.add_child(rect_paint);
                b.build(cx)
            };

            Some(cursor_box)
        } else {
            None
        };

        // ---- Vertical column of visible rows ---------------------------------
        let column_id = {
            let id = cx.next_node_id();
            let mut b = NodeBuilder::new(
                id,
                Op::Layout(LayoutOp::Flex {
                    direction: FlexDirection::Column,
                    wrap: fission::ir::op::FlexWrap::NoWrap,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    padding: [0.0; 4],
                    gap: None,
                    align_items: AlignItems::Stretch,
                    justify_content: fission::ir::op::JustifyContent::Start,
                }),
            );
            b.add_children(row_ids);
            b.build(cx)
        };
        let column_id = {
            let id = cx.next_node_id();
            let mut b = NodeBuilder::new(
                id,
                Op::Layout(LayoutOp::Positioned {
                    left: Some(0.0),
                    top: Some(first_line as f32 * self.line_height),
                    right: None,
                    bottom: None,
                    width: None,
                    height: Some((last_line.saturating_sub(first_line)) as f32 * self.line_height),
                }),
            );
            b.add_child(column_id);
            b.build(cx)
        };

        // ---- Outer ZStack: column of rows + cursor overlay -------------------
        // We use a ZStack so the cursor can be absolutely positioned on top of
        // the code lines without disturbing the flex layout.
        let zstack_id = {
            let id = cx.next_node_id();
            let mut b = NodeBuilder::new(id, Op::Layout(LayoutOp::ZStack));
            b.add_child(column_id);
            if let Some(cursor_id) = cursor_bar_id {
                b.add_child(cursor_id);
            }
            b.build(cx)
        };

        // ---- Background rect ------------------------------------------------
        let bg_paint = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(EDITOR_BG)),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
        )
        .build(cx);

        // ---- Outer box with explicit size ------------------------------------
        // Height = full content height so the parent Scroll can determine the
        // scroll extent, even though we only emit visible lines.
        let outer_id = {
            let id = cx.next_node_id();
            let mut b = NodeBuilder::new(
                id,
                Op::Layout(LayoutOp::Box {
                    width: Some(
                        self.viewport_width
                            .max(self.gutter_width + char_width * 4.0),
                    ),
                    height: Some(content_height),
                    min_width: None,
                    max_width: None,
                    min_height: Some(content_height),
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    aspect_ratio: None,
                }),
            );
            b.add_child(bg_paint);
            b.add_child(zstack_id);
            b.build(cx)
        };

        let clip_id = {
            let id = cx.next_node_id();
            let mut b = NodeBuilder::new(id, Op::Layout(LayoutOp::Clip { path: None }));
            b.add_child(outer_id);
            b.build(cx)
        };

        clip_id
    }

    fn stable_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.file_path.hash(&mut h);
        self.content.hash(&mut h);
        std::mem::discriminant(&self.language).hash(&mut h);
        std::mem::discriminant(&self.wrap_mode).hash(&mut h);
        self.editable.hash(&mut h);
        self.windowed_file.hash(&mut h);
        self.has_more_before.hash(&mut h);
        self.has_more_after.hash(&mut h);
        self.cursor_offset.hash(&mut h);
        self.anchor_offset.hash(&mut h);
        self.preedit_range.hash(&mut h);
        self.scroll_y.to_bits().hash(&mut h);
        self.viewport_height.to_bits().hash(&mut h);
        self.viewport_width.to_bits().hash(&mut h);
        "EditorRenderNode".hash(&mut h);
        h.finish()
    }
}

// ---------------------------------------------------------------------------
// CustomRenderObject -- hit-testing and event handling
// ---------------------------------------------------------------------------

impl CustomRenderObject for EditorRenderNode {
    fn accepts_text_input(&self) -> bool {
        true
    }

    fn hit_test(&self, local_point: LayoutPoint, _node_rect: LayoutRect) -> CustomHitResult {
        let byte_offset = self.point_to_offset(local_point);
        CustomHitResult::inside(Some(byte_offset))
    }

    fn handle_event(
        &self,
        node_id: WidgetId,
        event: &InputEvent,
        node_rect: LayoutRect,
    ) -> CustomEventResult {
        match event {
            // --- Click to place caret ---
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                let local_point =
                    LayoutPoint::new(point.x - node_rect.origin.x, point.y - node_rect.origin.y);
                let byte_offset = self.point_to_offset(local_point);

                let mut actions = Vec::new();
                if self.preedit_range.is_some() {
                    actions.push((
                        node_id,
                        ActionEnvelope::from(SetEditorPreedit {
                            text: String::new(),
                        }),
                    ));
                }
                actions.push((
                    node_id,
                    ActionEnvelope::from(UpdateCursorPosition {
                        caret: byte_offset,
                        anchor: byte_offset,
                    }),
                ));
                CustomEventResult::consumed_with(actions)
            }

            // --- Keyboard input ---
            InputEvent::Keyboard(KeyEvent::Down {
                key_code,
                modifiers,
            }) => self.handle_key(node_id, key_code, *modifiers),

            InputEvent::Ime(fission::core::event::ImeEvent::Preedit { text }) => {
                if !self.editable {
                    return CustomEventResult::consumed();
                }
                CustomEventResult::consumed_with(vec![(
                    node_id,
                    ActionEnvelope::from(SetEditorPreedit { text: text.clone() }),
                )])
            }

            InputEvent::Ime(fission::core::event::ImeEvent::Commit { text }) => {
                self.handle_ime_commit(node_id, text)
            }

            _ => CustomEventResult::ignored(),
        }
    }

    fn ime_cursor_area(&self, node_rect: LayoutRect) -> Option<LayoutRect> {
        Some(self.caret_rect(node_rect))
    }

    fn blur_actions(&self, node_id: WidgetId) -> Vec<(WidgetId, ActionEnvelope)> {
        if self.preedit_range.is_some() {
            vec![(
                node_id,
                ActionEnvelope::from(SetEditorPreedit {
                    text: String::new(),
                }),
            )]
        } else {
            Vec::new()
        }
    }
}

impl EditorRenderNode {
    fn logical_line_count(&self) -> usize {
        self.content.split('\n').count().max(1)
    }

    fn wraps_softly(&self) -> bool {
        self.wrap_mode == WrapMode::SoftWrap && self.wrap_columns > 0
    }

    fn code_box_width_for_visual_line(
        &self,
        visual_line: &VisualLine,
        char_width: f32,
    ) -> Option<f32> {
        if self.wraps_softly() {
            None
        } else {
            Some((visual_line.text.chars().count().max(1) as f32 * char_width) + 4.0)
        }
    }

    fn visible_visual_line_range(&self, total_visual_lines: usize) -> (usize, usize) {
        let total_visual_lines = total_visual_lines.max(1);
        let raw_first = (self.scroll_y.max(0.0) / self.line_height.max(1.0)).floor() as usize;
        let first_line = raw_first.min(total_visual_lines.saturating_sub(1));
        let visible_rows = (self.viewport_height / self.line_height.max(1.0)).ceil() as usize;
        let last_line =
            (first_line + visible_rows.max(1) + VISIBLE_LINE_OVERSCAN).min(total_visual_lines);
        (first_line, last_line.max(first_line + 1))
    }

    fn visual_lines(&self) -> Vec<VisualLine> {
        self.visual_lines_for_content(&self.content)
    }

    pub fn content_height(&self) -> f32 {
        self.visual_lines().len().max(1) as f32 * self.line_height
    }

    fn visual_lines_for_content(&self, content: &str) -> Vec<VisualLine> {
        let mut lines = Vec::new();
        let wrap_columns = self.wrap_columns.max(1);

        for (source_line, line_text) in content.split('\n').enumerate() {
            let char_count = line_text.chars().count();
            if !self.wraps_softly() || char_count <= wrap_columns {
                lines.push(VisualLine {
                    source_line,
                    start_col: 0,
                    end_col: char_count,
                    text: line_text.to_string(),
                    starts_source_line: true,
                });
                continue;
            }

            let chars: Vec<char> = line_text.chars().collect();
            let mut start_col = 0usize;
            while start_col < char_count {
                let end_col = (start_col + wrap_columns).min(char_count);
                let text: String = chars[start_col..end_col].iter().collect();
                lines.push(VisualLine {
                    source_line,
                    start_col,
                    end_col,
                    text,
                    starts_source_line: start_col == 0,
                });
                start_col = end_col;
            }
        }

        if lines.is_empty() {
            lines.push(VisualLine {
                source_line: 0,
                start_col: 0,
                end_col: 0,
                text: String::new(),
                starts_source_line: true,
            });
        }

        lines
    }

    fn visual_position_for_offset(&self, content: &str, offset: usize) -> (usize, usize) {
        if !self.wraps_softly() {
            return offset_to_line_col(content, offset);
        }

        let (source_line, source_col) = offset_to_line_col(content, offset);
        let visual_lines = self.visual_lines_for_content(content);
        let mut fallback = (0usize, 0usize);

        for (row, visual_line) in visual_lines.iter().enumerate() {
            if visual_line.source_line != source_line {
                continue;
            }
            let segment_len = visual_line.end_col.saturating_sub(visual_line.start_col);
            let col_in_segment = source_col.saturating_sub(visual_line.start_col);
            fallback = (row, col_in_segment.min(segment_len));
            if source_col >= visual_line.start_col && source_col <= visual_line.end_col {
                return fallback;
            }
        }

        fallback
    }

    fn offset_for_visual_position(&self, content: &str, row: usize, col: usize) -> usize {
        if !self.wraps_softly() {
            return line_col_to_offset(content, row, col);
        }

        let visual_lines = self.visual_lines_for_content(content);
        let visual_line = visual_lines
            .get(row)
            .unwrap_or_else(|| visual_lines.last().expect("visual lines should exist"));
        let segment_len = visual_line.end_col.saturating_sub(visual_line.start_col);
        let target_col = visual_line.start_col + col.min(segment_len);
        line_col_to_offset(content, visual_line.source_line, target_col)
    }

    /// Convert a local point (relative to the node's top-left) to a byte
    /// offset in `self.content`.
    fn point_to_offset(&self, local_point: LayoutPoint) -> usize {
        let char_width = self.font_size * 0.6;

        // Account for vertical scroll offset.
        let adjusted_y = local_point.y + self.scroll_y;
        let row = (adjusted_y / self.line_height).floor().max(0.0) as usize;

        // Horizontal: subtract gutter width, then divide by char width.
        let text_x = (local_point.x - self.gutter_width).max(0.0);
        let col = (text_x / char_width).round() as usize;

        self.offset_for_visual_position(&self.content, row, col)
    }

    /// Compute the scroll_y needed to keep a given line visible.
    /// Returns `Some(new_scroll_y)` if the scroll needs to change, `None`
    /// if the line is already in view.
    fn scroll_y_to_reveal_offset(&self, content: &str, offset: usize) -> Option<f32> {
        let total_lines = self.visual_lines_for_content(content).len().max(1);
        let content_height = total_lines as f32 * self.line_height;
        let viewport = self.viewport_height;

        let (line, _) = self.visual_position_for_offset(content, offset);
        let line_top = line as f32 * self.line_height;
        let line_bottom = line_top + self.line_height;

        let mut new_scroll = self.scroll_y;

        if line_top < self.scroll_y {
            // Cursor is above the viewport -- scroll up.
            new_scroll = line_top;
        } else if line_bottom > self.scroll_y + viewport {
            // Cursor is below the viewport -- scroll down.
            new_scroll = line_bottom - viewport;
        }

        // Clamp to valid range.
        let max_scroll = (content_height - viewport).max(0.0);
        new_scroll = new_scroll.clamp(0.0, max_scroll);

        if (new_scroll - self.scroll_y).abs() > 0.5 {
            Some(new_scroll)
        } else {
            None
        }
    }

    /// Handle a key press, producing a `CustomEventResult` with the
    /// appropriate actions to dispatch.
    fn handle_key(&self, node_id: WidgetId, key_code: &KeyCode, modifiers: u8) -> CustomEventResult {
        let shift = (modifiers & 1) != 0;
        let ctrl_or_cmd = (modifiers & 4) != 0 || (modifiers & 8) != 0;
        let content = self.content.as_str();
        let mut offset = self.cursor_offset.min(content.len());
        let mut anchor = self.anchor_offset.min(content.len());
        let content_len = content.len();
        let mut replacement: Option<(std::ops::Range<usize>, String)> = None;
        let mut window_shift: Option<bool> = None;

        if self.preedit_range.is_some() {
            return match key_code {
                KeyCode::Escape => CustomEventResult::consumed_with(vec![(
                    node_id,
                    ActionEnvelope::from(SetEditorPreedit {
                        text: String::new(),
                    }),
                )]),
                _ => CustomEventResult::ignored(),
            };
        }

        match key_code {
            // -- Ctrl/Cmd+A: select all --
            KeyCode::Char('a') | KeyCode::Char('A') if ctrl_or_cmd => {
                anchor = 0;
                offset = content_len;
            }

            // -- Ctrl/Cmd+E: jump to end of line --
            KeyCode::Char('e') | KeyCode::Char('E') if ctrl_or_cmd => {
                let (line, _) = offset_to_line_col(&content, offset);
                let line_text = content.lines().nth(line).unwrap_or("");
                offset = line_col_to_offset(&content, line, line_text.chars().count());
                if !shift {
                    anchor = offset;
                }
            }

            // -- Character insertion --
            KeyCode::Char(ch) => {
                // If ctrl/cmd is held, let the app-level handler deal with it.
                if ctrl_or_cmd {
                    return CustomEventResult::ignored();
                }
                if !self.editable {
                    return CustomEventResult::consumed();
                }
                let (start, end) = selection_bounds(offset, anchor);
                replacement = Some((start..end, ch.to_string()));
                offset = start + ch.len_utf8();
                anchor = offset;
            }
            KeyCode::Space => {
                if !self.editable {
                    return CustomEventResult::consumed();
                }
                let (start, end) = selection_bounds(offset, anchor);
                replacement = Some((start..end, " ".to_string()));
                offset = start + 1;
                anchor = offset;
            }
            KeyCode::Enter => {
                if !self.editable {
                    return CustomEventResult::consumed();
                }
                let (start, end) = selection_bounds(offset, anchor);
                replacement = Some((start..end, "\n".to_string()));
                offset = start + 1;
                anchor = offset;
            }
            KeyCode::Tab => {
                if !self.editable {
                    return CustomEventResult::consumed();
                }
                let (start, end) = selection_bounds(offset, anchor);
                replacement = Some((start..end, "    ".to_string()));
                offset = start + 4;
                anchor = offset;
            }

            // -- Deletion --
            KeyCode::Backspace => {
                if !self.editable {
                    return CustomEventResult::consumed();
                }
                let (sel_start, sel_end) = selection_bounds(offset, anchor);
                if sel_start != sel_end {
                    replacement = Some((sel_start..sel_end, String::new()));
                    offset = sel_start;
                    anchor = sel_start;
                } else if offset > 0 {
                    let prev_boundary = prev_char_boundary(content, offset);
                    replacement = Some((prev_boundary..offset, String::new()));
                    offset = prev_boundary;
                    anchor = offset;
                }
            }

            // -- Arrow key navigation --
            KeyCode::Left => {
                if offset > 0 {
                    offset = prev_char_boundary(content, offset);
                }
                if !shift {
                    anchor = offset;
                }
            }
            KeyCode::Right => {
                if offset < content.len() {
                    offset = next_char_boundary(content, offset);
                }
                if !shift {
                    anchor = offset;
                }
            }
            KeyCode::Up => {
                let before = offset;
                if self.wraps_softly() {
                    let (row, col) = self.visual_position_for_offset(content, offset);
                    if row > 0 {
                        offset = self.offset_for_visual_position(content, row - 1, col);
                    } else {
                        offset = 0;
                    }
                } else {
                    let (line, col) = offset_to_line_col(&content, offset);
                    if line > 0 {
                        offset = line_col_to_offset(&content, line - 1, col);
                    }
                }
                if !shift {
                    anchor = offset;
                }
                if self.windowed_file && !shift && offset == before && self.has_more_before {
                    window_shift = Some(false);
                }
            }
            KeyCode::Down => {
                let before = offset;
                if self.wraps_softly() {
                    let (row, col) = self.visual_position_for_offset(content, offset);
                    offset = self.offset_for_visual_position(content, row + 1, col);
                } else {
                    let (line, col) = offset_to_line_col(&content, offset);
                    offset = line_col_to_offset(&content, line + 1, col);
                }
                if !shift {
                    anchor = offset;
                }
                if self.windowed_file && !shift && offset == before && self.has_more_after {
                    window_shift = Some(true);
                }
            }
            KeyCode::Home => {
                // Move to start of current line.
                let (line, _) = offset_to_line_col(&content, offset);
                offset = line_col_to_offset(&content, line, 0);
                if !shift {
                    anchor = offset;
                }
            }
            KeyCode::End => {
                // Move to end of current line.
                let (line, _) = offset_to_line_col(&content, offset);
                let line_text = content.lines().nth(line).unwrap_or("");
                offset = line_col_to_offset(&content, line, line_text.chars().count());
                if !shift {
                    anchor = offset;
                }
            }

            KeyCode::Escape => {
                // If there is a selection, collapse it to the cursor position.
                if anchor != offset {
                    anchor = offset;
                } else {
                    return CustomEventResult::ignored();
                }
            }
            KeyCode::Delete | KeyCode::PageUp | KeyCode::PageDown => {
                return CustomEventResult::ignored();
            }
        }

        let mut actions: Vec<(WidgetId, ActionEnvelope)> = Vec::new();
        if let Some(forward) = window_shift {
            actions.push((
                node_id,
                ActionEnvelope::from(ShiftActiveFileWindow { forward }),
            ));
            return CustomEventResult::consumed_with(actions);
        }
        if let Some((range, new_text)) = replacement {
            let preview = apply_preview_edit(content, range.clone(), &new_text);
            actions.push((
                node_id,
                ActionEnvelope::from(ApplyEditorEdit {
                    range_start: range.start,
                    range_end: range.end,
                    new_text,
                    caret: offset,
                    anchor,
                }),
            ));

            if let Some(new_scroll) = self.scroll_y_to_reveal_offset(&preview, offset) {
                actions.push((node_id, ActionEnvelope::from(UpdateScrollY(new_scroll))));
            }
            return CustomEventResult::consumed_with(actions);
        }

        actions.push((
            node_id,
            ActionEnvelope::from(UpdateCursorPosition {
                caret: offset,
                anchor,
            }),
        ));

        if let Some(new_scroll) = self.scroll_y_to_reveal_offset(content, offset) {
            actions.push((node_id, ActionEnvelope::from(UpdateScrollY(new_scroll))));
        }

        CustomEventResult::consumed_with(actions)
    }

    fn handle_ime_commit(&self, node_id: WidgetId, text: &str) -> CustomEventResult {
        if !self.editable {
            return CustomEventResult::consumed();
        }
        if text.is_empty() {
            if self.preedit_range.is_some() {
                return CustomEventResult::consumed_with(vec![(
                    node_id,
                    ActionEnvelope::from(SetEditorPreedit {
                        text: String::new(),
                    }),
                )]);
            }
            return CustomEventResult::ignored();
        }

        let (start, end) = self
            .preedit_range
            .unwrap_or_else(|| selection_bounds(self.cursor_offset, self.anchor_offset));
        let caret = start + text.len();
        let mut actions = vec![(
            node_id,
            ActionEnvelope::from(ApplyEditorEdit {
                range_start: start,
                range_end: end,
                new_text: text.to_string(),
                caret,
                anchor: caret,
            }),
        )];

        let preview = apply_preview_edit(self.content.as_str(), start..end, text);
        if let Some(new_scroll) = self.scroll_y_to_reveal_offset(&preview, caret) {
            actions.push((node_id, ActionEnvelope::from(UpdateScrollY(new_scroll))));
        }

        CustomEventResult::consumed_with(actions)
    }

    fn caret_rect(&self, node_rect: LayoutRect) -> LayoutRect {
        let char_width = self.font_size * 0.6;
        let (line, col) = self
            .visual_position_for_offset(&self.content, self.cursor_offset.min(self.content.len()));
        LayoutRect::new(
            node_rect.origin.x + self.gutter_width + col as f32 * char_width,
            node_rect.origin.y + line as f32 * self.line_height - self.scroll_y,
            2.0,
            self.line_height,
        )
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn selection_bounds(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn clamp_to_char_boundary(content: &str, offset: usize) -> usize {
    let mut boundary = offset.min(content.len());
    while boundary > 0 && !content.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn prev_char_boundary(content: &str, offset: usize) -> usize {
    let boundary = clamp_to_char_boundary(content, offset);
    if boundary == 0 {
        return 0;
    }
    content[..boundary]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn next_char_boundary(content: &str, offset: usize) -> usize {
    let boundary = clamp_to_char_boundary(content, offset);
    if boundary >= content.len() {
        return content.len();
    }
    let ch = content[boundary..].chars().next().unwrap();
    boundary + ch.len_utf8()
}

fn apply_preview_edit(content: &str, range: std::ops::Range<usize>, new_text: &str) -> String {
    let start = clamp_to_char_boundary(content, range.start);
    let end = clamp_to_char_boundary(content, range.end.max(start));
    let mut preview = String::with_capacity(content.len() - (end - start) + new_text.len());
    preview.push_str(&content[..start]);
    preview.push_str(new_text);
    preview.push_str(&content[end..]);
    preview
}

fn char_col_to_byte(line: &str, col: usize) -> usize {
    if col == 0 {
        return 0;
    }
    for (char_idx, (byte_idx, _)) in line.char_indices().enumerate() {
        if char_idx == col {
            return byte_idx;
        }
    }
    line.len()
}

/// Convert a (line, col) pair to a byte offset in `content`.
pub(crate) fn line_col_to_offset(content: &str, line: usize, col: usize) -> usize {
    if content.is_empty() {
        return 0;
    }

    let mut offset = 0usize;
    let mut current_line = 0usize;
    for segment in content.split_inclusive('\n') {
        let text_line = segment.strip_suffix('\n').unwrap_or(segment);
        if current_line == line {
            return offset + char_col_to_byte(text_line, col);
        }
        offset += segment.len();
        current_line += 1;
    }

    if current_line == line {
        return content.len();
    }

    content.len()
}

/// Convert a byte offset to a (line, col) pair.
pub(crate) fn offset_to_line_col(content: &str, offset: usize) -> (usize, usize) {
    let offset = clamp_to_char_boundary(content, offset);
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in content.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Line count above which we skip tree-sitter highlighting.
const SYNTAX_HIGHLIGHT_LINE_LIMIT: usize = 1000;

/// Default (unstyled) text colour -- matches VS Code Dark+.
const DEFAULT_TEXT: IrColor = IrColor {
    r: 212,
    g: 212,
    b: 212,
    a: 255,
};

/// Gutter (line-number) colour.
const GUTTER_COLOR: IrColor = IrColor {
    r: 120,
    g: 120,
    b: 120,
    a: 255,
};

/// Cursor bar colour.
const CURSOR_COLOR: IrColor = IrColor {
    r: 255,
    g: 255,
    b: 255,
    a: 200,
};

/// Selection highlight colour (semi-transparent blue).
const SELECTION_COLOR: IrColor = IrColor {
    r: 38,
    g: 79,
    b: 120,
    a: 180,
};

/// Editor background colour.
const EDITOR_BG: IrColor = IrColor {
    r: 30,
    g: 30,
    b: 30,
    a: 255,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn render_node(content: &str, wrap_mode: WrapMode, wrap_columns: usize) -> EditorRenderNode {
        EditorRenderNode {
            content: content.to_string(),
            language: Language::Plain,
            wrap_mode,
            editable: true,
            windowed_file: false,
            has_more_before: false,
            has_more_after: false,
            cursor_offset: 0,
            anchor_offset: 0,
            preedit_range: None,
            syntax_cache: content
                .split('\n')
                .map(|line| {
                    vec![SyntaxSpan {
                        text: line.to_string(),
                        color: DEFAULT_TEXT,
                    }]
                })
                .collect(),
            line_height: 20.0,
            font_size: 13.0,
            gutter_width: 24.0,
            wrap_columns,
            scroll_y: 0.0,
            file_path: "test.txt".into(),
            viewport_height: 240.0,
            viewport_width: 320.0,
        }
    }

    #[test]
    fn soft_wrap_splits_long_lines_into_visual_rows() {
        let node = render_node("abcdefghijkl", WrapMode::SoftWrap, 4);
        let visual_lines = node.visual_lines();
        let texts: Vec<_> = visual_lines.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(texts, vec!["abcd", "efgh", "ijkl"]);
        assert!(visual_lines[0].starts_source_line);
        assert!(!visual_lines[1].starts_source_line);
    }

    #[test]
    fn soft_wrap_maps_visual_rows_back_to_offsets() {
        let node = render_node("abcdefghi", WrapMode::SoftWrap, 3);
        let offset = node.offset_for_visual_position(&node.content, 1, 1);
        assert_eq!(offset_to_line_col(&node.content, offset), (0, 4));
    }

    #[test]
    fn soft_wrap_reports_visual_position_for_offsets() {
        let node = render_node("abcdefghi", WrapMode::SoftWrap, 3);
        let offset = line_col_to_offset(&node.content, 0, 7);
        assert_eq!(
            node.visual_position_for_offset(&node.content, offset),
            (2, 1)
        );
    }

    #[test]
    fn visible_visual_line_range_tracks_scroll_offset() {
        let mut node = render_node(
            &(0..200)
                .map(|i| format!("line-{i}"))
                .collect::<Vec<_>>()
                .join("\n"),
            WrapMode::NoWrap,
            0,
        );
        node.viewport_height = 100.0;
        node.scroll_y = 400.0;
        let (first, last) = node.visible_visual_line_range(node.visual_lines().len());
        assert_eq!(first, 20);
        assert!(
            last <= 29,
            "expected a bounded visible window, got {first}..{last}"
        );
    }

    #[test]
    fn visible_visual_line_range_always_includes_at_least_one_row() {
        let mut node = render_node("a\nb\nc", WrapMode::NoWrap, 0);
        node.viewport_height = 0.0;
        node.scroll_y = 10_000.0;
        let (first, last) = node.visible_visual_line_range(node.visual_lines().len());
        assert_eq!((first, last), (2, 3));
    }

    #[test]
    fn no_wrap_lines_use_intrinsic_row_width() {
        let node = render_node(&"abcdefghijkl".repeat(8), WrapMode::NoWrap, 0);
        let visual_line = &node.visual_lines()[0];
        let width = node
            .code_box_width_for_visual_line(visual_line, node.font_size * 0.6)
            .expect("no-wrap lines should report an intrinsic width");
        assert!(
            width > node.viewport_width,
            "long no-wrap lines should extend past the viewport for clipping-based rendering"
        );
    }
}
