use crate::editor_render_node::EditorRenderNode;
use crate::minimap::Minimap;
use crate::model::{
    ApplyEditorEdit, EditorState, SetEditorPreedit, UpdateCursorPosition, UpdateScrollY,
};
use fission_core::op::Color;
use fission_core::ui::custom_render::CustomRenderObject;
use fission_core::ui::traits::LowerDyn;
use fission_core::ui::{Container, CustomNode, Node, Row, Scroll, Text};
use fission_core::{BuildCtx, FlexDirection, Handler, View, Widget};
use fission_widgets::{HStack, Spacer, VStack};
use std::sync::Arc;

pub struct EditorSurface;

impl Widget<EditorState> for EditorSurface {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        // If there is no active buffer, show the welcome screen.
        let render_node = match EditorRenderNode::from_state(view.state) {
            Some(rn) => rn,
            None => return self.build_welcome_screen(ctx, view),
        };

        let path = render_node.file_path.clone();
        let content_height = {
            let line_count = render_node.content.lines().count().max(1);
            line_count as f32 * render_node.line_height
        };

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
            .flex_grow(1.0)
            .min_height(content_height)
            .into_node();

        // ---- Outer scroll ---------------------------------------------------
        // A single Scroll wraps the EditorRenderNode so the cursor and gutter
        // scroll together. The render node reports full content height so the
        // scrollbar reflects the real document length.
        let scrollable = Scroll {
            id: Some(fission_ir::NodeId::explicit(&format!(
                "editor_scroll_{}",
                path
            ))),
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
