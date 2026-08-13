use fission::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextLabState {
    pub single_line: String,
    pub multiline: String,
    pub inline_combobox: String,
    pub modal_to: String,
    pub modal_subject: String,
    pub modal_body: String,
    pub show_modal: bool,
    pub menu_open: bool,
    pub status: String,
}

impl GlobalState for TextLabState {}

#[fission_reducer(SetSingleLine)]
pub(crate) fn set_single_line(state: &mut TextLabState, ctx: &mut ReducerContext<TextLabState>) {
    if let Some(change) = ctx.input.text_change() {
        state.single_line = change.new_text.clone();
    }
}

#[fission_reducer(SetMultiline)]
pub(crate) fn set_multiline(state: &mut TextLabState, ctx: &mut ReducerContext<TextLabState>) {
    if let Some(change) = ctx.input.text_change() {
        state.multiline = change.new_text.clone();
    }
}

#[fission_reducer(SetInlineCombobox)]
pub(crate) fn set_inline_combobox(
    state: &mut TextLabState,
    value: String,
    ctx: &mut ReducerContext<TextLabState>,
) {
    state.inline_combobox = ctx
        .input
        .text_change()
        .map(|change| change.new_text.clone())
        .unwrap_or(value);
}

#[fission_reducer(SetModalTo)]
pub(crate) fn set_modal_to(
    state: &mut TextLabState,
    value: String,
    ctx: &mut ReducerContext<TextLabState>,
) {
    state.modal_to = ctx
        .input
        .text_change()
        .map(|change| change.new_text.clone())
        .unwrap_or(value);
}

#[fission_reducer(SetModalSubject)]
pub(crate) fn set_modal_subject(state: &mut TextLabState, ctx: &mut ReducerContext<TextLabState>) {
    if let Some(change) = ctx.input.text_change() {
        state.modal_subject = change.new_text.clone();
    }
}

#[fission_reducer(SetModalBody)]
pub(crate) fn set_modal_body(state: &mut TextLabState, ctx: &mut ReducerContext<TextLabState>) {
    if let Some(change) = ctx.input.text_change() {
        state.modal_body = change.new_text.clone();
    }
}

#[fission_reducer(SetShowModal)]
pub(crate) fn set_show_modal(state: &mut TextLabState, value: bool) {
    state.show_modal = value;
}

#[fission_reducer(SetMenuOpen)]
pub(crate) fn set_menu_open(state: &mut TextLabState, value: bool) {
    state.menu_open = value;
}

#[fission_reducer(MenuPicked)]
pub(crate) fn menu_picked(state: &mut TextLabState, value: String) {
    state.status = format!("Menu action: {value}");
    state.menu_open = false;
}

#[fission_reducer(ApplyModal)]
pub(crate) fn apply_modal(state: &mut TextLabState) {
    state.status = format!(
        "Modal applied: to='{}' subject='{}' body_len={}",
        state.modal_to,
        state.modal_subject,
        state.modal_body.len()
    );
    state.show_modal = false;
}

pub(crate) fn filtered_suggestions(query: &str, values: &[&str]) -> Vec<String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    values
        .iter()
        .filter(|value| value.to_lowercase().contains(&query))
        .map(|value| (*value).to_string())
        .collect()
}
