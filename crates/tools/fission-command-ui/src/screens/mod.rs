mod dashboard;
mod devices;
mod doctor;
mod help;
mod logs;
mod project;
mod run_build_test;
mod settings;
mod site;

use crate::routes::UiRoute;
use crate::state::UiState;
use fission::prelude::*;

pub use dashboard::DashboardScreen;
pub use devices::DevicesScreen;
pub use doctor::DoctorScreen;
pub use help::HelpScreen;
pub use logs::LogsScreen;
pub use project::ProjectScreen;
pub use run_build_test::{BuildScreen, RunScreen, TestScreen};
pub use settings::SettingsScreen;
pub use site::SiteScreen;

#[derive(Clone)]
pub struct ActiveScreen;

impl From<ActiveScreen> for Widget {
    fn from(_component: ActiveScreen) -> Self {
        let (_ctx, view) = fission::build::current::<UiState>();
        match view.state().route {
            UiRoute::Dashboard => DashboardScreen.into(),
            UiRoute::Project => ProjectScreen.into(),
            UiRoute::Doctor => DoctorScreen.into(),
            UiRoute::Devices => DevicesScreen.into(),
            UiRoute::Run => RunScreen.into(),
            UiRoute::Build => BuildScreen.into(),
            UiRoute::Test => TestScreen.into(),
            UiRoute::Site => SiteScreen.into(),
            UiRoute::Logs => LogsScreen.into(),
            UiRoute::Settings => SettingsScreen.into(),
            UiRoute::Help => HelpScreen.into(),
        }
    }
}
pub fn title_block(
    title: &str,
    description: &str,
    title_color: fission::op::Color,
    text_color: fission::op::Color,
) -> Widget {
    Column {
        gap: Some(0.0),
        children: vec![
            Text::new(title).color(title_color).into(),
            Text::new(description).color(text_color).into(),
        ],
        ..Default::default()
    }
    .into()
}
