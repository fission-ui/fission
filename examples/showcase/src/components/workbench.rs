use super::workbench_detail::WorkbenchDetail;
use super::CatalogPanel;
use crate::catalog::ExampleDefinition;
use crate::state::{on_navigate, Navigate, ShowcaseState};
use fission::icons::material;
use fission::prelude::*;
use fission::widgets::{SplitDirection, SplitView};

const DESKTOP_CATALOG_RATIO: f32 = 0.30;

/// The width the preview pane gets in a window `window_width` wide.
///
/// Mirrors the layout below: under the medium breakpoint the example fills the
/// window; above it the split's handle is taken out and the rest is shared by
/// ratio. A mounted example sizes itself from this on its very first build,
/// before any layout has been measured, so its first frame matches later ones.
pub(crate) fn preview_pane_width(view: &ViewHandle<ShowcaseState>, window_width: f32) -> f32 {
    let theme = &view.env().theme;
    if window_width < theme.tokens.breakpoints.medium_max {
        return window_width;
    }
    // The same handle size SplitView uses.
    let handle = theme
        .recipe(fission::theme::recipes::SplitView)
        .try_part_named("handle")
        .and_then(|handle| handle.width.or(handle.height))
        .unwrap_or(theme.tokens.spacing.s);
    ((window_width - handle) * (1.0 - DESKTOP_CATALOG_RATIO)).max(0.0)
}

/// The catalog beside the open example.
///
/// Below the medium breakpoint there is no room for both panes, so the catalog
/// and the example each take the whole window: the home route lists examples,
/// and an example page has a way back to the list.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Workbench {
    pub(crate) example: ExampleDefinition,
}

impl From<Workbench> for Widget {
    fn from(component: Workbench) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let compact = view.viewport_size().width < view.env().theme.tokens.breakpoints.medium_max;
        if !compact {
            return WorkbenchExpanded {
                example: component.example,
            }
            .into();
        }
        if view.state().current_path == "/" {
            CatalogPage {
                selected_slug: component.example.slug,
            }
            .into()
        } else {
            ExamplePage {
                example: component.example,
            }
            .into()
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct WorkbenchExpanded {
    example: ExampleDefinition,
}

impl From<WorkbenchExpanded> for Widget {
    fn from(component: WorkbenchExpanded) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        SplitView {
            id: WidgetId::explicit("showcase.workbench.split"),
            direction: SplitDirection::Horizontal,
            split_ratio: DESKTOP_CATALOG_RATIO,
            on_resize: None,
            first: Scroll {
                id: Some(WidgetId::explicit("showcase.catalog.scroll")),
                child: Some(
                    Container::new(CatalogPanel {
                        selected_slug: component.example.slug.into(),
                    })
                    .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
                    .into(),
                ),
                ..Default::default()
            }
            .into(),
            second: WorkbenchDetail {
                example: component.example,
            }
            .into(),
        }
        .into()
    }
}

/// The catalog on its own, filling a narrow window.
#[derive(Clone, Copy, Debug)]
struct CatalogPage {
    selected_slug: &'static str,
}

impl From<CatalogPage> for Widget {
    fn from(component: CatalogPage) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        Scroll {
            id: Some(WidgetId::explicit("showcase.catalog.compact.scroll")),
            child: Some(
                Container::new(CatalogPanel {
                    selected_slug: component.selected_slug.into(),
                })
                .width_length(Length::percent(100.0))
                .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
                .into(),
            ),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into()
    }
}

/// One example filling a narrow window, with a way back to the catalog.
#[derive(Clone, Copy, Debug)]
struct ExamplePage {
    example: ExampleDefinition,
}

impl From<ExamplePage> for Widget {
    fn from(component: ExamplePage) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let back: Widget = Container::new(
            Button {
                variant: ButtonVariant::Ghost,
                size: ComponentSize::Sm,
                content: Some(
                    ButtonContent::new(TextContent::Key("showcase.nav.all_examples".into()))
                        .leading_icon(Icon::svg(material::navigation::arrow_back::round())),
                ),
                on_press: Some(with_reducer!(ctx, Navigate("/".into()), on_navigate)),
                ..Default::default()
            }
            .semantics_identifier("showcase.nav.all_examples"),
        )
        .width_length(Length::percent(100.0))
        .padding_lengths(Length::symmetric(
            Length::points(tokens.spacing.s),
            Length::points(tokens.spacing.xs),
        ))
        .bg(tokens.colors.surface)
        .into();

        Column {
            children: widgets![
                back,
                WorkbenchDetail {
                    example: component.example,
                },
            ],
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
        .into()
    }
}
