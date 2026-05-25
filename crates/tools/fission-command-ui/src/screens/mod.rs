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

impl Widget<UiState> for ActiveScreen {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        match view.state.route {
            UiRoute::Dashboard => DashboardScreen.build(ctx, view),
            UiRoute::Project => ProjectScreen.build(ctx, view),
            UiRoute::Doctor => DoctorScreen.build(ctx, view),
            UiRoute::Devices => DevicesScreen.build(ctx, view),
            UiRoute::Run => RunScreen.build(ctx, view),
            UiRoute::Build => BuildScreen.build(ctx, view),
            UiRoute::Test => TestScreen.build(ctx, view),
            UiRoute::Site => SiteScreen.build(ctx, view),
            UiRoute::Logs => LogsScreen.build(ctx, view),
            UiRoute::Settings => SettingsScreen.build(ctx, view),
            UiRoute::Help => HelpScreen.build(ctx, view),
        }
    }
}

pub fn title_block(
    title: &str,
    description: &str,
    title_color: fission::ir::op::Color,
    text_color: fission::ir::op::Color,
) -> Node {
    Column {
        gap: Some(0.0),
        children: vec![
            Text::new(title).color(title_color).into_node(),
            Text::new(description).color(text_color).into_node(),
        ],
        ..Default::default()
    }
    .into_node()
}
