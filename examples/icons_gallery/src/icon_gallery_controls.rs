use crate::model::{
    on_category_selected, on_search_changed, CategorySelected, SearchChanged, State,
};
use fission::prelude::*;

const CHIP_WRAP_BREAKPOINT: f32 = 720.0;

fission_component! {
    pub IconGalleryControls<State> {
        categories: Vec<&'static str>,
        shown: usize,
        total: usize,
    } |controls, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let wide = cx.view.viewport_size().width >= CHIP_WRAP_BREAKPOINT;
    } =>
        Column(
            gap: tokens.spacing.s,
            children: [
                TextInput(
                    id: WidgetId::explicit("icons-gallery.search"),
                    semantics_identifier: "icons-gallery.search".to_string(),
                    value: state.query.clone(),
                    placeholder: Some("Search icons".into()),
                    on_input: with_reducer!(cx.build, SearchChanged, on_search_changed),
                ),
                if wide {
                    CategoryChips(controls.categories.clone(), true)
                } else {
                    Scroll(
                        id: WidgetId::explicit("icons-gallery.categories.scroll"),
                        direction: ir_op::FlexDirection::Row,
                        show_scrollbar: true,
                        child: CategoryChips(controls.categories.clone(), false),
                    )
                },
                Text(match &state.category {
                    Some(category) => format!(
                        "{} of {} icon variants in {category}",
                        controls.shown,
                        controls.total,
                    ),
                    None => format!(
                        "{} of {} icon variants",
                        controls.shown,
                        controls.total,
                    ),
                })
                .size(tokens.typography.body_medium_size)
                .color(tokens.colors.text_secondary),
            ],
        )
}

fission_component! {
    CategoryChips<State> { categories: Vec<&'static str>, wrap: bool }
    |chips, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Row(
            id: WidgetId::explicit("icons-gallery.categories"),
            gap: tokens.spacing.s,
            wrap: if chips.wrap {
                ir_op::FlexWrap::Wrap
            } else {
                ir_op::FlexWrap::NoWrap
            },
            children: [
                Tag(
                    label: "All".to_string(),
                    on_press: with_reducer!(
                        cx.build,
                        CategorySelected(None),
                        on_category_selected,
                    ),
                    selected: state.category.is_none(),
                ),
                for category in chips.categories.iter().copied() {
                    Tag(
                        label: category.to_string(),
                        on_press: with_reducer!(
                            cx.build,
                            CategorySelected(Some(category.to_string())),
                            on_category_selected,
                        ),
                        selected: state.category.as_deref() == Some(category),
                        key: category,
                    )
                },
            ],
        )
}
