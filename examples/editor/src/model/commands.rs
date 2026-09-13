//! Commands offered by the menu bar, the command palette and the context menus.
//!
//! Each surface lists [`EditorCommand`] values. Pressing an item dispatches
//! [`RunCommand`], and [`EditorState::run_command`] performs it and closes
//! whichever surface offered it, so every surface behaves the same way.

use super::{BottomPanelTab, EditorState, FileEntry, FsOp, SidebarSection};
use fission::core::{ActionInput, ReducerContext};
use fission::prelude::fission_action;
use serde::{Deserialize, Serialize};
use std::path::Path;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, EditorState>;

/// Where a context menu opens when the event that asked for it had no pointer
/// position, such as a keyboard shortcut.
const CONTEXT_MENU_FALLBACK_POSITION: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditorCommand {
    NewFile,
    NewFolder,
    Save,
    SaveAll,
    CloseTab,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    FindReplace,
    ToggleSidebar,
    ToggleTerminal,
    CommandPalette,
    ShowExplorer,
    ShowSearch,
    ShowSourceControl,
    RefreshGitStatus,
    GoToLine,
    GoToDefinition,
    About,
    Rename,
    Delete,
}

use EditorCommand::*;

/// The menu bar's menus, in order, with the commands each one lists.
pub const MENUS: [(&str, &[EditorCommand]); 5] = [
    ("File", &[NewFile, NewFolder, Save, SaveAll, CloseTab]),
    ("Edit", &[Undo, Redo, Cut, Copy, Paste, FindReplace]),
    ("View", &[ToggleSidebar, ToggleTerminal, CommandPalette]),
    ("Go", &[GoToLine, GoToDefinition]),
    ("Help", &[About]),
];

pub const PALETTE_COMMANDS: &[EditorCommand] = &[
    Save,
    SaveAll,
    ToggleSidebar,
    ToggleTerminal,
    ShowExplorer,
    ShowSearch,
    ShowSourceControl,
    RefreshGitStatus,
];

/// Offered when the context menu was opened on a file tree entry.
pub const TREE_CONTEXT_COMMANDS: &[EditorCommand] = &[NewFile, NewFolder, Rename, Delete];

/// Offered when the context menu was opened in the editor.
pub const EDITOR_CONTEXT_COMMANDS: &[EditorCommand] =
    &[Undo, Redo, Copy, Cut, Paste, FindReplace, GoToDefinition];

impl EditorCommand {
    pub fn label(self) -> &'static str {
        match self {
            NewFile => "New File",
            NewFolder => "New Folder",
            Save => "Save",
            SaveAll => "Save All",
            CloseTab => "Close Tab",
            Undo => "Undo",
            Redo => "Redo",
            Cut => "Cut",
            Copy => "Copy",
            Paste => "Paste",
            FindReplace => "Find/Replace",
            ToggleSidebar => "Toggle Sidebar",
            ToggleTerminal => "Toggle Terminal",
            CommandPalette => "Command Palette",
            ShowExplorer => "Show Explorer",
            ShowSearch => "Show Search",
            ShowSourceControl => "Show Source Control",
            RefreshGitStatus => "Refresh Git Status",
            GoToLine => "Go to Line",
            GoToDefinition => "Go to Definition",
            About => "About",
            Rename => "Rename",
            Delete => "Delete",
        }
    }

    /// A short explanation shown beside the command in the palette.
    pub fn description(self) -> &'static str {
        match self {
            NewFile => "Create a file",
            NewFolder => "Create a folder",
            Save => "Save the active file",
            SaveAll => "Save all open files",
            CloseTab => "Close the active tab",
            Undo => "Undo the last edit",
            Redo => "Redo the last undone edit",
            Cut => "Cut the current line",
            Copy => "Copy the current line",
            Paste => "Paste from the clipboard",
            FindReplace => "Find and replace in the file",
            ToggleSidebar => "Show or hide the side bar",
            ToggleTerminal => "Show or hide the terminal panel",
            CommandPalette => "Search for a command",
            ShowExplorer => "Open the file explorer",
            ShowSearch => "Open the search panel",
            ShowSourceControl => "Open the git panel",
            RefreshGitStatus => "Fetch latest git status",
            GoToLine => "Jump to a line number",
            GoToDefinition => "Jump to where a symbol is defined",
            About => "Show the editor version",
            Rename => "Rename the selected entry",
            Delete => "Delete the selected entry",
        }
    }

    /// Whether the palette should list this command for `query`, which must
    /// already be lowercase. An empty query matches everything.
    pub fn matches(self, query: &str) -> bool {
        query.is_empty()
            || self.label().to_lowercase().contains(query)
            || self.description().to_lowercase().contains(query)
    }
}

