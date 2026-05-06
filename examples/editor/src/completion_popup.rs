//! Auto-complete popup widget that displays LSP completion suggestions.

use crate::model::*;
use fission_core::op::Color;
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, GestureDetector, Node, Positioned,
    Scroll, Text, ZStack,
};
use fission_core::{
    ActionEnvelope, BuildCtx, FlexDirection, Handler, PortalLayer, View, Widget, WidgetNodeId,
};
use fission_widgets::{HStack, Spacer, VStack};

pub struct CompletionPopup;

/// Map a completion kind string to a short icon/label for the list.
fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "function" | "method" => "fn",
        "variable" | "field" => "ab",
        "keyword" => "kw",
        "struct" | "class" => "St",
        "enum" => "En",
        "module" => "Md",
        "property" => "Pr",
        "constant" => "Co",
        "interface" | "trait" => "Tr",
        "type" => "Ty",
        "snippet" => "Sn",
        _ => "  ",
    }
}

/// Pick a color for the kind badge.
fn kind_color(kind: &str) -> Color {
    match kind {
        "function" | "method" => Color {
            r: 220,
            g: 170,
            b: 80,
            a: 255,
        }, // orange
        "variable" | "field" => Color {
            r: 100,
            g: 180,
            b: 255,
            a: 255,
        }, // blue
        "keyword" => Color {
            r: 200,
            g: 120,
            b: 220,
            a: 255,
        }, // purple
        "struct" | "class" | "enum" => Color {
            r: 80,
            g: 200,
            b: 170,
            a: 255,
        }, // teal
        "module" => Color {
            r: 220,
            g: 220,
            b: 100,
            a: 255,
        }, // yellow
        _ => Color {
            r: 180,
            g: 180,
            b: 180,
            a: 255,
        }, // grey
    }
}

impl Widget<EditorState> for CompletionPopup {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        if !view.state.show_completions || view.state.completions.is_empty() {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into_node();
        }

        let dismiss = ctx.bind(
            DismissCompletions,
            (|s: &mut EditorState, _, _| {
                s.show_completions = false;
                s.completions.clear();
                s.selected_completion = 0;
            }) as Handler<EditorState, DismissCompletions>,
        );

        let select_id = ctx
            .bind(
                SelectCompletion(0),
                (|s: &mut EditorState, a: SelectCompletion, _| {
                    let idx = a.0;
                    let label = s.completions.get(idx).map(|item| item.label.clone());
                    if let Some(label) = label {
                        // Insert the selected completion label into the active buffer
                        if let Some((_tab, buf)) = s.active_buffer_mut() {
                            let (caret, _anchor) = buf.current_offsets();
                            buf.apply_edit(caret..caret, &label);
                            let next = caret + label.len();
                            buf.set_selection_offsets(next, next);
                        }
                        s.mark_active_tab_dirty();
                        if let Some(tab) = s.open_tabs.get(s.active_tab) {
                            let path = tab.path.clone();
                            s.notify_buffer_changed(&path);
                        }
                    }
                    s.show_completions = false;
                    s.completions.clear();
                    s.selected_completion = 0;
                }) as Handler<EditorState, SelectCompletion>,
            )
            .id;

        let bg = Color {
            r: 37,
            g: 37,
            b: 38,
            a: 255,
        };
        let border_color = Color {
            r: 69,
            g: 69,
            b: 69,
            a: 255,
        };
        let text_color = Color {
            r: 220,
            g: 220,
            b: 220,
            a: 255,
        };
        let detail_color = Color {
            r: 140,
            g: 140,
            b: 140,
            a: 255,
        };
        let selected_bg = Color {
            r: 4,
            g: 57,
            b: 94,
            a: 255,
        };
        let viewport = view.viewport_size();
        let popup_width = (viewport.width - 80.0).clamp(220.0, 360.0);
        let popup_height = (viewport.height * 0.28).clamp(120.0, 220.0);

