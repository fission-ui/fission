#![cfg(feature = "test-shim")]

use fission_skia_sys::ffi;
use fission_skia_sys::{
    Color, Context, Engine, ErrorKind, Frame, FrameOp, ParagraphCapabilities, ParagraphColor,
    ParagraphDrawData, ParagraphEngine, ParagraphFontCatalog, ParagraphFontFace,
    ParagraphInlineObject, ParagraphPreedit, ParagraphRange, ParagraphRequest,
    ParagraphTextDirection, ParagraphTextStyleRun, PixelRect, Point, RasterSurface,
};

fn rich_request() -> ParagraphRequest {
    let text = "A😀e\u{301} אב\u{fffc}Z";
    assert_eq!(text.len(), 17);
    let mut request = ParagraphRequest::new(
        text,
        vec![ParagraphTextStyleRun::new(
            ParagraphRange::new(0, text.len() as u64),
            16.0,
            ParagraphColor::rgba(10, 20, 30, 255),
        )],
    );
    request.paragraph_style.text_direction = ParagraphTextDirection::Auto;
    request.width_constraint = Some(300.0);
    request.selection = Some(ParagraphRange::new(0, text.len() as u64));
    request.preedit = Some(ParagraphPreedit {
        range: ParagraphRange::new(5, 8),
        selection: ParagraphRange::new(8, 8),
    });
    request.inline_objects.push(ParagraphInlineObject {
        id: 42,
        range: ParagraphRange::new(13, 16),
        width: 8.0,
        height: 20.0,
        baseline: 15.0,
    });
    request
}

#[test]
fn owned_paragraph_output_preserves_utf8_grapheme_bidi_and_inline_geometry() {
    let engine = ParagraphEngine::new().expect("paragraph test engine");
    assert!(engine
        .capabilities()
        .supports(ParagraphCapabilities::BIDIRECTIONAL_TEXT));
    assert!(engine
        .capabilities()
        .supports(ParagraphCapabilities::INLINE_OBJECTS));
    assert!(engine
        .capabilities()
        .supports(ParagraphCapabilities::UNRESOLVED_GLYPHS));

    let output = engine
        .layout(&rich_request())
        .expect("owned paragraph output");
    assert_eq!(output.index_encoding, ffi::INDEX_UTF8);
    assert_eq!(output.capabilities.bits(), ffi::PARAGRAPH_ALL_CAPABILITIES);
    assert!(output
        .clusters
        .iter()
        .any(|cluster| cluster.range == ffi::TextRange { start: 1, end: 5 }));
    assert!(output
        .clusters
        .iter()
        .any(|cluster| cluster.range == ffi::TextRange { start: 5, end: 8 }));
    assert!(!output.clusters.iter().any(|cluster| {
        cluster.range.start == 6 || cluster.range.end == 6 || cluster.range.start == 7
    }));
    assert!(output.clusters.iter().any(|cluster| {
        cluster.range == ffi::TextRange { start: 9, end: 11 }
            && cluster.direction == ffi::DIRECTION_RTL
    }));
    assert_eq!(output.inline_boxes.len(), 1);
    assert_eq!(output.inline_boxes[0].id, 42);
    assert_eq!(
        output.inline_boxes[0].range,
        ffi::TextRange { start: 13, end: 16 }
    );
    assert!(output
        .carets
        .iter()
        .any(|caret| caret.index == 5 && caret.affinity == ffi::AFFINITY_DOWNSTREAM));
    assert!(output
        .carets
        .iter()
        .any(|caret| caret.index == 8 && caret.affinity == ffi::AFFINITY_UPSTREAM));

    // Access after layout proves the safe wrapper copied every native view
    // array before deterministically destroying the opaque result handle.
    assert_eq!(output.lines[0].range.end, 17);
}

