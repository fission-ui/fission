use super::workbench_detail::WorkbenchDetail;
use super::CatalogPanel;
use crate::catalog::ExampleDefinition;
use crate::state::ShowcaseState;
use fission::prelude::*;
use fission::widgets::{SplitDirection, SplitView};

const COMPACT_WORKBENCH_BREAKPOINT: f32 = 600.0;
const DESKTOP_CATALOG_RATIO: f32 = 0.30;
const COMPACT_CATALOG_RATIO: f32 = 0.38;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Workbench {
    pub(crate) example: ExampleDefinition,
}

impl From<Workbench> for Widget {
    fn from(component: Workbench) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        if view.viewport_size().width < COMPACT_WORKBENCH_BREAKPOINT {
            WorkbenchCompact {
                example: component.example,
            }
            .into()
        } else {
            WorkbenchExpanded {
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

#[derive(Clone, Copy, Debug)]
struct WorkbenchCompact {
    example: ExampleDefinition,
}

impl From<WorkbenchCompact> for Widget {
    fn from(component: WorkbenchCompact) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        SplitView {
            id: WidgetId::explicit("showcase.workbench.compact.split"),
            direction: SplitDirection::Vertical,
            split_ratio: COMPACT_CATALOG_RATIO,
            on_resize: None,
            first: Scroll {
                id: Some(WidgetId::explicit("showcase.catalog.compact.scroll")),
                child: Some(
                    Container::new(CatalogPanel {
                        selected_slug: component.example.slug.into(),
                    })
                    .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
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
