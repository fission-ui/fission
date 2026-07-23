pub mod accordion;
mod accordion_preview;
pub mod button;
mod button_preview;
pub mod carousel;
mod carousel_preview;
mod carousel_tile;
pub mod checkbox;
mod checkbox_preview;
pub mod common;
pub mod drawer;
mod drawer_preview;
pub mod modal;
mod modal_preview;
pub mod popover;
mod popover_menu;
mod popover_preview;
pub mod sidebar;
mod sidebar_preview;
mod sidebar_rail;
pub mod switch;
mod switch_preview;
pub mod tabs;
mod tabs_preview;
pub mod toast;
mod toast_preview;
pub mod tooltip;
mod tooltip_preview;

use common::WidgetSummary;

pub fn summaries() -> Vec<WidgetSummary> {
    vec![
        modal::SUMMARY,
        drawer::SUMMARY,
        accordion::SUMMARY,
        tabs::SUMMARY,
        button::SUMMARY,
        toast::SUMMARY,
        popover::SUMMARY,
        tooltip::SUMMARY,
        checkbox::SUMMARY,
        switch::SUMMARY,
        sidebar::SUMMARY,
        carousel::SUMMARY,
    ]
}
