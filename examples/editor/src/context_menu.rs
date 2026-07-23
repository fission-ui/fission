use crate::editor_menu_item::EditorMenuItem;
use crate::layout::{
    CONTEXT_MENU_MAX_HEIGHT, CONTEXT_MENU_MAX_WIDTH, CONTEXT_MENU_MIN_WIDTH, OVERLAY_EDGE_GUTTER,
    OVERLAY_HORIZONTAL_RESERVE, OVERLAY_MIN_INSET,
};
use crate::model::*;
use crate::palette::{FLYOUT_BG, FLYOUT_BORDER, INTERACTION_BACKDROP};
use fission::core::ui::{Container, GestureDetector, Positioned, Widget, ZStack};
use fission::core::{reduce_with, PortalLayer, WidgetId};
use fission::widgets::{Spacer, VStack};

pub(crate) struct ContextMenu;

impl From<ContextMenu> for Widget {
    fn from(_component: ContextMenu) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        if !view.state().context_menu_visible {
            return Spacer {
                height: Some(tokens.spacing.none),
                ..Default::default()
            }
            .into();
        }

        let dismiss = ctx.bind(
            DismissContextMenu,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.context_menu_visible = false;
                })
            ),
        );

        let toggle_find = ctx.bind(
            ToggleFindReplace,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.show_find_replace = true;
                    s.context_menu_visible = false;
                })
            ),
        );

        let new_file_ctx = ctx.bind(
            CreateFile(String::new()),
            reduce_with!(
                (|s: &mut EditorState, _: CreateFile, _| {
                    s.context_menu_visible = false;
                    if let Some(target) = s.context_menu_target.clone() {
                        let dir = if std::path::Path::new(&target).is_dir() {
                            target
                        } else {
                            std::path::Path::new(&target)
                                .parent()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|| s.root_path.to_string_lossy().to_string())
                        };
                        s.create_file(format!("{}/untitled.rs", dir));
                    }
                    s.context_menu_target = None;
                })
            ),
        );

        let new_folder_ctx = ctx.bind(
            CreateFolder(String::new()),
            reduce_with!(
                (|s: &mut EditorState, _: CreateFolder, _| {
                    s.context_menu_visible = false;
                    if let Some(target) = s.context_menu_target.clone() {
                        let dir = if std::path::Path::new(&target).is_dir() {
                            target
                        } else {
                            std::path::Path::new(&target)
                                .parent()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|| s.root_path.to_string_lossy().to_string())
                        };
                        s.create_folder(format!("{}/new_folder", dir));
                    }
                    s.context_menu_target = None;
                })
            ),
        );

        let rename_action = ctx.bind(
            RenameContextTarget,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.context_menu_visible = false;
                    if let Some(target) = s.context_menu_target.clone() {
                        s.start_rename(target);
                    } else {
                        s.status_message = Some("Nothing selected to rename".into());
                    }
                })
            ),
        );

        let delete_action = ctx.bind(
            DeleteContextTarget,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.context_menu_visible = false;
                    if let Some(target) = s.context_menu_target.clone() {
                        let path = std::path::Path::new(&target);
                        let result = if path.is_dir() {
                            std::fs::remove_dir_all(&target)
                        } else {
                            std::fs::remove_file(&target)
                        };
                        match result {
                            Ok(()) => {
                                s.request_tree_refresh();
                                s.status_message = Some(format!("Deleted '{}'", target));
                            }
                            Err(e) => {
                                s.status_message = Some(format!("Delete failed: {}", e));
                            }
                        }
                    }
                })
            ),
        );

        let go_to_def = ctx.bind(
            GoToDefinition,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.context_menu_visible = false;
                    s.status_message = Some("Go to Definition (placeholder)".into());
                })
            ),
        );

        let ctx_undo = ctx.bind(
            Undo,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.undo_active();
                    s.context_menu_visible = false;
                })
            ),
        );

        let ctx_redo = ctx.bind(
            Redo,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.redo_active();
                    s.context_menu_visible = false;
                })
            ),
        );

        let ctx_copy = ctx.bind(
            CopySelection,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.copy_line();
                    s.context_menu_visible = false;
                })
            ),
        );

        let ctx_cut = ctx.bind(
            CutSelection,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.cut_line();
                    s.context_menu_visible = false;
                })
            ),
        );

        let ctx_paste = ctx.bind(
            PasteClipboard,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.paste();
                    s.context_menu_visible = false;
                })
            ),
        );

        let items: Vec<Widget> = if view.state().context_menu_target.is_some() {
            // File tree context menu
            vec![
                EditorMenuItem::new("New File", new_file_ctx.clone()).into(),
                EditorMenuItem::new("New Folder", new_folder_ctx.clone()).into(),
                EditorMenuItem::new("Rename", rename_action.clone()).into(),
                EditorMenuItem::new("Delete", delete_action.clone()).into(),
            ]
        } else {
            // Editor context menu
            vec![
                EditorMenuItem::new("Undo", ctx_undo.clone()).into(),
                EditorMenuItem::new("Redo", ctx_redo.clone()).into(),
                EditorMenuItem::new("Copy", ctx_copy.clone()).into(),
                EditorMenuItem::new("Cut", ctx_cut.clone()).into(),
                EditorMenuItem::new("Paste", ctx_paste.clone()).into(),
                EditorMenuItem::new("Find/Replace", toggle_find.clone()).into(),
                EditorMenuItem::new("Go to Definition", go_to_def.clone()).into(),
            ]
        };

        let (cx, cy) = view.state().context_menu_position;
        let viewport = view.viewport_size();
        let card_width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
            .clamp(CONTEXT_MENU_MIN_WIDTH, CONTEXT_MENU_MAX_WIDTH);
        let clamped_left =
            cx.min((viewport.width - card_width - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));
        let clamped_top =
            cy.min((viewport.height - CONTEXT_MENU_MAX_HEIGHT).max(OVERLAY_MIN_INSET));

        let card = Container::new(VStack {
            spacing: Some(tokens.spacing.none),
            children: items,
        })
        .width(card_width)
        .bg(FLYOUT_BG)
        .border(FLYOUT_BORDER, 1.0)
        .border_radius(view.env().theme.tokens.radii.small)
        .into();

        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child: Container::new(Spacer::default())
                .bg(INTERACTION_BACKDROP)
                .flex_grow(1.0)
                .into(),
            ..Default::default()
        }
        .into();

        let overlay = ZStack {
            children: vec![
                Positioned {
                    left: Some(tokens.spacing.none),
                    right: Some(tokens.spacing.none),
                    top: Some(tokens.spacing.none),
                    bottom: Some(tokens.spacing.none),
                    child: Some(backdrop),
                    ..Default::default()
                }
                .into(),
                Positioned {
                    left: Some(clamped_left),
                    top: Some(clamped_top),
                    child: Some(card),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into();

        let positioned_root = Positioned {
            left: Some(0.0),
            right: Some(0.0),
            top: Some(0.0),
            bottom: Some(0.0),
            child: Some(overlay),
            ..Default::default()
        }
        .into();

        ctx.register_portal_with_layer(
            PortalLayer::Flyout,
            Some(WidgetId::explicit("context_menu")),
            positioned_root,
        );

        Spacer {
            height: Some(0.0),
            ..Default::default()
        }
        .into()
    }
}
