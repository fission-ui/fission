pub mod actions;
pub mod app_state;
pub mod compose;
pub mod detail;
pub mod email;
pub mod list;
pub mod navigation;
pub mod quick;
pub mod settings;

pub use actions::*;
pub use app_state::InboxState;
pub use email::{Category, Email, EmailMessage, Folder};
