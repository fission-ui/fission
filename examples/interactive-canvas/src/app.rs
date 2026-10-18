use crate::example_header::ExampleHeader;
use crate::example_layout::{CompactExampleLayout, WideExampleLayout};
use crate::state::{edit_canvas, track_viewport, CanvasExampleState, EditCanvas, TrackViewport};
use fission::prelude::*;

fission_component! {
    pub InteractiveCanvasExample<CanvasExampleState>
    |_app, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let edit_canvas = with_reducer!(cx.build, EditCanvas("workflow".into()), edit_canvas);
        let viewer_camera =
            with_reducer!(cx.build, TrackViewport("viewer".into()), track_viewport);
        let canvas_camera =
            with_reducer!(cx.build, TrackViewport("canvas".into()), track_viewport);

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

    } =>
    Container(
        Scroll(
            id: Some(WidgetId::explicit("interactive-canvas.page-scroll")),
            child: Container(
                Column(
                    gap: tokens.spacing.l,
                    children: [
                        ExampleHeader,
                        content,
                        Text(state.status.clone())
                            .size(tokens.typography.font_size_sm)
                            .color(tokens.colors.text_secondary),
                    ],
                ),
            )
            .width_length(Length::percent(100.0))
            .padding([0.0, tokens.spacing.s, 0.0, 0.0]),
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0_f32,
            flex_shrink: 1.0_f32,
        ),
    )
    .width_length(Length::vw(100.0))
    .height_length(Length::vh(100.0))
    .padding_all(tokens.spacing.l)
    .bg(tokens.colors.background)
}
