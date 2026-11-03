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
mod guide_card;
mod home_hero;
mod hub_path_step;
mod hub_sidebar;
mod hub_starter_code;
mod hub_task_card;
mod platform_item;
mod platform_strip;
mod product_benefit;
mod product_link_card;
mod site_footer;
mod site_header;
mod tutorial_card;
mod tutorial_gallery;
mod tutorial_group;
mod tutorial_list_item;

pub use browser_cta::BrowserCta;
pub use compose_section::ComposeSection;
pub use feature_section::FeatureSection;
pub use goal_chooser::GoalChooser;
pub use goal_intro::GoalIntro;
pub use goal_sidebar::GoalSidebar;
pub use goal_step::GoalStepSection;
pub use goal_toc::GoalToc;
pub use home_hero::HomeHero;
pub use hub_sidebar::HubSidebar;
pub use platform_strip::PlatformStrip;
pub(crate) use product_benefit::ProductBenefit;
pub(crate) use product_link_card::ProductLinkCard;
pub use site_footer::SiteFooter;
pub use site_header::SiteHeader;
pub use tutorial_gallery::TutorialGallery;
pub(crate) use tutorial_group::TutorialGroup;
pub(crate) use tutorial_list_item::TutorialListItem;

pub(crate) use feature_card::FeatureCard;
pub(crate) use goal_card::GoalCard;
pub(crate) use guide_card::GuideCard;
pub(crate) use hub_path_step::HubPathStep;
pub(crate) use hub_starter_code::HubStarterCode;
pub(crate) use hub_task_card::HubTaskCard;
pub(crate) use platform_item::PlatformItem;
pub(crate) use tutorial_card::{tutorial_image, TutorialCard};

use fission::prelude::{Text, TextContent};

pub(crate) fn copy(key: &str) -> Text {
    Text::new(TextContent::Key(key.into()))
}
