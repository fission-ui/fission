use crate::model::{on_category_selected, on_search_changed, CategorySelected, SearchChanged, State};
use fission::prelude::*;

/// Below this width the category chips scroll sideways instead of wrapping, so
/// nineteen filters cannot push the list itself off a phone screen.
const CHIP_WRAP_BREAKPOINT: f32 = 720.0;

pub struct IconGalleryControls {
    /// Every category in the icon set, in reflection order.
    pub categories: Vec<&'static str>,
    /// How many icons the current filters leave visible.
    pub shown: usize,
    /// How many icons the set holds in total.
    pub total: usize,
}

impl From<IconGalleryControls> for Widget {
    fn from(controls: IconGalleryControls) -> Self {
        let (ctx, view) = fission::build::current::<State>();
        let tokens = &view.env().theme.tokens;
        let selected = view.state().category.clone();

        let search = TextInput {
            id: Some(WidgetId::explicit("icons-gallery.search")),
            semantics_identifier: Some("icons-gallery.search".into()),
            value: view.state().query.clone(),
            placeholder: Some("Search icons".into()),
            on_input: Some(with_reducer!(ctx, SearchChanged, on_search_changed)),
            ..Default::default()
        };

        let mut chips: Vec<Widget> = vec![Tag {
            label: "All".to_string(),
            on_press: Some(with_reducer!(
                ctx,
                CategorySelected(None),
                on_category_selected
            )),
            selected: selected.is_none(),
            ..Default::default()
        }
        .into()];
        chips.extend(controls.categories.into_iter().map(|category| {
            Tag {
                label: category.to_string(),
                on_press: Some(with_reducer!(
                    ctx,
                    CategorySelected(Some(category.to_string())),
                    on_category_selected
                )),
                selected: selected.as_deref() == Some(category),
                ..Default::default()
            }
            .into()
        }));

        let wide = view.viewport_size().width >= CHIP_WRAP_BREAKPOINT;
        let chip_row = Row {
            id: Some(WidgetId::explicit("icons-gallery.categories")),
            gap: Some(tokens.spacing.s),
            wrap: if wide {
                ir_op::FlexWrap::Wrap
            } else {
                ir_op::FlexWrap::NoWrap
            },
            children: chips,
            ..Default::default()
        };
        let chip_rail: Widget = if wide {
            chip_row.into()
        } else {
            Scroll {
                id: Some(WidgetId::explicit("icons-gallery.categories.scroll")),
                direction: ir_op::FlexDirection::Row,
                show_scrollbar: true,
                child: Some(chip_row.into()),
                ..Default::default()
            }
            .into()
        };

        Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                search,
                chip_rail,
                // The chip rail scrolls on a narrow window, so the active
                // category can sit off-screen. The count names it, and the
                // filter is never a mystery.
                Text::new(match &selected {
                    Some(category) => format!(
                        "{} of {} icon variants in {category}",
                        controls.shown, controls.total
                    ),
                    None => format!("{} of {} icon variants", controls.shown, controls.total),
                })
                .size(tokens.typography.body_medium_size)
                .color(tokens.colors.text_secondary),
            ],
            ..Default::default()
        }
        .into()
    }
}
