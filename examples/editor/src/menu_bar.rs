use crate::model::*;
use fission_core::op::Color;
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, GestureDetector, Node, Positioned, Text, ZStack};
use fission_core::{ActionEnvelope, BuildCtx, Handler, PortalLayer, View, Widget, WidgetNodeId};
use fission_widgets::{HStack, VStack, Spacer};

pub struct MenuBar;

impl Widget<EditorState> for MenuBar {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let bg = Color { r: 51, g: 51, b: 51, a: 255 };
        let text_color = Color { r: 204, g: 204, b: 204, a: 255 };
        let active_bg = Color { r: 70, g: 70, b: 70, a: 255 };

        let menus = vec!["File", "Edit", "View", "Go", "Help"];

        let set_menu_id = ctx.bind(
            SetActiveMenu(None),
            (|s: &mut EditorState, a: SetActiveMenu, _| {
                if s.active_menu.as_deref() == a.0.as_deref() {
                    s.active_menu = None;
                } else {
                    s.active_menu = a.0;
                }
            }) as Handler<EditorState, SetActiveMenu>,
        ).id;

        let mut menu_buttons = Vec::new();
        for menu_name in &menus {
            let is_active = view.state.active_menu.as_deref() == Some(menu_name);
            let item_bg = if is_active { active_bg } else { bg };

            menu_buttons.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(Box::new(
                        Container::new(
                            Text::new(*menu_name).size(12.0).color(text_color).into_node(),
                        ).bg(item_bg).padding_all(6.0).into_node(),
                    )),
                    on_press: Some(ActionEnvelope {
                        id: set_menu_id,
                        payload: serde_json::to_vec(&SetActiveMenu(Some(menu_name.to_string()))).unwrap(),
                    }),
                    height: Some(28.0),
                    padding: Some([0.0; 4]),
                    ..Default::default()
                }.into_node(),
            );
        }

        menu_buttons.push(Spacer { flex_grow: 1.0, ..Default::default() }.into_node());

        let bar = Container::new(
            HStack {
                spacing: Some(0.0),
                children: menu_buttons,
            }.into_node(),
        )
        .bg(bg)
        .height(28.0)
        .flex_shrink(0.0)
        .into_node();

        // If a menu is active, render the dropdown as a portal
        if let Some(active) = &view.state.active_menu {
            self.render_dropdown(ctx, view, active);
        }

        bar
    }
}

