//! Custom render object for the code editor surface.
//!
//! `EditorRenderNode` replaces the previous `TextInput`-based editor with a
//! single `LowerDyn` implementation that:
//!
//!   1. Only renders **visible** lines (virtual scrolling),
//!   2. Performs its own hit-testing geometry,
//!   3. Manages cursor bar placement directly via `DrawRect`.
//!   4. Renders selection highlights behind text.
//!   5. Scrolls to follow the cursor.
//!
//! The struct is built from `EditorState` via [`EditorRenderNode::from_state`]
//! and wrapped in a `Node::Custom(CustomNode { .. })` by `EditorSurface`.

use crate::model::{EditorState, Language, UpdateCursorPosition, UpdateFileContent, UpdateScrollY};
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
    /// Viewport height in logical pixels (from the last layout pass).
    pub viewport_height: f32,
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
        let anchor_offset = line_col_to_offset(&content, buffer.anchor_line, buffer.anchor_col);

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
            viewport_height: 800.0,
        })
    }
}

// ---------------------------------------------------------------------------
// LowerDyn implementation -- the core rendering logic
// ---------------------------------------------------------------------------

impl LowerDyn for EditorRenderNode {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let total_lines = self.content.lines().count().max(1);

        // Render ALL lines — the outer Scroll widget handles viewport
        // clipping and only paints what's visible.  Virtual scrolling
        // (rendering only visible lines) requires access to the Scroll
        // widget's runtime offset which isn't available during lowering.
        // For files under ~500 lines this is fine; larger files are
        // already capped by MAX_GUTTER_LINES.
        let first_line = 0;
        let last_line = total_lines;

        // Collect source lines once.
        let all_lines: Vec<&str> = self.content.lines().collect();

        // Total content height (used for sizing the outer box so scrollbars
        // reflect the real document length).
        let content_height = total_lines as f32 * self.line_height;

        // Pre-compute selection range (in byte offsets) for highlight rendering.
        let sel_start = self.cursor_offset.min(self.anchor_offset);
        let sel_end = self.cursor_offset.max(self.anchor_offset);
        let has_selection = sel_start != sel_end;

        // Convert selection to line/col for per-line highlight rectangles.
        let (sel_start_line, sel_start_col) = offset_to_line_col(&self.content, sel_start);
        let (sel_end_line, sel_end_col) = offset_to_line_col(&self.content, sel_end);

        // We will accumulate row children (one per visible line). Each row
        // contains a gutter DrawText and a code DrawRichText, laid out in a
        // horizontal flex.
        let mut row_ids: Vec<NodeId> = Vec::with_capacity(last_line - first_line);

        let gutter_digits = format!("{}", total_lines).len();
        let (cursor_line, cursor_col) = offset_to_line_col(&self.content, self.cursor_offset);
        let char_width = self.font_size * 0.6;

