use crate::minimap::Minimap;
use crate::model::{EditorState, UpdateCursorPosition, UpdateEditorDocument};
use fission::core::op::Color;
use fission::core::ui::{Container, Row, Text, TextInput, Widget};
use fission::core::{reduce_with, BuildCtxHandle, ViewHandle};
use fission::widgets::{HStack, Spacer, VStack};
use fission::WidgetId;

pub struct EditorSurface;

impl From<EditorSurface> for Widget {
    fn from(component: EditorSurface) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        const MENU_BAR_HEIGHT: f32 = 28.0;
        const STATUS_BAR_HEIGHT: f32 = 26.0;
        const TAB_BAR_HEIGHT: f32 = 35.0;
        const BREADCRUMB_HEIGHT: f32 = 22.0;
        const FIND_REPLACE_HEIGHT: f32 = 60.0;
        const BOTTOM_PANEL_DIVIDER_HEIGHT: f32 = 1.0;

        let sidebar_width = view
            .state()
            .sidebar_width
            .min((view.viewport_size().width - 160.0).clamp(180.0, 360.0));
        let terminal_height = if view.state().terminal_visible {
            view.state()
                .terminal_height
                .min((view.viewport_size().height * 0.33).max(96.0))
        } else {
            0.0
        };
        let editor_viewport_height = (view.viewport_size().height
            - MENU_BAR_HEIGHT
            - STATUS_BAR_HEIGHT
            - TAB_BAR_HEIGHT
            - BREADCRUMB_HEIGHT
            - if view.state().show_find_replace {
                FIND_REPLACE_HEIGHT
            } else {
                0.0
            }
            - if view.state().terminal_visible {
                terminal_height + BOTTOM_PANEL_DIVIDER_HEIGHT
            } else {
                0.0
            })
        .max(120.0);
        let editor_viewport_width = (view.viewport_size().width
            - 48.0
            - if view.state().sidebar_visible {
                sidebar_width + 1.0
            } else {
                0.0
            }
            - 61.0
            - 24.0)
            .max(180.0);

        let Some((tab, buffer)) = view.state().active_buffer() else {
            return component.build_welcome_screen(ctx, view);
        };
        let path = tab.path.clone();

        let update_document = ctx.bind(
            UpdateEditorDocument(String::new()),
            reduce_with!(
                (|s: &mut EditorState, a: UpdateEditorDocument, _| {
                    if let Some(tab) = s.open_tabs.get(s.active_tab) {
                        let path = tab.path.clone();
                        if let Some(buf) = s.file_contents.get_mut(&path) {
                            if !buf.is_editable() {
                                s.status_message = Some("This document is not editable".into());
                                return;
                            }
                            buf.replace_document(&a.0);
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
            on_change: Some(update_document),
            on_cursor_change: Some(update_cursor),
            width: Some(editor_viewport_width),
            height: Some(editor_viewport_height),
            multiline: true,
            borderless: true,
            capture_tab: true,
            auto_indent: true,
            read_only: !buffer.is_editable(),
            font_size: Some(13.0),
            line_height: Some(20.0),
            text_color: Some(Color {
                r: 220,
                g: 220,
                b: 220,
                a: 255,
            }),
            cursor_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }),
            selection_color: Some(Color {
                r: 55,
                g: 100,
                b: 170,
                a: 160,
            }),
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
            .width(1.0)
            .bg(Color {
                r: 48,
                g: 48,
                b: 49,
                a: 255,
            })
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
            spacing: Some(0.0),
            children: vec![editor_row],
        }
        .into();

        Container::new(editor_column)
            .bg(Color {
                r: 30,
                g: 30,
                b: 30,
                a: 255,
            })
            .flex_grow(1.0)
            .flex_shrink(1.0)
            .into()
    }
}
impl EditorSurface {
    fn build_welcome_screen(
        &self,
        _ctx: BuildCtxHandle<EditorState>,
        _view: ViewHandle<EditorState>,
    ) -> Widget {
        let dim = Color {
            r: 100,
            g: 100,
            b: 100,
            a: 255,
        };
        let shortcut_color = Color {
            r: 130,
            g: 130,
            b: 130,
            a: 255,
        };
        let key_color = Color {
            r: 160,
            g: 160,
            b: 160,
            a: 255,
        };
        let heading_color = Color {
            r: 150,
            g: 150,
            b: 150,
            a: 255,
        };

        let shortcut_row = |keys: &str, desc: &str| -> Widget {
            HStack {
                spacing: Some(16.0),
                children: vec![
                    Container::new(Text::new(keys).size(12.0).color(key_color))
                        .width(140.0)
                        .into(),
                    Text::new(desc).size(12.0).color(shortcut_color).into(),
                ],
            }
            .into()
        };

        Container::new(fission::widgets::center::Center {
            child: VStack {
                spacing: Some(8.0),
                children: vec![
                    Text::new("Fission Editor")
                        .size(36.0)
                        .color(Color {
                            r: 80,
                            g: 80,
                            b: 80,
                            a: 255,
                        })
                        .into(),
                    Spacer {
                        height: Some(4.0),
                        ..Default::default()
                    }
                    .into(),
                    Text::new("Open a file from the explorer to begin")
                        .size(14.0)
                        .color(dim)
                        .into(),
                    Spacer {
                        height: Some(16.0),
                        ..Default::default()
                    }
                    .into(),
                    // Keyboard shortcuts section
                    Text::new("Keyboard Shortcuts")
                        .size(14.0)
                        .color(heading_color)
                        .into(),
                    Spacer {
                        height: Some(4.0),
                        ..Default::default()
                    }
                    .into(),
                    shortcut_row("Ctrl+Shift+P", "Command Palette"),
                    shortcut_row("Ctrl+B", "Toggle Sidebar"),
                    shortcut_row("Ctrl+`", "Toggle Terminal"),
                    shortcut_row("Ctrl+S", "Save File"),
                    Spacer {
                        height: Some(20.0),
                        ..Default::default()
                    }
                    .into(),
                    // Recent files section
                    Text::new("Recent Files")
                        .size(14.0)
                        .color(heading_color)
                        .into(),
                    Spacer {
                        height: Some(4.0),
                        ..Default::default()
                    }
                    .into(),
                    Text::new("No recent files").size(12.0).color(dim).into(),
                ],
            }
            .into(),
        })
        .bg(Color {
            r: 30,
            g: 30,
            b: 30,
            a: 255,
        })
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .into()
    }
}
