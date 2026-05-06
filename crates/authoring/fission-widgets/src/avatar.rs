use fission_core::ui::{Container, Image, Node, Text, TextContent};
use fission_core::{BuildCtx, View, Widget};
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

impl<S: fission_core::AppState> Widget<S> for Avatar {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let size = self.size.unwrap_or(40.0);
        let radius = size / 2.0;

        let content = if let Some(src) = &self.src {
            Image {
                source: src.clone(),
                width: Some(size),
                height: Some(size),
                fit: Some(fission_core::op::ImageFit::Cover),
                ..Default::default()
            }
            .into()
        } else {
            let initials = self
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

            fission_core::ui::Align::new(
                Text {
                    content: TextContent::Literal(initials),
                    font_size: Some(size * 0.4),
                    color: Some(tokens.colors.on_primary),
                    ..Default::default()
                }
                .into(),
            )
            .into_node()
        };

        Container::new(content)
            .size(size, size)
            .bg(tokens.colors.primary)
            .border_radius(radius)
            .into_node()
    }
}
