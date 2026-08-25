#[doc(hidden)]
pub(crate) mod custom_render;
#[doc(hidden)]
pub(crate) mod node;
#[doc(hidden)]
pub(crate) mod traits;
pub mod widgets;

pub use node::{CustomWidget, Widget, WidgetIdExt, WidgetKind};
pub use widgets::{
    provider, ActionScope, Align, BadgeTone, Builder, Button, ButtonContentAlign, ButtonHierarchy,
    ButtonMotion, ButtonVariant, CardPattern, Checkbox, Column, ComponentSize, ComponentState,
    Composite, Container, ContextMenu, ContextMenuEntry, ContextMenuItem, ContextMenuRegion,
    FocusScope, FontFeature, FontVariation, GestureDetector, Grid, GridItem, HttpHeader, Icon,
    Image, ImageAlignment, ImageCachePolicy, ImageErrorBehavior, ImageLoadingBehavior,
    ImageRequest, ImageSource, IosAudioSessionCategory, IosAudioSessionCategoryOption,
    IosAudioSessionMode, IosVideoAudioOptions, LayoutBuilder, LazyColumn, Overlay, Positioned,
    Pressable, PressableRole, PressableStyle, Provider, Radio, Responsive, ResponsiveCase,
    ResponsiveQuery, RichText, RichTextRun, Row, SafeArea, Scroll, SelectionPlatformStyle,
    SelectionRegion, SelectionRegionControls, SemanticsRegion, Slider, Spacer, Switch, Text,
    TextBaseline, TextContent, TextContextMenuAction, TextContextMenuConfig, TextDecoration,
    TextDecorationLines, TextDecorationStyle, TextFontStyle, TextHyphenation, TextInput,
    TextLeadingDistribution, TextLineBreakPolicy, TextRunStyle, TextScaler, TextShadow,
    TextTypography, Video, VideoAudioActivation, VideoAudioOptions, VideoAudioPolicy, VideoSource,
    ZStack,
};
#[cfg(feature = "interactive-canvas")]
pub use widgets::{
    InteractiveViewer, ViewportBoundary, ViewportClip, ViewportMargin, ViewportPanAxis,
    ViewportTransform, ViewportZoomPolicy, DEFAULT_MAX_VIEWPORT_SCALE, DEFAULT_MIN_VIEWPORT_SCALE,
    DEFAULT_VIEWPORT_FRICTION,
};
