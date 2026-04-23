use crate::minimap::Minimap;
use crate::model::{EditorState, FileBuffer, Language, ShowContextMenu, UpdateCursorPosition, UpdateFileContent};
use crate::syntax;
use fission_core::op::Color;
use fission_core::ui::{Container, GestureDetector, Node, Row, Scroll, Text, TextContent, TextInput};
use fission_core::{ActionEnvelope, BuildCtx, FlexDirection, Handler, View, Widget, WidgetNodeId};
use fission_widgets::{HStack, VStack, Spacer};
use serde_json;

/// Maximum lines to render in the gutter to avoid GPU buffer overflow.
/// An outer Scroll wraps both gutter and TextInput so they scroll in unison.
const MAX_GUTTER_LINES: usize = 10000;

/// Line count threshold above which syntax highlighting is skipped to avoid
/// generating too many IR nodes (TextRuns) that would stall the paint cycle.
const SYNTAX_HIGHLIGHT_LINE_LIMIT: usize = 1000;

pub struct EditorSurface;

impl Widget<EditorState> for EditorSurface {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let tokens = &view.env.theme.tokens;

        let Some((tab, buffer)) = view.state.active_buffer() else {
            return self.build_welcome_screen(ctx, view);
        };

        let content = &buffer.content;
        let path = tab.path.clone();
        let cursor_line = buffer.cursor_line;

        // --- Bind UpdateFileContent action ---
        let update_id = ctx.bind(
            UpdateFileContent(String::new()),
            (|s: &mut EditorState, a: UpdateFileContent, _| {
                // Dismiss context menu on any content change
                s.context_menu_visible = false;
                if let Some(tab) = s.open_tabs.get(s.active_tab) {
                    let path = tab.path.clone();
                    if let Some(buf) = s.file_contents.get_mut(&path) {
                        buf.push_undo();
                        buf.content = a.0;
                        buf.version += 1;
                    }
                    if let Some(tab) = s.open_tabs.get_mut(s.active_tab) {
                        tab.is_dirty = true;
                    }
                    // Notify LSP of the content change
                    if let Some(ref handle) = s.lsp_handle {
                        if let Some(buf) = s.file_contents.get(&path) {
                            handle.notify_change(&path, &buf.content);
                        }
                    }

                    // Auto-trigger completions on trigger characters (dot, colon, open-paren).
                    // Check the character just before the cursor, not the last byte of the file.
                    let should_complete = if let Some(buf) = s.file_contents.get(&path) {
                        if buf.content.is_empty() || buf.cursor_col == 0 {
                            false
                        } else {
                            // Find the byte offset of the cursor and look at the preceding byte
                            let mut byte_offset = 0usize;
                            for (i, line) in buf.content.lines().enumerate() {
                                if i == buf.cursor_line {
                                    byte_offset += buf.cursor_col.min(line.len());
                                    break;
                                }
                                byte_offset += line.len() + 1; // +1 for '\n'
                            }
                            let prev_byte = if byte_offset > 0 {
                                buf.content.as_bytes().get(byte_offset - 1)
                            } else {
                                None
                            };
                            matches!(prev_byte, Some(b'.') | Some(b':') | Some(b'('))
                        }
                    } else {
                        false
                    };
                    if should_complete {
                        if let Some(ref handle) = s.lsp_handle {
                            if let Some(buf) = s.file_contents.get(&path) {
                                handle.request_completions(&path, buf.cursor_line as usize, buf.cursor_col as usize);
                            }
                        }
                    }
                }
            }) as Handler<EditorState, UpdateFileContent>,
        );

        // --- Bind UpdateCursorPosition action ---
        let cursor_id = ctx.bind(
            UpdateCursorPosition { caret: 0, anchor: 0 },
            (|s: &mut EditorState, a: UpdateCursorPosition, _| {
                if let Some(tab) = s.open_tabs.get(s.active_tab) {
                    let path = tab.path.clone();
                    if let Some(buf) = s.file_contents.get_mut(&path) {
                        // Convert byte offset to line/col
                        let mut line = 0;
                        let mut col = 0;
                        for (i, ch) in buf.content.char_indices() {
                            if i >= a.caret {
                                break;
                            }
                            if ch == '\n' {
                                line += 1;
                                col = 0;
                            } else {
                                col += 1;
                            }
                        }
                        buf.cursor_line = line;
                        buf.cursor_col = col;
                    }
                }
            }) as Handler<EditorState, UpdateCursorPosition>,
        );

