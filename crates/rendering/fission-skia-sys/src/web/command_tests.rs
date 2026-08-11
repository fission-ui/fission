use crate::{
    Affine, BoxShadow, Color, FillRule, GradientStop, ImageSampling, LineCap, LineJoin, Paint,
    Path, PathCommand, Point, Rect, Stroke,
};

use super::*;

fn handle(slot: u32) -> ResourceHandle {
    ResourceHandle {
        slot,
        generation: 2,
    }
}

fn gradient() -> Paint {
    Paint::LinearGradient {
        start: Point::new(1.0, 2.0),
        end: Point::new(20.0, 30.0),
        stops: vec![
            GradientStop::new(1.0, Color::rgba(0.0, 0.5, 1.0, 1.0)),
            GradientStop::new(0.0, Color::rgba(1.0, 0.0, 0.25, 0.75)),
        ],
    }
}

fn stroke() -> Stroke {
    Stroke {
        paint: gradient(),
        width: 3.0,
        dash_array: Some(vec![2.0, 4.0, 6.0]),
        line_cap: LineCap::Round,
        line_join: LineJoin::Bevel,
    }
}

fn path() -> Path {
    Path::new(
        FillRule::EvenOdd,
        vec![
            PathCommand::MoveTo { x: 1.0, y: 2.0 },
            PathCommand::LineTo { x: 3.0, y: 4.0 },
            PathCommand::QuadTo {
                cx: 5.0,
                cy: 6.0,
                x: 7.0,
                y: 8.0,
            },
            PathCommand::CubicTo {
                c1x: 9.0,
                c1y: 10.0,
                c2x: 11.0,
                c2y: 12.0,
                x: 13.0,
                y: 14.0,
            },
            PathCommand::Close,
        ],
    )
}

#[test]
fn every_command_round_trips_with_canonical_normalization() {
    let commands = vec![
        WebCommand::Clear(Color::rgba(0.1, 0.2, 0.3, 1.0)),
        WebCommand::Save,
        WebCommand::OpacityLayer {
            bounds: Rect::new(0.0, 0.0, 100.0, 50.0),
            alpha: 0.75,
        },
        WebCommand::ClipRect(Rect::new(1.0, 2.0, 30.0, 40.0)),
        WebCommand::ClipRoundedRect {
            rect: Rect::new(2.0, 3.0, 20.0, 10.0),
            radius: 4.0,
        },
        WebCommand::ConcatAffine(Affine {
            scale_x: 2.0,
            skew_x: 0.25,
            translate_x: 5.0,
            skew_y: -0.5,
            scale_y: 3.0,
            translate_y: 6.0,
        }),
        WebCommand::FillRect {
            rect: Rect::new(4.0, 5.0, 10.0, 11.0),
            radius: 2.0,
            paint: gradient(),
        },
        WebCommand::StrokeRect {
            rect: Rect::new(6.0, 7.0, 12.0, 13.0),
            radius: 3.0,
            stroke: stroke(),
        },
        WebCommand::FillPath {
            path: path(),
            paint: Paint::solid(Color::rgba(1.0, 0.0, 0.0, 1.0)),
        },
        WebCommand::StrokePath {
            path: path(),
            stroke: stroke(),
        },
        WebCommand::BoxShadow {
            rect: Rect::new(8.0, 9.0, 14.0, 15.0),
            radius: 5.0,
            shadow: BoxShadow {
                color: Color::rgba(0.0, 0.0, 0.0, 0.5),
                blur_radius: 7.0,
                spread_radius: -1.0,
                offset: Point::new(2.0, 3.0),
                inset: true,
            },
        },
        WebCommand::DrawParagraph {
            paragraph: handle(1),
            origin: Point::new(12.0, 13.0),
            scale_factor: 2.0,
        },
        WebCommand::DrawImage {
            image: handle(2),
            source: Rect::new(0.0, 0.0, 16.0, 16.0),
            destination: Rect::new(20.0, 30.0, 32.0, 32.0),
            sampling: ImageSampling::Linear,
        },
        WebCommand::DrawImageFit {
            image: handle(2),
            target: Rect::new(2.0, 4.0, 80.0, 60.0),
            fit: WebImageFit::Cover,
            alignment: WebImageAlignment::BottomEnd,
            sampling: ImageSampling::Linear,
        },
        WebCommand::BackdropBlur {
            bounds: Rect::new(0.0, 0.0, 50.0, 50.0),
            corner_radius: 6.0,
            sigma: 8.0,
        },
        WebCommand::DrawSvg {
            document: handle(3),
            destination: Rect::new(1.0, 1.0, 80.0, 60.0),
        },
        WebCommand::DrawPicture { picture: handle(4) },
        WebCommand::Restore,
        WebCommand::Restore,
    ];

    let encoded = encode_commands(&commands).unwrap();
    let decoded = decode_commands(&encoded).unwrap();
    assert_eq!(decoded.len(), commands.len());
    assert_eq!(encode_commands(&decoded).unwrap(), encoded);
    let WebCommand::StrokeRect { stroke, .. } = &decoded[7] else {
        panic!("expected stroke rectangle");
    };
    assert_eq!(
        stroke.dash_array.as_deref(),
        Some(&[2.0, 4.0, 6.0, 2.0, 4.0, 6.0][..])
    );
    let WebCommand::FillRect {
        paint: Paint::LinearGradient { stops, .. },
        ..
    } = &decoded[6]
    else {
        panic!("expected linear-gradient rectangle");
    };
    assert_eq!(stops[0].offset, 0.0);
    assert_eq!(stops[1].offset, 1.0);
}