#[test]
fn owned_font_catalog_generation_is_required_and_retired_deterministically() {
    let engine = ParagraphEngine::new().expect("paragraph test engine");
    let catalog = ParagraphFontCatalog::new(&[ParagraphFontFace::new(
        "Inter",
        include_bytes!("../../../core/fission-theme/fonts/Inter/static/Inter_24pt-Regular.ttf")
            .to_vec(),
    )])
    .expect("owned font catalogue");
    let mut request = rich_request();
    request.font_catalog_generation = catalog.generation();
    request.fallback_families.push("Inter".into());
    engine
        .layout(&request)
        .expect("live owned font catalogue participates in layout");

    drop(catalog);
    let error = engine
        .layout(&request)
        .expect_err("retired catalogue generation must not silently fall back");
    assert_eq!(error.kind, ErrorKind::InvalidHandle);
    assert_eq!(error.operation, "paragraph_layout");
    assert_ne!(error.sequence, 0);
}

#[test]
fn safe_font_catalog_rejects_missing_faces_before_ffi() {
    let error = ParagraphFontCatalog::new(&[]).expect_err("empty catalogue must fail");
    assert_eq!(error.kind, ErrorKind::InvalidArgument);
    assert_eq!(error.operation, "ParagraphFontCatalog::new");
}

#[test]
fn retained_paragraph_picture_survives_layout_owner_and_scales_at_playback() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ParagraphDrawData>();

    let paragraph_engine = ParagraphEngine::new().expect("paragraph test engine");
    let retained = paragraph_engine
        .layout_retained(&rich_request())
        .expect("retained paragraph output");
    assert!(retained.draw_data.approximate_bytes() > 0);
    let logical_width = retained.output.size.width;
    let frame = Frame::new([FrameOp::DrawParagraph {
        data: retained.draw_data.clone(),
        origin: Point::new(2.0, 3.0),
        scale_factor: 2.0,
    }]);
    drop(retained);

    let engine = Engine::new().expect("raster engine");
    let context = Context::new_raster(&engine).expect("raster context");
    let mut surface = RasterSurface::new(&context, 48, 48).expect("raster surface");
    surface
        .execute_frame(&Frame::new([FrameOp::Clear(Color::TRANSPARENT)]))
        .expect("clear frame");
    surface.execute_frame(&frame).expect("paragraph frame");
    let pixel = surface
        .read_pixels_rgba8888(Some(PixelRect::new(2, 3, 1, 1)))
        .expect("paragraph pixel");
    assert_eq!(pixel, [10, 20, 30, 255]);
    let scaled_only_pixel = surface
        .read_pixels_rgba8888(Some(PixelRect::new(18, 3, 1, 1)))
        .expect("scaled paragraph pixel");
    assert_eq!(scaled_only_pixel, [10, 20, 30, 255]);
    let outside_scaled_height = surface
        .read_pixels_rgba8888(Some(PixelRect::new(2, 43, 1, 1)))
        .expect("pixel below scaled paragraph");
    assert_eq!(outside_scaled_height, [0, 0, 0, 0]);
    assert_eq!(logical_width, 300.0);
}

