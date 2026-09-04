//! Renderer-independent two-dimensional contact geometry.

use serde::{Deserialize, Serialize};

use crate::{Bounds2D, Place, Px};

/// A gameplay object's exact contact shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum TouchArea {
    None,
    Circle { center: Place, radius: Px },
    Rect { bounds: Bounds2D },
    Union(Vec<Self>),
}

impl TouchArea {
    pub const fn none() -> Self {
        Self::None
    }

    pub const fn circle(center: Place, radius: Px) -> Self {
        Self::Circle { center, radius }
    }

    pub const fn rect(bounds: Bounds2D) -> Self {
        Self::Rect { bounds }
    }

    pub fn union(parts: Vec<Self>) -> Self {
        Self::Union(parts)
    }

    /// Returns an enclosing axis-aligned bound, or `None` for an empty shape.
    pub fn bounds(&self) -> Option<Bounds2D> {
        match self {
            Self::None => None,
            Self::Circle { center, radius } => {
                let radius = Px(radius.0.max(0.0));
                Some(Bounds2D {
                    min: Place::new(center.x - radius, center.y - radius),
                    max: Place::new(center.x + radius, center.y + radius),
                })
            }
            Self::Rect { bounds } => Some(*bounds),
            Self::Union(parts) => {
                let mut bounds = parts.iter().filter_map(Self::bounds);
                let first = bounds.next()?;
                Some(bounds.fold(first, |combined, next| Bounds2D {
                    min: Place::new(
                        Px(combined.min.x.0.min(next.min.x.0)),
                        Px(combined.min.y.0.min(next.min.y.0)),
                    ),
                    max: Place::new(
                        Px(combined.max.x.0.max(next.max.x.0)),
                        Px(combined.max.y.0.max(next.max.y.0)),
                    ),
                }))
            }
        }
    }

    /// Performs an exact supported narrow-phase test.
    ///
    /// Contact is inclusive: tangential circles and touching rectangle edges
    /// count as touching.
    pub fn touches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, _) | (_, Self::None) => false,
            (Self::Union(parts), other) | (other, Self::Union(parts)) => {
                parts.iter().any(|part| part.touches(other))
            }
            (Self::Rect { bounds: left }, Self::Rect { bounds: right }) => left.overlaps(*right),
            (
                Self::Circle {
                    center: left,
                    radius: left_radius,
                },
                Self::Circle {
                    center: right,
                    radius: right_radius,
                },
            ) => {
                let dx = left.x.0 - right.x.0;
                let dy = left.y.0 - right.y.0;
                let radius = left_radius.0.max(0.0) + right_radius.0.max(0.0);
                dx * dx + dy * dy <= radius * radius
            }
            (Self::Circle { center, radius }, Self::Rect { bounds })
            | (Self::Rect { bounds }, Self::Circle { center, radius }) => {
                circle_touches_rect(*center, radius.0.max(0.0), *bounds)
            }
        }
    }
}

fn circle_touches_rect(center: Place, radius: f32, bounds: Bounds2D) -> bool {
    let nearest_x = center.x.0.clamp(bounds.min.x.0, bounds.max.x.0);
    let nearest_y = center.y.0.clamp(bounds.min.y.0, bounds.max.y.0);
    let dx = center.x.0 - nearest_x;
    let dy = center.y.0 - nearest_y;
    dx * dx + dy * dy <= radius * radius
}

/// Supplies the gameplay contact shape for a domain object.
pub trait Touchable2D {
    fn touch_area(&self) -> TouchArea;
}

/// Supplies the world bounds for a bounded gameplay area.
pub trait Area2D {
    fn bounds(&self) -> Bounds2D;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Size;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> TouchArea {
        TouchArea::rect(Bounds2D::from_top_left(
            Place::new(Px(x), Px(y)),
            Size::new(Px(width), Px(height)),
        ))
    }

    #[test]
    fn exact_shapes_treat_tangent_edges_as_contact() {
        let circle = TouchArea::circle(Place::new(Px(10.0), Px(10.0)), Px(5.0));
        assert!(circle.touches(&TouchArea::circle(Place::new(Px(20.0), Px(10.0)), Px(5.0))));
        assert!(circle.touches(&rect(15.0, 8.0, 3.0, 4.0)));
        assert!(rect(0.0, 0.0, 5.0, 5.0).touches(&rect(5.0, 5.0, 2.0, 2.0)));
    }

    #[test]
    fn unions_use_exact_parts_and_produce_enclosing_bounds() {
        let union = TouchArea::union(vec![
            rect(-5.0, 0.0, 2.0, 2.0),
            TouchArea::circle(Place::new(Px(10.0), Px(10.0)), Px(3.0)),
        ]);
        assert!(union.touches(&rect(12.0, 9.0, 2.0, 2.0)));
        assert!(!union.touches(&rect(0.0, 0.0, 1.0, 1.0)));
        assert_eq!(
            union.bounds(),
            Some(Bounds2D {
                min: Place::new(Px(-5.0), Px(0.0)),
                max: Place::new(Px(13.0), Px(13.0)),
            })
        );
    }
}
