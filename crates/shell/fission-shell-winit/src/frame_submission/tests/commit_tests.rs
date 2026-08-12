use std::cell::RefCell;

use super::*;

#[test]
fn ganesh_publication_follows_success_and_uses_retained_scroll_clip_geometry_once() {
    let video_id = WidgetId::explicit("commit.video");
    let mut ir = CoreIR::new();
    let slot = add_embed(
        &mut ir,
        WidgetId::derived(3, &[1]),
        video_id,
        EmbedKind::Video,
    );
    let initial_scene = scene_with_slots(&[slot]);
    let producer = video_frame(video_id, 41);
    let native_views = native_view_registry();
    let mut state = FrameSubmissionState::default();
    let initial = state
        .prepare(
            &initial_scene,
            &ir,
            &[producer],
            &[],
            &[],
            &native_views,
            SurfacePresenterCapabilities::fully_capable(),
            LayoutSize::new(300.0, 200.0),
            PhysicalSize::new(300, 200),
            1.0,
        )
        .unwrap();
    let published = RefCell::new(Vec::new());
    state
        .commit(&initial, |frames| {
            published.borrow_mut().push(frames.video()[0].rect);
        })
        .unwrap();

    let draw_rect = LayoutRect::new(50.0, 60.0, 80.0, 70.0);
    let mut list = DisplayList::new(draw_rect);
    list.push(DisplayOp::DrawSurface {
        rect: draw_rect,
        surface_id: slot.0,
        position: 0,
        bounds: draw_rect,
        node_id: None,
    });
    let mut translated = fission_render::RenderLayer::new(draw_rect);
    translated.style.transform = Some(translation_matrix(-25.0, -40.0));
    translated.children.push(RenderNode::Paint(list));
    let mut clipped = fission_render::RenderLayer::new(LayoutRect::new(0.0, 0.0, 100.0, 100.0));
    clipped.style.clip = Some(LayerClip::Rect(clipped.bounds));
    clipped.children.push(RenderNode::Layer(translated));
    let mut scrolled_scene = RenderScene::new(LayoutRect::new(0.0, 0.0, 300.0, 200.0));
    scrolled_scene.roots.push(RenderNode::Layer(clipped));
    let next = state
        .prepare(
            &scrolled_scene,
            &ir,
            &[producer],
            &[],
            &[],
            &native_views,
            SurfacePresenterCapabilities::fully_capable(),
            LayoutSize::new(300.0, 200.0),
            PhysicalSize::new(300, 200),
            1.0,
        )
        .unwrap();

    let mut ganesh_backend =
        GraphicsCapabilities::empty(BackendIdentity::new("skia", "1", "ganesh"));
    ganesh_backend.display_ops = fission_render::capabilities::DisplayOpKind::ALL
        .into_iter()
        .filter(|operation| *operation != fission_render::capabilities::DisplayOpKind::DrawSurface)
        .collect();
    let ganesh_host = capabilities::winit_skia_ganesh_capabilities(&ganesh_backend);
    assert!(next.validate_for(&scrolled_scene, &ganesh_host).is_ok());

    let failed = state.commit_after_presentation(&next, Err::<(), _>("present failed"), |_| {
        panic!("a failed Ganesh presentation must not publish native views")
    });
    assert!(matches!(
        failed,
        Err(PresentedFrameCommitError::Presentation("present failed"))
    ));
    assert_eq!(
        state.last_committed_frame_id(),
        Some(initial.metadata().frame_id)
    );
    assert_eq!(published.borrow().len(), 1);

    state
        .commit_after_presentation(&next, Ok::<_, &str>(()), |frames| {
            let frame = frames.video()[0];
            assert_eq!(frame.rect, LayoutRect::new(25.0, 20.0, 80.0, 70.0));
            assert_eq!(frame.visible_rect, LayoutRect::new(25.0, 20.0, 75.0, 70.0));
            published.borrow_mut().push(frame.rect);
        })
        .unwrap();
    let publish_count = published.borrow().len();
    assert!(matches!(
        state.commit(&next, |_| panic!("duplicate commit published twice")),
        Err(FrameSubmissionError::NonMonotonicCommit { .. })
    ));
    assert_eq!(published.borrow().len(), publish_count);
}
