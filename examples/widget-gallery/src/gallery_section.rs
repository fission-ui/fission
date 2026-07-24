use fission::prelude::*;

pub struct GallerySection {
    pub title: String,
    pub children: Vec<Widget>,
}

impl GallerySection {
    pub fn new(title: impl Into<String>, children: Vec<Widget>) -> Self {
        Self {
            title: title.into(),
            children,
        }
    }
}

impl From<GallerySection> for Widget {
    fn from(section: GallerySection) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let mut children = widgets![
            Spacer {
                height: Some(tokens.spacing.s),
                ..Default::default()
            },
            Text::new(section.title)
                .size(typography.font_size_lg)
                .line_height(typography.font_size_lg * typography.line_height_heading)
                .weight(typography.font_weight_bold),
            Divider::default(),
        ];
        children.extend(section.children);

        Column {
            gap: Some(tokens.spacing.s),
            children,
            ..Default::default()
        }
        .into()
    }
}
