use super::GalleryBuildExt;
use crate::state::GalleryState;
use fission::charts::{Axis, BarSeries, Chart, DataValue, Dataset, Encode, Legend, LineSeries};
use fission::core::op::Color;
use fission::core::ui::Widget;
use fission::core::{BuildCtxHandle, ViewHandle};
use fission::three_d::{Point3D, Primitive3D, Scene3D};

pub(super) fn build_chart(
    chart: usize,
    ctx: BuildCtxHandle<GalleryState>,
    view: ViewHandle<GalleryState>,
    content_width: f32,
    s: f32,
) -> Widget {
    match chart {
        0 => dataset_demo(view, s).build_in_gallery(ctx, view, content_width),
        1 => scene3d_demo().build_in_gallery(ctx, view, content_width),
        2 => bar3d_scene(s).build_in_gallery(ctx, view, content_width),
        3 => scatter3d_scene(s).build_in_gallery(ctx, view, content_width),
        4 => surface3d_scene(s).build_in_gallery(ctx, view, content_width),
        5 => line3d_scene(s).build_in_gallery(ctx, view, content_width),
        6 => point_cloud_scene(s).build_in_gallery(ctx, view, content_width),
        7 => globe_scene(s).build_in_gallery(ctx, view, content_width),
        8 => graph3d_scene(s).build_in_gallery(ctx, view, content_width),
        9 => terrain_scene(s).build_in_gallery(ctx, view, content_width),
        _ => unreachable!("chart catalog and 3D/dataset builder are out of sync"),
    }
}

pub(crate) fn dataset_demo(view: ViewHandle<GalleryState>, s: f32) -> Chart {
    Chart::new()
        .title("Dataset Engine: Encoded Line & Bar")
        .dataset(
            Dataset::new()
                .dimensions(vec![
                    "product".into(),
                    "2015".into(),
                    "2016".into(),
                    "2017".into(),
                ])
                .source(vec![
                    vec![
                        DataValue::String("Matcha Latte".into()),
                        DataValue::Number(43.3 * s),
                        DataValue::Number(85.8 * s),
                        DataValue::Number(93.7 * s),
                    ],
                    vec![
                        DataValue::String("Milk Tea".into()),
                        DataValue::Number(83.1 * s),
                        DataValue::Number(73.4 * s),
                        DataValue::Number(55.1 * s),
                    ],
                    vec![
                        DataValue::String("Cheese Cocoa".into()),
                        DataValue::Number(86.4 * s),
                        DataValue::Number(65.2 * s),
                        DataValue::Number(82.5 * s),
                    ],
                    vec![
                        DataValue::String("Walnut Brownie".into()),
                        DataValue::Number(72.4 * s),
                        DataValue::Number(53.9 * s),
                        DataValue::Number(39.1 * s),
                    ],
                ]),
        )
        .x_axis(Axis::category(vec![
            "Matcha Latte",
            "Milk Tea",
            "Cheese Cocoa",
            "Walnut Brownie",
        ]))
        .y_axis(Axis::value())
        .legend(Legend::top_right())
        .series(vec![
            BarSeries::new("2015")
                .encode(Encode::new().x("product").y("2015"))
                .color(Color {
                    r: 84,
                    g: 112,
                    b: 198,
                    a: 255,
                })
                .into(),
            BarSeries::new("2016")
                .encode(Encode::new().x("product").y("2016"))
                .color(Color {
                    r: 145,
                    g: 204,
                    b: 117,
                    a: 255,
                })
                .into(),
            LineSeries::new("2017")
                .encode(Encode::new().x("product").y("2017"))
                .color(Color {
                    r: 250,
                    g: 204,
                    b: 20,
                    a: 255,
                })
                .smooth(view.state().smooth)
                .into(),
        ])
}

