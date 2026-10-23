use super::PropertyCase;
use crate::state::AnimationGalleryState;
use crate::style;
use crate::widgets::common::{policy_allows_motion, preview_active};
use fission::motion::Motion;
use fission::prelude::*;

const DEMO_WIDTH: f32 = 180.0;
const DEMO_HEIGHT: f32 = 110.0;
const PREVIEW_HEIGHT: f32 = 220.0;

fission_component! {
    pub(super) PropertyPreview<AnimationGalleryState> { property: PropertyCase }
    |preview, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            if preview_active(state) && policy_allows_motion(state) {
                Motion(
                    id: WidgetId::explicit(preview.property.id),
                    tracks: vec![preview.property.track.clone()],
                    clip_to_bounds: preview.property.title == "Clip / Reveal",
                    child: PropertyDemo(preview.property.clone()),
                )
            } else {
                PropertyDemo(preview.property.clone())
            },
        )
        .height(PREVIEW_HEIGHT)
        .padding_all(tokens.spacing.xxl)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface_sunken())
}

fission_component! {
    PropertyDemo<AnimationGalleryState> { property: PropertyCase }
    |demo, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Text(demo.property.demo_label)
                .size(tokens.typography.body_large_size)
                .color(style::on_primary()),
        )
        .width(DEMO_WIDTH)
        .height(DEMO_HEIGHT)
        .padding_all(tokens.spacing.xl)
        .border_radius(tokens.radii.xl)
        .bg(demo.property.color)
}
