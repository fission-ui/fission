use fission_core::env::LayoutDirection;
use fission_core::op::Fill;
use fission_core::ui::{
    Align, Container, Image, Positioned, Row, SemanticsRegion, Text, TextContent, Widget,
};
use fission_core::WidgetId;
use fission_ir::{Role, Semantics};
use serde::{Deserialize, Serialize};

const IMPLICIT_AVATAR_GROUP_ID_SALT: u32 = 0x4156_4752;
const OVERFLOW_ID_PATH: &[u32] = &[0x4f56_464c];

/// A circular user avatar displaying an image or initials.
///
/// When `src` is provided, the avatar renders the image with `Cover` fit.
/// Otherwise, it extracts up to two initials from `name` (e.g., "John Doe" -> "JD")
/// and displays them centered using the active avatar fallback recipe. A
/// non-empty `name` also becomes the accessible image label; omit it for a
/// decorative avatar.
///
/// # Fields
///
/// * `name` - User's display name (used for initials fallback).
/// * `src` - Image URL or asset path.
/// * `size` - Diameter in logical pixels (default 40).
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Avatar {
    /// Display name used to derive up to two initials when no image is supplied.
    pub name: Option<String>,
    /// Network URL or application asset path for the avatar image.
    pub src: Option<String>,
    /// Diameter in logical pixels; defaults to 40 when omitted.
    pub size: Option<f32>,
}

impl From<Avatar> for Widget {
    fn from(component: Avatar) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let fallback = &view.env().theme.components.avatar.fallback_style;
        let size = this.size.unwrap_or(40.0);
        let radius = size / 2.0;

        let content: Widget = if let Some(src) = &this.src {
            let image = if src.starts_with("https://") || src.starts_with("http://") {
                Image::network(src.clone())
            } else {
                Image::asset(src.clone())
            };
            image
                .size(size, size)
                .fit(fission_core::op::ImageFit::Cover)
                .into()
        } else {
            let initials = this
                .name
                .as_deref()
                .map(|n| {
                    n.split_whitespace()
                        .take(2)
                        .map(|s| s.chars().next().unwrap_or(' '))
                        .collect::<String>()
                        .to_uppercase()
                })
                .filter(|initials| !initials.is_empty())
                .unwrap_or("?".into());

            fission_core::ui::Align::new(Text {
                content: TextContent::Literal(initials),
                font_size: Some(size * 0.4),
                color: Some(fallback.text_color.unwrap_or(tokens.colors.on_primary)),
                ..Default::default()
            })
            .into()
        };

        let visual: Widget = Container::new(content)
            .size(size, size)
            .bg_fill(
                fallback
                    .background
                    .clone()
                    .unwrap_or(Fill::Solid(tokens.colors.primary)),
            )
            .border_radius(radius)
            .clip_overflow(true)
            .into();

        if let Some(label) = this.name.as_ref().filter(|name| !name.trim().is_empty()) {
            SemanticsRegion {
                label: Some(label.clone()),
                role: Role::Image,
                focusable: Some(false),
                sequential_focusable: false,
                child: Some(visual),
                ..Default::default()
            }
            .into()
        } else {
            visual
        }
    }
}

/// One durably identified avatar in an [`AvatarGroup`].
///
/// Use an identity derived from the represented person rather than the
/// avatar's current position, so retained state continues to follow it when
/// the group is reordered.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvatarGroupItem {
    /// Stable identity for the represented person.
    pub id: WidgetId,
    /// Avatar presentation for the represented person.
    pub avatar: Avatar,
}

impl AvatarGroupItem {
    pub fn new(id: WidgetId, avatar: Avatar) -> Self {
        Self { id, avatar }
    }
}

/// A compact, overlapping semantic group of retained avatars.
///
/// `max_visible` limits the number of people shown directly. When more people
/// are present, a final `+N` indicator describes the hidden count.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AvatarGroup {
    /// Optional identity for the semantic group.
    pub id: Option<WidgetId>,
    /// Durably identified people in display order.
    pub avatars: Vec<AvatarGroupItem>,
    /// Maximum number of avatars shown before the overflow indicator.
    pub max_visible: Option<usize>,
    /// Amount by which adjacent items overlap, in logical pixels.
    pub overlap: Option<f32>,
    /// Optional diameter override for every item in the group.
    pub size: Option<f32>,
    /// Optional accessible label for the group.
    pub semantics_label: Option<String>,
}

