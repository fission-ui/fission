use super::PropertyCase;
use crate::state::AnimationGalleryState;
use crate::style::{color, BORDER};
use crate::widgets::common::{policy_allows_motion, preview_active};
use fission::motion::Motion;
use fission::prelude::*;

const DEMO_WIDTH: f32 = 180.0;
const DEMO_HEIGHT: f32 = 110.0;
const PREVIEW_HEIGHT: f32 = 220.0;

pub(super) struct PropertyPreview<'a> {
    pub property: &'a PropertyCase,
    pub state: &'a AnimationGalleryState,
}

impl From<PropertyPreview<'_>> for Widget {
    fn from(preview: PropertyPreview<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let child: Widget = Container::new(
            Text::new(preview.property.demo_label)
                .size(tokens.typography.body_large_size)
                .color(Color::WHITE),
        )
        .width(DEMO_WIDTH)
        .height(DEMO_HEIGHT)
        .padding_all(tokens.spacing.xl)
        .border_radius(tokens.radii.xl)
        .bg(preview.property.color)
        .into();

        let preview_child = if preview_active(preview.state) && policy_allows_motion(preview.state)
        {
            Motion {
                id: WidgetId::explicit(preview.property.id),
                tracks: vec![preview.property.track.clone()],
                child,
                clip_to_bounds: preview.property.title == "Clip / Reveal",
                ..Default::default()
            }
            .into()
        } else {
            child
        };

        Container::new(preview_child)
            .height(PREVIEW_HEIGHT)
            .padding_all(tokens.spacing.xxl)
            .border(BORDER, 1.0)
            .border_radius(tokens.radii.xl)
            .bg(color(242, 248, 252, 255))
            .into()
    }
}
