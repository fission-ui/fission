use crate::model::*;
use fission::core::op::Color;
use fission::core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, GestureDetector, Widget, Positioned, Text, ZStack};
use fission::core::{ActionEnvelope, BuildCtxHandle, reduce_with, PortalLayer, ViewHandle, WidgetId};
use fission::widgets::{VStack, Spacer};

pub struct ContextMenu;

impl From<ContextMenu> for Widget {
    fn from(component: ContextMenu) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        if !view.state().context_menu_visible {
            return Spacer { height: Some(0.0), ..Default::default() }.into();
        }

        let bg = Color { r: 45, g: 45, b: 46, a: 255 };
        let border = Color { r: 69, g: 69, b: 69, a: 255 };
        let text_color = Color { r: 204, g: 204, b: 204, a: 255 };
        let dim = Color { r: 140, g: 140, b: 140, a: 255 };

        let dismiss = ctx.bind(
            DismissContextMenu,
            reduce_with!((|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
                s.context_menu_target = None;
            })),
        );

        let menu_item = |label: &str, shortcut: &str, action: ActionEnvelope| -> Widget {
            Button {
                variant: ButtonVariant::Ghost,
                content_align: ButtonContentAlign::Start,
                child: Some(
                    Container::new(
                        fission::widgets::HStack {
                            spacing: Some(0.0),
                            children: vec![
                                Text::new(label).size(12.0).color(text_color).flex_grow(1.0).into(),
                                Text::new(shortcut).size(11.0).color(dim).into(),
                            ],
                        },
                    ).width(200.0).into(),
                ),
                on_press: Some(action),
                height: Some(26.0),
                padding: Some([4.0, 8.0, 0.0, 0.0]),
                ..Default::default()
            }.into()
        };

        let separator = || -> Widget {
            Container::new(Spacer::default())
                .height(1.0)
                .bg(border)
                .into()
        };

        let mut items = Vec::new();

        if let Some(target) = &view.state().context_menu_target {
            // File tree context menu
            let target_clone = target.clone();

            let new_file = ctx.bind(
                CreateFile(String::new()),
                reduce_with!((|s: &mut EditorState, _, _| {
                    if let Some(target) = &s.context_menu_target {
                        let dir = if std::path::Path::new(target).is_dir() {
                            target.clone()
                        } else {
                            std::path::Path::new(target).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
                        };
                        let path = format!("{}/untitled.rs", dir);
                        s.create_file(path);
                    }
                    s.context_menu_visible = false;
                    s.context_menu_target = None;
                })),
            );

            let new_folder = ctx.bind(
                CreateFolder(String::new()),
                reduce_with!((|s: &mut EditorState, _, _| {
                    if let Some(target) = &s.context_menu_target {
                        let dir = if std::path::Path::new(target).is_dir() {
                            target.clone()
                        } else {
                            std::path::Path::new(target).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
                        };
                        let path = format!("{}/new_folder", dir);
                        s.create_folder(path);
                    }
                    s.context_menu_visible = false;
                    s.context_menu_target = None;
                })),
            );

            let delete = ctx.bind(
                DeleteFile(String::new()),
                reduce_with!((|s: &mut EditorState, _, _| {
                    if let Some(target) = s.context_menu_target.take() {
                        s.delete_file(target);
                    }
                    s.context_menu_visible = false;
                })),
            );

            items.push(menu_item("New File", "", new_file));
            items.push(menu_item("New Folder", "", new_folder));
            items.push(separator());
            items.push(menu_item("Delete", "Del", delete));
        } else {
            // Editor context menu
            let undo_ctx = ctx.bind(
                Undo,
                reduce_with!((|s: &mut EditorState, _, _| {
                    s.undo_active();
                    s.context_menu_visible = false;
                })),
            );

            let redo_ctx = ctx.bind(
                Redo,
                reduce_with!((|s: &mut EditorState, _, _| {
                    s.redo_active();
                    s.context_menu_visible = false;
                })),
            );

            let copy_ctx = ctx.bind(
                CopySelection,
                reduce_with!((|s: &mut EditorState, _, _| {
                    s.copy_line();
                    s.context_menu_visible = false;
                })),
            );

            let cut_ctx = ctx.bind(
                CutSelection,
                reduce_with!((|s: &mut EditorState, _, _| {
                    s.cut_line();
                    s.context_menu_visible = false;
                })),
            );

            let paste_ctx = ctx.bind(
                PasteClipboard,
                reduce_with!((|s: &mut EditorState, _, _| {
                    s.paste();
                    s.context_menu_visible = false;
                })),
            );

            let find = ctx.bind(
                ToggleFindReplace,
                reduce_with!((|s: &mut EditorState, _, _| {
                    s.show_find_replace = true;
                    s.context_menu_visible = false;
                })),
            );

            let go_to_def = ctx.bind(
                GoToDefinition,
                reduce_with!((|s: &mut EditorState, _, _| {
                    s.status_message = Some("Go to Definition: not yet connected to LSP".into());
                    s.context_menu_visible = false;
                })),
            );

            items.push(menu_item("Undo", "Ctrl+Z", undo_ctx));
            items.push(menu_item("Redo", "Ctrl+Shift+Z", redo_ctx));
            items.push(separator());
            items.push(menu_item("Cut", "Ctrl+X", cut_ctx));
            items.push(menu_item("Copy", "Ctrl+C", copy_ctx));
            items.push(menu_item("Paste", "Ctrl+V", paste_ctx));
            items.push(separator());
            items.push(menu_item("Find / Replace", "Ctrl+F", find));
            items.push(separator());
            items.push(menu_item("Go to Definition", "F12", go_to_def));
        }

        let menu_card = Container::new(
            VStack { spacing: Some(0.0), children: items },
        )
        .bg(bg)
        .border(border, 1.0)
        .border_radius(4.0)
        .padding_all(4.0)
        .into();

        let (x, y) = view.state().context_menu_position;

        // Backdrop to dismiss
        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child:
                Container::new(Spacer::default())
                    .bg(Color { r: 0, g: 0, b: 0, a: 1 }) // Nearly transparent
                    .flex_grow(1.0)
                    .into(),
            ..Default::default()
        }.into();

        let overlay = Container::new(
            ZStack {
                children: vec![
                    Positioned {
                        left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
                        child: Some(backdrop),
                        ..Default::default()
                    }.into(),
                    Positioned {
                        left: Some(x),
                        top: Some(y),
                        child: Some(menu_card),
                        ..Default::default()
                    }.into(),
                ],
                ..Default::default()
            },
        )
        .flex_grow(1.0)
        .into();

        let positioned_root = Positioned {
            left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
            child: Some(overlay),
            ..Default::default()
        }.into();

        ctx.register_portal_with_layer(PortalLayer::Flyout, Some(WidgetId::explicit("context_menu")), positioned_root);

        Spacer { height: Some(0.0), ..Default::default() }.into()

    }
}