pub(crate) fn scene3d_demo() -> Scene3D {
    Scene3D::new()
        .add_primitive(Primitive3D::Cube {
            center: Point3D::new(0.0, 0.0, 0.0),
            size: 2.0,
            color: Color::RED,
        })
        .add_primitive(Primitive3D::Sphere {
            center: Point3D::new(3.0, 3.0, 3.0),
            radius: 1.5,
            color: Color::BLUE,
        })
}

pub(crate) fn bar3d_scene(s: f32) -> Scene3D {
    let values = [1.0, 2.4, 1.8, 3.2, 2.8, 1.4, 2.2, 3.6, 2.0];
    values
        .iter()
        .enumerate()
        .fold(Scene3D::new(), |scene, (idx, value)| {
            let col = (idx % 3) as f32;
            let row = (idx / 3) as f32;
            let height = value * s;
            scene.add_primitive(cuboid(
                Point3D::new(col * 1.35 - 1.35, height / 2.0 - 1.2, row * 1.35 - 1.35),
                0.52,
                height.max(0.25),
                0.52,
                Color {
                    r: 84,
                    g: 112,
                    b: 198,
                    a: 255,
                },
            ))
        })
}

pub(crate) fn scatter3d_scene(s: f32) -> Scene3D {
    let points = [
        (-1.8, -0.4, -1.4, 0.28),
        (-1.1, 0.8, -0.6, 0.38),
        (-0.2, 0.2, 0.5, 0.32),
        (0.7, 1.2, -0.1, 0.46),
        (1.4, -0.1, 1.2, 0.34),
        (2.0, 0.7, 0.4, 0.42),
    ];
    points
        .iter()
        .fold(Scene3D::new(), |scene, (x, y, z, radius)| {
            scene.add_primitive(Primitive3D::Sphere {
                center: Point3D::new(x * s, y * s, z * s),
                radius: radius * s.max(0.5),
                color: Color {
                    r: 20,
                    g: 184,
                    b: 166,
                    a: 255,
                },
            })
        })
}

