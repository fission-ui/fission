use crate::editor_welcome_screen::EditorWelcomeScreen;
use crate::layout::{
    ACTIVITY_BAR_WIDTH, BREADCRUMB_HEIGHT, DIVIDER_THICKNESS, EDITOR_HORIZONTAL_RESERVE,
    FIND_REPLACE_REGION_HEIGHT, MENU_BAR_HEIGHT, MINIMAP_WIDTH, MIN_EDITOR_HEIGHT,
    MIN_EDITOR_WIDTH, MIN_TERMINAL_HEIGHT, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, STATUS_BAR_HEIGHT,
    TAB_BAR_HEIGHT, TERMINAL_HEIGHT_FRACTION,
};
use crate::minimap::Minimap;
use crate::model::{EditorState, UpdateCursorPosition, UpdateEditorDocument};
use crate::palette::{BORDER_COLOR, BRIGHT_TEXT, EDITOR_SELECTION, WELCOME_BG};
use fission::core::ui::{Container, Row, TextInput, Widget};
use fission::core::{reduce_with, ReducerContext};
use fission::widgets::{Spacer, VStack};
use fission::WidgetId;

pub struct EditorSurface;

impl From<EditorSurface> for Widget {
    fn from(_component: EditorSurface) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;

        let sidebar_width = view.state().sidebar_width.min(
            (view.viewport_size().width - EDITOR_HORIZONTAL_RESERVE)
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
        );
        let terminal_height = if view.state().terminal_visible {
            view.state().terminal_height.min(
                (view.viewport_size().height * TERMINAL_HEIGHT_FRACTION).max(MIN_TERMINAL_HEIGHT),
            )
        } else {
            0.0
        };
        let editor_viewport_height = (view.viewport_size().height
            - MENU_BAR_HEIGHT
            - STATUS_BAR_HEIGHT
            - TAB_BAR_HEIGHT
            - BREADCRUMB_HEIGHT
            - if view.state().show_find_replace {
                FIND_REPLACE_REGION_HEIGHT
            } else {
                0.0
            }
            - if view.state().terminal_visible {
                terminal_height + DIVIDER_THICKNESS
            } else {
                0.0
            })
        .max(MIN_EDITOR_HEIGHT);
        let editor_viewport_width = (view.viewport_size().width
            - ACTIVITY_BAR_WIDTH
            - if view.state().sidebar_visible {
                sidebar_width + DIVIDER_THICKNESS
            } else {
                tokens.spacing.none
            }
            - MINIMAP_WIDTH
            - tokens.spacing.l)
            .max(MIN_EDITOR_WIDTH);

        let Some((tab, buffer)) = view.state().active_buffer() else {
            return EditorWelcomeScreen.into();
        };
        let path = tab.path.clone();

        let update_document = ctx.bind(
            UpdateEditorDocument,
            reduce_with!(
                (|s: &mut EditorState,
                  _a: UpdateEditorDocument,
                  ctx: &mut ReducerContext<EditorState>| {
                    let Some(change) = ctx.input.text_change() else {
                        return;
                    };
                    if let Some(tab) = s.open_tabs.get(s.active_tab) {
                        let path = tab.path.clone();
                        if let Some(buf) = s.file_contents.get_mut(&path) {
                            if !buf.is_editable() {
                                s.status_message = Some("This document is not editable".into());
                                return;
                            }
                            buf.replace_document(&change.new_text);
                        }
                        s.mark_active_tab_dirty();
                        s.notify_buffer_changed(&path);
                    }
                })
            ),
        );

        let update_cursor = ctx.bind(
            UpdateCursorPosition {
                caret: 0,
                anchor: 0,
            },
            reduce_with!(
                (|s: &mut EditorState, a: UpdateCursorPosition, _| {
                    if let Some((_tab, buf)) = s.active_buffer_mut() {
                        buf.clear_preedit();
                        buf.set_selection_offsets(a.caret, a.anchor);
                    }
                })
            ),
        );

        let editor_input: Widget = TextInput {
            id: Some(WidgetId::explicit(&format!("editor_input_{}", path))),
            value: buffer.display_content(),
            on_input: Some(update_document),
            on_cursor_change: Some(update_cursor),
            width: Some(editor_viewport_width),
            height: Some(editor_viewport_height),
            multiline: true,
            borderless: true,
            capture_tab: true,
            auto_indent: true,
            read_only: !buffer.is_editable(),
            font_size: Some(tokens.typography.font_size_sm),
            line_height: Some(
                tokens.typography.font_size_sm * tokens.typography.line_height_normal,
            ),
            text_color: Some(BRIGHT_TEXT),
            cursor_color: Some(fission::op::Color::WHITE),
            selection_color: Some(EDITOR_SELECTION),
            spell_check: false,
            smart_dashes: false,
            smart_quotes: false,
            ..Default::default()
        }
        .into();

        let editor_area = Container::new(editor_input)
            .height(editor_viewport_height)
            .min_height(editor_viewport_height)
            .flex_grow(1.0)
            .flex_shrink(1.0)
            .into();

        let minimap_separator = Container::new(Spacer::default())
            .width(DIVIDER_THICKNESS)
            .bg(BORDER_COLOR)
            .flex_shrink(0.0)
            .into();

        let minimap_node = Minimap.into();

        let editor_row = Row {
            children: vec![editor_area, minimap_separator, minimap_node],
            align_items: fission::op::AlignItems::Stretch,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into();

        let editor_column: Widget = VStack {
            spacing: Some(tokens.spacing.none),
            children: vec![editor_row],
        }
        .into();

        Container::new(editor_column)
            .bg(WELCOME_BG)
            .flex_grow(1.0)
            .flex_shrink(1.0)
            .into()
    }
}