impl From<AvatarGroup> for Widget {
    fn from(component: AvatarGroup) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.avatar_group;
        let tokens = &view.env().theme.tokens;
        let group_id = fission_core::build::current_widget_id()
            .or(component.id)
            .or_else(|| fission_core::build::next_implicit_widget_id(IMPLICIT_AVATAR_GROUP_ID_SALT))
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.avatar-group"));

        let size = component
            .size
            .or(theme.avatar_style.width)
            .or(theme.avatar_style.height)
            .unwrap_or(32.0)
            .max(0.0);
        let radius = theme.avatar_style.radius.unwrap_or(size / 2.0);
        let overlap = component.overlap.unwrap_or(theme.overlap).max(0.0);
        let visible_count = component
            .max_visible
            .unwrap_or(theme.max_visible)
            .min(component.avatars.len());
        let hidden_count = component.avatars.len().saturating_sub(visible_count);
        let mut children = Vec::with_capacity(visible_count + usize::from(hidden_count > 0));

        for (index, item) in component
            .avatars
            .into_iter()
            .take(visible_count)
            .enumerate()
        {
            let mut avatar = item.avatar;
            avatar.size = Some(size);
            let mut surface = Container::new(avatar)
                .size(size, size)
                .border_radius(radius)
                .clip_overflow(true);
            surface.id = Some(item.id);
            if let Some(background) = theme.avatar_style.background.clone() {
                surface = surface.bg_fill(background);
            }
            if let Some(border) = theme.avatar_style.border.as_ref() {
                if let Fill::Solid(color) = &border.fill {
                    surface = surface.border(*color, border.width);
                }
            }
            let surface: Widget = surface.into();
            if index == 0 {
                children.push(surface);
            } else {
                let overlap = overlap.min(size);
                let (left, right) = match view.env().layout_direction {
                    LayoutDirection::LeftToRight => (Some(-overlap), None),
                    LayoutDirection::RightToLeft => (None, Some(-overlap)),
                };
                children.push(
                    Container::new(Positioned {
                        left,
                        right,
                        top: Some(0.0),
                        child: Some(surface),
                        ..Default::default()
                    })
                    .size(size - overlap, size)
                    .clip_overflow(false)
                    .into(),
                );
            }
        }

        if hidden_count > 0 {
            let style = &theme.overflow_style;
            let overflow_size = component
                .size
                .or(style.width)
                .or(style.height)
                .unwrap_or(size)
                .max(0.0);
            let mut overflow = Container::new(Align::new(
                Text::new(format!("+{hidden_count}"))
                    .size(style.font_size.unwrap_or(tokens.typography.font_size_xs))
                    .weight(
                        style
                            .font_weight
                            .unwrap_or(tokens.typography.font_weight_medium),
                    )
                    .line_height(style.line_height.unwrap_or(overflow_size))
                    .color(style.text_color.unwrap_or(tokens.colors.text_secondary)),
            ))
            .size(overflow_size, overflow_size)
            .border_radius(style.radius.unwrap_or(overflow_size / 2.0))
            .clip_overflow(true);
            overflow.id = Some(WidgetId::derived(group_id.as_u128(), OVERFLOW_ID_PATH));
            if let Some(background) = style.background.clone() {
                overflow = overflow.bg_fill(background);
            }
            if let Some(border) = style.border.as_ref() {
                if let Fill::Solid(color) = &border.fill {
                    overflow = overflow.border(*color, border.width);
                }
            }
            let overflow: Widget = overflow.into();
            if visible_count == 0 {
                children.push(overflow);
            } else {
                let overlap = overlap.min(overflow_size);
                let (left, right) = match view.env().layout_direction {
                    LayoutDirection::LeftToRight => (Some(-overlap), None),
                    LayoutDirection::RightToLeft => (None, Some(-overlap)),
                };
                children.push(
                    Container::new(Positioned {
                        left,
                        right,
                        top: Some(0.0),
                        child: Some(overflow),
                        ..Default::default()
                    })
                    .size(overflow_size - overlap, overflow_size)
                    .clip_overflow(false)
                    .into(),
                );
            }
        }

        Row {
            id: Some(group_id),
            children,
            semantics: Some(Semantics {
                role: Role::Group,
                label: component.semantics_label,
                ..Default::default()
            }),
            gap: Some(0.0),
            ..Default::default()
        }
        .into()
    }
}
