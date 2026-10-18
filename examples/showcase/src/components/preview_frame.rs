use crate::catalog::ExampleDefinition;
use crate::previews::PreviewRouter;
use crate::state::{PreviewViewport, ShowcaseState};
use fission::prelude::*;
use fission::widgets::Center;

pub(crate) const MOBILE_PREVIEW_WIDTH: f32 = 390.0;

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(crate) PreviewFrame<ShowcaseState> {
        example: ExampleDefinition,
    }
    |component, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let mobile = state.preview_viewport == PreviewViewport::Mobile;
        // The frame shows through wherever the example does not paint, so it takes the
        // preview's design system ground, not the showcase's, to read as one surface.
        let preview_background = state
            .design_system
            .theme(state.theme_mode)
            .tokens
            .colors
            .background;
    } =>
        Container(
            if mobile {
                Center(
                    Container(PreviewRouter(component.example))
                        .width_length(Length::points(MOBILE_PREVIEW_WIDTH))
                        .height_length(Length::percent(100.0))
                        .bg(preview_background)
                        .border(tokens.colors.border, tokens.sizing.border_hairline)
                        .border_radius(tokens.radii.large)
                        .clip_overflow(true)
                        .id(WidgetId::explicit("showcase.preview.mobile"))
                )
            } else {
                Container(PreviewRouter(component.example))
                    .width_length(Length::percent(100.0))
                    .height_length(Length::percent(100.0))
                    .bg(preview_background)
                    .id(WidgetId::explicit("showcase.preview.desktop"))
            }
        )
            .width_length(Length::percent(100.0))
            .height_length(Length::percent(100.0))
            .flex_grow(1.0)
            .min_height(0.0)
            .padding_all(if mobile { tokens.spacing.l } else { 0.0 })
            .bg(tokens.colors.surface_sunken)
            .id(WidgetId::explicit("showcase.preview.viewport"))
}