#[test]
fn clear_and_save_have_stable_golden_encoding() {
    let commands = [
        WebCommand::Clear(Color::rgba(1.0, 0.5, 0.25, 1.0)),
        WebCommand::Save,
        WebCommand::Restore,
    ];
    let expected = [
        0x46, 0x53, 0x43, 0x4d, 0x01, 0x00, 0x00, 0x00, 0x38, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00,
        0x00, 0x01, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00,
        0x00, 0x3f, 0x00, 0x00, 0x80, 0x3e, 0x00, 0x00, 0x80, 0x3f, 0x02, 0x00, 0x00, 0x00, 0x08,
        0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00,
    ];
    assert_eq!(encode_commands(&commands).unwrap(), expected);
    assert_eq!(decode_commands(&expected).unwrap(), commands);
}

#[test]
fn malformed_or_unbalanced_streams_fail_before_execution() {
    assert_eq!(
        encode_commands(&[WebCommand::Restore]),
        Err(CommandStreamError::UnbalancedRestore)
    );
    assert_eq!(
        encode_commands(&[WebCommand::Save]),
        Err(CommandStreamError::UnclosedSaveDepth(1))
    );

    let mut unknown = encode_commands(&[]).unwrap();
    unknown[8..12].copy_from_slice(&24_u32.to_le_bytes());
    unknown[12..16].copy_from_slice(&1_u32.to_le_bytes());
    unknown.extend_from_slice(&99_u16.to_le_bytes());
    unknown.extend_from_slice(&0_u16.to_le_bytes());
    unknown.extend_from_slice(&8_u32.to_le_bytes());
    assert_eq!(
        decode_commands(&unknown),
        Err(CommandStreamError::UnknownCommand(99))
    );

    let mut trailing = encode_commands(&[]).unwrap();
    trailing.push(0);
    assert_eq!(
        decode_commands(&trailing),
        Err(CommandStreamError::LengthMismatch)
    );
}

#[test]
fn invalid_resource_geometry_and_numeric_values_are_rejected() {
    assert!(matches!(
        encode_commands(&[WebCommand::DrawImage {
            image: ResourceHandle {
                slot: 0,
                generation: 1
            },
            source: Rect::new(0.0, 0.0, 1.0, 1.0),
            destination: Rect::new(0.0, 0.0, 1.0, 1.0),
            sampling: ImageSampling::Nearest,
        }]),
        Err(CommandStreamError::InvalidValue("resource handle"))
    ));
    assert!(matches!(
        encode_commands(&[WebCommand::ClipRect(Rect::new(f32::NAN, 0.0, 1.0, 1.0,))]),
        Err(CommandStreamError::InvalidValue("rectangle"))
    ));
}

#[test]
fn noncanonical_paint_and_dash_encodings_are_rejected() {
    let mut solid_alias = encode_commands(&[WebCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        radius: 0.0,
        paint: Paint::solid(Color::rgba(1.0, 0.0, 0.0, 1.0)),
    }])
    .unwrap();
    // Stream + entry + rectangle + radius + paint tag/reserved + solid color.
    solid_alias[64..68].copy_from_slice(&1.0_f32.to_le_bytes());
    assert_eq!(
        decode_commands(&solid_alias),
        Err(CommandStreamError::InvalidValue("paint kind or payload"))
    );

    let mut unordered_stops = encode_commands(&[WebCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        radius: 0.0,
        paint: gradient(),
    }])
    .unwrap();
    // The encoder sorts the two stops to 0.0 and 1.0. Alias the second to 0.0.
    unordered_stops[108..112].copy_from_slice(&0.0_f32.to_le_bytes());
    assert_eq!(
        decode_commands(&unordered_stops),
        Err(CommandStreamError::InvalidValue("gradient stop ordering"))
    );

    let mut zero_dashes = encode_commands(&[WebCommand::StrokeRect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        radius: 0.0,
        stroke: stroke(),
    }])
    .unwrap();
    // Stroke header and the two-stop gradient end at byte 140 for this fixture.
    zero_dashes[140..164].fill(0);
    assert_eq!(
        decode_commands(&zero_dashes),
        Err(CommandStreamError::InvalidValue("dash intervals"))
    );
}
