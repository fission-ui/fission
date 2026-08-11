use serde::{Deserialize, Serialize};

/// The scalar type used for all layout measurements.
///
/// Currently `f32`. Matches [`fission_ir::op::LayoutUnit`].
pub type LayoutUnit = f32;

/// Returns `value` if it is finite, otherwise `fallback`.
pub(crate) fn finite_or(value: LayoutUnit, fallback: LayoutUnit) -> LayoutUnit {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}
/// A 2D point in layout coordinate space.
///
/// Represents an (x, y) position in logical pixels. Used for node origins and
/// coordinate calculations throughout the layout engine.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutPoint {
    /// Horizontal position in logical pixels.
    pub x: LayoutUnit,
    /// Vertical position in logical pixels.
    pub y: LayoutUnit,
}

impl LayoutPoint {
    /// The origin point: `(0.0, 0.0)`.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Creates a new point from x and y coordinates.
    pub fn new(x: LayoutUnit, y: LayoutUnit) -> Self {
        Self { x, y }
    }
}

/// A 2D size in layout coordinate space.
///
/// Represents a width and height in logical pixels. Used as the output of layout
/// measurement and as input to constraints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutSize {
    /// Width in logical pixels.
    pub width: LayoutUnit,
    /// Height in logical pixels.
    pub height: LayoutUnit,
}

impl LayoutSize {
    /// A zero-sized size: `(0.0, 0.0)`.
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    /// Creates a new size from width and height values.
    pub fn new(width: LayoutUnit, height: LayoutUnit) -> Self {
        Self { width, height }
    }
}

/// Minimum and maximum width/height bounds passed from parent to child during layout.
///
/// `BoxConstraints` is the fundamental mechanism for top-down size negotiation. A
/// parent creates constraints describing the space available to a child, and the
/// child returns a [`LayoutSize`] that satisfies those constraints.
///
/// There are two common patterns:
///
/// * **Tight constraints** -- `min == max`, forcing the child to a specific size.
///   Created with [`BoxConstraints::tight`].
/// * **Loose constraints** -- `min == 0`, giving the child freedom to be smaller
///   than the max. Created with [`BoxConstraints::loose`].
///
/// # Example
///
/// ```rust
/// use fission_layout::{BoxConstraints, LayoutSize};
///
/// let constraints = BoxConstraints::loose(800.0, 600.0);
/// assert_eq!(constraints.min_w, 0.0);
///
/// let child_wants = LayoutSize::new(300.0, 200.0);
/// let actual = constraints.constrain(child_wants);
/// assert_eq!(actual, child_wants); // fits within the constraints
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxConstraints {
    /// Minimum width the child must occupy.
    pub min_w: LayoutUnit,
    /// Maximum width the child may occupy. Can be `f32::INFINITY` for unbounded.
    pub max_w: LayoutUnit,
    /// Minimum height the child must occupy.
    pub min_h: LayoutUnit,
    /// Maximum height the child may occupy. Can be `f32::INFINITY` for unbounded.
    pub max_h: LayoutUnit,
}

impl BoxConstraints {
    /// Creates tight constraints that force a child to exactly `size`.
    ///
    /// Both min and max are set to the given width/height.
    pub fn tight(size: LayoutSize) -> Self {
        Self {
            min_w: size.width,
            max_w: size.width,
            min_h: size.height,
            max_h: size.height,
        }
    }

    /// Creates loose constraints: min is zero, max is the given values.
    ///
    /// The child can be anywhere from zero to `max_w` x `max_h`.
    pub fn loose(max_w: LayoutUnit, max_h: LayoutUnit) -> Self {
        Self {
            min_w: 0.0,
            max_w,
            min_h: 0.0,
            max_h,
        }
    }

    /// Returns `true` if the maximum width is finite (not `f32::INFINITY`).
    pub fn is_width_bounded(&self) -> bool {
        self.max_w.is_finite()
    }

    /// Returns `true` if the maximum height is finite (not `f32::INFINITY`).
    pub fn is_height_bounded(&self) -> bool {
        self.max_h.is_finite()
    }

    /// Clamps `size` so it falls within these constraints.
    ///
    /// The returned width is `max(min_w, min(size.width, max_w))`, and likewise
    /// for height.
    pub fn constrain(&self, size: LayoutSize) -> LayoutSize {
        LayoutSize {
            width: size.width.max(self.min_w).min(self.max_w),
            height: size.height.max(self.min_h).min(self.max_h),
        }
    }

    /// Returns the smallest size that satisfies these constraints: `(min_w, min_h)`.
    pub fn smallest(&self) -> LayoutSize {
        LayoutSize::new(self.min_w, self.min_h)
    }

    /// Returns new constraints shrunk inward by `padding`.
    ///
    /// Padding is `[left, right, top, bottom]`. Horizontal padding reduces the
    /// width bounds; vertical padding reduces the height bounds. Bounds are
    /// clamped to zero.
    pub fn deflate(&self, padding: [LayoutUnit; 4]) -> Self {
        let horiz = padding[0] + padding[1];
        let vert = padding[2] + padding[3];
        let max_w = (self.max_w - horiz).max(0.0);
        let max_h = (self.max_h - vert).max(0.0);
        let min_w = (self.min_w - horiz).max(0.0).min(max_w);
        let min_h = (self.min_h - vert).max(0.0).min(max_h);
        Self {
            min_w,
            max_w,
            min_h,
            max_h,
        }
    }

