//! The todo list's state and the reducers that change it.

use fission::prelude::*;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, TodoState>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TodoItem {
    pub(crate) id: usize,
    pub(crate) title: String,
    pub(crate) done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoState {
    next_id: usize,
    pub(crate) draft: String,
    pub(crate) items: Vec<TodoItem>,
    pub(crate) theme_mode: DesignMode,
}

impl Default for TodoState {
    fn default() -> Self {
        let items = [
            ("Connect the DSP package", true),
            ("Use generated component styles", false),
            ("Ship the themed app", false),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (title, done))| TodoItem {
            id: index + 1,
            title: title.into(),
            done,
        })
        .collect::<Vec<_>>();
        Self {
            next_id: items.len() + 1,
            draft: String::new(),
            items,
            theme_mode: DesignMode::Light,
        }
    }
}

impl GlobalState for TodoState {}

impl TodoState {
    pub(crate) fn completed_count(&self) -> usize {
        self.items.iter().filter(|item| item.done).count()
    }

    /// Adds the draft as a task, without surrounding whitespace. A blank draft
    /// adds nothing.
    fn add_draft(&mut self) {
        let title = self.draft.trim();
        if title.is_empty() {
            return;
        }
        self.items.push(TodoItem {
            id: self.next_id,
            title: title.to_string(),
            done: false,
        });
        self.next_id += 1;
        self.draft.clear();
    }

    fn toggle(&mut self, id: usize) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            item.done = !item.done;
        }
    }

    fn clear_completed(&mut self) {
        self.items.retain(|item| !item.done);
    }
}

#[fission_action]
pub struct UpdateDraft;

#[fission_action]
pub struct AddTodo;

#[fission_action]
pub struct ToggleTodo(pub usize);

#[fission_action]
pub struct ClearCompleted;

#[fission_action]
pub struct SetThemeMode(pub DesignMode);

pub(crate) fn update_draft(state: &mut TodoState, _: UpdateDraft, cx: &mut Cx<'_, '_, '_>) {
    if let Some(change) = cx.input.text_change() {
        state.draft = change.new_text.clone();
    }
}

pub(crate) fn add_todo(state: &mut TodoState, _: AddTodo, _: &mut Cx<'_, '_, '_>) {
    state.add_draft();
}

pub(crate) fn toggle_todo(state: &mut TodoState, action: ToggleTodo, _: &mut Cx<'_, '_, '_>) {
    state.toggle(action.0);
}

pub(crate) fn clear_completed(state: &mut TodoState, _: ClearCompleted, _: &mut Cx<'_, '_, '_>) {
    state.clear_completed();
}

pub(crate) fn set_theme_mode(state: &mut TodoState, action: SetThemeMode, _: &mut Cx<'_, '_, '_>) {
    state.theme_mode = action.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_trims_the_draft_and_clears_it() {
        let mut state = TodoState::default();
        let before = state.items.len();
        state.draft = "  Write docs  ".into();
        state.add_draft();
        assert_eq!(state.items.len(), before + 1);
        assert_eq!(
            state.items.last().map(|item| item.title.as_str()),
            Some("Write docs")
        );
        assert!(state.draft.is_empty());
    }

    #[test]
    fn a_blank_draft_adds_nothing() {
        let mut state = TodoState::default();
        let before = state.items.clone();
        state.draft = "   ".into();
        state.add_draft();
        assert_eq!(state.items, before);
    }

    #[test]
    fn new_tasks_get_ids_no_existing_task_uses() {
        let mut state = TodoState {
            draft: "One more".into(),
            ..TodoState::default()
        };
        state.add_draft();
        let mut ids: Vec<usize> = state.items.iter().map(|item| item.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), state.items.len());
    }

    #[test]
    fn toggling_then_clearing_removes_only_completed_tasks() {
        let mut state = TodoState::default();
        let second = state.items[1].id;
        state.toggle(second);
        state.clear_completed();
        assert_eq!(state.completed_count(), 0);
        assert_eq!(state.items.len(), 1);
    }
}
