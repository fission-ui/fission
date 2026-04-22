use crate::model::*;
use fission_core::op::Color;
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, GestureDetector, Node, Positioned, Text, ZStack};
use fission_core::{ActionEnvelope, BuildCtx, Handler, PortalLayer, View, Widget, WidgetNodeId};
use fission_widgets::{VStack, Spacer};

pub struct ContextMenu;

impl Widget<EditorState> for ContextMenu {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        if !view.state.context_menu_visible {
            return Spacer { height: Some(0.0), ..Default::default() }.into_node();
        }

        let bg = Color { r: 45, g: 45, b: 46, a: 255 };
        let border = Color { r: 69, g: 69, b: 69, a: 255 };
        let text_color = Color { r: 204, g: 204, b: 204, a: 255 };
        let dim = Color { r: 140, g: 140, b: 140, a: 255 };

        let dismiss = ctx.bind(
            DismissContextMenu,
            (|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
                s.context_menu_target = None;
            }) as Handler<EditorState, DismissContextMenu>,
        );

        let menu_item = |label: &str, shortcut: &str, action: ActionEnvelope| -> Node {
            Button {
                variant: ButtonVariant::Ghost,
                content_align: ButtonContentAlign::Start,
                child: Some(Box::new(
                    Container::new(
                        fission_widgets::HStack {
                            spacing: Some(0.0),
                            children: vec![
                                Text::new(label).size(12.0).color(text_color).flex_grow(1.0).into_node(),
                                Text::new(shortcut).size(11.0).color(dim).into_node(),
                            ],
                        }.into_node(),
                    ).width(200.0).into_node(),
                )),
                on_press: Some(action),
                height: Some(26.0),
                padding: Some([4.0, 8.0, 0.0, 0.0]),
                ..Default::default()
            }.into_node()
        };

        let separator = || -> Node {
            Container::new(Spacer::default().into_node())
                .height(1.0)
                .bg(border)
                .into_node()
        };

        let mut items = Vec::new();

        if let Some(target) = &view.state.context_menu_target {
            // File tree context menu
            let target_clone = target.clone();

            let new_file = ctx.bind(
                CreateFile(String::new()),
                (|s: &mut EditorState, _, _| {
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
                }) as Handler<EditorState, CreateFile>,
            );

            let new_folder = ctx.bind(
                CreateFolder(String::new()),
                (|s: &mut EditorState, _, _| {
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
                }) as Handler<EditorState, CreateFolder>,
            );

            let delete = ctx.bind(
                DeleteFile(String::new()),
                (|s: &mut EditorState, _, _| {
                    if let Some(target) = s.context_menu_target.take() {
                        s.delete_file(target);
                    }
                    s.context_menu_visible = false;
                }) as Handler<EditorState, DeleteFile>,
            );

            items.push(menu_item("New File", "", new_file));
            items.push(menu_item("New Folder", "", new_folder));
            items.push(separator());
            items.push(menu_item("Delete", "Del", delete));
        } else {
            // Editor context menu
            let undo_ctx = ctx.bind(
                Undo,
                (|s: &mut EditorState, _, _| {
                    s.undo_active();
                    s.context_menu_visible = false;
                }) as Handler<EditorState, Undo>,
            );

            let redo_ctx = ctx.bind(
                Redo,
                (|s: &mut EditorState, _, _| {
                    s.redo_active();
                    s.context_menu_visible = false;
                }) as Handler<EditorState, Redo>,
            );

            let copy_ctx = ctx.bind(
                CopySelection,
                (|s: &mut EditorState, _, _| {
                    s.copy_line();
                    s.context_menu_visible = false;
                }) as Handler<EditorState, CopySelection>,
            );

            let cut_ctx = ctx.bind(
                CutSelection,
                (|s: &mut EditorState, _, _| {
                    s.cut_line();
                    s.context_menu_visible = false;
                }) as Handler<EditorState, CutSelection>,
            );

            let paste_ctx = ctx.bind(
                PasteClipboard,
                (|s: &mut EditorState, _, _| {
                    s.paste();
                    s.context_menu_visible = false;
                }) as Handler<EditorState, PasteClipboard>,
            );

            let find = ctx.bind(
                ToggleFindReplace,
                (|s: &mut EditorState, _, _| {
                    s.show_find_replace = true;
                    s.context_menu_visible = false;
                }) as Handler<EditorState, ToggleFindReplace>,
            );

            let go_to_def = ctx.bind(
                GoToDefinition,
                (|s: &mut EditorState, _, _| {
                    s.status_message = Some("Go to Definition: not yet connected to LSP".into());
                    s.context_menu_visible = false;
                }) as Handler<EditorState, GoToDefinition>,
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
            VStack { spacing: Some(0.0), children: items }.into_node(),
        )
        .bg(bg)
        .border(border, 1.0)
        .border_radius(4.0)
        .padding_all(4.0)
        .into_node();

        let (x, y) = view.state.context_menu_position;

        // Backdrop to dismiss
        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child: Box::new(
                Container::new(Spacer::default().into_node())
                    .bg(Color { r: 0, g: 0, b: 0, a: 1 }) // Nearly transparent
                    .flex_grow(1.0)
                    .into_node(),
            ),
            ..Default::default()
        }.into_node();

        let overlay = Container::new(
            ZStack {
                children: vec![
                    Positioned {
                        left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
                        child: Some(Box::new(backdrop)),
                        ..Default::default()
                    }.into_node(),
                    Positioned {
                        left: Some(x),
                        top: Some(y),
                        child: Some(Box::new(menu_card)),
                        ..Default::default()
                    }.into_node(),
                ],
                ..Default::default()
            }.into_node(),
        )
        .flex_grow(1.0)
        .into_node();

        let positioned_root = Positioned {
            left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
            child: Some(Box::new(overlay)),
            ..Default::default()
        }.into_node();

        ctx.register_portal_with_layer(PortalLayer::Flyout, Some(WidgetNodeId::explicit("context_menu")), positioned_root);

        Spacer { height: Some(0.0), ..Default::default() }.into_node()
    }
}
