use crate::layout::BREADCRUMB_HEIGHT;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::core::ui::{Container, Row, Text, Widget};
use fission::widgets::Spacer;

pub(crate) struct Breadcrumb;

impl From<Breadcrumb> for Widget {
    fn from(_component: Breadcrumb) -> Self {
        let (_ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        // Only shown when a file is open
        if view.state().open_tabs.is_empty() || view.state().breadcrumb_path.is_empty() {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into();
        }

        let segments = &view.state().breadcrumb_path;
        let mut children: Vec<Widget> = Vec::new();

        for (i, seg) in segments.iter().enumerate() {
            if i > 0 {
                children.push(
                    Text::new(" > ")
                        .size(tokens.typography.font_size_xs)
                        .color(palette.dim_text)
                        .into(),
                );
            }
            children.push(
                Text::new(seg.as_str())
                    .size(tokens.typography.font_size_xs)
                    .color(palette.dim_text)
                    .into(),
            );
        }

        Container::new(Row {
            children,
            align_items: fission::op::AlignItems::Center,
            ..Default::default()
        })
        .height(BREADCRUMB_HEIGHT)
        .padding_all(tokens.spacing.xs)
        .bg(palette.surface_bg)
        .flex_shrink(0.0)
        .into()
    }
}