#[test]
fn raw_contract_rejects_unknown_flags_and_destroyed_results() {
    let text = b"ok";
    let style = ffi::TextStyleRun {
        struct_size: std::mem::size_of::<ffi::TextStyleRun>() as u32,
        flags: 0,
        range: ffi::TextRange { start: 0, end: 2 },
        font_size: 14.0,
        color: ffi::Rgba8 {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 255,
        },
        font_family: empty_utf8(),
        locale: empty_utf8(),
        font_weight: 400,
        font_slant: ffi::FONT_SLANT_NORMAL,
        line_height: 0.0,
        letter_spacing: 0.0,
        background_color: ffi::Rgba8::default(),
        font_width: 1.0,
        word_spacing: 0.0,
        variations: std::ptr::null(),
        variation_count: 0,
        features: std::ptr::null(),
        feature_count: 0,
    };
    let paragraph_style = ffi::ParagraphStyle {
        struct_size: std::mem::size_of::<ffi::ParagraphStyle>() as u32,
        flags: ffi::PARAGRAPH_STYLE_APPLY_FIRST_ASCENT | ffi::PARAGRAPH_STYLE_APPLY_LAST_DESCENT,
        text_align: ffi::TEXT_ALIGN_LEFT,
        overflow: ffi::TEXT_OVERFLOW_CLIP,
        text_direction: ffi::TEXT_DIRECTION_LTR,
        text_width_basis: ffi::TEXT_WIDTH_BASIS_PARENT,
        max_lines: 0,
        strut_line_height: 0.0,
        reserved: 0,
    };
    let mut request = ffi::ParagraphRequest {
        struct_size: std::mem::size_of::<ffi::ParagraphRequest>() as u32,
        flags: ffi::PARAGRAPH_REQUEST_WRAP,
        text: ffi::Utf8Slice {
            data: text.as_ptr(),
            length: text.len(),
        },
        style_runs: &style,
        style_run_count: 1,
        paragraph_style,
        width_constraint: 0.0,
        reserved: 0,
        locale: empty_utf8(),
        inline_objects: std::ptr::null(),
        inline_object_count: 0,
        selection: ffi::TextRange::default(),
        preedit: ffi::Preedit::default(),
        font_catalog_generation: 0,
        fallback_families: std::ptr::null(),
        fallback_family_count: 0,
    };
    request.flags |= 1 << 31;
    let mut handle = 0;
    let mut error = ffi::Error::default();
    // SAFETY: all pointers are valid; the unknown flag is deliberately invalid.
    let status = unsafe { ffi::fission_skia_paragraph_layout(&request, &mut handle, &mut error) };
    assert_eq!(status, ffi::STATUS_INVALID_ARGUMENT);
    assert_eq!(handle, 0);

    request.flags &= !(1 << 31);
    let mut error = ffi::Error::default();
    // SAFETY: request owns all nested buffers for the synchronous call.
    let status = unsafe { ffi::fission_skia_paragraph_layout(&request, &mut handle, &mut error) };
    assert_eq!(status, ffi::STATUS_OK);
    assert_ne!(handle, 0);
    let mut view = ffi::ParagraphResultView::default();
    // SAFETY: handle is live and view is initialized for writes.
    let status =
        unsafe { ffi::fission_skia_paragraph_result_get_view(handle, &mut view, &mut error) };
    assert_eq!(status, ffi::STATUS_OK);
    assert_eq!(view.index_encoding, ffi::INDEX_UTF8);
    // SAFETY: the live handle is destroyed exactly once.
    assert_eq!(
        unsafe { ffi::fission_skia_paragraph_result_destroy(handle, &mut error) },
        ffi::STATUS_OK
    );
    // SAFETY: use-after-destroy is deliberately checked as a numeric handle error.
    assert_eq!(
        unsafe { ffi::fission_skia_paragraph_result_get_view(handle, &mut view, &mut error) },
        ffi::STATUS_INVALID_HANDLE
    );

    // A raw frame cannot turn a stale paragraph handle into a silent no-op.
    // SAFETY: FrameOp contains only C scalar records and all-zero is a valid
    // starting representation before the required fields are initialized.
    let mut operation: ffi::FrameOp = unsafe { std::mem::zeroed() };
    operation.struct_size = std::mem::size_of::<ffi::FrameOp>() as u32;
    operation.kind = ffi::FRAME_DRAW_PARAGRAPH;
    operation.rect.x = 0.0;
    operation.rect.y = 0.0;
    operation.radius = 1.0;
    operation.path_offset = handle as u32;
    operation.path_count = (handle >> 32) as u32;
    let frame = ffi::Frame {
        struct_size: std::mem::size_of::<ffi::Frame>() as u32,
        reserved: 0,
        operations: &operation,
        operation_count: 1,
        path_commands: std::ptr::null(),
        path_command_count: 0,
        gradient_stops: std::ptr::null(),
        gradient_stop_count: 0,
        dash_intervals: std::ptr::null(),
        dash_interval_count: 0,
    };
    // SAFETY: the frame arrays remain alive; both numeric handles are
    // deliberately stale/invalid and must be diagnosed structurally.
    assert_eq!(
        unsafe { ffi::fission_skia_surface_execute_frame(u64::MAX, &frame, &mut error) },
        ffi::STATUS_INVALID_HANDLE
    );
    assert_eq!(error.code, ffi::STATUS_INVALID_HANDLE);
    assert_eq!(error.operation[0] as u8, b'd');
}

fn empty_utf8() -> ffi::Utf8Slice {
    ffi::Utf8Slice {
        data: std::ptr::null(),
        length: 0,
    }
}