pub(crate) fn surface3d_scene(s: f32) -> Scene3D {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let n = 6u32;
    for z in 0..n {
        for x in 0..n {
            let xf = x as f32 / (n - 1) as f32 * 4.0 - 2.0;
            let zf = z as f32 / (n - 1) as f32 * 4.0 - 2.0;
            let y = ((xf * 1.4).sin() + (zf * 1.1).cos()) * 0.35 * s;
            vertices.push(Point3D::new(xf, y, zf));
        }
    }
    for z in 0..(n - 1) {
        for x in 0..(n - 1) {
            let a = z * n + x;
            let b = a + 1;
            let c = a + n;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    Scene3D::new().add_primitive(Primitive3D::Mesh {
        vertices,
        indices,
        color: Color {
            r: 59,
            g: 130,
            b: 246,
            a: 255,
        },
    })
}

pub(crate) fn line3d_scene(s: f32) -> Scene3D {
    let points: Vec<Point3D> = (0..18)
        .map(|idx| {
            let t = idx as f32 / 17.0 * std::f32::consts::TAU * 1.35;
            Point3D::new(
                (t.cos() * 1.6) * s,
                (idx as f32 / 17.0 * 2.8 - 1.4) * s,
                (t.sin() * 1.6) * s,
            )
        })
        .collect();
    let mut scene = Scene3D::new();
    for pair in points.windows(2) {
        scene = scene.add_primitive(segment_prism(
            pair[0].clone(),
            pair[1].clone(),
            0.045,
            Color {
                r: 250,
                g: 204,
                b: 21,
                a: 255,
            },
        ));
    }
    points.into_iter().fold(scene, |scene, point| {
        scene.add_primitive(Primitive3D::Sphere {
            center: point,
            radius: 0.16 * s.max(0.6),
            color: Color {
                r: 250,
                g: 204,
                b: 21,
                a: 255,
            },
        })
    })
}

pub(crate) fn point_cloud_scene(s: f32) -> Scene3D {
    (0..42).fold(Scene3D::new(), |scene, idx| {
        let i = idx as f32;
        let x = ((i * 12.9898).sin() * 2.0).clamp(-2.0, 2.0);
        let y = ((i * 78.233).cos() * 1.5).clamp(-1.5, 1.5);
        let z = (((i + 4.0) * 37.719).sin() * 2.0).clamp(-2.0, 2.0);
        scene.add_primitive(Primitive3D::Sphere {
            center: Point3D::new(x * s, y * s, z * s),
            radius: 0.09 + (idx % 5) as f32 * 0.018,
            color: Color {
                r: 20,
                g: 184,
                b: 166,
                a: 255,
            },
        })
    })
}

pub(crate) fn globe_scene(s: f32) -> Scene3D {
    Scene3D::new()
        .add_primitive(Primitive3D::Sphere {
            center: Point3D::new(0.0, 0.0, 0.0),
            radius: 1.85 * s.max(0.8),
            color: Color {
                r: 59,
                g: 130,
                b: 246,
                a: 255,
            },
        })
        .add_primitive(Primitive3D::Sphere {
            center: Point3D::new(-1.1 * s, 0.72 * s, -1.34 * s),
            radius: 0.24 * s.max(0.7),
            color: Color {
                r: 250,
                g: 204,
                b: 21,
                a: 255,
            },
        })
        .add_primitive(Primitive3D::Sphere {
            center: Point3D::new(0.48 * s, 0.94 * s, -1.52 * s),
            radius: 0.21 * s.max(0.7),
            color: Color {
                r: 239,
                g: 68,
                b: 68,
                a: 255,
            },
        })
        .add_primitive(Primitive3D::Sphere {
            center: Point3D::new(-1.18 * s, -0.42 * s, -1.32 * s),
            radius: 0.19 * s.max(0.7),
            color: Color {
                r: 20,
                g: 184,
                b: 166,
                a: 255,
            },
        })
}

pub(crate) fn graph3d_scene(s: f32) -> Scene3D {
    let nodes = [
        (-1.4, -0.2, -0.8, 0.34),
        (0.0, 0.9, 0.0, 0.48),
        (1.4, -0.1, -0.7, 0.32),
        (-0.2, -1.0, 1.0, 0.38),
        (1.0, 0.6, 1.2, 0.26),
    ];
    let points: Vec<Point3D> = nodes
        .iter()
        .map(|(x, y, z, _)| Point3D::new(x * s, y * s, z * s))
        .collect();
    let mut scene = Scene3D::new();
    for (from, to) in [(0, 1), (1, 2), (1, 3), (2, 4), (3, 4)] {
        scene = scene.add_primitive(segment_prism(
            points[from].clone(),
            points[to].clone(),
            0.035,
            Color {
                r: 226,
                g: 232,
                b: 240,
                a: 255,
            },
        ));
    }
    nodes.iter().fold(scene, |scene, (x, y, z, radius)| {
        scene.add_primitive(Primitive3D::Sphere {
            center: Point3D::new(x * s, y * s, z * s),
            radius: radius * s.max(0.7),
            color: Color {
                r: 145,
                g: 204,
                b: 117,
                a: 255,
            },
        })
    })
}

pub(crate) fn terrain_scene(s: f32) -> Scene3D {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let n = 8u32;
    for z in 0..n {
        for x in 0..n {
            let xf = x as f32 / (n - 1) as f32 * 4.4 - 2.2;
            let zf = z as f32 / (n - 1) as f32 * 4.4 - 2.2;
            let ridge = (-(xf * xf + zf * zf) * 0.18).exp();
            let y = (ridge * 1.2 + (xf * 1.7).sin() * 0.18 + (zf * 1.2).cos() * 0.16) * s;
            vertices.push(Point3D::new(xf, y - 0.7, zf));
        }
    }
    for z in 0..(n - 1) {
        for x in 0..(n - 1) {
            let a = z * n + x;
            let b = a + 1;
            let c = a + n;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    Scene3D::new().add_primitive(Primitive3D::Mesh {
        vertices,
        indices,
        color: Color {
            r: 34,
            g: 197,
            b: 94,
            a: 255,
        },
    })
}

fn cuboid(center: Point3D, width: f32, height: f32, depth: f32, color: Color) -> Primitive3D {
    let hx = width / 2.0;
    let hy = height / 2.0;
    let hz = depth / 2.0;
    let vertices = vec![
        Point3D::new(center.x - hx, center.y - hy, center.z - hz),
        Point3D::new(center.x + hx, center.y - hy, center.z - hz),
        Point3D::new(center.x + hx, center.y + hy, center.z - hz),
        Point3D::new(center.x - hx, center.y + hy, center.z - hz),
        Point3D::new(center.x - hx, center.y - hy, center.z + hz),
        Point3D::new(center.x + hx, center.y - hy, center.z + hz),
        Point3D::new(center.x + hx, center.y + hy, center.z + hz),
        Point3D::new(center.x - hx, center.y + hy, center.z + hz),
    ];
    let indices = vec![
        0, 1, 2, 0, 2, 3, // front
        1, 5, 6, 1, 6, 2, // right
        5, 4, 7, 5, 7, 6, // back
        4, 0, 3, 4, 3, 7, // left
        3, 2, 6, 3, 6, 7, // top
        4, 5, 1, 4, 1, 0, // bottom
    ];
    Primitive3D::Mesh {
        vertices,
        indices,
        color,
    }
}

fn segment_prism(from: Point3D, to: Point3D, thickness: f32, color: Color) -> Primitive3D {
    let dir = normalize((to.x - from.x, to.y - from.y, to.z - from.z));
    let reference = if dir.1.abs() > 0.92 {
        (1.0, 0.0, 0.0)
    } else {
        (0.0, 1.0, 0.0)
    };
    let side = scale(normalize(cross(dir, reference)), thickness);
    let up = scale(normalize(cross(side, dir)), thickness);
    let corners = [
        add3(&from, add(side, up)),
        add3(&from, add(neg(side), up)),
        add3(&from, add(neg(side), neg(up))),
        add3(&from, add(side, neg(up))),
        add3(&to, add(side, up)),
        add3(&to, add(neg(side), up)),
        add3(&to, add(neg(side), neg(up))),
        add3(&to, add(side, neg(up))),
    ];
    Primitive3D::Mesh {
        vertices: corners.to_vec(),
        indices: vec![
            0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7, 0, 4, 7, 0, 7,
            3, 1, 2, 6, 1, 6, 5,
        ],
        color,
    }
}

fn add3(point: &Point3D, offset: (f32, f32, f32)) -> Point3D {
    Point3D::new(point.x + offset.0, point.y + offset.1, point.z + offset.2)
}

fn cross(a: (f32, f32, f32), b: (f32, f32, f32)) -> (f32, f32, f32) {
    (
        a.1 * b.2 - a.2 * b.1,
        a.2 * b.0 - a.0 * b.2,
        a.0 * b.1 - a.1 * b.0,
    )
}

fn normalize(v: (f32, f32, f32)) -> (f32, f32, f32) {
    let len = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt().max(f32::EPSILON);
    (v.0 / len, v.1 / len, v.2 / len)
}

fn scale(v: (f32, f32, f32), scale: f32) -> (f32, f32, f32) {
    (v.0 * scale, v.1 * scale, v.2 * scale)
}

fn add(a: (f32, f32, f32), b: (f32, f32, f32)) -> (f32, f32, f32) {
    (a.0 + b.0, a.1 + b.1, a.2 + b.2)
}

fn neg(v: (f32, f32, f32)) -> (f32, f32, f32) {
    (-v.0, -v.1, -v.2)
}
