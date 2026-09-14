//! Reducers for the workspace chrome: tabs, the activity bar, search, git,
//! completions, the bottom panel, the document itself, hover and find/replace.

use super::{
    BottomPanelTab, CloseTab, DismissCompletions, DismissHover, EditorState, ExecuteSearch,
    FindNext, FindPrevious, RefreshGitStatus, ReplaceAll, ReplaceOne, SelectCompletion, SelectTab,
    SetBottomPanelTab, SetSidebarSection, ToggleFindReplace, UpdateCursorPosition,
    UpdateEditorDocument, UpdateFindQuery, UpdateReplaceQuery, UpdateSearchQuery,
};
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, EditorState>;

pub fn on_select_tab(state: &mut EditorState, action: SelectTab, _: &mut Cx<'_, '_, '_>) {
    state.active_tab = action.0;
    state.update_breadcrumb();
}

pub fn on_close_tab(state: &mut EditorState, action: CloseTab, _: &mut Cx<'_, '_, '_>) {
    state.close_tab(action.0);
}

/// Shows a sidebar section, or hides the sidebar when that section is already
/// showing.
pub fn on_select_sidebar_section(
    state: &mut EditorState,
    action: SetSidebarSection,
    _: &mut Cx<'_, '_, '_>,
) {
    if state.sidebar_visible && state.sidebar_section == action.0 {
        state.sidebar_visible = false;
    } else {
        state.sidebar_section = action.0;
        state.sidebar_visible = true;
    }
}

pub fn on_update_search_query(
    state: &mut EditorState,
    _: UpdateSearchQuery,
    cx: &mut Cx<'_, '_, '_>,
) {
    if let Some(change) = cx.input.text_change() {
        state.search_query = change.new_text.clone();
    }
}

pub fn on_execute_search(state: &mut EditorState, _: ExecuteSearch, _: &mut Cx<'_, '_, '_>) {
    state.run_search();
}

pub fn on_refresh_git_status(state: &mut EditorState, _: RefreshGitStatus, _: &mut Cx<'_, '_, '_>) {
    state.refresh_git_status();
}

pub fn on_dismiss_completions(
    state: &mut EditorState,
    _: DismissCompletions,
    _: &mut Cx<'_, '_, '_>,
) {
    state.close_completions();
}

/// Inserts the chosen completion at the caret and closes the popup.
pub fn on_select_completion(
    state: &mut EditorState,
    action: SelectCompletion,
    _: &mut Cx<'_, '_, '_>,
) {
    let label = state
        .completions
        .get(action.0)
        .map(|item| item.label.clone());
    if let Some(label) = label {
        if let Some((_tab, buf)) = state.active_buffer_mut() {
            let (caret, _anchor) = buf.current_offsets();
            buf.apply_edit(caret..caret, &label);
            let next = caret + label.len();
            buf.set_selection_offsets(next, next);
        }
        state.mark_active_tab_dirty();
        if let Some(tab) = state.open_tabs.get(state.active_tab) {
            let path = tab.path.clone();
            state.notify_buffer_changed(&path);
        }
    }
    state.close_completions();
}

/// Switches the bottom panel, starting a terminal when it shows the terminal.
pub fn on_set_bottom_panel_tab(
    state: &mut EditorState,
    action: SetBottomPanelTab,
    _: &mut Cx<'_, '_, '_>,
) {
    state.bottom_panel_tab = action.0;
    if action.0 == BottomPanelTab::Terminal {
        state.ensure_terminal_session();
    }
}

/// Replaces the active document with the edited text, unless it is read-only.
pub fn on_update_editor_document(
    state: &mut EditorState,
    _: UpdateEditorDocument,
    cx: &mut Cx<'_, '_, '_>,
) {
    let Some(change) = cx.input.text_change() else {
        return;
    };
    if let Some(tab) = state.open_tabs.get(state.active_tab) {
        let path = tab.path.clone();
        if let Some(buf) = state.file_contents.get_mut(&path) {
            if !buf.is_editable() {
                state.status_message = Some("This document is not editable".into());
                return;
            }
            buf.replace_document(&change.new_text);
        }
        state.mark_active_tab_dirty();
        state.notify_buffer_changed(&path);
    }
}

pub fn on_update_cursor_position(
    state: &mut EditorState,
    action: UpdateCursorPosition,
    _: &mut Cx<'_, '_, '_>,
) {
    if let Some((_tab, buf)) = state.active_buffer_mut() {
        buf.clear_preedit();
        buf.set_selection_offsets(action.caret, action.anchor);
    }
}

pub fn on_dismiss_hover(state: &mut EditorState, _: DismissHover, _: &mut Cx<'_, '_, '_>) {
    state.show_hover = false;
    state.hover_info = None;
}

/// Updates the find query and jumps to the first match as the user types.
pub fn on_update_find_query(state: &mut EditorState, _: UpdateFindQuery, cx: &mut Cx<'_, '_, '_>) {
    let Some(change) = cx.input.text_change() else {
        return;
    };
    state.find_query = change.new_text.clone();
    state.find_next();
}

pub fn on_update_replace_query(
    state: &mut EditorState,
    _: UpdateReplaceQuery,
    cx: &mut Cx<'_, '_, '_>,
) {
    if let Some(change) = cx.input.text_change() {
        state.replace_query = change.new_text.clone();
    }
}

pub fn on_close_find_replace(
    state: &mut EditorState,
    _: ToggleFindReplace,
    _: &mut Cx<'_, '_, '_>,
) {
    state.show_find_replace = false;
}

pub fn on_find_next(state: &mut EditorState, _: FindNext, _: &mut Cx<'_, '_, '_>) {
    state.find_next();
}

pub fn on_find_previous(state: &mut EditorState, _: FindPrevious, _: &mut Cx<'_, '_, '_>) {
    state.find_previous();
}

pub fn on_replace_one(state: &mut EditorState, _: ReplaceOne, _: &mut Cx<'_, '_, '_>) {
    state.replace_one();
}

pub fn on_replace_all(state: &mut EditorState, _: ReplaceAll, _: &mut Cx<'_, '_, '_>) {
    state.replace_all();
}

impl EditorState {
    fn close_completions(&mut self) {
        self.show_completions = false;
        self.completions.clear();
        self.selected_completion = 0;
    }
}
