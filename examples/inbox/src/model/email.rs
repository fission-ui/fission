use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email {
    pub id: usize,
    pub subject: String,
    pub sender: String,
    pub preview: String,
    pub body: String,
    pub folder: Folder,
    pub is_read: bool,
    pub is_flagged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Folder {
    Inbox,
    Starred,
    Sent,
    Drafts,
    Trash,
    Custom(String),
}

impl Default for Folder {
    fn default() -> Self {
        Folder::Inbox
    }
}

impl ToString for Folder {
    fn to_string(&self) -> String {
        match self {
            Folder::Inbox => "Inbox".into(),
            Folder::Starred => "Starred".into(),
            Folder::Sent => "Sent".into(),
            Folder::Drafts => "Drafts".into(),
            Folder::Trash => "Trash".into(),
            Folder::Custom(s) => s.clone(),
        }
    }
}
