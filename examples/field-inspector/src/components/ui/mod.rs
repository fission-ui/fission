mod action_button;
mod body_text;
mod metric;
mod muted_text;
mod panel_card;
mod responsive_grid;
mod small_button;
mod soft_panel;
mod status_pill;
mod title_text;

pub use action_button::ActionButton;
pub use body_text::BodyText;
pub use metric::Metric;
pub use muted_text::MutedText;
pub use panel_card::PanelCard;
pub use responsive_grid::ResponsiveGrid;
pub use small_button::SmallButton;
pub use soft_panel::SoftPanel;
pub use status_pill::StatusPill;
pub use title_text::{TitleScale, TitleText};

use fission::prelude::Color;

pub fn color(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
