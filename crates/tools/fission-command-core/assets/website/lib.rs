pub mod design;
pub mod footer;
pub mod navigation;
pub mod page;
pub mod state;

use fission::prelude::*;
use page::{Page, SitePage};
use state::WebsiteState;

#[cfg(not(target_arch = "wasm32"))]
pub fn build_site() -> anyhow::Result<()> {
    let site = fission::site::FissionSite::new()
        .with_env(design::environment()?)
        .with_design_system::<design::WebsiteDesignSystem>(DesignMode::Light)
        .route_widget::<WebsiteState, _>(
            "/",
            "Your website",
            Some("A Fission website starter.".into()),
            SitePage(Page::Home),
        )
        .route_widget::<WebsiteState, _>(
            "/about/",
            "About this starter",
            Some("Replace this page with your own story.".into()),
            SitePage(Page::About),
        );
    fission::site::build_from_cli(site)
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
struct Website;

#[cfg(target_arch = "wasm32")]
impl From<Website> for Widget {
    fn from(_: Website) -> Self {
        let (_, view) = fission::build::current::<WebsiteState>();
        Scroll {
            child: Some(
                Router::<WebsiteState>::new()
                    .with_path(view.state().path.clone())
                    .route_component("/", SitePage(Page::Home))
                    .route_component("/about/", SitePage(Page::About))
                    .not_found(|| Text::new(design::message("not_found")))
                    .into(),
            ),
            ..Default::default()
        }
        .into()
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let pathname = web_sys::window()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("browser window is unavailable"))?
        .location()
        .pathname()?;
    // Hash routes belong to this host directory, including a Pages project mount.
    let base_path = pathname.strip_suffix("/index.html").unwrap_or(&pathname);
    let env = design::environment()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    WebApp::<WebsiteState, _>::new(Website)
        .with_mount_selector("#fission-web-mount")
        .with_title("Your website")
        .with_env(env)
        .with_design_system::<design::WebsiteDesignSystem>(DesignMode::Light)
        .with_navigation(WebNavigationConfig::new(WebRouteStrategy::Hash).base_path(base_path))
        .with_route_handler(state::route_changed)
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
