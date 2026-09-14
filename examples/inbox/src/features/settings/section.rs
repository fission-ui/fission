use fission::core::ui::{Text, TextContent, Widget};
use fission::widgets::divider::Orientation;
use fission::widgets::Divider;

/// The heading that opens a settings section.
pub(super) struct SectionHeading {
    pub key: &'static str,
}

impl From<SectionHeading> for Widget {
    fn from(heading: SectionHeading) -> Self {
        let (_, view) = fission::build::current::<()>();
        Text::new(TextContent::Key(heading.key.into()))
            .size(view.env().theme.tokens.typography.font_size_base)
            .into()
    }
}

/// Separates adjacent settings sections.
pub(super) struct SectionDivider;

impl From<SectionDivider> for Widget {
    fn from(_: SectionDivider) -> Self {
        Divider {
            orientation: Orientation::Horizontal,
            ..Default::default()
        }
        .into()
    }
}
