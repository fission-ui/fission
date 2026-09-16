use super::email::{seed_mailbox, Email, Folder};
use chrono::NaiveDate;
use fission::core::GlobalState;
use fission::i18n::Locale;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxState {
    pub locale: Locale,
    // Router
    pub current_path: String,

    // ... (existing) ...
    pub selected_folder: Folder,
    pub selected_email_id: Option<usize>,
    pub selected_emails: Vec<usize>,

    // Mail data
    pub emails: Vec<Email>,
    pub next_email_id: usize,
    pub next_message_id: usize,

    // List View
    pub page: usize,
    pub total_pages: usize,
    pub filter_mode: usize, // 0=All, 1=Unread, 2=Starred

    // Compose State
    pub compose_to: String,
    /// Why the last Send was refused, shown under the recipient field.
    pub compose_to_error: Option<String>,
    pub compose_subject: String,
    pub compose_body: String,
    pub compose_attachments: Vec<String>,
    pub schedule_date: Option<NaiveDate>,
    pub schedule_time: Option<(u32, u32)>,
    pub is_date_picker_open: bool,
    pub is_time_picker_open: bool, // Not used by standard TimePicker (inline) but maybe for modal?

    // Filters / Toolbar
    pub sort_option: String,
    pub show_advanced_filters: bool,
    pub date_filter: (Option<NaiveDate>, Option<NaiveDate>),
    pub date_filter_start_open: bool,
    pub date_filter_end_open: bool,
    /// Inclusive size range in megabytes; the full slider range filters nothing.
    pub size_filter_mb: (f32, f32),
    pub zoom_level: f32,
    pub inbox_type: String,

    // Settings / Labs
    pub signature: String,
    pub signature_editing: bool,
    pub smart_compose_enabled: bool,
    pub offline_enabled: bool,
    pub auto_advance_enabled: bool,
    pub show_inbox_type_select: bool,
    pub show_theme_select: bool,
    pub show_density_select: bool,

    // Right Sidebar / Meet
    pub meet_camera_on: bool,
    pub meet_mic_on: bool,
    pub calendar_selected: Option<NaiveDate>,

    // UX
    pub show_help_popover: bool,
    pub last_drag_label: Option<String>,
    pub show_quick_tip: bool,

    // UI State
    pub search_query: String,
    pub show_sort_menu: bool,
    pub active_tab: usize,
    /// The label the list is narrowed to, if any.
    pub label_filter: Option<String>,
    pub reply_mode: usize,
    pub reply_body: String,
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
    pub show_browser_demo: bool,
    pub browser_url: String,

    // Preferences
    pub theme_mode: String,
    pub density_mode: String,

    // Tree View State
    pub expanded_folders: HashSet<String>,
    pub contact_selected_ids: Vec<String>,
    pub drag_in_progress: bool,

    /// Whether another app, such as the example showcase, mounts the inbox. The
    /// host already names the example, so the sidebar drops the app brand.
    #[serde(default)]
    pub embedded: bool,
}

/// Upper end of the size filter, in megabytes.
pub const SIZE_FILTER_MAX_MB: f32 = 100.0;

impl Default for InboxState {
    fn default() -> Self {
        let seeded = seed_mailbox();
        let total_pages = ((seeded.emails.len() + 7) / 8).max(1);
        Self {
            locale: Locale("en-US".into()),
            current_path: "/inbox".into(),
            selected_folder: Folder::Inbox,
            selected_email_id: None,
            selected_emails: vec![],
            emails: seeded.emails,
            next_email_id: seeded.next_email_id,
            next_message_id: seeded.next_message_id,

            page: 1,
            total_pages,
            filter_mode: 0,

            compose_to: "".into(),
            compose_to_error: None,
            compose_subject: "".into(),
            compose_body: "".into(),
            compose_attachments: vec![],
            schedule_date: None,
            schedule_time: None,
            is_date_picker_open: false,
            is_time_picker_open: false,
            sort_option: "Newest".into(),
            show_advanced_filters: false,
            date_filter: (None, None),
            date_filter_start_open: false,
            date_filter_end_open: false,
            size_filter_mb: (0.0, SIZE_FILTER_MAX_MB),
            zoom_level: 1.0,
            inbox_type: "Default".into(),
            signature: "Best regards,\nFission Team".into(),
            signature_editing: false,
            smart_compose_enabled: true,
            offline_enabled: false,
            auto_advance_enabled: true,
            show_inbox_type_select: false,
            show_theme_select: false,
            show_density_select: false,
            meet_camera_on: true,
            meet_mic_on: true,
            calendar_selected: None,
            show_help_popover: false,
            last_drag_label: None,
            show_quick_tip: false,

            search_query: "".into(),
            show_sort_menu: false,
            active_tab: 0,
            label_filter: None,
            reply_mode: 0,
            reply_body: "".into(),
            notifications_enabled: true,
            details_expanded: true,
            storage_usage: 0.3,
            show_settings: false,
            show_contacts: false,
            show_compose: false,
            show_toast: false,
            show_mobile_menu: false,
            show_browser_demo: false,
            browser_url: "https://example.com".into(),
            toast_message: None,
            theme_mode: "Light".into(),
            density_mode: "Comfortable".into(),
            expanded_folders: HashSet::new(),
            contact_selected_ids: Vec::new(),
            drag_in_progress: false,
            embedded: false,
        }
    }
}

impl GlobalState for InboxState {}

impl InboxState {
    pub fn navigate_to(&mut self, path: String) {
        self.current_path = path.clone();
        let mut parts = path.split('/').filter(|p| !p.is_empty());
        if let Some(folder_part) = parts.next() {
            self.selected_folder = match folder_part.to_lowercase().as_str() {
                "inbox" => Folder::Inbox,
                "starred" => Folder::Starred,
                "sent" => Folder::Sent,
                "drafts" => Folder::Drafts,
                "trash" => Folder::Trash,
                other => Folder::Custom(other.to_string()),
            };
        }
        self.selected_email_id = parts.next().and_then(|p| p.parse::<usize>().ok());
    }
}
