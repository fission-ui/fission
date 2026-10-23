use crate::editor_welcome_screen::EditorWelcomeScreen;
use crate::highlight;
use crate::layout::{
    ACTIVITY_BAR_WIDTH, BREADCRUMB_HEIGHT, DIVIDER_THICKNESS, EDITOR_HORIZONTAL_RESERVE,
    FIND_REPLACE_REGION_HEIGHT, MENU_BAR_HEIGHT, MINIMAP_WIDTH, MIN_EDITOR_HEIGHT,
    MIN_EDITOR_WIDTH, MIN_TERMINAL_HEIGHT, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, STATUS_BAR_HEIGHT,
    TAB_BAR_HEIGHT, TERMINAL_HEIGHT_FRACTION,
};
use crate::minimap::Minimap;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub EditorSurface<EditorState> |_surface, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let sidebar_width = state.sidebar_width.min(
            (cx.view.viewport_size().width - EDITOR_HORIZONTAL_RESERVE)
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
        );
        let terminal_height = if state.terminal_visible {
            state.terminal_height.min(
                (cx.view.viewport_size().height * TERMINAL_HEIGHT_FRACTION)
                    .max(MIN_TERMINAL_HEIGHT),
            )
        } else {
            0.0
        };
        let height = (cx.view.viewport_size().height
            - MENU_BAR_HEIGHT
            - STATUS_BAR_HEIGHT
            - TAB_BAR_HEIGHT
            - BREADCRUMB_HEIGHT
            - if state.show_find_replace { FIND_REPLACE_REGION_HEIGHT } else { 0.0 }
            - if state.terminal_visible { terminal_height + DIVIDER_THICKNESS } else { 0.0 })
            .max(MIN_EDITOR_HEIGHT);
        let width = (cx.view.viewport_size().width
            - ACTIVITY_BAR_WIDTH
            - if state.sidebar_visible { sidebar_width + DIVIDER_THICKNESS } else { 0.0 }
            - MINIMAP_WIDTH
            - tokens.spacing.l)
            .max(MIN_EDITOR_WIDTH);
        let active = state.active_buffer();
    } =>
    if let Some((tab, buffer)) = active {
        Container(
            VStack(
                spacing: tokens.spacing.none,
                children: [
                    Row(
                        align_items: ir_op::AlignItems::Stretch,
                        flex_grow: 1.0,
                        children: [
                            Container(
                                TextInput(
                                    id: Some(WidgetId::explicit(&format!("editor_input_{}", tab.path))),
                                    value: buffer.display_content(),
                                    styled_runs: highlight::highlighted_runs(
                                        &buffer.display_content(),
                                        buffer.language,
                                        &palette,
                                        tokens.typography.font_size_sm,
                                        tokens.typography.font_size_sm
                                            * tokens.typography.line_height_normal,
                                    ),
                                    on_input: |_surface, state, cx| {
                                        if let Some(change) = cx.input.text_change() {
                                            if let Some(tab) = state.open_tabs.get(state.active_tab) {
                                                let path = tab.path.clone();
                                                if let Some(buffer) = state.file_contents.get_mut(&path) {
                                                    if !buffer.is_editable() {
                                                        state.status_message = Some(
                                                            "This document is not editable".into(),
                                                        );
                                                        return;
                                                    }
                                                    buffer.replace_document(&change.new_text);
                                                }
                                                state.mark_active_tab_dirty();
                                                state.notify_buffer_changed(&path);
                                            }
                                        }
                                    },
                                    on_cursor_change: |_surface, state, cx| {
                                        if let Some(change) = cx.input.text_selection_change() {
                                            if let Some((_tab, buffer)) = state.active_buffer_mut() {
                                                buffer.clear_preedit();
                                                buffer.set_selection_offsets(
                                                    change.value.selection.extent.utf8_offset(),
                                                    change.value.selection.base.utf8_offset(),
                                                );
                                            }
                                        }
                                    },
                                    width: width,
                                    height: height,
                                    multiline: true,
                                    borderless: true,
                                    capture_tab: true,
                                    auto_indent: true,
                                    read_only: !buffer.is_editable(),
                                    font_size: tokens.typography.font_size_sm,
                                    line_height: tokens.typography.font_size_sm
                                        * tokens.typography.line_height_normal,
                                    text_color: palette.bright_text,
                                    cursor_color: palette.bright_text,
                                    selection_color: palette.editor_selection,
                                ),
                            )
                            .height(height)
                            .min_height(height)
                            .flex_grow(1.0)
                            .flex_shrink(1.0),
                            Container(Spacer())
                                .width(DIVIDER_THICKNESS)
                                .bg(palette.border_color)
                                .flex_shrink(0.0),
                            Minimap(),
                        ],
                    ),
                ],
            ),
        )
        .bg(palette.welcome_bg)
        .flex_grow(1.0)
        .flex_shrink(1.0)
    } else {
        EditorWelcomeScreen()
    }
}
