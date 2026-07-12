use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use fission_ir::{
    op::{EmbedKind, LayoutOp, Op},
    WidgetId,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A platform-native video player widget.
///
/// The video is rendered by the platform's native player and embedded into the
/// Fission layout as an opaque surface. Use
/// [`crate::internal::BuildCtx::video_controls`] to create play/pause/seek
/// action envelopes.
///
/// # Example
///
/// ```rust,ignore
/// Video::network("https://example.com/clip.mp4")
///     .size(640.0, 360.0)
///     .autoplay(true)
///     .loop_playback(false)
///     .audio(VideoAudioOptions::playback())
///     .into();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Video {
    /// Stable widget identity (auto-derived from `source` if `None`).
    pub id: Option<WidgetId>,
    /// Typed video source consumed by the active shell.
    pub source: VideoSource,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Whether to start playing immediately.
    pub autoplay: bool,
    /// Whether to loop playback when the video ends.
    pub loop_playback: bool,
    /// Platform-neutral audio-session behavior for this video.
    ///
    /// The default is [`VideoAudioOptions::system_default`], which lets the
    /// host platform keep its normal audio policy. Use
    /// [`VideoAudioOptions::playback`] for foreground media playback that
    /// should behave like a video player rather than incidental UI audio.
    #[serde(default)]
    pub audio: VideoAudioOptions,
}

impl Default for Video {
    fn default() -> Self {
        Self {
            id: None,
            source: VideoSource::default(),
            width: None,
            height: None,
            autoplay: false,
            loop_playback: false,
            audio: VideoAudioOptions::default(),
        }
    }
}

/// Source of media for a [`Video`] widget.
///
/// The source is platform-neutral. Shells translate it into the native media
/// API they own: static and SSR targets emit HTML `<video>` sources, web uses
/// `HtmlVideoElement`, and native shells use their platform video backends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VideoSource {
    /// App-bundled asset path, relative to the app/project asset root.
    Asset { path: String },
    /// Local filesystem path.
    File { path: String },
    /// Network URL. Backend support depends on the active shell and platform.
    Network { url: String },
}

impl Default for VideoSource {
    fn default() -> Self {
        Self::Asset {
            path: String::new(),
        }
    }
}

impl VideoSource {
    /// Returns the shell-facing source string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Asset { path } | Self::File { path } => path,
            Self::Network { url } => url,
        }
    }

    /// Returns a stable source key for fallback IDs and runtime comparisons.
    pub fn key(&self) -> String {
        match self {
            Self::Asset { path } => format!("asset:{path}"),
            Self::File { path } => format!("file:{path}"),
            Self::Network { url } => format!("network:{url}"),
        }
    }
}

impl From<&str> for VideoSource {
    fn from(source: &str) -> Self {
        infer_video_source(source)
    }
}

impl From<String> for VideoSource {
    fn from(source: String) -> Self {
        infer_video_source(&source)
    }
}

fn infer_video_source(source: &str) -> VideoSource {
    if source.contains("://") {
        VideoSource::Network {
            url: source.to_string(),
        }
    } else if Path::new(source).is_absolute() {
        VideoSource::File {
            path: source.to_string(),
        }
    } else {
        VideoSource::Asset {
            path: source.to_string(),
        }
    }
}

impl Video {
    /// Creates a video from an app-bundled asset path.
    pub fn asset(path: impl Into<String>) -> Self {
        Self::from_source(VideoSource::Asset { path: path.into() })
    }

    /// Creates a video from a local filesystem path.
    pub fn file(path: impl Into<String>) -> Self {
        Self::from_source(VideoSource::File { path: path.into() })
    }

    /// Creates a video from a network URL.
    pub fn network(url: impl Into<String>) -> Self {
        Self::from_source(VideoSource::Network { url: url.into() })
    }

    /// Creates a video from a typed source.
    pub fn from_source(source: impl Into<VideoSource>) -> Self {
        Self {
            source: source.into(),
            ..Self::default()
        }
    }

    /// Sets an explicit widget identity.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets a fixed width in layout points.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets a fixed height in layout points.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Sets a fixed width and height in layout points.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Sets whether playback starts automatically.
    pub fn autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }

    /// Sets whether playback loops after reaching the end.
    pub fn loop_playback(mut self, loop_playback: bool) -> Self {
        self.loop_playback = loop_playback;
        self
    }

    /// Sets platform-neutral audio behavior.
    pub fn audio(mut self, audio: VideoAudioOptions) -> Self {
        self.audio = audio;
        self
    }
}

/// Audio-session behavior requested by a [`Video`] widget.
///
/// This describes product intent independently from any single host API. Shells
/// translate the policy into their native platform API where one exists, and
/// ignore unsupported fields rather than changing the product default globally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoAudioOptions {
    /// Cross-platform audio-session policy.
    pub policy: VideoAudioPolicy,
    /// When the shell should activate the platform audio session.
    pub activation: VideoAudioActivation,
    /// Whether this video may mix with other active audio where supported.
    pub mix_with_others: bool,
    /// Whether this video may temporarily duck other active audio where supported.
    pub duck_others: bool,
    /// iOS-specific AVAudioSession overrides.
    #[serde(default)]
    pub ios: IosVideoAudioOptions,
}

impl Default for VideoAudioOptions {
    fn default() -> Self {
        Self::system_default()
    }
}

