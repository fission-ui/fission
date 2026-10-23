mod account_card;
mod action_card;
mod empty_card;
mod footer;
mod general_panel;
mod guided_flow;
mod header;
mod heading;
mod tabs;

use fission::prelude::*;

use self::footer::QualityFooter;
use self::header::QualityHeader;
use self::heading::QualityHeading;
use self::tabs::QualityTabs;

const DESKTOP_FRAME_WIDTH: f32 = 1_072.0;
const MOBILE_BREAKPOINT: f32 = 640.0;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function qualityGalleryTheme() {
  return new URLSearchParams(globalThis.location.search).get("theme") ?? "light";
}

export function qualityGalleryOverlay() {
  return new URLSearchParams(globalThis.location.search).get("overlay") ?? "none";
}

export function qualityGalleryDirection() {
  return new URLSearchParams(globalThis.location.search).get("direction") ?? "ltr";
}

export function qualityGalleryPage() {
  return new URLSearchParams(globalThis.location.search).get("page") ?? "quality";
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = qualityGalleryTheme)]
    fn quality_gallery_theme() -> String;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = qualityGalleryOverlay)]
    fn quality_gallery_overlay() -> String;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = qualityGalleryDirection)]
    fn quality_gallery_direction() -> String;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = qualityGalleryPage)]
    fn quality_gallery_page() -> String;
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn initial_design_mode() -> DesignMode {
    if quality_gallery_theme() == "dark" {
        DesignMode::Dark
    } else {
        DesignMode::Light
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn initial_env() -> Env {
    let mut env = Env::default();
    if quality_gallery_direction() == "rtl" {
        env.layout_direction = LayoutDirection::RightToLeft;
    }
    env
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn configure_initial_state(state: &mut crate::GalleryState) {
    match quality_gallery_overlay().as_str() {
        "menu" => state.menu_open = true,
        "select" => state.select_open = true,
        "dialog" => state.modal_open = true,
        _ => {}
    }
}

// A cohesive account-settings surface used for browser visual qualification.
// Unlike the desktop inventory gallery, this app deliberately composes the
// widgets into a realistic product screen. That makes density, hierarchy,
// responsive behaviour, and overlay treatment visible in one stable fixture.
fission_component! {
    pub QualityGalleryApp<crate::GalleryState>
    |_app, _state, cx| {
        #[cfg(target_arch = "wasm32")]
        let show_guided_flow = quality_gallery_page() == "guided-flow";
        #[cfg(not(target_arch = "wasm32"))]
        let show_guided_flow = false;

        let tokens = &cx.view.env().theme.tokens;
        let viewport_width = cx.view.viewport_size().width;
        let compact = viewport_width.is_finite()
            && viewport_width > 0.0
            && viewport_width < MOBILE_BREAKPOINT;

    } =>
    if show_guided_flow {
        guided_flow::GuidedFlowPage
    } else {
        Container(
            Scroll(
                direction: FlexDirection::Column,
                child: Center(
                    // The page owns controlled portals. Build only the active
                    // width variant so an inactive responsive branch cannot
                    // register a duplicate menu, select, or modal surface.
                    QualityPage(compact),
                ),
                show_scrollbar: true,
                flex_grow: 1.0_f32,
                flex_shrink: 1.0_f32,
            ),
        )
        .bg(tokens.colors.surface_sunken)
        .min_height_length(Length::percent(100.0))
    }
}

fission_component! {
    QualityPage<crate::GalleryState> { compact: bool }
    |page, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let outer_padding = if page.compact {
            tokens.spacing.s + tokens.spacing.xs
        } else {
            tokens.spacing.l
        };
        let region_gap = if page.compact {
            tokens.spacing.s + tokens.spacing.xs
        } else {
            tokens.spacing.m
        };

    } =>
        Container(Column(
            gap: region_gap,
            children: [
                QualityHeader(page.compact),
                QualityHeading(page.compact),
                QualityTabs(page.compact),
                QualityFooter(page.compact),
            ],
        ))
        .width_length(Length::percent(100.0))
        .max_width_length(Length::points(DESKTOP_FRAME_WIDTH))
        .padding_all(outer_padding)
}
