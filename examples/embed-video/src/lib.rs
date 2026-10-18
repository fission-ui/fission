use fission::prelude::*;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct VideoEmbedState;

impl GlobalState for VideoEmbedState {}

fission_component! {
    pub VideoEmbedApp<VideoEmbedState>
    |_app, _state, cx| {
        let colours = &cx.view.env().theme.tokens.colors;
        let video = video_file!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/demo.mp4"))
            .id(WidgetId::explicit("embed-video.demo"))
            .size(480.0, 270.0)
            .autoplay(true)
            .loop_playback(true)
            .audio(VideoAudioOptions::playback());
    } =>
    Container(
        Column(
            gap: 16.0,
            children: [
                Text("Video embed").size(28.0),
                Text("A bounded host-backed video surface.")
                    .size(14.0)
                    .color(colours.text_secondary),
                Container(video)
                    .width(480.0)
                    .height(270.0)
                    .border(colours.border, 1.0),
            ],
        ),
    )
    .padding_all(32.0)
}
