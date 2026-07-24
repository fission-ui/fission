use crate::layout::{INPUT_HEIGHT, SEARCH_ACTION_HEIGHT, SEARCH_ACTION_WIDTH, SEARCH_RESULT_LIMIT};
use crate::model::{EditorState, ExecuteSearch, OpenFile, UpdateSearchQuery};
use crate::palette::{DIM_TEXT, INPUT_BG, INPUT_BORDER, PANEL_TEXT};
use crate::search_result_item::SearchResultItem;
use fission::prelude::*;
use fission::widgets::{HStack, VStack};

pub struct SearchPanel;

impl From<SearchPanel> for Widget {
    fn from(_component: SearchPanel) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;

        let update_query = ctx.bind(
            UpdateSearchQuery(String::new()),
            reduce_with!((|s: &mut EditorState, a: UpdateSearchQuery, _| s.search_query = a.0)),
        );

        let execute = ctx.bind(
            ExecuteSearch,
            reduce_with!((|s: &mut EditorState, _, _| s.run_search())),
        );

        let open_id = ctx
            .bind(
                OpenFile(String::new()),
                reduce_with!((|s: &mut EditorState, a: OpenFile, _| s.open_file(a.0))),
            )
            .id;

        let search_row = Container::new(HStack {
            spacing: Some(tokens.spacing.none),
            children: widgets![
                TextInput {
                    id: Some(fission::WidgetId::explicit("editor_search_query_input")),
                    value: view.state().search_query.clone(),
                    placeholder: Some("Search...".into()),
                    on_change: Some(update_query),
                    borderless: true,
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Text::new("Go")
                            .size(tokens.typography.font_size_xs)
                            .color(PANEL_TEXT)
                            .into(),
                    ),
                    on_press: Some(execute),
                    width: Some(SEARCH_ACTION_WIDTH),
                    height: Some(SEARCH_ACTION_HEIGHT),
                    padding: Some([tokens.spacing.none; 4]),
                    ..Default::default()
                },
            ],
        })
        .bg(INPUT_BG)
        .border(INPUT_BORDER, 1.0)
        .border_radius(tokens.radii.small)
        .height(INPUT_HEIGHT)
        .into();

        let mut children = vec![search_row];

        if !view.state().search_results.is_empty() {
            children.push(
                Text::new(format!("{} results", view.state().search_results.len()))
                    .size(tokens.typography.font_size_xs)
                    .color(DIM_TEXT)
                    .into(),
            );

            let result_nodes = view
                .state()
                .search_results
                .iter()
                .take(SEARCH_RESULT_LIMIT)
                .cloned()
                .map(|result| SearchResultItem { result, open_id }.into())
                .collect();

            children.push(
                Scroll {
                    direction: FlexDirection::Column,
                    child: Some(
                        VStack {
                            spacing: Some(tokens.spacing.xs),
                            children: result_nodes,
                        }
                        .into(),
                    ),
                    show_scrollbar: true,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..Default::default()
                }
                .into(),
            );
        } else if !view.state().search_query.is_empty() {
            children.push(
                Text::new("No results found")
                    .size(tokens.typography.font_size_sm)
                    .color(DIM_TEXT)
                    .into(),
            );
        }

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children,
            flex_grow: 1.0,
            justify_content: fission::core::op::JustifyContent::Start,
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .flex_grow(1.0)
        .into()
    }
}
