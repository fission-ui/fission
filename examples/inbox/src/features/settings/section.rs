use fission::prelude::*;
use fission::widgets::divider::Orientation;

fission_component! {
/// The heading that opens a settings section.
    pub(super) SectionHeading<()> { key: &'static str } |heading, _state, cx| =>
    Text(TextContent::Key(heading.key.into()))
        .size(cx.view.env().theme.tokens.typography.font_size_base)
}

fission_component! {
/// Separates adjacent settings sections.
    pub(super) SectionDivider<()> |_divider, _state, _cx| =>
    Divider(orientation: Orientation::Horizontal)
}
