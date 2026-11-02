mod browser_cta;
mod compose_section;
mod feature_card;
mod feature_section;
mod goal_card;
mod goal_chooser;
mod goal_intro;
mod goal_sidebar;
mod goal_step;
mod goal_toc;
mod home_hero;
mod platform_item;
mod platform_strip;
mod site_footer;
mod site_header;
mod tutorial_card;
mod tutorial_gallery;

pub use browser_cta::BrowserCta;
pub use compose_section::ComposeSection;
pub use feature_section::FeatureSection;
pub use goal_chooser::GoalChooser;
pub use goal_intro::GoalIntro;
pub use goal_sidebar::GoalSidebar;
pub use goal_step::GoalStepSection;
pub use goal_toc::GoalToc;
pub use home_hero::HomeHero;
pub use platform_strip::PlatformStrip;
pub use site_footer::SiteFooter;
pub use site_header::SiteHeader;
pub use tutorial_gallery::TutorialGallery;

pub(crate) use feature_card::FeatureCard;
pub(crate) use goal_card::GoalCard;
pub(crate) use platform_item::PlatformItem;
pub(crate) use tutorial_card::TutorialCard;

use fission::prelude::{Text, TextContent};

pub(crate) fn copy(key: &str) -> Text {
    Text::new(TextContent::Key(key.into()))
}
