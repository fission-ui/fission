//! The todo app's widgets. Each one reads its styling from the active theme and
//! its text from the translations, so none of them names a colour or a label.

mod completed_footer;
mod empty_state;
mod theme_switch;
mod todo_app;
mod todo_composer;
mod todo_header;
mod todo_list;
mod todo_row;

use completed_footer::CompletedFooter;
use empty_state::EmptyState;
use theme_switch::ThemeSwitch;
use todo_composer::TodoComposer;
use todo_header::TodoHeader;
use todo_list::TodoList;
use todo_row::TodoRow;

pub use todo_app::TodoApp;
