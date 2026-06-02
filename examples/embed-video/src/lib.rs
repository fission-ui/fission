use fission::prelude::*;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct VideoEmbedState;

impl GlobalState for VideoEmbedState {}

#[derive(Clone)]
pub struct VideoEmbedApp;

impl From<VideoEmbedApp> for Widget {
    fn from(_component: VideoEmbedApp) -> Self {
        let (_ctx, view) = fission::build::current::<VideoEmbedState>();
        let tokens = &view.env().theme.tokens.colors;
        Container::new(Column {
            gap: Some(16.0),
            children: vec![
                Text::new("Video embed").size(28.0).into(),
                Text::new("A bounded host-backed video surface.")
                    .size(14.0)
                    .color(tokens.text_secondary)
                    .into(),
                Container::new(Video {
                    id: Some(WidgetId::explicit("embed-video.demo")),
                    source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/demo.mp4").into(),
                    width: Some(480.0),
                    height: Some(270.0),
                    autoplay: true,
                    loop_playback: true,
                })
                .width(480.0)
                .height(270.0)
                .border(tokens.border, 1.0)
                .into(),
            ],
            ..Default::default()
        })
        .padding_all(32.0)
        .into()
    }
}