        for line_idx in first_line..last_line {
            // ---- Selection highlight for this line ------------------------------
            let mut line_children: Vec<NodeId> = Vec::new();

            if has_selection && line_idx >= sel_start_line && line_idx <= sel_end_line {
                let line_text = all_lines.get(line_idx).copied().unwrap_or("");
                let line_len = line_text.len();

                let highlight_start_col = if line_idx == sel_start_line {
                    sel_start_col
                } else {
                    0
                };
                let highlight_end_col = if line_idx == sel_end_line {
                    sel_end_col
                } else {
                    line_len
                };

                if highlight_end_col > highlight_start_col {
                    let sel_x = self.gutter_width + highlight_start_col as f32 * char_width;
                    let sel_w = (highlight_end_col - highlight_start_col) as f32 * char_width;

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
            let text_row_id = {
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

            // If there are selection highlights, wrap in a ZStack so the
            // highlight is behind the text row.
            let row_id = if line_children.is_empty() {
                text_row_id
            } else {
                let id = cx.next_node_id();
                let mut b = NodeBuilder::new(id, Op::Layout(LayoutOp::ZStack));
                // Selection highlight first (behind).
                for child_id in line_children {
                    b.add_child(child_id);
                }
                // Text row on top.
                b.add_child(text_row_id);
                b.build(cx)
            };

            row_ids.push(row_id);
        }

        // ---- Cursor bar (thin vertical rectangle) ----------------------------
        // Only emit the cursor if it falls within the visible range.
        let cursor_bar_id = if cursor_line >= first_line && cursor_line < last_line {
            let cursor_y = (cursor_line - first_line) as f32 * self.line_height;
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
// CustomRenderObject -- hit-testing and event handling
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

    /// Compute the scroll_y needed to keep a given line visible.
    /// Returns `Some(new_scroll_y)` if the scroll needs to change, `None`
    /// if the line is already in view.
    fn scroll_y_to_reveal_line(&self, line: usize) -> Option<f32> {
        let total_lines = self.content.lines().count().max(1);
        let content_height = total_lines as f32 * self.line_height;
        let viewport = self.viewport_height;

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

    /// Find word boundaries around a byte offset.
    /// Returns `(word_start, word_end)` as byte offsets.
    fn word_at_offset(&self, offset: usize) -> (usize, usize) {
        let bytes = self.content.as_bytes();
        let len = bytes.len();
        let clamped = offset.min(len);

        // Walk backward to find word start.
        let mut start = clamped;
        while start > 0 {
            let prev = start - 1;
            if is_word_char(bytes[prev]) {
                start = prev;
            } else {
                break;
            }
        }

        // Walk forward to find word end.
        let mut end = clamped;
        while end < len {
            if is_word_char(bytes[end]) {
                end += 1;
            } else {
                break;
            }
        }

        (start, end)
    }

    /// Handle a key press, producing a `CustomEventResult` with the
    /// appropriate actions to dispatch.
    fn handle_key(
        &self,
        node_id: NodeId,
        key_code: &KeyCode,
        modifiers: u8,
    ) -> CustomEventResult {
        let shift = (modifiers & 1) != 0;
        let ctrl_or_cmd = (modifiers & 4) != 0 || (modifiers & 8) != 0;

        let mut content = self.content.clone();
        let mut offset = self.cursor_offset;
        let mut anchor = self.anchor_offset;
        let content_len = content.len();

        // Helper: delete the current selection (if any) before inserting.
        let delete_selection = |content: &mut String, cursor: &mut usize, anch: &mut usize| {
            let sel_start = (*cursor).min(*anch);
            let sel_end = (*cursor).max(*anch);
            if sel_start != sel_end {
                content.replace_range(sel_start..sel_end, "");
                *cursor = sel_start;
                *anch = sel_start;
                true
            } else {
                false
            }
        };

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
                offset = line_col_to_offset(&content, line, line_text.len());
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
                delete_selection(&mut content, &mut offset, &mut anchor);
                let clamped = offset.min(content.len());
                content.insert(clamped, *ch);
                offset = clamped + ch.len_utf8();
                anchor = offset;
            }
            KeyCode::Space => {
                delete_selection(&mut content, &mut offset, &mut anchor);
                let clamped = offset.min(content.len());
                content.insert(clamped, ' ');
                offset = clamped + 1;
                anchor = offset;
            }
            KeyCode::Enter => {
                delete_selection(&mut content, &mut offset, &mut anchor);
                let clamped = offset.min(content.len());
                content.insert(clamped, '\n');
                offset = clamped + 1;
                anchor = offset;
            }
            KeyCode::Tab => {
                delete_selection(&mut content, &mut offset, &mut anchor);
                let clamped = offset.min(content.len());
                content.insert_str(clamped, "    ");
                offset = clamped + 4;
                anchor = offset;
            }

            // -- Deletion --
            KeyCode::Backspace => {
                let sel_start = offset.min(anchor);
                let sel_end = offset.max(anchor);
                if sel_start != sel_end {
                    // Delete the selection.
                    content.replace_range(sel_start..sel_end, "");
                    offset = sel_start;
                    anchor = sel_start;
                } else if offset > 0 {
                    // Find the start of the preceding character.
                    let prev_boundary = content[..offset]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    content.remove(prev_boundary);
                    offset = prev_boundary;
                    anchor = offset;
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
                if !shift {
                    anchor = offset;
                }
            }
            KeyCode::Right => {
                if offset < content.len() {
                    offset = content[offset..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| offset + i)
                        .unwrap_or(content.len());
                }
                if !shift {
                    anchor = offset;
                }
            }
            KeyCode::Up => {
                let (line, col) = offset_to_line_col(&content, offset);
                if line > 0 {
                    offset = line_col_to_offset(&content, line - 1, col);
                }
                if !shift {
                    anchor = offset;
                }
            }
            KeyCode::Down => {
                let (line, col) = offset_to_line_col(&content, offset);
                offset = line_col_to_offset(&content, line + 1, col);
                if !shift {
                    anchor = offset;
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
                offset = line_col_to_offset(&content, line, line_text.len());
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
        }

        // Build actions: always update cursor, and update file content if it
        // changed.
        let mut actions: Vec<(NodeId, ActionEnvelope)> = Vec::new();

        if content != self.content {
            actions.push((
                node_id,
                ActionEnvelope::from(UpdateFileContent(content.clone())),
            ));
        }

        actions.push((
            node_id,
            ActionEnvelope::from(UpdateCursorPosition {
                caret: offset,
                anchor,
            }),
        ));

        // Scroll to keep cursor visible.
        let (cursor_line, _) = offset_to_line_col(&content, offset);
        if let Some(new_scroll) = self.scroll_y_to_reveal_line(cursor_line) {
            actions.push((
                node_id,
                ActionEnvelope::from(UpdateScrollY(new_scroll)),
            ));
        }

        CustomEventResult::consumed_with(actions)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns true if the byte is a "word character" (alphanumeric or underscore).
fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Convert a (line, col) pair to a byte offset in `content`.
pub(crate) fn line_col_to_offset(content: &str, line: usize, col: usize) -> usize {
    let mut offset = 0;
    for (i, text_line) in content.lines().enumerate() {
        if i == line {
            return offset + col.min(text_line.len());
        }
        offset += text_line.len() + 1; // +1 for '\n'
    }
    // Past the end -- clamp to content length.
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
