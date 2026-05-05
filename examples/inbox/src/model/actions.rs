#![allow(dead_code)]

use super::email::Folder;
use chrono::NaiveDate;
use fission_macros::Action;
use serde::{Deserialize, Serialize};

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetPage(pub usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetFilterMode(pub usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct SetComposeTo(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct SetComposeSubject(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct SetComposeBody(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetScheduleDate(pub NaiveDate);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetScheduleTime(pub u32, pub u32);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetDatePickerOpen(pub bool);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FileSelected;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetLocale(pub fission_i18n::Locale);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ToggleBrowserDemo(pub bool);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct OpenSystemLink(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct OpenInAppLink(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct StartAuth;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SelectFolder(pub Folder);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Navigate(pub String);

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
pub struct SetSettingsOpen(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetContactsOpen(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetComposeOpen(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetMobileMenuOpen(pub bool);

// Mail actions
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendCompose;

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetReplyBody(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendReply(pub usize);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleEmailRead(pub usize);

// Toast
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleToast(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowToast(pub String);

// Settings
#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetTheme(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetDensity(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetInboxType(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetInboxTypeSelectOpen(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetThemeSelectOpen(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetDensitySelectOpen(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SetStorageUsage(pub f32);

impl Eq for SetStorageUsage {}

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAdvancedFiltersOpen(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSortOption(pub String);

#[derive(Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SetZoomLevel(pub f32);

impl Eq for SetZoomLevel {}

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SetSignature(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSignatureEditing(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSmartComposeEnabled(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetOfflineEnabled(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAutoAdvanceEnabled(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetHelpPopoverOpen(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelDropped(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetMeetCameraOn(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetMeetMicOn(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetQuickTipOpen(pub bool);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetCalendarSelected(pub NaiveDate);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleContactSelection(pub String);

#[derive(Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetDragInProgress(pub bool);
