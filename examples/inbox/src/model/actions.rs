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
pub struct SetScheduleDate(pub NaiveDate);

#[fission_action]
pub struct SetScheduleTime(pub u32, pub u32);

#[fission_action]
pub struct SetDatePickerOpen(pub bool);

#[fission_action]
pub struct SetLocale(pub fission::i18n::Locale);

#[fission_action]
pub struct ToggleBrowserDemo(pub bool);

#[fission_action]
pub struct OpenSystemLink(pub String);

#[fission_action]
pub struct OpenInAppLink(pub String);

#[fission_action]
pub struct SelectFolder(pub Folder);

// Email Ops
// Tabs & UI
#[fission_action]
pub struct SelectTab(pub usize);

/// Turns a label filter on, or off when it is already the active one.
#[fission_action]
pub struct ToggleDetails;

// Modals
#[fission_action]
pub struct SetSettingsOpen(pub bool);

#[fission_action]
pub struct SetContactsOpen(pub bool);

#[fission_action]
pub struct SetComposeOpen(pub bool);

// Mail actions
#[fission_action]
pub struct SendCompose;

// Toast
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
pub struct SetSortOption(pub String);

/// Closes compose and throws the draft away.
#[fission_action]
pub struct DiscardCompose;

/// The date range the list is narrowed to; either end may be open.
#[fission_action]
pub struct SetDateFilter(pub Option<NaiveDate>, pub Option<NaiveDate>);

#[fission_action(no_eq)]
#[serde(transparent)]
pub struct SetZoomLevel(pub f32);

impl Eq for SetZoomLevel {}

#[fission_action]
pub struct SetCalendarSelected(pub NaiveDate);

#[fission_action]
pub struct ToggleContactSelection(pub String);
