use serde::{Deserialize, Serialize};
use fission_core::AppState;
use std::collections::HashSet;
use super::email::Folder;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxState {
    // Router
    pub current_path: String,
    
    // Data (Mock)
    // In a real app these would be in a DB/Store, here we keep UI state separate from data mostly,
    // but for the example we store selection state here.
    pub selected_folder: Folder,
    pub selected_email_id: Option<usize>,
    pub selected_emails: Vec<usize>,
    
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
            current_path: "/inbox".into(),
            selected_folder: Folder::Inbox,
            selected_email_id: None,
            selected_emails: vec![],
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
            toast_message: None,
            theme_mode: "Light".into(),
            density_mode: "Comfortable".into(),
            expanded_folders: HashSet::new(),
        }
    }
}

impl AppState for InboxState {}
