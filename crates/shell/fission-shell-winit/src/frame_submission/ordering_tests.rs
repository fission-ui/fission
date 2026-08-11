use super::*;

#[test]
fn overlapping_surfaces_require_one_presenter_ordering_domain() {
    let geometry = NativeViewGeometry {
        rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
        visible_rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
        transform: None,
        opacity: 1.0,
        paint_order: 0,
    };
    let plans = [
        NativeViewPlan {
            descriptor: ProducerDescriptor {
                widget_id: WidgetId::explicit("overlap.video"),
                kind: ProducerDescriptorKind::Video,
            },
            slot_id: ExternalSurfaceSlotId(1),
            geometry,
            capabilities: PlatformSurfaceCapabilities::FULL,
            presenter_domain: NativeViewPresenterDomain::Video,
        },
        NativeViewPlan {
            descriptor: ProducerDescriptor {
                widget_id: WidgetId::explicit("overlap.web"),
                kind: ProducerDescriptorKind::Web,
            },
            slot_id: ExternalSurfaceSlotId(2),
            geometry,
            capabilities: PlatformSurfaceCapabilities::FULL,
            presenter_domain: NativeViewPresenterDomain::Web,
        },
    ];

    assert!(matches!(
        validate_native_view_overlap_order(&plans),
        Err(FrameSubmissionError::UnsupportedPlatformSurfaceSemantics {
            semantic: PlatformSurfaceSemantic::PaintOrder,
            ..
        })
    ));
}

#[cfg(feature = "three-d")]
#[test]
fn direct_target_rejects_rotation_and_carries_supported_opacity() {
    let widget_id = WidgetId::explicit("direct-target.unsupported");
    let slot_id = ExternalSurfaceSlotId(91);
    let viewport = LayoutRect::new(0.0, 0.0, 10.0, 10.0);
    let rotated = ResolvedSurfacePlacement {
        viewport,
        clip: ClipRegion::Unbounded,
        transform: Affine2d::IDENTITY,
        opacity: 1.0,
        paint_order: 0,
        issue: Some(SurfacePlacementIssue::NonAxisAlignedTransform),
    };
    let translucent = ResolvedSurfacePlacement {
        viewport,
        clip: ClipRegion::Unbounded,
        transform: Affine2d::IDENTITY,
        opacity: 0.5,
        paint_order: 0,
        issue: None,
    };

    assert!(matches!(
        rotated
            .direct_target_geometry(widget_id, slot_id)
            .unwrap_err(),
        FrameSubmissionError::UnsupportedDirectTargetPlacement {
            issue: SurfacePlacementIssue::NonAxisAlignedTransform,
            ..
        }
    ));
    let (_, _, opacity, _) = translucent
        .direct_target_geometry(widget_id, slot_id)
        .unwrap()
        .unwrap();
    assert!((opacity - 0.5).abs() <= f32::EPSILON);
}
