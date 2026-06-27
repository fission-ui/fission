pub mod accordion;
pub mod button;
pub mod carousel;
pub mod checkbox;
pub mod common;
pub mod drawer;
pub mod modal;
pub mod popover;
pub mod sidebar;
pub mod switch;
pub mod tabs;
pub mod toast;
pub mod tooltip;

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
