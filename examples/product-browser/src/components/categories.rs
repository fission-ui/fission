use crate::api::{ApiError, ProductCategory};
use crate::model::{on_category_selected, CategorySelected, ProductBrowserState};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct CategoryRail {
    pub snapshot: AsyncSnapshot<Vec<ProductCategory>, ApiError>,
}

impl From<CategoryRail> for Widget {
    fn from(component: CategoryRail) -> Self {
        let (ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
        let mut children = vec![
            Text::new("Categories")
                .size(16.0)
                .weight(700)
                .color(tokens.colors.text_primary)
                .into(),
            category_button(ctx, view, "All products".to_string(), None),
        ];

        match component.snapshot.connection_state {
            AsyncConnectionState::Waiting => children.push(
                Text::new("Loading categories...")
                    .size(13.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
            ),
            _ if component.snapshot.has_error() => children.push(
                Text::new("Categories unavailable")
                    .size(13.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
            ),
            _ => {
                if let Some(categories) = component.snapshot.data() {
                    children.extend(categories.iter().map(|category| {
                        category_button(
                            ctx,
                            view,
                            category.name.clone(),
                            Some(category.slug.clone()),
                        )
                    }));
                }
            }
        }

        Container::new(Scroll {
            child: Some(
                Column {
                    gap: Some(8.0),
                    children,
                    ..Default::default()
                }
                .into(),
            ),
            flex_grow: 1.0,
            ..Default::default()
        })
        .width(220.0)
        .padding_all(16.0)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(22.0)
        .into()
    }
}
fn category_button(
    ctx: BuildCtxHandle<ProductBrowserState>,
    view: ViewHandle<ProductBrowserState>,
    label: String,
    category: Option<String>,
) -> Widget {
    let selected = view.state().selected_category == category;
    let action = with_reducer!(ctx, CategorySelected(category), on_category_selected);
    Button {
        on_press: Some(action),
        variant: if selected {
            ButtonVariant::Filled
        } else {
            ButtonVariant::Ghost
        },
        content_align: ButtonContentAlign::Start,
        width: Some(188.0),
        child: Some(Text::new(label).max_lines(1).into()),
        ..Default::default()
    }
    .into()
}
