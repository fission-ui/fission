use super::{composition, diagnostics, not_found_page::NotFoundPage, overview, policy, properties};
use crate::state::AnimationGalleryState;
use crate::widgets;
use fission::prelude::*;
use std::sync::Arc;

pub struct GalleryRouter {
    pub current_path: String,
}

macro_rules! route {
    ($path:expr, $builder:expr) => {
        Route {
            path: $path.into(),
            builder: Arc::new($builder),
        }
    };
}

/// A route to a page built from only the build context and gallery state.
macro_rules! page_route {
    ($path:expr, $page:path) => {
        route!($path, |ctx, view, _| {
            $page {
                ctx,
                state: view.state(),
            }
            .into()
        })
    };
}

impl From<GalleryRouter> for Widget {
    fn from(router: GalleryRouter) -> Self {
        Router::<AnimationGalleryState> {
            current_path: router.current_path,
            routes: vec![
                page_route!("/overview", overview::OverviewPage),
                page_route!(widgets::modal::PATH, widgets::modal::ModalPage),
                page_route!(widgets::drawer::PATH, widgets::drawer::DrawerPage),
                page_route!(widgets::popover::PATH, widgets::popover::PopoverPage),
                page_route!(widgets::tooltip::PATH, widgets::tooltip::TooltipPage),
                page_route!(widgets::toast::PATH, widgets::toast::ToastPage),
                page_route!(widgets::accordion::PATH, widgets::accordion::AccordionPage),
                page_route!(widgets::tabs::PATH, widgets::tabs::TabsPage),
                page_route!(widgets::button::PATH, widgets::button::ButtonPage),
                page_route!(widgets::checkbox::PATH, widgets::checkbox::CheckboxPage),
                page_route!(widgets::switch::PATH, widgets::switch::SwitchPage),
                page_route!(widgets::sidebar::PATH, widgets::sidebar::SidebarPage),
                page_route!(widgets::carousel::PATH, widgets::carousel::CarouselPage),
                route!("/properties/:property", |ctx, view, params| {
                    let path = format!(
                        "/properties/{}",
                        params
                            .get("property")
                            .map(String::as_str)
                            .unwrap_or("opacity")
                    );
                    properties::PropertiesPage {
                        ctx,
                        state: view.state(),
                        path,
                    }
                    .into()
                }),
                route!("/composition/:case", |ctx, view, params| {
                    let path = format!(
                        "/composition/{}",
                        params.get("case").map(String::as_str).unwrap_or("additive")
                    );
                    composition::CompositionPage {
                        ctx,
                        state: view.state(),
                        path,
                    }
                    .into()
                }),
                page_route!("/policy/:policy", policy::PolicyPage),
                route!("/diagnostics/:panel", |ctx, view, params| {
                    let path = format!(
                        "/diagnostics/{}",
                        params
                            .get("panel")
                            .map(String::as_str)
                            .unwrap_or("declarations")
                    );
                    diagnostics::DiagnosticsPage {
                        ctx,
                        state: view.state(),
                        path,
                    }
                    .into()
                }),
            ],
            not_found: Some(Arc::new(|_, _, _| NotFoundPage.into())),
        }
        .into()
    }
}
