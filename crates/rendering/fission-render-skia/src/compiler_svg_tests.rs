use super::*;

fn color() -> Color {
    Color {
        r: 12,
        g: 34,
        b: 56,
        a: 255,
    }
}

fn svg_scene(
    content: &str,
    fill: Option<Fill>,
    stroke: Option<Stroke>,
    bounds: LayoutRect,
    node_id: fission_ir::WidgetId,
) -> RenderScene {
    let mut list = DisplayList::new(bounds);
    list.push(DisplayOp::DrawSvg {
        content: content.into(),
        fill,
        stroke,
        bounds,
        node_id: Some(node_id),
    });
    RenderScene::from_display_list(list)
}

#[test]
fn fission_paint_svg_uses_view_box_transform_and_existing_paint_commands() {
    let bounds = LayoutRect::new(10.0, 20.0, 100.0, 100.0);
    let scene = svg_scene(
        "<svg viewBox='0 0 200 100'><path d='M0 0L200 0L200 100Z'/></svg>",
        Some(Fill::LinearGradient {
            start: (0.0, 0.0),
            end: (1.0, 0.0),
            stops: vec![
                (0.0, color()),
                (
                    1.0,
                    Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                ),
            ],
        }),
        Some(Stroke {
            fill: Fill::Solid(color()),
            width: 2.0,
            dash_array: Some(vec![1.0, 2.0]),
            line_cap: LineCap::Round,
            line_join: LineJoin::Bevel,
        }),
        bounds,
        fission_ir::WidgetId::explicit("svg.override"),
    );

    let compiled = compile_scene(&scene, 2.0, color()).unwrap();

    assert_eq!(compiled.source_operations, 1);
    assert_eq!(compiled.frame.commands.len(), 6);
    assert!(matches!(compiled.frame.commands[1], RasterCommand::Save));
    assert_eq!(
        compiled.frame.commands[2],
        RasterCommand::ConcatAffine(RasterAffine {
            scale_x: 0.5,
            skew_x: 0.0,
            translate_x: 20.0,
            skew_y: 0.0,
            scale_y: 0.5,
            translate_y: 90.0,
        })
    );
    let RasterCommand::FillPath { path, paint } = &compiled.frame.commands[3] else {
        panic!("expected SVG fill path")
    };
    assert_eq!(
        path.commands[1],
        RasterPathCommand::LineTo { x: 400.0, y: 0.0 }
    );
    assert!(matches!(
        paint,
        RasterPaint::LinearGradient {
            start: RasterPoint { x: 0.0, y: 0.0 },
            end: RasterPoint { x: 400.0, y: 0.0 },
            ..
        }
    ));
    let RasterCommand::StrokePath { stroke, .. } = &compiled.frame.commands[4] else {
        panic!("expected SVG stroke path")
    };
    assert_eq!(stroke.width, 4.0);
    assert_eq!(stroke.dash_array.as_deref(), Some(&[2.0, 4.0][..]));
    assert_eq!(stroke.line_cap, RasterLineCap::Round);
    assert_eq!(stroke.line_join, RasterLineJoin::Bevel);
    assert!(matches!(compiled.frame.commands[5], RasterCommand::Restore));
}

#[test]
fn invalid_override_geometry_reports_stable_code_and_node_provenance() {
    let node_id = fission_ir::WidgetId::explicit("svg.invalid");
    let scene = svg_scene(
        "<svg><circle cx='2' cy='2' r='2'/></svg>",
        Some(Fill::Solid(color())),
        None,
        LayoutRect::new(0.0, 0.0, 10.0, 10.0),
        node_id,
    );

    let error = compile_scene(&scene, 1.0, color()).unwrap_err();

    assert!(matches!(
        error.kind,
        CompileErrorKind::Svg(SvgError::GeometryParse { .. })
    ));
    assert_eq!(error.diagnostic_code(), "skia-svg-geometry-invalid");
    assert_eq!(error.diagnostic_category(), DiagnosticCategory::Resource);
    assert_eq!(error.provenance.node_id, Some(node_id));
    assert_eq!(error.provenance.operation_index(), Some(0));
}

#[cfg(feature = "test-shim")]
#[test]
fn document_paint_svg_is_parsed_and_pinned_in_one_device_scaled_command() {
    let content = "<svg viewBox='0 0 8 4'><rect width='8' height='4' fill='#123456'/></svg>";
    let scene = svg_scene(
        content,
        None,
        None,
        LayoutRect::new(3.0, 5.0, 8.0, 4.0),
        fission_ir::WidgetId::explicit("svg.document"),
    );

    let compiled = compile_scene(&scene, 2.0, color()).unwrap();

    assert_eq!(compiled.frame.commands.len(), 2);
    let RasterCommand::DrawSvg {
        document,
        destination,
    } = &compiled.frame.commands[1]
    else {
        panic!("expected retained SVG document")
    };
    assert_eq!(document.source_bytes_len(), content.len());
    assert_eq!(
        *destination,
        RasterRect {
            left: 6.0,
            top: 10.0,
            right: 22.0,
            bottom: 18.0,
        }
    );
}
