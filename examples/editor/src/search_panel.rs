use crate::layout::{INPUT_HEIGHT, SEARCH_ACTION_HEIGHT, SEARCH_ACTION_WIDTH, SEARCH_RESULT_LIMIT};
use crate::model::{
    on_execute_search, on_update_search_query, EditorState, ExecuteSearch, UpdateSearchQuery,
};
use crate::palette::EditorPalette;
use crate::search_result_item::SearchResultItem;
use fission::prelude::*;

fission_component! {
    pub SearchPanel<EditorState> |_panel, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                flex_grow: 1.0,
                justify_content: ir_op::JustifyContent::Start,
                children: [
                    Container(
                        HStack(
                            spacing: tokens.spacing.none,
                            children: [
                                TextInput(
                                    id: WidgetId::explicit("editor_search_query_input"),
                                    value: state.search_query.clone(),
                                    placeholder: Some("Search...".into()),
                                    on_input: cx.bind(
                                        UpdateSearchQuery,
                                        reduce_with!(on_update_search_query),
                                    ),
                                    borderless: true,
                                ),
                                Button(
                                    variant: ButtonVariant::Ghost,
                                    child: Text("Go")
                                        .size(tokens.typography.font_size_xs)
                                        .color(palette.panel_text),
                                    on_press: cx.bind(
                                        ExecuteSearch,
                                        reduce_with!(on_execute_search),
                                    ),
                                    width: SEARCH_ACTION_WIDTH,
                                    height: SEARCH_ACTION_HEIGHT,
                                    padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
                                ),
                            ],
                        ),
                    )
                    .bg(palette.input_bg)
                    .border(palette.input_border, 1.0)
                    .border_radius(tokens.radii.small)
                    .height(INPUT_HEIGHT),
                    if !state.search_results.is_empty() {
                        Text(format!("{} results", state.search_results.len()))
                            .size(tokens.typography.font_size_xs)
                            .color(palette.dim_text)
                    },
                    if !state.search_results.is_empty() {
                        Scroll(
                            direction: FlexDirection::Column,
                            show_scrollbar: true,
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            child: VStack(
                                spacing: tokens.spacing.xs,
                                children: [
                                    for result in state.search_results
                                        .iter()
                                        .take(SEARCH_RESULT_LIMIT)
                                        .cloned()
                                    {
                                        SearchResultItem(
                                            result.clone(),
                                            key: format!("{}:{}:{}", result.path, result.line, result.col),
                                        )
                                    },
                                ],
                            ),
                        )
                    } else if !state.search_query.is_empty() {
                        Text("No results found")
                            .size(tokens.typography.font_size_sm)
                            .color(palette.dim_text)
                    },
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .flex_grow(1.0)
}
