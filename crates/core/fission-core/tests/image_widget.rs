use fission_core::env::{Env, RuntimeState};
use fission_core::internal::InternalLoweringCx;
use fission_core::ui::Image;
use fission_ir::op::{ImageAlignment, ImageCachePolicy, ImageFit, ImageSource, Op, PaintOp};
use fission_ir::CoreIR;

fn lower_image(image: Image) -> CoreIR {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&image.into(), &mut cx);
    cx.ir.root = Some(root);
    cx.ir
}

fn draw_image_op(ir: &CoreIR) -> Option<&PaintOp> {
    ir.nodes.values().find_map(|node| match &node.op {
        Op::Paint(op @ PaintOp::DrawImage { .. }) => Some(op),
        _ => None,
    })
}

#[test]
fn network_image_lowers_to_typed_draw_image_request() {
    let ir = lower_image(
        Image::network("https://cdn.example.com/product.webp")
            .header("Accept", "image/webp")
            .cache_policy(ImageCachePolicy::Disk)
            .cache_size(320, 180)
            .semantic_label("Product photo")
            .size(160.0, 90.0)
            .fit(ImageFit::Cover)
            .alignment(ImageAlignment::TopStart),
    );

    let Some(PaintOp::DrawImage {
        request,
        fit,
        alignment,
    }) = draw_image_op(&ir)
    else {
        panic!("expected DrawImage paint op");
    };

    assert_eq!(*fit, ImageFit::Cover);
    assert_eq!(*alignment, ImageAlignment::TopStart);
    assert_eq!(request.cache_width, Some(320));
    assert_eq!(request.cache_height, Some(180));
    assert_eq!(request.semantic_label.as_deref(), Some("Product photo"));

    let ImageSource::Network {
        url,
        headers,
        cache_policy,
    } = &request.source
    else {
        panic!("expected network image source");
    };
    assert_eq!(url, "https://cdn.example.com/product.webp");
    assert_eq!(*cache_policy, ImageCachePolicy::Disk);
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].name, "Accept");
    assert_eq!(headers[0].value, "image/webp");
}

#[test]
fn image_lowering_keeps_sized_layout_parent_and_draw_image_child() {
    let ir = lower_image(
        Image::network("https://cdn.example.com/product.webp")
            .size(88.0, 44.0)
            .fit(ImageFit::Contain),
    );

    let root_id = ir.root.expect("image root");
    let root = ir.nodes.get(&root_id).expect("root node");
    match &root.op {
        Op::Layout(fission_ir::op::LayoutOp::Box {
            width,
            height,
            flex_shrink,
            ..
        }) => {
            assert_eq!(*width, Some(88.0));
            assert_eq!(*height, Some(44.0));
            assert_eq!(*flex_shrink, 1.0);
        }
        other => panic!("expected image root to be a sized layout box, got {other:?}"),
    }
    assert_eq!(root.children.len(), 1);

    let paint = ir.nodes.get(&root.children[0]).expect("image paint child");
    assert!(matches!(
        &paint.op,
        Op::Paint(PaintOp::DrawImage {
            request,
            fit: ImageFit::Contain,
            ..
        }) if request.source.network_url() == Some("https://cdn.example.com/product.webp")
    ));
}

#[test]
fn svg_text_lowers_to_draw_svg() {
    let ir = lower_image(Image::svg_text("<svg viewBox=\"0 0 10 10\"></svg>").size(10.0, 10.0));

    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Paint(PaintOp::DrawSvg { content, .. }) if content.contains("<svg")
    )));
}

#[test]
fn asset_and_network_constructors_create_typed_sources() {
    assert!(matches!(
        Image::network("https://example.com/a.png").request.source,
        ImageSource::Network { .. }
    ));
    assert!(matches!(
        Image::asset("assets/a.png").request.source,
        ImageSource::Asset { .. }
    ));
}
