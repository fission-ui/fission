use super::{composition, diagnostics, not_found_page::NotFoundPage, overview, policy, properties};
use crate::state::AnimationGalleryState;
use crate::widgets;
use fission::prelude::*;
use std::sync::Arc;

fission_component! {
    pub GalleryRouter<AnimationGalleryState> { current_path: String }
    |router, _state, _cx| {} =>
        Router::<AnimationGalleryState>::new()
            .with_path(router.current_path.clone())
            .route("/overview", overview::OverviewPage::new)
            .route(widgets::modal::PATH, widgets::modal::ModalPage::new)
            .route(widgets::drawer::PATH, widgets::drawer::DrawerPage::new)
            .route(widgets::popover::PATH, widgets::popover::PopoverPage::new)
            .route(widgets::tooltip::PATH, widgets::tooltip::TooltipPage::new)
            .route(widgets::toast::PATH, widgets::toast::ToastPage::new)
            .route(widgets::accordion::PATH, widgets::accordion::AccordionPage::new)
            .route(widgets::tabs::PATH, widgets::tabs::TabsPage::new)
            .route(widgets::button::PATH, widgets::button::ButtonPage::new)
            .route(widgets::checkbox::PATH, widgets::checkbox::CheckboxPage::new)
            .route(widgets::switch::PATH, widgets::switch::SwitchPage::new)
            .route(widgets::sidebar::PATH, widgets::sidebar::SidebarPage::new)
            .route(widgets::carousel::PATH, widgets::carousel::CarouselPage::new)
            .route_builder("/properties/:property", Arc::new(|_, _, params| {
                properties::PropertiesPage::new(format!(
                    "/properties/{}",
                    params.get("property").map(String::as_str).unwrap_or("opacity"),
                )).into()
            }))
            .route_builder("/composition/:case", Arc::new(|_, _, params| {
                composition::CompositionPage::new(format!(
                    "/composition/{}",
                    params.get("case").map(String::as_str).unwrap_or("additive"),
                )).into()
            }))
            .route("/policy/:policy", policy::PolicyPage::new)
            .route_builder("/diagnostics/:panel", Arc::new(|_, _, params| {
                diagnostics::DiagnosticsPage::new(format!(
                    "/diagnostics/{}",
                    params.get("panel").map(String::as_str).unwrap_or("declarations"),
                )).into()
            }))
            .not_found(NotFoundPage::new)
}
