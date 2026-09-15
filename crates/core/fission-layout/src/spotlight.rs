//! Regions that surround a spotlighted target inside its bounds.

use super::*;

pub(crate) fn spotlight_regions(
    bounds: LayoutRect,
    target: Option<LayoutRect>,
    padding: LayoutUnit,
) -> [LayoutRect; 5] {
    let zero = LayoutRect::new(bounds.x(), bounds.y(), 0.0, 0.0);
    let Some(target) = target else {
        return [bounds, zero, zero, zero, zero];
    };

    let padding = if padding.is_finite() {
        padding.max(0.0)
    } else {
        0.0
    };
    let left = (target.x() - padding).clamp(bounds.x(), bounds.right());
    let top = (target.y() - padding).clamp(bounds.y(), bounds.bottom());
    let right = (target.right() + padding).clamp(bounds.x(), bounds.right());
    let bottom = (target.bottom() + padding).clamp(bounds.y(), bounds.bottom());

    if right <= left || bottom <= top {
        return [bounds, zero, zero, zero, zero];
    }

    let hole_width = right - left;
    let hole_height = bottom - top;
    [
        LayoutRect::new(bounds.x(), bounds.y(), bounds.width(), top - bounds.y()),
        LayoutRect::new(bounds.x(), bottom, bounds.width(), bounds.bottom() - bottom),
        LayoutRect::new(bounds.x(), top, left - bounds.x(), hole_height),
        LayoutRect::new(left + hole_width, top, bounds.right() - right, hole_height),
        LayoutRect::new(left, top, hole_width, hole_height),
    ]
}