impl MenuBar {
    fn render_dropdown(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>, menu: &str) {
        let bg = Color { r: 45, g: 45, b: 46, a: 255 };
        let border = Color { r: 69, g: 69, b: 69, a: 255 };
        let text_color = Color { r: 204, g: 204, b: 204, a: 255 };
        let dim = Color { r: 140, g: 140, b: 140, a: 255 };

        let dismiss = ctx.bind(
            SetActiveMenu(None),
            (|s: &mut EditorState, _, _| s.active_menu = None)
                as Handler<EditorState, SetActiveMenu>,
        );

        let menu_item = |label: &str, shortcut: &str, action: ActionEnvelope| -> Node {
            Button {
                variant: ButtonVariant::Ghost,
                content_align: ButtonContentAlign::Start,
                child: Some(Box::new(
                    Container::new(
                        HStack {
                            spacing: Some(0.0),
                            children: vec![
                                Text::new(label).size(12.0).color(text_color).flex_grow(1.0).into_node(),
                                Text::new(shortcut).size(11.0).color(dim).into_node(),
                            ],
                        }.into_node(),
                    ).width(220.0).into_node(),
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

        // Build actions
        let save = ctx.bind(SaveFile, (|s: &mut EditorState, _, _| { s.save_active_file(); s.active_menu = None; }) as Handler<EditorState, SaveFile>);
        let save_all = ctx.bind(SaveAllFiles, (|s: &mut EditorState, _, _| { s.save_all_files(); s.active_menu = None; }) as Handler<EditorState, SaveAllFiles>);
        let toggle_find = ctx.bind(ToggleFindReplace, (|s: &mut EditorState, _, _| { s.show_find_replace = !s.show_find_replace; s.active_menu = None; }) as Handler<EditorState, ToggleFindReplace>);
        let toggle_sidebar = ctx.bind(ToggleSidebar, (|s: &mut EditorState, _, _| { s.sidebar_visible = !s.sidebar_visible; s.active_menu = None; }) as Handler<EditorState, ToggleSidebar>);
        let toggle_terminal = ctx.bind(ToggleTerminal, (|s: &mut EditorState, _, _| { s.terminal_visible = !s.terminal_visible; s.active_menu = None; }) as Handler<EditorState, ToggleTerminal>);
        let cmd_palette = ctx.bind(ToggleCommandPalette, (|s: &mut EditorState, _, _| { s.show_command_palette = true; s.active_menu = None; }) as Handler<EditorState, ToggleCommandPalette>);
        let close_tab_action = ctx.bind(CloseTab(0), (|s: &mut EditorState, _, _| { let idx = s.active_tab; s.close_tab(idx); s.active_menu = None; }) as Handler<EditorState, CloseTab>);

        let new_file = ctx.bind(CreateFile(String::new()), (|s: &mut EditorState, _, _| {
            let path = format!("{}/untitled.rs", s.root_path.to_string_lossy());
            s.create_file(path);
            s.active_menu = None;
        }) as Handler<EditorState, CreateFile>);

        let new_folder = ctx.bind(CreateFolder(String::new()), (|s: &mut EditorState, _, _| {
            let path = format!("{}/new_folder", s.root_path.to_string_lossy());
            s.create_folder(path);
            s.active_menu = None;
        }) as Handler<EditorState, CreateFolder>);

        let go_to_line = ctx.bind(GoToLine(0), (|s: &mut EditorState, _, _| {
            s.show_command_palette = true;
            s.command_query = "Go to Line: ".to_string();
            s.active_menu = None;
        }) as Handler<EditorState, GoToLine>);

        let go_to_def = ctx.bind(GoToDefinition, (|s: &mut EditorState, _, _| {
            s.status_message = Some("Go to Definition: LSP not connected".into());
            s.active_menu = None;
        }) as Handler<EditorState, GoToDefinition>);

        let about = ctx.bind(ShowMenuStatus("Fission Editor v0.1.0 — Built with Fission UI Framework".into()), (|s: &mut EditorState, a: ShowMenuStatus, _| {
            s.status_message = Some(a.0);
            s.active_menu = None;
        }) as Handler<EditorState, ShowMenuStatus>);

        let undo_action = ctx.bind(Undo, (|s: &mut EditorState, _, _| {
            s.undo_active();
            s.active_menu = None;
        }) as Handler<EditorState, Undo>);

        let redo_action = ctx.bind(Redo, (|s: &mut EditorState, _, _| {
            s.redo_active();
            s.active_menu = None;
        }) as Handler<EditorState, Redo>);

        let copy_action = ctx.bind(CopySelection, (|s: &mut EditorState, _, _| {
            s.copy_line();
            s.active_menu = None;
        }) as Handler<EditorState, CopySelection>);

        let cut_action = ctx.bind(CutSelection, (|s: &mut EditorState, _, _| {
            s.cut_line();
            s.active_menu = None;
        }) as Handler<EditorState, CutSelection>);

        let paste_action = ctx.bind(PasteClipboard, (|s: &mut EditorState, _, _| {
            s.paste();
            s.active_menu = None;
        }) as Handler<EditorState, PasteClipboard>);

        let items: Vec<Node> = match menu {
            "File" => vec![
                menu_item("New File", "Ctrl+N", new_file),
                menu_item("New Folder", "", new_folder),
                separator(),
                menu_item("Save", "Ctrl+S", save),
                menu_item("Save All", "Ctrl+Shift+S", save_all),
                separator(),
                menu_item("Close Tab", "Ctrl+W", close_tab_action),
            ],
            "Edit" => vec![
                menu_item("Undo", "Ctrl+Z", undo_action),
                menu_item("Redo", "Ctrl+Shift+Z", redo_action),
                separator(),
                menu_item("Cut", "Ctrl+X", cut_action),
                menu_item("Copy", "Ctrl+C", copy_action),
                menu_item("Paste", "Ctrl+V", paste_action),
                separator(),
                menu_item("Find / Replace", "Ctrl+F", toggle_find),
            ],
            "View" => vec![
                menu_item("Command Palette", "Ctrl+Shift+P", cmd_palette),
                separator(),
                menu_item("Toggle Sidebar", "Ctrl+B", toggle_sidebar),
                menu_item("Toggle Terminal", "Ctrl+`", toggle_terminal),
            ],
            "Go" => vec![
                menu_item("Go to Line...", "Ctrl+G", go_to_line),
                menu_item("Go to Definition", "F12", go_to_def),
            ],
            "Help" => vec![
                menu_item("About Fission Editor", "", about),
            ],
            _ => vec![],
        };

        // Position: offset from left based on menu name
        let x_offset = match menu {
            "File" => 0.0,
            "Edit" => 40.0,
            "View" => 80.0,
            "Go" => 120.0,
            "Help" => 150.0,
            _ => 0.0,
        };

        let dropdown = Container::new(
            VStack { spacing: Some(0.0), children: items }.into_node(),
        )
        .bg(bg)
        .border(border, 1.0)
        .border_radius(4.0)
        .padding_all(4.0)
        .into_node();

        // Backdrop to dismiss
        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child: Box::new(
                Container::new(Spacer::default().into_node())
                    .bg(Color { r: 0, g: 0, b: 0, a: 1 })
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
                        left: Some(x_offset),
                        top: Some(28.0), // Below menu bar
                        child: Some(Box::new(dropdown)),
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

        ctx.register_portal_with_layer(PortalLayer::Flyout, Some(WidgetNodeId::explicit("menu_dropdown")), positioned_root);
    }
}