    /// Makes the constraints tighter by fixing the width and/or height.
    ///
    /// If `width` is `Some`, both `min_w` and `max_w` are set to that value
    /// (clamped to the current bounds). Same for `height`.
    pub fn tighten(&self, width: Option<LayoutUnit>, height: Option<LayoutUnit>) -> Self {
        let mut out = *self;
        if let Some(w) = width {
            let clamped = w.min(out.max_w).max(out.min_w);
            out.min_w = clamped;
            out.max_w = clamped;
        }
        if let Some(h) = height {
            let clamped = h.min(out.max_h).max(out.min_h);
            out.min_h = clamped;
            out.max_h = clamped;
        }
        if out.max_w < out.min_w {
            out.max_w = out.min_w;
        }
        if out.max_h < out.min_h {
            out.max_h = out.min_h;
        }
        out
    }

    /// Applies additional min/max constraints on top of the current ones.
    ///
    /// Each `Some` value further restricts the corresponding bound. `None` values
    /// leave the bound unchanged. After adjustment, max is clamped to be at least
    /// min.
    pub fn apply_min_max(
        &self,
        min_w: Option<LayoutUnit>,
        max_w: Option<LayoutUnit>,
        min_h: Option<LayoutUnit>,
        max_h: Option<LayoutUnit>,
    ) -> Self {
        let mut out = *self;
        if let Some(w) = min_w {
            out.min_w = out.min_w.max(w);
        }
        if let Some(h) = min_h {
            out.min_h = out.min_h.max(h);
        }
        if let Some(w) = max_w {
            out.max_w = out.max_w.min(w);
        }
        if let Some(h) = max_h {
            out.max_h = out.max_h.min(h);
        }
        if out.max_w < out.min_w {
            out.max_w = out.min_w;
        }
        if out.max_h < out.min_h {
            out.max_h = out.min_h;
        }
        out
    }

    /// Returns loose constraints with the same maximums but zeroed minimums.
    ///
    /// Useful when a parent wants to let a child be as small as it likes while
    /// still capping its maximum size.
    pub fn loosen(&self) -> Self {
        Self {
            min_w: 0.0,
            max_w: self.max_w,
            min_h: 0.0,
            max_h: self.max_h,
        }
    }
}
pub(crate) fn intersect_rect(left: LayoutRect, right: LayoutRect) -> LayoutRect {
    let x = left.x().max(right.x());
    let y = left.y().max(right.y());
    let right_edge = left.right().min(right.right());
    let bottom_edge = left.bottom().min(right.bottom());
    LayoutRect::new(x, y, (right_edge - x).max(0.0), (bottom_edge - y).max(0.0))
}

pub(crate) fn union_rect(left: LayoutRect, right: LayoutRect) -> LayoutRect {
    let x = left.x().min(right.x());
    let y = left.y().min(right.y());
    let right_edge = left.right().max(right.right());
    let bottom_edge = left.bottom().max(right.bottom());
    LayoutRect::new(x, y, right_edge - x, bottom_edge - y)
}

/// An axis-aligned rectangle: an origin point plus a size.
///
/// `LayoutRect` is the final output for every node after layout: it says exactly
/// where the node sits on screen and how large it is.
///
/// # Example
///
/// ```rust
/// use fission_layout::{LayoutRect, LayoutPoint};
///
/// let rect = LayoutRect::new(10.0, 20.0, 300.0, 200.0);
/// assert_eq!(rect.right(), 310.0);
/// assert!(rect.contains(LayoutPoint::new(15.0, 25.0)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutRect {
    /// The top-left corner of the rectangle.
    pub origin: LayoutPoint,
    /// The width and height of the rectangle.
    pub size: LayoutSize,
}

impl LayoutRect {
    /// Creates a rectangle from x, y, width, and height.
    pub fn new(x: LayoutUnit, y: LayoutUnit, width: LayoutUnit, height: LayoutUnit) -> Self {
        Self {
            origin: LayoutPoint { x, y },
            size: LayoutSize { width, height },
        }
    }

    /// The x coordinate of the left edge.
    pub fn x(&self) -> LayoutUnit {
        self.origin.x
    }
    /// The y coordinate of the top edge.
    pub fn y(&self) -> LayoutUnit {
        self.origin.y
    }
    /// The width of the rectangle.
    pub fn width(&self) -> LayoutUnit {
        self.size.width
    }
    /// The height of the rectangle.
    pub fn height(&self) -> LayoutUnit {
        self.size.height
    }

    /// The x coordinate of the right edge (`x + width`).
    pub fn right(&self) -> LayoutUnit {
        self.origin.x + self.size.width
    }
    /// The y coordinate of the bottom edge (`y + height`).
    pub fn bottom(&self) -> LayoutUnit {
        self.origin.y + self.size.height
    }

    /// Returns `true` if the point `p` lies within this rectangle (inclusive on
    /// the left/top edges, exclusive on the right/bottom edges).
    pub fn contains(&self, p: LayoutPoint) -> bool {
        p.x >= self.x() && p.x < self.right() && p.y >= self.y() && p.y < self.bottom()
    }
}
