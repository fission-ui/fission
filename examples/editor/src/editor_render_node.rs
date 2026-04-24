//! Custom render object for the code editor surface.
//!
//! `EditorRenderNode` replaces the previous `TextInput`-based editor with a
//! single `LowerDyn` implementation that:
//!
//!   1. Only renders **visible** lines (virtual scrolling),
//!   2. Performs its own hit-testing geometry,
//!   3. Manages cursor bar placement directly via `DrawRect`.
//!
//! The struct is built from `EditorState` via [`EditorRenderNode::from_state`]
//! and wrapped in a `Node::Custom(CustomNode { .. })` by `EditorSurface`.

use crate::model::{EditorState, Language, UpdateCursorPosition, UpdateFileContent};
use crate::syntax;
use fission_core::action::ActionEnvelope;
use fission_core::event::{InputEvent, KeyCode, KeyEvent, PointerEvent};
use fission_core::lowering::{LoweringContext, NodeBuilder};
use fission_core::ui::custom_render::{CustomEventResult, CustomHitResult, CustomRenderObject};
use fission_core::ui::traits::LowerDyn;
use fission_ir::op::{
    AlignItems, Color as IrColor, Fill, FlexDirection, LayoutOp, Op, PaintOp, TextRun, TextStyle,
};
use fission_ir::NodeId;
use fission_core::{LayoutPoint, LayoutRect};
use std::fmt;

// ---------------------------------------------------------------------------
// SyntaxSpan — per-fragment colouring info for a single visible line
// ---------------------------------------------------------------------------

/// A span of text within a single line together with its display colour.
/// Produced by the syntax module and consumed during lowering to build
/// `TextRun` sequences for `DrawRichText`.
#[derive(Debug, Clone)]
pub struct SyntaxSpan {
    pub text: String,
    pub color: IrColor,
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
    /// Byte offset of the cursor (caret) within `content`.
    pub cursor_offset: usize,
    /// Byte offset of the selection anchor within `content`.
    pub anchor_offset: usize,
    /// Pre-computed syntax spans, one `Vec<SyntaxSpan>` per source line.
    pub syntax_cache: Vec<Vec<SyntaxSpan>>,
    /// Height of a single line in logical pixels.
    pub line_height: f32,
    /// Font size used for code text.
    pub font_size: f32,
    /// Width of the gutter (line-number column) in logical pixels.
    pub gutter_width: f32,
    /// Current vertical scroll offset in logical pixels.
    pub scroll_y: f32,
    /// Stable string used to derive deterministic `NodeId`s across rebuilds.
    pub file_path: String,
}

impl fmt::Debug for EditorRenderNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EditorRenderNode")
            .field("language", &self.language)
            .field("cursor_offset", &self.cursor_offset)
            .field("anchor_offset", &self.anchor_offset)
            .field("line_height", &self.line_height)
            .field("font_size", &self.font_size)
            .field("gutter_width", &self.gutter_width)
            .field("scroll_y", &self.scroll_y)
            .field("content_len", &self.content.len())
            .field("syntax_lines", &self.syntax_cache.len())
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
    pub fn from_state(state: &EditorState) -> Option<Self> {
        let (tab, buffer) = state.active_buffer()?;
        let content = buffer.content();
        let language = buffer.language;
        let file_path = tab.path.clone();

        // --- Compute cursor byte offset from (line, col) ---
        let cursor_offset = line_col_to_offset(&content, buffer.cursor_line, buffer.cursor_col);
        let anchor_offset = cursor_offset; // no selection tracking yet

        // --- Build syntax cache ---
        let line_count = content.lines().count().max(1);
        let syntax_cache: Vec<Vec<SyntaxSpan>> =
            if line_count > SYNTAX_HIGHLIGHT_LINE_LIMIT {
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

        Some(Self {
            content,
            language,
            cursor_offset,
            anchor_offset,
            syntax_cache,
            line_height: 20.0,
            font_size: 13.0,
            gutter_width,
            scroll_y: state.scroll_offset_y,
            file_path,
        })
    }
}

// ---------------------------------------------------------------------------
// LowerDyn implementation — the core rendering logic
// ---------------------------------------------------------------------------

impl LowerDyn for EditorRenderNode {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let total_lines = self.content.lines().count().max(1);

        // Use a generous default viewport height when layout has not run yet.
        let viewport_height = 800.0_f32;
        let first_line = (self.scroll_y / self.line_height).floor().max(0.0) as usize;
        let visible_count =
            (viewport_height / self.line_height).ceil() as usize + 1; // +1 for partial line
        let last_line = (first_line + visible_count).min(total_lines);

        // Collect source lines once.
        let all_lines: Vec<&str> = self.content.lines().collect();

        // Total content height (used for sizing the outer box so scrollbars
        // reflect the real document length).
        let content_height = total_lines as f32 * self.line_height;

