use crate::editor_render_node::EditorRenderNode;
use crate::minimap::Minimap;
use crate::model::{
    ApplyEditorEdit, EditorState, SetEditorPreedit, ShiftActiveFileWindow, UpdateCursorPosition,
    UpdateScrollY,
};
use fission_core::op::Color;
use fission_core::ui::custom_render::CustomRenderObject;
use fission_core::ui::traits::LowerDyn;
use fission_core::ui::{Container, CustomNode, Node, Row, Scroll, Text};
use fission_core::{BuildCtx, FlexDirection, Handler, View, Widget};
use fission_ir::NodeId as IrNodeId;
use fission_widgets::{HStack, Spacer, VStack};
use std::sync::Arc;

pub struct EditorSurface;

impl Widget<EditorState> for EditorSurface {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        // If there is no active buffer, show the welcome screen.
        const MENU_BAR_HEIGHT: f32 = 28.0;
        const STATUS_BAR_HEIGHT: f32 = 26.0;
        const TAB_BAR_HEIGHT: f32 = 35.0;
        const BREADCRUMB_HEIGHT: f32 = 22.0;
        const FIND_REPLACE_HEIGHT: f32 = 60.0;
        const BOTTOM_PANEL_DIVIDER_HEIGHT: f32 = 1.0;

        let sidebar_width = view
            .state
            .sidebar_width
            .min((view.viewport_size().width - 160.0).clamp(180.0, 360.0));
        let terminal_height = if view.state.terminal_visible {
            view.state
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
            - if view.state.show_find_replace {
                FIND_REPLACE_HEIGHT
            } else {
                0.0
            }
            - if view.state.terminal_visible {
                terminal_height + BOTTOM_PANEL_DIVIDER_HEIGHT
            } else {
                0.0
            })
        .max(120.0);
        let editor_viewport_width = (view.viewport_size().width
            - 48.0
            - if view.state.sidebar_visible {
                sidebar_width + 1.0
            } else {
                0.0
            }
            - 61.0
            - 24.0)
            .max(180.0);
        let active_path = view.state.active_buffer().map(|(tab, _)| tab.path.clone());
        let editor_scroll_id = active_path
            .as_ref()
            .map(|path| IrNodeId::explicit(&format!("editor_scroll_{}", path)));
        let scroll_y = editor_scroll_id
            .and_then(|id| view.runtime.scroll.offsets.get(&id).copied())
            .unwrap_or(view.state.scroll_offset_y);
        let render_node = match EditorRenderNode::from_state(
            view.state,
            editor_viewport_width,
            editor_viewport_height,
            scroll_y,
        ) {
            Some(rn) => rn,
            None => return self.build_welcome_screen(ctx, view),
        };

        let path = render_node.file_path.clone();
        let content_height = render_node.content_height();
        let editor_canvas_height = content_height.max(editor_viewport_height);

        // ---- Register reducers for actions dispatched by the render object ---
        ctx.bind(
            UpdateCursorPosition {
                caret: 0,
                anchor: 0,
            },
            (|s: &mut EditorState, a: UpdateCursorPosition, _| {
                if let Some((_tab, buf)) = s.active_buffer_mut() {
                    buf.clear_preedit();
                    buf.set_selection_offsets(a.caret, a.anchor);
                }
            }) as Handler<EditorState, UpdateCursorPosition>,
        );

        ctx.bind(
            ApplyEditorEdit {
                range_start: 0,
                range_end: 0,
                new_text: String::new(),
                caret: 0,
                anchor: 0,
            },
            (|s: &mut EditorState, a: ApplyEditorEdit, _| {
                if let Some(tab) = s.open_tabs.get(s.active_tab) {
                    let path = tab.path.clone();
                    if let Some(buf) = s.file_contents.get_mut(&path) {
                        if !buf.is_editable() {
                            s.status_message = Some("This document is not editable".into());
                            return;
                        }
                        buf.apply_edit(a.range_start..a.range_end, &a.new_text);
                        buf.set_selection_offsets(a.caret, a.anchor);
                    }
                    s.mark_active_tab_dirty();
                    s.notify_buffer_changed(&path);
                }
            }) as Handler<EditorState, ApplyEditorEdit>,
        );

        ctx.bind(
            SetEditorPreedit {
                text: String::new(),
            },
            (|s: &mut EditorState, a: SetEditorPreedit, _| {
                if let Some((_tab, buf)) = s.active_buffer_mut() {
                    buf.set_preedit(a.text);
                }
            }) as Handler<EditorState, SetEditorPreedit>,
        );