        let selected_idx = view.state.selected_completion;

        let mut items = Vec::new();
        for (i, completion) in view.state.completions.iter().enumerate() {
            let icon_text = kind_icon(&completion.kind);
            let icon_color = kind_color(&completion.kind);
            let is_selected = i == selected_idx;

            let row = HStack {
                spacing: Some(6.0),
                children: vec![
                    // Kind badge
                    Container::new(
                        Text::new(icon_text)
                            .size(10.0)
                            .color(icon_color)
                            .into_node(),
                    )
                    .width(20.0)
                    .into_node(),
                    // Label
                    Text::new(completion.label.as_str())
                        .size(12.0)
                        .color(text_color)
                        .into_node(),
                    // Spacer between label and detail
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                    // Detail (if present), truncated
                    if let Some(detail) = &completion.detail {
                        let truncated: String = detail.chars().take(30).collect();
                        Text::new(truncated)
                            .size(11.0)
                            .color(detail_color)
                            .into_node()
                    } else {
                        Spacer {
                            width: Some(0.0),
                            ..Default::default()
                        }
                        .into_node()
                    },
                ],
            }
            .into_node();

            let mut btn_container = Button {
                variant: ButtonVariant::Ghost,
                content_align: ButtonContentAlign::Start,
                child: Some(Box::new(row)),
                on_press: Some(ActionEnvelope {
                    id: select_id,
                    payload: serde_json::to_vec(&SelectCompletion(i)).unwrap(),
                }),
                height: Some(24.0),
                padding: Some([2.0, 4.0, 2.0, 4.0]),
                ..Default::default()
            }
            .into_node();

            // Highlight the selected item
            if is_selected {
                btn_container = Container::new(btn_container).bg(selected_bg).into_node();
            }

            items.push(btn_container);
        }

        // Compute position near cursor. Use the hover_position as a rough proxy
        // for cursor screen location; the editor surface can set this more precisely.
        let (popup_x, popup_y) = view.state.hover_position;
        // Offset slightly below the cursor line
        let popup_y = (popup_y + 18.0).min((viewport.height - popup_height - 16.0).max(8.0));
        let popup_x = popup_x.min((viewport.width - popup_width - 16.0).max(8.0));

        let list = Container::new(
            Scroll {
                direction: FlexDirection::Column,
                child: Some(Box::new(
                    VStack {
                        spacing: Some(0.0),
                        children: items,
                    }
                    .into_node(),
                )),
                show_scrollbar: true,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            }
            .into_node(),
        )
        .bg(bg)
        .border(border_color, 1.0)
        .border_radius(4.0)
        .max_height(popup_height)
        .width(popup_width)
        .into_node();

        let positioned_popup = Positioned {
            left: Some(popup_x),
            top: Some(popup_y),
            child: Some(Box::new(list)),
            ..Default::default()
        }
        .into_node();

        // Transparent backdrop to dismiss on tap outside
        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child: Box::new(
                Container::new(Spacer::default().into_node())
                    .bg(Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    })
                    .flex_grow(1.0)
                    .into_node(),
            ),
            ..Default::default()
        }
        .into_node();

        let overlay = Container::new(
            ZStack {
                children: vec![
                    Positioned {
                        left: Some(0.0),
                        right: Some(0.0),
                        top: Some(0.0),
                        bottom: Some(0.0),
                        child: Some(Box::new(backdrop)),
                        ..Default::default()
                    }
                    .into_node(),
                    positioned_popup,
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .flex_grow(1.0)
        .into_node();

        let portal_root = Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(Box::new(overlay)),
            ..Default::default()
        }
        .into_node();

        ctx.register_portal_with_layer(
            PortalLayer::Flyout,
            Some(WidgetNodeId::explicit("completion_popup")),
            portal_root,
        );

        Spacer {
            height: Some(0.0),
            ..Default::default()
        }
        .into_node()
    }
}