        // We will accumulate row children (one per visible line). Each row
        // contains a gutter DrawText and a code DrawRichText, laid out in a
        // horizontal flex.
        let mut row_ids: Vec<NodeId> = Vec::with_capacity(last_line - first_line);

        let gutter_digits = format!("{}", total_lines).len();
        let (cursor_line, cursor_col) = offset_to_line_col(&self.content, self.cursor_offset);

        for line_idx in first_line..last_line {
            // ---- Gutter line number ------------------------------------------
            let line_num_text = format!("{:>width$}", line_idx + 1, width = gutter_digits);

            let gutter_paint_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText {
                    text: line_num_text,
                    size: self.font_size,
                    color: GUTTER_COLOR,
                    underline: false,
                    caret_index: None,
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
                        padding: [0.0, 4.0, 0.0, 4.0], // top, right, bottom, left
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        aspect_ratio: None,
                    }),
                );
                b.add_child(gutter_paint_id);
                b.build(cx)
            };

            // ---- Code text (syntax-highlighted rich text) --------------------
            let runs: Vec<TextRun> = if line_idx < self.syntax_cache.len() {
                self.syntax_cache[line_idx]
                    .iter()
                    .map(|span| TextRun {
                        text: span.text.clone(),
                        style: TextStyle {
                            font_size: self.font_size,
                            color: span.color,
                            underline: false,
                            background_color: None,
                        },
                    })
                    .collect()
            } else {
                // Fallback: unstyled line (should not happen if syntax_cache is
                // built correctly, but defensive).
                let text = all_lines
                    .get(line_idx)
                    .copied()
                    .unwrap_or("")
                    .to_string();
                vec![TextRun {
                    text,
                    style: TextStyle {
                        font_size: self.font_size,
                        color: DEFAULT_TEXT,
                        underline: false,
                        background_color: None,
                    },
                }]
            };

            let code_paint_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRichText {
                    runs,
                    caret_index: None,
                }),
            )
            .build(cx);

            let code_box_id = {
                let id = cx.next_node_id();
                let mut b = NodeBuilder::new(
                    id,
                    Op::Layout(LayoutOp::Box {
                        width: None,
                        height: Some(self.line_height),
                        min_width: None,
                        max_width: None,
                        min_height: None,
                        max_height: None,
                        padding: [0.0; 4],
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        aspect_ratio: None,
                    }),
                );
                b.add_child(code_paint_id);
                b.build(cx)
            };

            // ---- Compose gutter + code into a horizontal flex row ------------
            let row_id = {
                let id = cx.next_node_id();
                let mut b = NodeBuilder::new(
                    id,
                    Op::Layout(LayoutOp::Flex {
                        direction: FlexDirection::Row,
                        wrap: fission_ir::op::FlexWrap::NoWrap,
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        padding: [0.0; 4],
                        gap: None,
                        align_items: AlignItems::Center,
                        justify_content: fission_ir::op::JustifyContent::Start,
                    }),
                );
                b.add_child(gutter_box_id);
                b.add_child(code_box_id);
                b.build(cx)
            };

            row_ids.push(row_id);
        }

        // ---- Cursor bar (thin vertical rectangle) ----------------------------
        // Only emit the cursor if it falls within the visible range.
        let cursor_bar_id = if cursor_line >= first_line && cursor_line < last_line {
            let cursor_y = (cursor_line - first_line) as f32 * self.line_height;
            // Approximate cursor x from column: mono-width estimate.
            let char_width = self.font_size * 0.6;
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
                    wrap: fission_ir::op::FlexWrap::NoWrap,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    padding: [0.0; 4],
                    gap: None,
                    align_items: AlignItems::Stretch,
                    justify_content: fission_ir::op::JustifyContent::Start,
                }),
            );
            b.add_children(row_ids);
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
                    width: None,
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

        outer_id
    }

    fn stable_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.file_path.hash(&mut h);
        "EditorRenderNode".hash(&mut h);
        h.finish()
    }
}

// ---------------------------------------------------------------------------
// CustomRenderObject — hit-testing and event handling
// ---------------------------------------------------------------------------

impl CustomRenderObject for EditorRenderNode {
    fn hit_test(&self, local_point: LayoutPoint, _node_rect: LayoutRect) -> CustomHitResult {
        let byte_offset = self.point_to_offset(local_point);
        CustomHitResult::inside(Some(byte_offset))
    }

    fn handle_event(
        &self,
        node_id: NodeId,
        event: &InputEvent,
        node_rect: LayoutRect,
    ) -> CustomEventResult {
        match event {
            // --- Click to place caret ---
            InputEvent::Pointer(PointerEvent::Down { point, .. }) => {
                let local_point = LayoutPoint::new(
                    point.x - node_rect.origin.x,
                    point.y - node_rect.origin.y,
                );
                let byte_offset = self.point_to_offset(local_point);

                let action = UpdateCursorPosition {
                    caret: byte_offset,
                    anchor: byte_offset,
                };
                let envelope = ActionEnvelope::from(action);
                CustomEventResult::consumed_with(vec![(node_id, envelope)])
            }

            // --- Keyboard input ---
            InputEvent::Keyboard(KeyEvent::Down { key_code, modifiers }) => {
                self.handle_key(node_id, key_code, *modifiers)
            }

            _ => CustomEventResult::ignored(),
        }
    }
}

