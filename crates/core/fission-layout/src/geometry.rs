//! Rectangle intersection and union helpers.

use super::LayoutRect;

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
