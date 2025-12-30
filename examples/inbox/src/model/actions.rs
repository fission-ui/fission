use serde::{Deserialize, Serialize};
use fission_macros::Action;

// Navigation
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Navigate(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectFolder(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectEmail(pub usize);

// Email Ops
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleEmailSelection(pub usize);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleFlag(pub usize);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteEmail(pub usize);

// Search & Filter
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateSearch(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleFilterDropdown;

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DismissDropdown;

// Tabs & UI
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectTab(pub usize);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectReplyMode(pub usize);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleNotifications;

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleDetails;

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleFolderExpand(pub String);

// Modals
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleSettings;

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleContacts;

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleCompose;

// Toast
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleToast(pub bool);

// Settings
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetTheme(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetDensity(pub String);

#[derive(Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SetStorageUsage(pub f32);

impl Eq for SetStorageUsage {}