impl EditorRenderNode {
    /// Convert a local point (relative to the node's top-left) to a byte
    /// offset in `self.content`.
    fn point_to_offset(&self, local_point: LayoutPoint) -> usize {
        let char_width = self.font_size * 0.6;

        // Account for vertical scroll offset.
        let adjusted_y = local_point.y + self.scroll_y;
        let line = (adjusted_y / self.line_height).floor().max(0.0) as usize;

        // Horizontal: subtract gutter width, then divide by char width.
        let text_x = (local_point.x - self.gutter_width).max(0.0);
        let col = (text_x / char_width).round() as usize;

        line_col_to_offset(&self.content, line, col)
    }

    /// Handle a key press, producing a `CustomEventResult` with the
    /// appropriate actions to dispatch.
    fn handle_key(
        &self,
        node_id: NodeId,
        key_code: &KeyCode,
        _modifiers: u8,
    ) -> CustomEventResult {
        let mut content = self.content.clone();
        let mut offset = self.cursor_offset;
        let content_len = content.len();

        match key_code {
            // -- Character insertion --
            KeyCode::Char(ch) => {
                let clamped = offset.min(content_len);
                content.insert(clamped, *ch);
                offset = clamped + ch.len_utf8();
            }
            KeyCode::Space => {
                let clamped = offset.min(content_len);
                content.insert(clamped, ' ');
                offset = clamped + 1;
            }
            KeyCode::Enter => {
                let clamped = offset.min(content_len);
                content.insert(clamped, '\n');
                offset = clamped + 1;
            }
            KeyCode::Tab => {
                let clamped = offset.min(content_len);
                content.insert_str(clamped, "    ");
                offset = clamped + 4;
            }

            // -- Deletion --
            KeyCode::Backspace => {
                if offset > 0 {
                    // Find the start of the preceding character.
                    let prev_boundary = content[..offset]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    content.remove(prev_boundary);
                    offset = prev_boundary;
                }
            }

            // -- Arrow key navigation --
            KeyCode::Left => {
                if offset > 0 {
                    offset = content[..offset]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }
            KeyCode::Right => {
                if offset < content_len {
                    offset = content[offset..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| offset + i)
                        .unwrap_or(content_len);
                }
            }
            KeyCode::Up => {
                let (line, col) = offset_to_line_col(&content, offset);
                if line > 0 {
                    offset = line_col_to_offset(&content, line - 1, col);
                }
            }
            KeyCode::Down => {
                let (line, col) = offset_to_line_col(&content, offset);
                offset = line_col_to_offset(&content, line + 1, col);
            }
            KeyCode::Home => {
                // Move to start of current line.
                let (line, _) = offset_to_line_col(&content, offset);
                offset = line_col_to_offset(&content, line, 0);
            }
            KeyCode::End => {
                // Move to end of current line.
                let (line, _) = offset_to_line_col(&content, offset);
                let line_text = content.lines().nth(line).unwrap_or("");
                offset = line_col_to_offset(&content, line, line_text.len());
            }

            KeyCode::Escape => {
                return CustomEventResult::ignored();
            }
        }

        // Build actions: always update cursor, and update file content if it
        // changed.
        let mut actions: Vec<(NodeId, ActionEnvelope)> = Vec::new();

        if content != self.content {
            actions.push((
                node_id,
                ActionEnvelope::from(UpdateFileContent(content)),
            ));
        }

        actions.push((
            node_id,
            ActionEnvelope::from(UpdateCursorPosition {
                caret: offset,
                anchor: offset,
            }),
        ));

        CustomEventResult::consumed_with(actions)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a (line, col) pair to a byte offset in `content`.
pub(crate) fn line_col_to_offset(content: &str, line: usize, col: usize) -> usize {
    let mut offset = 0;
    for (i, text_line) in content.lines().enumerate() {
        if i == line {
            return offset + col.min(text_line.len());
        }
        offset += text_line.len() + 1; // +1 for '\n'
    }
    // Past the end — clamp to content length.
    content.len()
}

/// Convert a byte offset to a (line, col) pair.
pub(crate) fn offset_to_line_col(content: &str, offset: usize) -> (usize, usize) {
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

/// Default (unstyled) text colour — matches VS Code Dark+.
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

/// Editor background colour.
const EDITOR_BG: IrColor = IrColor {
    r: 30,
    g: 30,
    b: 30,
    a: 255,
};
