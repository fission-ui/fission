use crate::catalog::TargetFilter;
use fission::core::{OpenUrlRequest, ReducerContext, OPEN_URL};
use fission::i18n::Locale;
use fission::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum PreviewViewport {
    #[default]
    Desktop,
    Mobile,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShowcaseState {
    pub(crate) current_path: String,
    pub(crate) search: String,
    pub(crate) target_filter: TargetFilter,
    pub(crate) theme_mode: DesignMode,
    pub(crate) locale: Locale,
    pub(crate) preview_viewport: PreviewViewport,
    pub(crate) preview_generation: u64,
}

impl Default for ShowcaseState {
    fn default() -> Self {
        Self {
            current_path: "/".into(),
            search: String::new(),
            target_filter: TargetFilter::All,
            theme_mode: DesignMode::Light,
            locale: Locale::from("en-US"),
            preview_viewport: PreviewViewport::Desktop,
            preview_generation: 0,
        }
    }
}

impl GlobalState for ShowcaseState {}

#[fission_reducer(Navigate)]
pub(crate) fn on_navigate(state: &mut ShowcaseState, path: String) {
    state.current_path = path;
}

#[fission_reducer(SearchChanged)]
pub(crate) fn on_search_changed(state: &mut ShowcaseState, query: String) {
    state.search = query;
}

#[fission_reducer(FilterChanged)]
pub(crate) fn on_filter_changed(state: &mut ShowcaseState, filter: TargetFilter) {
    state.target_filter = filter;
}

#[fission_reducer(SetTheme)]
pub(crate) fn on_set_theme(state: &mut ShowcaseState, theme: DesignMode) {
    state.theme_mode = theme;
}

#[fission_reducer(SetLocale)]
pub(crate) fn on_set_locale(state: &mut ShowcaseState, locale: String) {
    state.locale = Locale::from(locale.as_str());
}

#[fission_reducer(SetPreviewViewport)]
pub(crate) fn on_set_preview_viewport(state: &mut ShowcaseState, viewport: PreviewViewport) {
    state.preview_viewport = viewport;
    state.preview_generation = state.preview_generation.wrapping_add(1);
}

#[fission_reducer(ResetPreview)]
pub(crate) fn on_reset_preview(state: &mut ShowcaseState) {
    state.preview_generation = state.preview_generation.wrapping_add(1);
}

#[fission_reducer(OpenSource)]
pub(crate) fn on_open_source(
    _state: &mut ShowcaseState,
    url: String,
    ctx: &mut ReducerContext<ShowcaseState>,
) {
    ctx.effects
        .capability(OPEN_URL, OpenUrlRequest { url, in_app: false });
}
