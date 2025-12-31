use serde::{Deserialize, Serialize};
use fission_core::AppState;
use std::collections::HashSet;
use super::email::Folder;
use chrono::NaiveDate;
use fission_i18n::Locale;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxState {
    pub locale: Locale,
    // Router
    pub current_path: String,
    
    // ... (existing) ...
    pub selected_folder: Folder,
    pub selected_email_id: Option<usize>,
    pub selected_emails: Vec<usize>,
    
    // List View
    pub page: usize,
    pub total_pages: usize,
    pub filter_mode: usize, // 0=All, 1=Unread, 2=Starred
    
    // Compose State
    pub compose_to: String,
    pub compose_subject: String,
    pub compose_body: String,
    pub compose_attachments: Vec<String>,
    pub schedule_date: Option<NaiveDate>,
    pub schedule_time: Option<(u32, u32)>,
    pub is_date_picker_open: bool,
    pub is_time_picker_open: bool, // Not used by standard TimePicker (inline) but maybe for modal?
    
    // UI State
    pub search_query: String,
    pub show_filter_dropdown: bool,
    pub active_tab: usize, 
    pub reply_mode: usize, 
    pub notifications_enabled: bool,
    pub details_expanded: bool,
    pub storage_usage: f32,
    
    // Modals
    pub show_settings: bool,
    pub show_contacts: bool,
    pub show_compose: bool,
    pub show_toast: bool,
    pub show_mobile_menu: bool,
    pub toast_message: Option<String>,
    
    // Preferences
    pub theme_mode: String,
    pub density_mode: String,
    
    // Tree View State
    pub expanded_folders: HashSet<String>,
}

impl Default for InboxState {
    fn default() -> Self {
        Self {
            locale: Locale("en-US".into()),
            current_path: "/inbox".into(),
            selected_folder: Folder::Inbox,
            selected_email_id: None,
            selected_emails: vec![],
            
            page: 1,
            total_pages: 5,
            filter_mode: 0,
            
            compose_to: "".into(),
            compose_subject: "".into(),
            compose_body: "".into(),
            compose_attachments: vec![],
            schedule_date: None,
            schedule_time: None,
            is_date_picker_open: false,
            is_time_picker_open: false,
            
            search_query: "".into(),
            show_filter_dropdown: false,
            active_tab: 0,
            reply_mode: 0,
            notifications_enabled: true,
            details_expanded: true,
            storage_usage: 0.3,
            show_settings: false,
            show_contacts: false,
            show_compose: false,
            show_toast: false,
            show_mobile_menu: false,
            toast_message: None,
            theme_mode: "Light".into(),
            density_mode: "Comfortable".into(),
            expanded_folders: HashSet::new(),
        }
    }
}

impl AppState for InboxState {}
