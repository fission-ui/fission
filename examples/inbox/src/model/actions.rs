#![allow(dead_code)]

use super::email::Folder;
use chrono::NaiveDate;
use fission::prelude::fission_action;

#[fission_action]
pub struct SetPage(pub usize);

#[fission_action]
pub struct SetFilterMode(pub usize);

#[fission_action]
#[serde(transparent)]
pub struct SetComposeTo(pub String);

#[fission_action]
#[serde(transparent)]
pub struct SetComposeSubject(pub String);

#[fission_action]
#[serde(transparent)]
pub struct SetComposeBody(pub String);

#[fission_action]
pub struct SetScheduleDate(pub NaiveDate);

#[fission_action]
pub struct SetScheduleTime(pub u32, pub u32);

#[fission_action]
pub struct SetDatePickerOpen(pub bool);

#[fission_action]
pub struct FileSelected;

#[fission_action]
pub struct SetLocale(pub fission_i18n::Locale);

#[fission_action]
pub struct ToggleBrowserDemo(pub bool);

#[fission_action]
pub struct OpenSystemLink(pub String);

#[fission_action]
pub struct OpenInAppLink(pub String);

#[fission_action]
pub struct SelectFolder(pub Folder);

#[fission_action]
pub struct Navigate(pub String);

#[fission_action]
pub struct SelectEmail(pub usize);

// Email Ops
#[fission_action]
pub struct ToggleEmailSelection(pub usize);

#[fission_action]
pub struct ToggleFlag(pub usize);

#[fission_action]
pub struct DeleteEmail(pub usize);

// Search & Filter
#[fission_action]
pub struct UpdateSearch(pub String);

#[fission_action]
pub struct ToggleFilterDropdown;

#[fission_action]
pub struct DismissDropdown;

// Tabs & UI
#[fission_action]
pub struct SelectTab(pub usize);

#[fission_action]
pub struct SelectReplyMode(pub usize);

#[fission_action]
pub struct ToggleNotifications;

#[fission_action]
pub struct ToggleDetails;

#[fission_action]
pub struct ToggleFolderExpand(pub String);

// Modals
#[fission_action]
pub struct SetSettingsOpen(pub bool);

#[fission_action]
pub struct SetContactsOpen(pub bool);

#[fission_action]
pub struct SetComposeOpen(pub bool);

#[fission_action]
pub struct SetMobileMenuOpen(pub bool);

// Mail actions
#[fission_action]
pub struct SendCompose;

#[fission_action]
pub struct SetReplyBody(pub String);

#[fission_action]
pub struct SendReply(pub usize);

#[fission_action]
pub struct ToggleEmailRead(pub usize);

// Toast
#[fission_action]
pub struct ToggleToast(pub bool);

#[fission_action]
pub struct ShowToast(pub String);

// Settings
#[fission_action]
pub struct SetTheme(pub String);

#[fission_action]
pub struct SetDensity(pub String);

#[fission_action]
pub struct SetInboxType(pub String);

#[fission_action]
pub struct SetInboxTypeSelectOpen(pub bool);

#[fission_action]
pub struct SetThemeSelectOpen(pub bool);

#[fission_action]
pub struct SetDensitySelectOpen(pub bool);

#[fission_action(no_eq)]
#[serde(transparent)]
pub struct SetStorageUsage(pub f32);

impl Eq for SetStorageUsage {}

#[fission_action]
pub struct SetAdvancedFiltersOpen(pub bool);

#[fission_action]
pub struct SetSortOption(pub String);

#[fission_action(no_eq)]
#[serde(transparent)]
pub struct SetZoomLevel(pub f32);

impl Eq for SetZoomLevel {}

#[fission_action]
#[serde(transparent)]
pub struct SetSignature(pub String);

#[fission_action]
pub struct SetSignatureEditing(pub bool);

#[fission_action]
pub struct SetSmartComposeEnabled(pub bool);

#[fission_action]
pub struct SetOfflineEnabled(pub bool);

#[fission_action]
pub struct SetAutoAdvanceEnabled(pub bool);

#[fission_action]
pub struct SetHelpPopoverOpen(pub bool);

#[fission_action]
pub struct LabelDropped(pub String);

#[fission_action]
pub struct SetMeetCameraOn(pub bool);

#[fission_action]
pub struct SetMeetMicOn(pub bool);

#[fission_action]
pub struct SetQuickTipOpen(pub bool);

#[fission_action]
pub struct SetCalendarSelected(pub NaiveDate);

#[fission_action]
pub struct ToggleContactSelection(pub String);

#[fission_action]
pub struct SetDragInProgress(pub bool);
