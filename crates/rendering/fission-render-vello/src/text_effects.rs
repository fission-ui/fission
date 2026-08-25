use fission_ir::op::{Color, TextDecorationStyle, TextShadow};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DecorationPrimitive {
    Rect { x0: f64, y0: f64, x1: f64, y1: f64 },
    Circle { x: f64, y: f64, radius: f64 },
    Wave { points: Vec<(f64, f64)>, width: f64 },
}

pub(crate) fn decoration_primitives(
    style: TextDecorationStyle,
    x0: f64,
    x1: f64,
    y: f64,
    thickness: f64,
) -> Vec<DecorationPrimitive> {
    if x1 <= x0 {
        return Vec::new();
    }
    let thickness = thickness.max(1.0);
    match style {
        TextDecorationStyle::Solid => vec![DecorationPrimitive::Rect {
            x0,
            y0: y,
            x1,
            y1: y + thickness,
        }],
        TextDecorationStyle::Double => {
            let stroke = (thickness / 3.0).max(1.0);
            vec![
                DecorationPrimitive::Rect {
                    x0,
                    y0: y - stroke,
                    x1,
                    y1: y,
                },
                DecorationPrimitive::Rect {
                    x0,
                    y0: y + stroke,
                    x1,
                    y1: y + stroke * 2.0,
                },
            ]
        }
        TextDecorationStyle::Dotted => {
            let radius = thickness * 0.5;
            let step = (thickness * 2.0).max(2.0);
            let mut result = Vec::new();
            let mut x = x0 + radius;
            while x <= x1 {
                result.push(DecorationPrimitive::Circle {
                    x,
                    y: y + radius,
                    radius,
                });
                x += step;
            }
            result
        }
        TextDecorationStyle::Dashed => {
            let dash = (thickness * 3.0).max(3.0);
            let mut result = Vec::new();
            let mut x = x0;
            while x < x1 {
                result.push(DecorationPrimitive::Rect {
                    x0: x,
                    y0: y,
                    x1: (x + dash).min(x1),
                    y1: y + thickness,
                });
                x += dash * 2.0;
            }
            result
        }
        TextDecorationStyle::Wavy => {
            let amplitude = thickness.max(1.0);
            let half_wave = (thickness * 2.0).max(2.0);
            let mut points = Vec::new();
            let mut x = x0;
            let mut high = true;
            while x < x1 {
                points.push((x, y + if high { 0.0 } else { amplitude * 2.0 }));
                high = !high;
                x += half_wave;
            }
            points.push((x1, y + if high { 0.0 } else { amplitude * 2.0 }));
            vec![DecorationPrimitive::Wave {
                points,
                width: thickness,
            }]
        }
    }
}

/// Approximate a Gaussian glyph shadow using a bounded convolution kernel.
/// Vello currently exposes shape blur but not a glyph-mask blur primitive.
pub(crate) fn shadow_samples(shadow: TextShadow) -> Vec<(f64, f64, Color)> {
    let radius = shadow.blur_radius.max(0.0) as f64;
    if radius <= 0.01 {
        return vec![(shadow.offset.0 as f64, shadow.offset.1 as f64, shadow.color)];
    }

    let sigma = (radius * 0.5).max(0.5);
    let step = (radius / 2.0).max(0.5);
    let mut samples = Vec::with_capacity(25);
    let mut weight_total = 0.0;
    for iy in -2..=2 {
        for ix in -2..=2 {
            let dx = ix as f64 * step;
            let dy = iy as f64 * step;
            let weight = (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp();
            samples.push((dx, dy, weight));
            weight_total += weight;
        }
    }
    samples
        .into_iter()
        .map(|(dx, dy, weight)| {
            let mut color = shadow.color;
            color.a = ((color.a as f64 * weight / weight_total).round() as u8).max(1);
            (
                shadow.offset.0 as f64 + dx,
                shadow.offset.1 as f64 + dy,
                color,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoration_styles_produce_distinct_geometry() {
        let solid = decoration_primitives(TextDecorationStyle::Solid, 0.0, 40.0, 8.0, 2.0);
        let double = decoration_primitives(TextDecorationStyle::Double, 0.0, 40.0, 8.0, 2.0);
        let dotted = decoration_primitives(TextDecorationStyle::Dotted, 0.0, 40.0, 8.0, 2.0);
        let dashed = decoration_primitives(TextDecorationStyle::Dashed, 0.0, 40.0, 8.0, 2.0);
        let wavy = decoration_primitives(TextDecorationStyle::Wavy, 0.0, 40.0, 8.0, 2.0);
        assert_eq!(solid.len(), 1);
        assert_eq!(double.len(), 2);
        assert!(dotted.len() > dashed.len());
        assert!(matches!(
            wavy.as_slice(),
            [DecorationPrimitive::Wave { .. }]
        ));
    }

    #[test]
    fn blurred_shadow_uses_bounded_normalized_kernel() {
        let samples = shadow_samples(TextShadow {
            color: Color {
                r: 10,
                g: 20,
                b: 30,
                a: 200,
            },
            offset: (3.0, 4.0),
            blur_radius: 8.0,
        });
        assert_eq!(samples.len(), 25);
        let alpha: u32 = samples.iter().map(|(_, _, color)| color.a as u32).sum();
        assert!((alpha as i32 - 200).abs() <= 12);
        assert!(samples.iter().any(|(x, y, _)| *x != 3.0 || *y != 4.0));
    }
}
