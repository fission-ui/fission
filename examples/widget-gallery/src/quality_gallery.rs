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

/// A cohesive account-settings surface used for browser visual qualification.
///
/// Unlike the desktop inventory gallery, this app deliberately composes the
/// widgets into a realistic product screen. That makes density, hierarchy,
/// responsive behavior, and overlay treatment visible in one stable fixture.
#[derive(Clone)]
pub struct QualityGalleryApp;

impl From<QualityGalleryApp> for Widget {
    fn from(_app: QualityGalleryApp) -> Self {
        #[cfg(target_arch = "wasm32")]
        if quality_gallery_page() == "guided-flow" {
            return guided_flow::GuidedFlowPage.into();
        }

        let (_, view) = fission::build::current::<crate::GalleryState>();
        let tokens = &view.env().theme.tokens;
        let viewport_width = view.viewport_size().width;
        let compact = viewport_width.is_finite()
            && viewport_width > 0.0
            && viewport_width < MOBILE_BREAKPOINT;

        Container::new(Scroll {
            direction: FlexDirection::Column,
            child: Some(
                Center {
                    // The page owns controlled portals. Build only the active
                    // width variant so an inactive responsive branch cannot
                    // register a duplicate menu, select, or modal surface.
                    child: QualityPage { compact }.into(),
                }
                .into(),
            ),
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .bg(tokens.colors.surface_sunken)
        .min_height_length(Length::percent(100.0))
        .into()
    }
}

struct QualityPage {
    compact: bool,
}

impl From<QualityPage> for Widget {
    fn from(page: QualityPage) -> Self {
        let (_, view) = fission::build::current::<crate::GalleryState>();
        let tokens = &view.env().theme.tokens;
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

        Container::new(Column {
            gap: Some(region_gap),
            children: widgets![
                QualityHeader {
                    compact: page.compact,
                },
                QualityHeading {
                    compact: page.compact,
                },
                QualityTabs {
                    compact: page.compact,
                },
                QualityFooter {
                    compact: page.compact,
                },
            ],
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .max_width_length(Length::points(DESKTOP_FRAME_WIDTH))
        .padding_all(outer_padding)
        .into()
    }
}
