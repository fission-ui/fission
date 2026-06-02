use fission_core::ui::{Container, Image, Text, TextContent, Widget};
use serde::{Deserialize, Serialize};

/// A circular user avatar displaying an image or initials.
///
/// When `src` is provided, the avatar renders the image with `Cover` fit.
/// Otherwise, it extracts up to two initials from `name` (e.g., "John Doe" -> "JD")
/// and displays them centered on the primary-colored circle.
///
/// # Fields
///
/// * `name` - User's display name (used for initials fallback).
/// * `src` - Image URL or asset path.
/// * `size` - Diameter in logical pixels (default 40).
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Avatar {
    pub name: Option<String>,
    pub src: Option<String>,
    pub size: Option<f32>,
}

impl From<Avatar> for Widget {
    fn from(component: Avatar) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let size = this.size.unwrap_or(40.0);
        let radius = size / 2.0;

        let content: Widget = if let Some(src) = &this.src {
            let image = if src.starts_with("https://") || src.starts_with("http://") {
                Image::network(src.clone())
            } else {
                Image::asset(src.clone())
            };
            image
                .size(size, size)
                .fit(fission_core::op::ImageFit::Cover)
                .into()
        } else {
            let initials = this
                .name
                .as_deref()
                .map(|n| {
                    n.split_whitespace()
                        .take(2)
                        .map(|s| s.chars().next().unwrap_or(' '))
                        .collect::<String>()
                        .to_uppercase()
                })
                .unwrap_or("?".into());

            fission_core::ui::Align::new(Text {
                content: TextContent::Literal(initials),
                font_size: Some(size * 0.4),
                color: Some(tokens.colors.on_primary),
                ..Default::default()
            })
            .into()
        };

        Container::new(content)
            .size(size, size)
            .bg(tokens.colors.primary)
            .border_radius(radius)
            .into()
    }
}
