use crate::op::LayoutUnit;
use serde::{Deserialize, Serialize};
use std::hash::Hash;

/// A uniform 2D camera transform used by interactive viewports.
///
/// Screen coordinates are calculated as `world * scale + translation`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewportTransform {
    /// Translation from world origin to screen origin in logical points.
    pub translation: [LayoutUnit; 2],
    /// Uniform scale. Invalid values normalize to `1.0` before use.
    pub scale: f32,
}

impl ViewportTransform {
    pub const IDENTITY: Self = Self {
        translation: [0.0, 0.0],
        scale: 1.0,
    };

    pub fn new(translation_x: LayoutUnit, translation_y: LayoutUnit, scale: f32) -> Self {
        Self {
            translation: [translation_x, translation_y],
            scale,
        }
        .normalized()
    }

    pub fn normalized(self) -> Self {
        Self {
            translation: [
                finite_or_zero(self.translation[0]),
                finite_or_zero(self.translation[1]),
            ],
            scale: if self.scale.is_finite() && self.scale > 0.0 {
                self.scale
            } else {
                1.0
            },
        }
    }

    pub fn world_to_screen(self, world: [LayoutUnit; 2]) -> [LayoutUnit; 2] {
        let transform = self.normalized();
        [
            world[0] * transform.scale + transform.translation[0],
            world[1] * transform.scale + transform.translation[1],
        ]
    }

    pub fn screen_to_world(self, screen: [LayoutUnit; 2]) -> [LayoutUnit; 2] {
        let transform = self.normalized();
        [
            (screen[0] - transform.translation[0]) / transform.scale,
            (screen[1] - transform.translation[1]) / transform.scale,
        ]
    }

    /// Changes scale while keeping the world point below `screen_focal_point`
    /// fixed on screen.
    pub fn with_scale_around(self, screen_focal_point: [LayoutUnit; 2], scale: f32) -> Self {
        let current = self.normalized();
        let world_focal_point = current.screen_to_world(screen_focal_point);
        let next_scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            current.scale
        };
        Self {
            translation: [
                screen_focal_point[0] - world_focal_point[0] * next_scale,
                screen_focal_point[1] - world_focal_point[1] * next_scale,
            ],
            scale: next_scale,
        }
    }
}

impl Default for ViewportTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Hash for ViewportTransform {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.translation[0].to_bits().hash(state);
        self.translation[1].to_bits().hash(state);
        self.scale.to_bits().hash(state);
    }
}

fn finite_or_zero(value: LayoutUnit) -> LayoutUnit {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ViewportPanAxis {
    /// Disable panning while retaining configured zoom behavior.
    None,
    /// Permit horizontal translation only.
    Horizontal,
    /// Permit vertical translation only.
    Vertical,
    /// Permit translation on both axes.
    #[default]
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct ViewportMargin {
    pub left: LayoutUnit,
    pub right: LayoutUnit,
    pub top: LayoutUnit,
    pub bottom: LayoutUnit,
}

impl ViewportMargin {
    pub const ZERO: Self = Self {
        left: 0.0,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
    };

    pub fn all(value: LayoutUnit) -> Self {
        let value = non_negative_finite(value);
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    pub fn normalized(self) -> Self {
        Self {
            left: non_negative_finite(self.left),
            right: non_negative_finite(self.right),
            top: non_negative_finite(self.top),
            bottom: non_negative_finite(self.bottom),
        }
    }
}

impl Hash for ViewportMargin {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.left.to_bits().hash(state);
        self.right.to_bits().hash(state);
        self.top.to_bits().hash(state);
        self.bottom.to_bits().hash(state);
    }
}

fn non_negative_finite(value: LayoutUnit) -> LayoutUnit {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ViewportBoundary {
    /// Do not constrain the camera translation.
    #[default]
    Unbounded,
    /// Constrain the visible world to finite bounds plus an edge margin.
    Finite {
        min_x: LayoutUnit,
        min_y: LayoutUnit,
        max_x: LayoutUnit,
        max_y: LayoutUnit,
        margin: ViewportMargin,
    },
}

impl ViewportBoundary {
    pub fn finite(
        min_x: LayoutUnit,
        min_y: LayoutUnit,
        max_x: LayoutUnit,
        max_y: LayoutUnit,
        margin: ViewportMargin,
    ) -> Self {
        Self::Finite {
            min_x,
            min_y,
            max_x,
            max_y,
            margin,
        }
        .normalized()
    }

    pub fn normalized(self) -> Self {
        match self {
            Self::Unbounded => Self::Unbounded,
            Self::Finite {
                min_x,
                min_y,
                max_x,
                max_y,
                margin,
            } if min_x.is_finite()
                && min_y.is_finite()
                && max_x.is_finite()
                && max_y.is_finite() =>
            {
                Self::Finite {
                    min_x: min_x.min(max_x),
                    min_y: min_y.min(max_y),
                    max_x: min_x.max(max_x),
                    max_y: min_y.max(max_y),
                    margin: margin.normalized(),
                }
            }
            Self::Finite { .. } => Self::Unbounded,
        }
    }
}

impl Hash for ViewportBoundary {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Unbounded => 0_u8.hash(state),
            Self::Finite {
                min_x,
                min_y,
                max_x,
                max_y,
                margin,
            } => {
                1_u8.hash(state);
                min_x.to_bits().hash(state);
                min_y.to_bits().hash(state);
                max_x.to_bits().hash(state);
                max_y.to_bits().hash(state);
                margin.hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ViewportClip {
    /// Allow transformed content to paint beyond the viewport.
    None,
    /// Clip to the rectangular viewport without antialiasing its edge.
    #[default]
    HardEdge,
    /// Clip to the rectangular viewport with an antialiased edge.
    AntiAlias,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ViewportZoomPolicy {
    /// Disable every zoom gesture while retaining configured panning.
    Disabled,
    /// Accept touch or trackpad pinch gestures, but never wheel zoom.
    PinchOnly,
    /// Accept pinch gestures and zoom a wheel only while Control or Meta is held.
    #[default]
    WheelWithModifier,
    /// Accept pinch gestures and use wheel or trackpad scroll deltas for zoom.
    WheelAndTrackpad,
}