impl EditorState {
    /// Performs `command` and closes the menu, palette or context menu that
    /// offered it. File tree commands act on the context menu's target.
    pub fn run_command(&mut self, command: EditorCommand) {
        let target = self.context_menu_target.take();
        let palette_was_open = self.show_command_palette;
        self.active_menu = None;
        self.context_menu_visible = false;
        self.show_command_palette = false;

        match command {
            NewFile => {
                let dir = self.directory_for(target.as_deref());
                self.create_file(format!("{}/untitled", dir));
            }
            NewFolder => {
                let dir = self.directory_for(target.as_deref());
                self.create_folder(format!("{}/new_folder", dir));
            }
            Save => self.save_active_file(),
            SaveAll => self.save_all_files(),
            CloseTab => {
                let index = self.active_tab;
                self.close_tab(index);
            }
            Undo => self.undo_active(),
            Redo => self.redo_active(),
            Cut => self.cut_line(),
            Copy => self.copy_line(),
            Paste => self.paste(),
            FindReplace => self.show_find_replace = true,
            ToggleSidebar => self.sidebar_visible = !self.sidebar_visible,
            ToggleTerminal => {
                self.terminal_visible = !self.terminal_visible;
                if self.terminal_visible {
                    self.bottom_panel_tab = BottomPanelTab::Terminal;
                    self.ensure_terminal_session();
                }
            }
            CommandPalette => self.show_command_palette = !palette_was_open,
            ShowExplorer => self.show_sidebar_section(SidebarSection::Explorer),
            ShowSearch => self.show_sidebar_section(SidebarSection::Search),
            ShowSourceControl => self.show_sidebar_section(SidebarSection::Git),
            RefreshGitStatus => self.refresh_git_status(),
            GoToLine => {
                self.show_command_palette = true;
                self.command_query = "Go to Line:".into();
            }
            GoToDefinition => {
                self.status_message = Some("Go to Definition: LSP not connected".into());
            }
            About => {
                self.status_message =
                    Some(concat!("Fission Editor v", env!("CARGO_PKG_VERSION")).into());
            }
            Rename => match target {
                Some(path) => self.start_rename(path),
                None => self.status_message = Some("Nothing selected to rename".into()),
            },
            Delete => match target {
                Some(path) => {
                    self.queue_fs(FsOp::Delete { path });
                }
                None => self.status_message = Some("Nothing selected to delete".into()),
            },
        }

        if palette_was_open && !self.show_command_palette {
            self.command_query.clear();
        }
    }

    fn show_sidebar_section(&mut self, section: SidebarSection) {
        self.sidebar_section = section;
        self.sidebar_visible = true;
    }

    /// The folder a new entry goes in: the target itself when it is a folder,
    /// the folder containing it otherwise, and the workspace root without one.
    fn directory_for(&self, target: Option<&str>) -> String {
        let root = self.root_path.to_string_lossy().to_string();
        let Some(target) = target else {
            return root;
        };
        if find_entry(&self.cached_tree_entries, target).is_some_and(|entry| entry.is_dir) {
            return target.to_string();
        }
        Path::new(target)
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or(root)
    }
}

