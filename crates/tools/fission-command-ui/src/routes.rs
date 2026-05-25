use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum UiRoute {
    #[default]
    Dashboard,
    Project,
    Doctor,
    Devices,
    Run,
    Build,
    Test,
    Site,
    Logs,
    Settings,
    Help,
}

impl UiRoute {
    #[cfg(test)]
    pub const ALL: [Self; 11] = [
        Self::Dashboard,
        Self::Project,
        Self::Doctor,
        Self::Devices,
        Self::Run,
        Self::Build,
        Self::Test,
        Self::Site,
        Self::Logs,
        Self::Settings,
        Self::Help,
    ];

    pub const SIDEBAR: [Self; 7] = [
        Self::Dashboard,
        Self::Project,
        Self::Run,
        Self::Site,
        Self::Logs,
        Self::Settings,
        Self::Help,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Project => "Project",
            Self::Doctor => "Doctor",
            Self::Devices => "Devices",
            Self::Run => "Run",
            Self::Build => "Build",
            Self::Test => "Test",
            Self::Site => "Site",
            Self::Logs => "Logs",
            Self::Settings => "Settings",
            Self::Help => "Help",
        }
    }
}
