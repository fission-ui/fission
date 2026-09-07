mod brand_logo;
mod crates;
mod footer;
mod home;
mod home_nav;
mod home_sections;
mod home_widgets;
mod localized;
mod marketing;
mod state;
mod value_home;

pub(crate) use crates::{CrateDetailPage, CrateDirectoryPage};
pub(crate) use footer::DocsFooter;
pub(crate) use home::RoutedHomePage;
pub(crate) use home_nav::ContentPageNav;
pub(crate) use localized::LocalizedLandingPage;
pub(crate) use marketing::{MarketingPageKind, ProductMarketingPage};
pub(crate) use state::DocsState;
