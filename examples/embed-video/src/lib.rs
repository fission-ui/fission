use fission::prelude::*;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct VideoEmbedState;

impl AppState for VideoEmbedState {}

pub struct VideoEmbedApp;

impl Widget<VideoEmbedState> for VideoEmbedApp {
    fn build(&self, _ctx: &mut BuildCtx<VideoEmbedState>, view: &View<VideoEmbedState>) -> Node {
        let tokens = &view.env.theme.tokens.colors;
        Container::new(
            Column {
                gap: Some(16.0),
                children: vec![
                    Text::new("Video embed").size(28.0).into_node(),
                    Text::new("A bounded host-backed video surface.")
                        .size(14.0)
                        .color(tokens.text_secondary)
                        .into_node(),
                    Container::new(
                        Video {
                            id: Some(WidgetNodeId::explicit("embed-video.demo")),
                            source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/demo.mp4").into(),
                            width: Some(480.0),
                            height: Some(270.0),
                            autoplay: true,
                            loop_playback: true,
                        }
                        .build(_ctx, view),
                    )
                    .width(480.0)
                    .height(270.0)
                    .border(tokens.border, 1.0)
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(32.0)
        .into_node()
    }
}
