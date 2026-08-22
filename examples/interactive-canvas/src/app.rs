use crate::example_header::ExampleHeader;
use crate::example_layout::{CompactExampleLayout, WideExampleLayout};
use crate::state::{edit_canvas, track_viewport, CanvasExampleState, EditCanvas, TrackViewport};
use fission::prelude::*;

#[derive(Clone, Copy)]
pub struct InteractiveCanvasExample;

impl From<InteractiveCanvasExample> for Widget {
    fn from(_app: InteractiveCanvasExample) -> Self {
        let (ctx, view) = fission::build::current::<CanvasExampleState>();
        let tokens = &view.env().theme.tokens;
        let edit_canvas = with_reducer!(ctx, EditCanvas("workflow".into()), edit_canvas);
        let viewer_camera = with_reducer!(ctx, TrackViewport("viewer".into()), track_viewport);
        let canvas_camera = with_reducer!(ctx, TrackViewport("canvas".into()), track_viewport);

        let content = Responsive::new(WideExampleLayout {
            edit_canvas: edit_canvas.clone(),
            viewer_camera: viewer_camera.clone(),
            canvas_camera: canvas_camera.clone(),
        })
        .id(WidgetId::explicit("interactive-canvas.layout"))
        .case(ResponsiveCase::max_width(
            860.0,
            CompactExampleLayout {
                edit_canvas,
                viewer_camera,
                canvas_camera,
            },
        ));

        Container::new(Scroll {
            id: Some(WidgetId::explicit("interactive-canvas.page-scroll")),
            child: Some(
                Container::new(Column {
                    gap: Some(tokens.spacing.l),
                    children: widgets![
                        ExampleHeader,
                        content,
                        Text::new(view.state().status.clone())
                            .size(tokens.typography.font_size_sm)
                            .color(tokens.colors.text_secondary),
                    ],
                    ..Default::default()
                })
                .width_length(Length::percent(100.0))
                .padding([0.0, tokens.spacing.s, 0.0, 0.0])
                .into(),
            ),
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .width_length(Length::vw(100.0))
        .height_length(Length::vh(100.0))
        .padding_all(tokens.spacing.l)
        .bg(tokens.colors.background)
        .into()
    }
}