fn find_entry<'a>(entries: &'a [FileEntry], path: &str) -> Option<&'a FileEntry> {
    entries.iter().find_map(|entry| {
        if entry.path == path {
            Some(entry)
        } else {
            find_entry(&entry.children, path)
        }
    })
}

#[fission_action]
pub struct RunCommand(pub EditorCommand);

#[fission_action]
pub struct DismissCommandPalette;

pub fn on_run_command(state: &mut EditorState, action: RunCommand, _: &mut Cx<'_, '_, '_>) {
    state.run_command(action.0);
}

/// Opens the named menu, or closes it when it is already open.
pub fn on_toggle_menu(
    state: &mut EditorState,
    action: super::SetActiveMenu,
    _: &mut Cx<'_, '_, '_>,
) {
    state.active_menu = if state.active_menu == action.0 {
        None
    } else {
        action.0
    };
}

pub fn on_dismiss_menu(state: &mut EditorState, _: super::DismissMenu, _: &mut Cx<'_, '_, '_>) {
    state.active_menu = None;
}

pub fn on_dismiss_context_menu(
    state: &mut EditorState,
    _: super::DismissContextMenu,
    _: &mut Cx<'_, '_, '_>,
) {
    state.context_menu_visible = false;
    state.context_menu_target = None;
}

pub fn on_dismiss_command_palette(
    state: &mut EditorState,
    _: DismissCommandPalette,
    _: &mut Cx<'_, '_, '_>,
) {
    state.show_command_palette = false;
    state.command_query.clear();
}

pub fn on_update_command_query(
    state: &mut EditorState,
    _: super::UpdateCommandQuery,
    cx: &mut Cx<'_, '_, '_>,
) {
    if let Some(change) = cx.input.text_change() {
        state.command_query = change.new_text.clone();
    }
}

/// Opens the context menu at the pointer, for the tree entry in `target` or
/// for the editor when there is none.
pub fn on_show_context_menu(
    state: &mut EditorState,
    action: super::ShowContextMenu,
    cx: &mut Cx<'_, '_, '_>,
) {
    let (x, y) = match cx.input {
        ActionInput::Pointer { x, y, .. } => (*x, *y),
        _ => (action.x, action.y),
    };
    let or_fallback = |value: f32| {
        if value < 10.0 {
            CONTEXT_MENU_FALLBACK_POSITION
        } else {
            value
        }
    };
    state.context_menu_visible = true;
    state.context_menu_position = (or_fallback(x), or_fallback(y));
    state.context_menu_target = action.target;
}

pub fn on_toggle_tree_node(
    state: &mut EditorState,
    action: super::ToggleTreeNode,
    _: &mut Cx<'_, '_, '_>,
) {
    if !state.tree_expanded.remove(&action.0) {
        state.tree_expanded.insert(action.0);
    }
}

pub fn on_open_file(state: &mut EditorState, action: super::OpenFile, _: &mut Cx<'_, '_, '_>) {
    state.open_file(action.0);
}

pub fn on_create_file(state: &mut EditorState, action: super::CreateFile, _: &mut Cx<'_, '_, '_>) {
    state.create_file(action.0);
}

pub fn on_create_folder(
    state: &mut EditorState,
    action: super::CreateFolder,
    _: &mut Cx<'_, '_, '_>,
) {
    state.create_folder(action.0);
}

/// Collapses the tree and rescans it from disk.
pub fn on_refresh_tree(state: &mut EditorState, _: super::RefreshTree, _: &mut Cx<'_, '_, '_>) {
    state.tree_expanded.clear();
    state.request_tree_refresh();
}

pub fn on_update_rename_input(
    state: &mut EditorState,
    _: super::UpdateRenameInput,
    cx: &mut Cx<'_, '_, '_>,
) {
    if let Some(change) = cx.input.text_change() {
        state.rename_input = change.new_text.clone();
    }
}
