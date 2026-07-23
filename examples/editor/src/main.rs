use fission::prelude::DesktopApp;
use std::path::PathBuf;

mod activity_bar;
mod breadcrumb;
mod command_palette;
mod command_palette_item;
mod completion_popup;
mod completion_popup_item;
mod context_menu;
mod diagnostic_item;
mod diagnostics_panel;
mod editor_app;
mod editor_menu_button;
mod editor_menu_item;
mod editor_shortcut_row;
mod editor_surface;
mod editor_tab;
mod editor_welcome_screen;
mod file_tree;
mod file_tree_entry;
mod find_replace_bar;
mod git_panel;
mod git_status_item;
mod hover_tooltip;
mod layout;
mod lsp;
mod menu_bar;
mod minimap;
mod model;
mod palette;
mod plugin;
mod search_panel;
mod search_result_item;
mod status_bar;
mod syntax;
mod tab_bar;
mod terminal_panel;
mod terminal_panel_tab;

use editor_app::EditorApp;
use model::*;

fn main() -> anyhow::Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let root_for_startup = root.clone();
    let app = DesktopApp::<EditorState, _>::new(EditorApp)
        .with_title("Fission Editor")
        .with_state_init(move |state: &mut EditorState| {
            state.root_path = root_for_startup.clone();
            state.request_tree_refresh();
            state.refresh_git_status();
            state.ensure_terminal_session();
            if std::env::var("FISSION_TEST_CONTROL_PORT").is_err() && state.lsp_handle.is_none() {
                state.lsp_handle = Some(LspHandle::new(&state.root_path));
            }
        })
        .with_async(|asyncs| {
            asyncs.register_job(TREE_SCAN_JOB, |request: TreeScanRequest, _| async move {
                run_tree_scan(request)
            });
            asyncs.register_job(GIT_STATUS_JOB, |request: GitStatusRequest, _| async move {
                run_git_status(request)
            });
        })
        .with_sync_env(move |_state: &EditorState, env: &mut fission::core::Env| {
            env.theme = fission::theme::Theme::dark();
        })
        .with_key_handler(
            move |state: &mut EditorState, key: &fission::core::KeyCode, mods: u8| -> bool {
                // Async resources handle background scanning and polling.

                let ctrl = (mods & fission::core::event::MOD_CTRL) != 0
                    || (mods & fission::core::event::MOD_SUPER) != 0;
                let shift = (mods & fission::core::event::MOD_SHIFT) != 0;

                // Dismiss context menu on any keystroke (except Escape which handles it explicitly)
                if !matches!(key, fission::core::KeyCode::Escape) {
                    state.context_menu_visible = false;
                }

                // Enter confirms rename if one is in progress
                if matches!(key, fission::core::KeyCode::Enter) && !ctrl {
                    if state.renaming_path.is_some() {
                        state.confirm_rename();
                        return true;
                    }
                    return false;
                }

                if state.renaming_path.is_some() && !ctrl {
                    let should_replace_rename_text = state
                        .renaming_path
                        .as_ref()
                        .and_then(|path| std::path::Path::new(path).file_name())
                        .and_then(|value| value.to_str())
                        .map(|name| state.rename_input == name)
                        .unwrap_or(false);
                    match key {
                        fission::core::KeyCode::Backspace => {
                            if should_replace_rename_text {
                                state.rename_input.clear();
                            } else {
                                state.rename_input.pop();
                            }
                            return true;
                        }
                        fission::core::KeyCode::Space => {
                            if should_replace_rename_text {
                                state.rename_input.clear();
                            }
                            state.rename_input.push(' ');
                            return true;
                        }
                        fission::core::KeyCode::Char(ch) => {
                            if should_replace_rename_text {
                                state.rename_input.clear();
                            }
                            state.rename_input.push(*ch);
                            return true;
                        }
                        _ => {}
                    }
                }

                // Escape dismisses menus / context menus / find bar / command palette / rename
                if matches!(key, fission::core::KeyCode::Escape) {
                    let mut handled = false;
                    if state.renaming_path.is_some() {
                        state.cancel_rename();
                        handled = true;
                    }
                    if state.active_menu.is_some() {
                        state.active_menu = None;
                        handled = true;
                    }
                    if state.context_menu_visible {
                        state.context_menu_visible = false;
                        handled = true;
                    }
                    if state.show_find_replace {
                        state.show_find_replace = false;
                        handled = true;
                    }
                    if state.show_command_palette {
                        state.show_command_palette = false;
                        state.command_query.clear();
                        handled = true;
                    }
                    return handled;
                }

                if !ctrl {
                    return false;
                }

                match key {
                    fission::core::KeyCode::Char('s') | fission::core::KeyCode::Char('S') => {
                        if shift {
                            state.save_all_files();
                        } else {
                            state.save_active_file();
                        }
                        true
                    }
                    fission::core::KeyCode::Char('p') | fission::core::KeyCode::Char('P')
                        if shift =>
                    {
                        state.show_command_palette = !state.show_command_palette;
                        if !state.show_command_palette {
                            state.command_query.clear();
                        }
                        true
                    }
                    fission::core::KeyCode::Char('b') | fission::core::KeyCode::Char('B') => {
                        state.sidebar_visible = !state.sidebar_visible;
                        true
                    }
                    fission::core::KeyCode::Char('`') => {
                        state.terminal_visible = !state.terminal_visible;
                        if state.terminal_visible {
                            state.bottom_panel_tab = BottomPanelTab::Terminal;
                            state.ensure_terminal_session();
                        }
                        true
                    }
                    // Ctrl+F: toggle find/replace
                    fission::core::KeyCode::Char('f') | fission::core::KeyCode::Char('F') => {
                        state.context_menu_visible = false;
                        state.show_find_replace = !state.show_find_replace;
                        true
                    }
                    // Ctrl+G: go to line (toggle command palette with prompt)
                    fission::core::KeyCode::Char('g') | fission::core::KeyCode::Char('G') => {
                        state.show_command_palette = !state.show_command_palette;
                        if state.show_command_palette {
                            state.command_query = "Go to Line:".into();
                        } else {
                            state.command_query.clear();
                        }
                        true
                    }
                    // Ctrl+W: close active tab
                    fission::core::KeyCode::Char('w') | fission::core::KeyCode::Char('W') => {
                        let idx = state.active_tab;
                        state.close_tab(idx);
                        true
                    }
                    // Let the focused editor TextInput own text-history shortcuts so
                    // its retained editing buffer and the app model stay in sync.
                    fission::core::KeyCode::Char('z') | fission::core::KeyCode::Char('Z') => false,
                    fission::core::KeyCode::Char('y') | fission::core::KeyCode::Char('Y') => false,
                    fission::core::KeyCode::Char('c') | fission::core::KeyCode::Char('C') => false,
                    fission::core::KeyCode::Char('x') | fission::core::KeyCode::Char('X') => false,
                    fission::core::KeyCode::Char('v') | fission::core::KeyCode::Char('V') => false,
                    _ => false,
                }
            },
        );

    app.run()
}
