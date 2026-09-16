use crate::catalog::ExampleDefinition;
use crate::previews::PreviewRouter;
use crate::state::{PreviewViewport, ShowcaseState};
use fission::prelude::*;
use fission::widgets::Center;

pub(crate) const MOBILE_PREVIEW_WIDTH: f32 = 390.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PreviewFrame {
    pub(crate) example: ExampleDefinition,
}

impl From<PreviewFrame> for Widget {
    fn from(component: PreviewFrame) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        let mobile = state.preview_viewport == PreviewViewport::Mobile;
        // The frame shows through wherever the example does not paint, so it takes the
        // preview's design system ground, not the showcase's, to read as one surface.
        let preview_background = state
            .design_system
            .theme(state.theme_mode)
            .tokens
            .colors
            .background;
        let preview: Widget = if mobile {
            Center {
                child: Container::new(PreviewRouter {
                    example: component.example,
                })
                .width_length(Length::points(MOBILE_PREVIEW_WIDTH))
                .height_length(Length::percent(100.0))
                .bg(preview_background)
                .border(tokens.colors.border, tokens.sizing.border_hairline)
                .border_radius(tokens.radii.large)
                .clip_overflow(true)
                .id(WidgetId::explicit("showcase.preview.mobile"))
                .into(),
            }
            .into()
        } else {
            Container::new(PreviewRouter {
                example: component.example,
            })
            .width_length(Length::percent(100.0))
            .height_length(Length::percent(100.0))
            .bg(preview_background)
            .id(WidgetId::explicit("showcase.preview.desktop"))
            .into()
        };

        Container::new(preview)
            .width_length(Length::percent(100.0))
            .height_length(Length::percent(100.0))
            .flex_grow(1.0)
            .min_height(0.0)
            .padding_all(if mobile { tokens.spacing.l } else { 0.0 })
            .bg(tokens.colors.surface_sunken)
            .id(WidgetId::explicit("showcase.preview.viewport"))
            .into()
    }
}