        // --- Bind dismiss-context-menu on primary click ---
        let dismiss_context = ctx.bind(
            crate::model::DismissContextMenu,
            (|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
            }) as Handler<EditorState, crate::model::DismissContextMenu>,
        );

        // --- Bind context menu action for right-click ---
        let context_menu_action = ctx.bind(
            ShowContextMenu { x: 0.0, y: 0.0, target: None },
            (|s: &mut EditorState, a: ShowContextMenu, rctx: &mut fission_core::ReducerContext<EditorState>| {
                // Use pointer position from the ActionInput if available,
                // falling back to the payload position.
                let (px, py) = match rctx.input {
                    fission_core::ActionInput::Pointer { x, y, .. } => (*x, *y),
                    _ => (a.x, a.y),
                };
                // Clamp to reasonable bounds (avoid 0,0 overlap with sidebar)
                let final_x = if px < 10.0 { 250.0 } else { px };
                let final_y = if py < 10.0 { 100.0 } else { py };
                s.context_menu_visible = true;
                s.context_menu_position = (final_x, final_y);
                s.context_menu_target = a.target;
            }) as Handler<EditorState, ShowContextMenu>,
        );

        let line_count = content.lines().count().max(1);
        let visible_lines = line_count.min(MAX_GUTTER_LINES);
        let gutter_width = format!("{}", line_count).len() as f32 * 9.0 + 16.0;
        let is_large_file = line_count > MAX_GUTTER_LINES;

        // Height of the full content area. Must match the per-line height used
        // for gutter rows (20 px) plus the gutter padding (4 px top + 4 px bottom).
        // By giving both the gutter and the TextInput this explicit height the
        // TextInput's internal Scroll becomes inert (content fits) and a single
        // outer Scroll keeps gutter + text in sync.
        let line_height: f32 = 20.0;
        let content_height = visible_lines as f32 * line_height;

        // --- Line numbers gutter (single Text widget to minimize node count) ---
        let gutter_digits = format!("{}", line_count).len();
        let gutter_color = Color { r: 120, g: 120, b: 120, a: 255 };
        let mut line_numbers = String::with_capacity(visible_lines * (gutter_digits + 1));
        for i in 1..=visible_lines {
            use std::fmt::Write;
            let _ = write!(line_numbers, "{:>width$}", i, width = gutter_digits);
            if i < visible_lines {
                line_numbers.push('\n');
            }
        }
        if is_large_file {
            use std::fmt::Write;
            let _ = write!(line_numbers, "\n... +{} lines", line_count - MAX_GUTTER_LINES);
        }

        let gutter = Container::new(
            Text::new(line_numbers)
                .size(13.0)
                .color(gutter_color)
                .into_node(),
        )
        .width(gutter_width)
        .height(content_height + 8.0)
        .min_height(content_height + 8.0)
        .padding_all(4.0)
        .bg(Color { r: 37, g: 37, b: 38, a: 255 })
        .flex_shrink(0.0)
        .into_node();

        // --- Editable text area ---
        // For very large files, only put first N lines in the TextInput to avoid GPU overflow
        let edit_content = if is_large_file {
            content.lines().take(MAX_GUTTER_LINES).collect::<Vec<_>>().join("\n")
        } else {
            content.clone()
        };

        // --- Build find-match highlight ranges ---
        let highlight_ranges: Vec<(usize, usize, fission_ir::op::Color)> =
            if view.state.show_find_replace && !view.state.find_query.is_empty() {
                let query_len = view.state.find_query.len();
                view.state
                    .find_matches
                    .iter()
                    .filter_map(|(match_path, line, col)| {
                        if match_path != &path {
                            return None;
                        }
                        // Convert (line, col) to byte offset in edit_content
                        let mut byte_offset = 0;
                        for (i, text_line) in edit_content.lines().enumerate() {
                            if i == *line {
                                byte_offset += col;
                                break;
                            }
                            byte_offset += text_line.len() + 1; // +1 for '\n'
                        }
                        if byte_offset + query_len <= edit_content.len() {
                            Some((
                                byte_offset,
                                byte_offset + query_len,
                                fission_ir::op::Color { r: 255, g: 200, b: 0, a: 80 },
                            ))
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };

        // --- Generate syntax-highlighted runs ---
        // For files larger than SYNTAX_HIGHLIGHT_LINE_LIMIT we skip per-line
        // highlighting entirely and emit a single unstyled run.  This avoids
        // creating hundreds of TextRun IR nodes that would overflow the GPU
        // buffer or stall the build/layout/paint cycle.
        let lang = buffer.language;
        let visible_line_count = edit_content.lines().count().max(1);
        let styled_runs: Vec<fission_ir::op::TextRun> = if line_count > SYNTAX_HIGHLIGHT_LINE_LIMIT {
            // Large file — single run, no syntax colours
            vec![fission_ir::op::TextRun {
                text: edit_content.clone(),
                style: fission_ir::op::TextStyle {
                    font_size: 13.0,
                    color: fission_ir::op::Color { r: 212, g: 212, b: 212, a: 255 },
                    underline: false,
                    background_color: None,
                },
            }]
        } else {
            // Use document-level highlighting (tree-sitter for Rust, cached
            // by content hash so unchanged files are essentially free).
            let doc_spans = syntax::highlight_document(&edit_content, lang);
            doc_spans
                .into_iter()
                .enumerate()
                .flat_map(|(i, spans)| {
                    let mut runs: Vec<fission_ir::op::TextRun> = spans
                        .into_iter()
                        .map(|span| fission_ir::op::TextRun {
                            text: span.text,
                            style: fission_ir::op::TextStyle {
                                font_size: 13.0,
                                color: fission_ir::op::Color {
                                    r: span.color.r,
                                    g: span.color.g,
                                    b: span.color.b,
                                    a: span.color.a,
                                },
                                underline: false,
                                background_color: None,
                            },
                        })
                        .collect();
                    // Add newline between lines (except last)
                    if i < visible_line_count - 1 {
                        runs.push(fission_ir::op::TextRun {
                            text: "\n".to_string(),
                            style: fission_ir::op::TextStyle {
                                font_size: 13.0,
                                color: fission_ir::op::Color { r: 212, g: 212, b: 212, a: 255 },
                                underline: false,
                                background_color: None,
                            },
                        });
                    }
                    runs
                })
                .collect()
        };

        let editor_input = TextInput {
            id: Some(fission_ir::NodeId::explicit(&format!("editor_{}", path))),
            value: edit_content,
            placeholder: None,
            on_change: Some(update_id),
            on_cursor_change: Some(cursor_id),
            width: None,
            height: Some(content_height), // Fill content height — outer Scroll handles scrolling
            multiline: true,
            min_lines: None,
            max_lines: None,
            obscure_text: false,
            obscuring_character: '•',
            mask: None,
            styled_runs: Some(styled_runs),
            borderless: true,
            capture_tab: true,
            auto_indent: true,
            highlight_ranges,
        }
        .into_node();

        let editor_area = Container::new(editor_input)
            .flex_grow(1.0)
            .min_height(content_height)
            .bg(Color { r: 30, g: 30, b: 30, a: 255 })
            .into_node();

        // Wrap editor area in a GestureDetector for right-click context menu.
        // `on_tap` dismisses any visible context menu on primary click so that
        // left-clicking the editor never leaves a stale context menu open.
        let editor_with_gesture = GestureDetector {
            child: Box::new(editor_area),
            on_tap: Some(dismiss_context),
            on_secondary_click: Some(context_menu_action),
            ..Default::default()
        }
        .into_node();

        // 1px gutter separator
        let gutter_separator = Container::new(Spacer::default().into_node())
            .width(1.0)
            .bg(Color { r: 48, g: 48, b: 49, a: 255 })
            .flex_shrink(0.0)
            .into_node();

        // Build the editor column children
        let mut editor_column_children = Vec::new();

        // Large file indicator banner
        if is_large_file {
            let indicator = Container::new(
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        Text::new(format!(
                            "Large file mode \u{2014} showing first {} of {} lines",
                            MAX_GUTTER_LINES, line_count
                        ))
                        .size(11.0)
                        .color(Color { r: 180, g: 160, b: 80, a: 255 })
                        .into_node(),
                    ],
                }
                .into_node(),
            )
            .padding_all(4.0)
            .bg(Color { r: 50, g: 45, b: 25, a: 255 })
            .into_node();

            editor_column_children.push(indicator);
        }

        // 1px separator between editor content and minimap
        let minimap_separator = Container::new(Spacer::default().into_node())
            .width(1.0)
            .bg(Color { r: 48, g: 48, b: 49, a: 255 })
            .flex_shrink(0.0)
            .into_node();

        // Minimap: scaled-down overview of the file on the right
        let minimap_node = Minimap.build(ctx, view);

        // Inner row: gutter + separator + editor text area.
        // Both gutter and TextInput have explicit heights equal to the full
        // content, so the TextInput's internal scroll is inert.  A single
        // outer Scroll keeps them moving together.
        let scrollable_row = Row {
            children: vec![gutter, gutter_separator, editor_with_gesture],
            align_items: fission_ir::op::AlignItems::Stretch,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        // Outer scroll wraps both gutter and editor so they scroll in unison.
        // A stable ID keyed on the file path ensures the scroll offset survives
        // widget-tree rebuilds between frames.
        let scrollable = Scroll {
            id: Some(fission_ir::NodeId::explicit(&format!("editor_scroll_{}", path))),
            child: Some(Box::new(scrollable_row)),
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into_node();

        // Outer row: scrollable editor area | minimap separator | minimap
        let editor_row = Row {
            children: vec![scrollable, minimap_separator, minimap_node],
            align_items: fission_ir::op::AlignItems::Stretch,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        editor_column_children.push(editor_row);

        let editor_column = VStack {
            spacing: Some(0.0),
            children: editor_column_children,
        }
        .into_node();

        Container::new(editor_column)
            .bg(Color { r: 30, g: 30, b: 30, a: 255 })
            .flex_grow(1.0)
            .flex_shrink(1.0)
            .into_node()
    }
}

impl EditorSurface {
    fn build_welcome_screen(
        &self,
        ctx: &mut BuildCtx<EditorState>,
        view: &View<EditorState>,
    ) -> Node {
        let dim = Color { r: 100, g: 100, b: 100, a: 255 };
        let shortcut_color = Color { r: 130, g: 130, b: 130, a: 255 };
        let key_color = Color { r: 160, g: 160, b: 160, a: 255 };
        let heading_color = Color { r: 150, g: 150, b: 150, a: 255 };

        let shortcut_row = |keys: &str, desc: &str| -> Node {
            HStack {
                spacing: Some(16.0),
                children: vec![
                    Container::new(
                        Text::new(keys).size(12.0).color(key_color).into_node(),
                    ).width(140.0).into_node(),
                    Text::new(desc).size(12.0).color(shortcut_color).into_node(),
                ],
            }.into_node()
        };

        Container::new(
            fission_widgets::center::Center {
                child: Box::new(
                    VStack {
                        spacing: Some(8.0),
                        children: vec![
                            Text::new("Fission Editor")
                                .size(36.0)
                                .color(Color { r: 80, g: 80, b: 80, a: 255 })
                                .into_node(),
                            Spacer { height: Some(4.0), ..Default::default() }.into_node(),
                            Text::new("Open a file from the explorer to begin")
                                .size(14.0)
                                .color(dim)
                                .into_node(),
                            Spacer { height: Some(16.0), ..Default::default() }.into_node(),
                            // Keyboard shortcuts section
                            Text::new("Keyboard Shortcuts")
                                .size(14.0)
                                .color(heading_color)
                                .into_node(),
                            Spacer { height: Some(4.0), ..Default::default() }.into_node(),
                            shortcut_row("Ctrl+Shift+P", "Command Palette"),
                            shortcut_row("Ctrl+B", "Toggle Sidebar"),
                            shortcut_row("Ctrl+`", "Toggle Terminal"),
                            shortcut_row("Ctrl+S", "Save File"),
                            Spacer { height: Some(20.0), ..Default::default() }.into_node(),
                            // Recent files section
                            Text::new("Recent Files")
                                .size(14.0)
                                .color(heading_color)
                                .into_node(),
                            Spacer { height: Some(4.0), ..Default::default() }.into_node(),
                            Text::new("No recent files")
                                .size(12.0)
                                .color(dim)
                                .into_node(),
                        ],
                    }
                    .into_node(),
                ),
            }
            .build(ctx, view),
        )
        .bg(Color { r: 30, g: 30, b: 30, a: 255 })
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .into_node()
    }
}
