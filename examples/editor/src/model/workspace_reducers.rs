//! Reducers for the workspace chrome: tabs, the activity bar, search, git,
//! completions, the bottom panel, the document itself, hover and find/replace.

use super::EditorState;

impl EditorState {
    /// Inserts the chosen completion at the caret and closes the popup.
    pub(crate) fn select_completion(&mut self, index: usize) {
        let label = self.completions.get(index).map(|item| item.label.clone());
        if let Some(label) = label {
            if let Some((_tab, buf)) = self.active_buffer_mut() {
                let (caret, _anchor) = buf.current_offsets();
                buf.apply_edit(caret..caret, &label);
                let next = caret + label.len();
                buf.set_selection_offsets(next, next);
            }
            self.mark_active_tab_dirty();
            if let Some(tab) = self.open_tabs.get(self.active_tab) {
                let path = tab.path.clone();
                self.notify_buffer_changed(&path);
            }
        }
        self.close_completions();
    }
}

impl EditorState {
    pub(crate) fn close_completions(&mut self) {
        self.show_completions = false;
        self.completions.clear();
        self.selected_completion = 0;
    }
}
