//! An interactive viewport lays its content out at its natural size, at least as large as the view.

use fission_ir::{
    LayoutOp as IrLayoutOp, ViewportBoundary, ViewportClip, ViewportPanAxis, ViewportTransform,
    ViewportZoomPolicy, WidgetId,
};
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutSize};

const VIEW: LayoutSize = LayoutSize {
    width: 700.0,
    height: 350.0,
};

fn node(
    id: WidgetId,
    parent_id: Option<WidgetId>,
    op: IrLayoutOp,
    children_ids: Vec<WidgetId>,
    size: (Option<f32>, Option<f32>),
) -> LayoutInputNode {
    LayoutInputNode {
        id,
        parent_id,
        op,
        children_ids,
        debug_name: format!("{id:?}"),
        width: size.0,
        height: size.1,
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    }
}

fn viewport(child: WidgetId) -> LayoutInputNode {
    node(
        WidgetId::from_u128(1),
        None,
        IrLayoutOp::InteractiveViewport {
            initial_transform: ViewportTransform::IDENTITY,
            controlled_transform: None,
            pan_axis: ViewportPanAxis::Both,
            boundary: ViewportBoundary::Unbounded,
            clip: ViewportClip::HardEdge,
            zoom_policy: ViewportZoomPolicy::WheelWithModifier,
            min_scale: 0.25,
            max_scale: 4.0,
            friction: 0.0,
            on_interaction_start: None,
            on_interaction_update: None,
            on_interaction_end: None,
        },
        vec![child],
        (None, None),
    )
}

fn sized_box(id: WidgetId, parent: WidgetId, width: f32, height: f32) -> LayoutInputNode {
    node(
        id,
        Some(parent),
        IrLayoutOp::Box {
            width: Some(width),
            height: Some(height),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        },
        Vec::new(),
        (Some(width), Some(height)),
    )
}

#[test]
fn viewport_content_keeps_a_size_larger_than_the_view() {
    let root = WidgetId::from_u128(1);
    let scene = WidgetId::from_u128(2);
    let nodes = vec![viewport(scene), sized_box(scene, root, 620.0, 540.0)];
    let mut engine = LayoutEngine::new();
    engine.update(&nodes);
    let snapshot = engine
        .compute_layout(&nodes, root, VIEW, &|_| 0.0)
        .expect("layout");

    let view = snapshot.get_node_rect(root).expect("viewport rect");
    let content = snapshot.get_node_rect(scene).expect("scene rect");
    assert_eq!((view.width(), view.height()), (VIEW.width, VIEW.height));
    assert!(
        content.height() >= 540.0 && content.width() >= 620.0,
        "content taller than the view keeps its size instead of being squeezed, got {content:?}"
    );
}

#[test]
fn viewport_layers_of_positioned_children_still_cover_the_view() {
    let root = WidgetId::from_u128(1);
    let world = WidgetId::from_u128(2);
    let tile = WidgetId::from_u128(3);
    let tile_box = WidgetId::from_u128(4);
    let nodes = vec![
        viewport(world),
        node(
            world,
            Some(root),
            IrLayoutOp::ZStack,
            vec![tile],
            (None, None),
        ),
        node(
            tile,
            Some(world),
            IrLayoutOp::Positioned {
                top: Some(10.0),
                left: Some(10.0),
                bottom: None,
                right: None,
                width: Some(50.0),
                height: Some(50.0),
            },
            vec![tile_box],
            (Some(50.0), Some(50.0)),
        ),
        sized_box(tile_box, tile, 50.0, 50.0),
    ];
    let mut engine = LayoutEngine::new();
    engine.update(&nodes);
    let snapshot = engine
        .compute_layout(&nodes, root, VIEW, &|_| 0.0)
        .expect("layout");

    let layer = snapshot.get_node_rect(world).expect("world rect");
    assert!(
        layer.width() >= VIEW.width && layer.height() >= VIEW.height,
        "a layer of positioned children covers the whole view, got {layer:?}"
    );
}