        ctx.bind(
            UpdateScrollY(0.0),
            (|s: &mut EditorState, a: UpdateScrollY, _| {
                s.scroll_offset_y = a.0;
            }) as Handler<EditorState, UpdateScrollY>,
        );

        ctx.bind(
            ShiftActiveFileWindow { forward: true },
            (|s: &mut EditorState, a: ShiftActiveFileWindow, _| {
                s.shift_active_file_window(a.forward);
            }) as Handler<EditorState, ShiftActiveFileWindow>,
        );

        // ---- Editor surface via CustomNode ----------------------------------
        let rn = Arc::new(render_node);
        let lowerer: Arc<dyn LowerDyn> = rn.clone();
        let render_obj: Arc<dyn CustomRenderObject> = rn;

        let editor_custom = Node::Custom(CustomNode {
            debug_tag: format!("EditorRenderNode({})", path),
            lowerer: Some(lowerer),
            render_object: Some(render_obj),
        });

        // Wrap the custom node in a Container that fills available space.
        let editor_area = Container::new(editor_custom)
            .height(editor_canvas_height)
            .min_height(editor_canvas_height)
            .flex_grow(0.0)
            .flex_shrink(0.0)
            .into_node();

        // ---- Outer scroll ---------------------------------------------------
        // A single Scroll wraps the EditorRenderNode so the cursor and gutter
        // scroll together. The render node reports full content height so the
        // scrollbar reflects the real document length.
        let scrollable = Scroll {
            id: Some(IrNodeId::explicit(&format!("editor_scroll_{}", path))),
            child: Some(Box::new(editor_area)),
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into_node();

        // ---- Minimap (kept as a separate widget) ----------------------------
        let minimap_separator = Container::new(Spacer::default().into_node())
            .width(1.0)
            .bg(Color {
                r: 48,
                g: 48,
                b: 49,
                a: 255,
            })
            .flex_shrink(0.0)
            .into_node();

        let minimap_node = Minimap.build(ctx, view);

        // Outer row: scrollable editor | separator | minimap
        let editor_row = Row {
            children: vec![scrollable, minimap_separator, minimap_node],
            align_items: fission_ir::op::AlignItems::Stretch,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        let editor_column = VStack {
            spacing: Some(0.0),
            children: vec![editor_row],
        }
        .into_node();

        Container::new(editor_column)
            .bg(Color {
                r: 30,
                g: 30,
                b: 30,
                a: 255,
            })
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

        let shortcut_row = |keys: &str, desc: &str| -> Node {
            HStack {
                spacing: Some(16.0),
                children: vec![
                    Container::new(Text::new(keys).size(12.0).color(key_color).into_node())
                        .width(140.0)
                        .into_node(),
                    Text::new(desc).size(12.0).color(shortcut_color).into_node(),
                ],
            }
            .into_node()
        };

        Container::new(
            fission_widgets::center::Center {
                child: Box::new(
                    VStack {
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
                                .into_node(),
                            Spacer {
                                height: Some(4.0),
                                ..Default::default()
                            }
                            .into_node(),
                            Text::new("Open a file from the explorer to begin")
                                .size(14.0)
                                .color(dim)
                                .into_node(),
                            Spacer {
                                height: Some(16.0),
                                ..Default::default()
                            }
                            .into_node(),
                            // Keyboard shortcuts section
                            Text::new("Keyboard Shortcuts")
                                .size(14.0)
                                .color(heading_color)
                                .into_node(),
                            Spacer {
                                height: Some(4.0),
                                ..Default::default()
                            }
                            .into_node(),
                            shortcut_row("Ctrl+Shift+P", "Command Palette"),
                            shortcut_row("Ctrl+B", "Toggle Sidebar"),
                            shortcut_row("Ctrl+`", "Toggle Terminal"),
                            shortcut_row("Ctrl+S", "Save File"),
                            Spacer {
                                height: Some(20.0),
                                ..Default::default()
                            }
                            .into_node(),
                            // Recent files section
                            Text::new("Recent Files")
                                .size(14.0)
                                .color(heading_color)
                                .into_node(),
                            Spacer {
                                height: Some(4.0),
                                ..Default::default()
                            }
                            .into_node(),
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
        .bg(Color {
            r: 30,
            g: 30,
            b: 30,
            a: 255,
        })
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .into_node()
    }
}