impl VideoAudioOptions {
    /// Uses the platform's default audio-session behavior.
    ///
    /// This is Fission's default because whether video should ignore the iOS
    /// silent switch, mix with background audio, or claim exclusive playback is
    /// a product decision.
    pub fn system_default() -> Self {
        Self {
            policy: VideoAudioPolicy::SystemDefault,
            activation: VideoAudioActivation::OnDemand,
            mix_with_others: false,
            duck_others: false,
            ios: IosVideoAudioOptions::default(),
        }
    }

    /// Requests ambient media behavior: mixable, non-disruptive playback that
    /// respects platform silent/mute conventions where supported.
    pub fn ambient() -> Self {
        Self {
            policy: VideoAudioPolicy::Ambient,
            mix_with_others: true,
            ..Self::system_default()
        }
    }

    /// Requests foreground playback behavior for a primary media player.
    ///
    /// On iOS this maps to `AVAudioSessionCategoryPlayback`, so it may play
    /// even when the silent switch is engaged.
    pub fn playback() -> Self {
        Self {
            policy: VideoAudioPolicy::Playback,
            ..Self::system_default()
        }
    }

    /// Adds the platform "mix with others" hint.
    pub fn mix_with_others(mut self, value: bool) -> Self {
        self.mix_with_others = value;
        self
    }

    /// Adds the platform "duck others" hint.
    pub fn duck_others(mut self, value: bool) -> Self {
        self.duck_others = value;
        self
    }

    /// Overrides the default activation behavior.
    pub fn activation(mut self, activation: VideoAudioActivation) -> Self {
        self.activation = activation;
        self
    }

    /// Adds iOS-specific AVAudioSession overrides.
    pub fn ios(mut self, ios: IosVideoAudioOptions) -> Self {
        self.ios = ios;
        self
    }
}

/// Cross-platform audio-session policy for host-backed video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoAudioPolicy {
    /// Do not configure platform audio-session state.
    SystemDefault,
    /// Non-disruptive media that should respect silent/mute conventions where possible.
    Ambient,
    /// Foreground media playback that may ignore silent/mute conventions where supported.
    Playback,
}

/// When a platform audio session should be activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoAudioActivation {
    /// Activate only when playback is requested.
    OnDemand,
    /// Configure eagerly when the host player is created.
    OnPlayerCreate,
    /// Configure category/options, but do not activate automatically.
    Manual,
}

/// iOS AVAudioSession-specific overrides for [`VideoAudioOptions`].
///
/// Use this only when the cross-platform policy is not precise enough. Raw
/// strings are accepted so apps can adopt new AVAudioSession categories or
/// modes before Fission grows first-class enum variants.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IosVideoAudioOptions {
    /// Overrides the AVAudioSession category derived from [`VideoAudioPolicy`].
    pub category: Option<IosAudioSessionCategory>,
    /// Overrides the AVAudioSession mode. Defaults to `Default`.
    pub mode: Option<IosAudioSessionMode>,
    /// Additional AVAudioSession category options.
    #[serde(default)]
    pub category_options: Vec<IosAudioSessionCategoryOption>,
}

impl IosVideoAudioOptions {
    /// Creates an override that forces an AVAudioSession category.
    pub fn category(category: IosAudioSessionCategory) -> Self {
        Self {
            category: Some(category),
            ..Default::default()
        }
    }

    /// Creates an override that forces an AVAudioSession mode.
    pub fn mode(mode: IosAudioSessionMode) -> Self {
        Self {
            mode: Some(mode),
            ..Default::default()
        }
    }

    /// Adds an AVAudioSession category option.
    pub fn with_option(mut self, option: IosAudioSessionCategoryOption) -> Self {
        self.category_options.push(option);
        self
    }
}

/// iOS AVAudioSession categories exposed by Fission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IosAudioSessionCategory {
    Ambient,
    SoloAmbient,
    Playback,
    Record,
    PlayAndRecord,
    MultiRoute,
    /// Raw AVAudioSession category constant name, for example
    /// `"AVAudioSessionCategoryPlayback"`.
    Raw(String),
}

/// iOS AVAudioSession modes exposed by Fission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IosAudioSessionMode {
    Default,
    MoviePlayback,
    SpokenAudio,
    VideoRecording,
    Measurement,
    VoiceChat,
    VideoChat,
    GameChat,
    /// Raw AVAudioSession mode constant name, for example
    /// `"AVAudioSessionModeMoviePlayback"`.
    Raw(String),
}

/// iOS AVAudioSession category options exposed by Fission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IosAudioSessionCategoryOption {
    MixWithOthers,
    DuckOthers,
    AllowBluetoothHfp,
    DefaultToSpeaker,
    InterruptSpokenAudioAndMixWithOthers,
    AllowBluetoothA2dp,
    AllowAirPlay,
    OverrideMutedMicrophoneInterruption,
    /// Raw AVAudioSessionCategoryOptions bit.
    Raw(u64),
}

impl InternalLower for Video {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let widget_id = self
            .id
            .unwrap_or_else(|| WidgetId::explicit(&self.source.key()));
        let layout_id = cx.widget_node_id(widget_id);

        let embed_id = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Video,
                widget_id,
                width: self.width,
                height: self.height,
            }),
        )
        .build(cx);

        let mut layout_builder = InternalIrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        );
        layout_builder.add_child(embed_id);
        layout_builder.build(cx)
    }
}
