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

impl From<GalleryRouter> for Widget {
    fn from(router: GalleryRouter) -> Self {
        Router::<AnimationGalleryState> {
            current_path: router.current_path,
            routes: vec![
                route!("/overview", |ctx, view, _| {
                    overview::OverviewPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::modal::PATH, |ctx, view, _| {
                    widgets::modal::ModalPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::drawer::PATH, |ctx, view, _| {
                    widgets::drawer::DrawerPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::popover::PATH, |ctx, view, _| {
                    widgets::popover::PopoverPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::tooltip::PATH, |ctx, view, _| {
                    widgets::tooltip::TooltipPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::toast::PATH, |ctx, view, _| {
                    widgets::toast::ToastPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::accordion::PATH, |ctx, view, _| {
                    widgets::accordion::AccordionPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::tabs::PATH, |ctx, view, _| {
                    widgets::tabs::TabsPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::button::PATH, |ctx, view, _| {
                    widgets::button::ButtonPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::checkbox::PATH, |ctx, view, _| {
                    widgets::checkbox::CheckboxPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::switch::PATH, |ctx, view, _| {
                    widgets::switch::SwitchPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::sidebar::PATH, |ctx, view, _| {
                    widgets::sidebar::SidebarPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
                route!(widgets::carousel::PATH, |ctx, view, _| {
                    widgets::carousel::CarouselPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
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
                route!("/policy/:policy", |ctx, view, _| {
                    policy::PolicyPage {
                        ctx,
                        state: view.state(),
                    }
                    .into()
                }),
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
