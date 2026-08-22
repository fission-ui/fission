pub mod action_scope;
pub mod align;
pub mod builder;
pub mod button;
pub mod checkbox;
pub mod column;
pub mod composite;
pub mod container;
pub mod context_menu;
pub mod grid;
pub mod icon;
pub mod image;
pub mod lazy_column;
pub mod overlay;
pub mod positioned;
pub mod pressable;
pub mod provider;
pub mod radio;
pub mod responsive;
pub mod row;
pub mod safe_area;
pub mod scroll;
pub mod semantics_region;
pub mod slider;
pub mod spacer;
pub mod stack;
pub mod switch;
pub mod text;
pub mod text_input;
pub mod transform;
pub mod video;

pub use action_scope::ActionScope;
pub use align::Align;
pub use builder::{Builder, LayoutBuilder};
pub use button::{Button, ButtonContentAlign, ButtonMotion, ButtonVariant};
pub use checkbox::Checkbox;
pub use column::Column;
pub use composite::Composite;
pub use container::Container;
pub use context_menu::{
    ContextMenu, ContextMenuEntry, ContextMenuItem, ContextMenuRegion, TextContextMenuAction,
    TextContextMenuConfig,
};
pub use fission_ir::op::ResponsiveQuery;
pub use fission_theme::{BadgeTone, ButtonHierarchy, CardPattern, ComponentSize, ComponentState};
pub use grid::{Grid, GridItem};
pub use icon::Icon;
pub use image::{
    HttpHeader, Image, ImageAlignment, ImageCachePolicy, ImageErrorBehavior, ImageLoadingBehavior,
    ImageRequest, ImageSource,
};
pub use lazy_column::LazyColumn;
pub use overlay::Overlay;
pub use positioned::Positioned;
pub use pressable::{Pressable, PressableRole, PressableStyle};
pub use provider::{provider, Provider};
pub use radio::Radio;
pub use responsive::{Responsive, ResponsiveCase};
pub use row::Row;
pub use safe_area::SafeArea;
pub use scroll::Scroll;
pub use semantics_region::SemanticsRegion;
pub use slider::Slider;
pub use spacer::Spacer;
pub use stack::ZStack;
pub use switch::Switch;
pub use text::{RichText, RichTextRun, Text, TextContent, TextFontStyle, TextRunStyle};
pub use text_input::TextInput;
pub use transform::Transform;
pub use video::{
    IosAudioSessionCategory, IosAudioSessionCategoryOption, IosAudioSessionMode,
    IosVideoAudioOptions, Video, VideoAudioActivation, VideoAudioOptions, VideoAudioPolicy,
    VideoSource,
};

pub mod gesture_detector;
pub use gesture_detector::GestureDetector;

pub mod clip;
pub use clip::Clip;

pub mod focus_scope;
pub use focus_scope::FocusScope;

fn add_axis_edges(
    value: fission_ir::op::Length,
    start: &fission_ir::op::Length,
    end: &fission_ir::op::Length,
) -> fission_ir::op::Length {
    value + start.clone() + end.clone()
}

pub(super) fn split_box_margin(
    style: &mut fission_ir::op::BoxStyle,
) -> Option<fission_ir::op::BoxStyle> {
    use fission_ir::op::{BoxAlignment, BoxStyle, Length};

    let margin = style.margin.take()?;
    let [left, right, top, bottom] = margin.clone();
    let had_width = style.width.is_some();
    let had_height = style.height.is_some();
    let outer = BoxStyle {
        width: style
            .width
            .take()
            .map(|value| add_axis_edges(value, &left, &right)),
        height: style
            .height
            .take()
            .map(|value| add_axis_edges(value, &top, &bottom)),
        min_width: style
            .min_width
            .take()
            .map(|value| add_axis_edges(value, &left, &right)),
        max_width: style
            .max_width
            .take()
            .map(|value| add_axis_edges(value, &left, &right)),
        min_height: style
            .min_height
            .take()
            .map(|value| add_axis_edges(value, &top, &bottom)),
        max_height: style
            .max_height
            .take()
            .map(|value| add_axis_edges(value, &top, &bottom)),
        padding: Some(margin),
        alignment: BoxAlignment::Stretch,
        ..Default::default()
    };
    if had_width {
        style.width = Some(Length::percent(100.0));
    }
    if had_height {
        style.height = Some(Length::percent(100.0));
    }
    Some(outer)
}
