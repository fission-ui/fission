use fission_core::env::LayoutDirection;
use fission_core::internal::BuildCtx;
use fission_core::internal::{build_layout_tree, InternalLoweringCx};
use fission_core::op::Overflow;
use fission_core::{build, Env, GlobalState, RuntimeState, View, Widget, WidgetId};
use fission_ir::Role;
use fission_layout::{LayoutEngine, LayoutSize};
use fission_widgets::{Avatar, AvatarGroup, AvatarGroupItem};

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

fn build_avatar(avatar: Avatar) -> Widget {
    let state = TestState;
    let runtime_state = RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime_state, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    build::enter(&mut ctx, &view, || avatar.into())
}

fn build_avatar_group(group: AvatarGroup) -> Widget {
    build_avatar_group_with_env(group, &Env::default())
}

fn build_avatar_group_with_env(group: AvatarGroup, env: &Env) -> Widget {
    let state = TestState;
    let runtime_state = RuntimeState::default();
    let view = View::new(&state, &runtime_state, env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    build::enter(&mut ctx, &view, || group.into())
}

#[test]
fn image_avatar_clips_to_its_circular_bounds() {
    let widget = build_avatar(Avatar {
        name: Some("Ada Lovelace".into()),
        src: Some("assets/ada.png".into()),
        size: Some(48.0),
    });
    let container = fission_core::internal::widget_as_container(&widget).expect("avatar container");

    assert_eq!(container.width, Some(48.0));
    assert_eq!(container.height, Some(48.0));
    assert_eq!(container.border_radius, 24.0);
    assert_eq!(container.box_style.overflow, Overflow::Clip);
}

#[test]
fn avatar_group_preserves_item_identity_and_exposes_one_semantic_group() {
    let ada = WidgetId::explicit("person.ada");
    let grace = WidgetId::explicit("person.grace");
    let widget = build_avatar_group(AvatarGroup {
        id: Some(WidgetId::explicit("team")),
        avatars: vec![
            AvatarGroupItem::new(
                ada,
                Avatar {
                    name: Some("Ada Lovelace".into()),
                    ..Default::default()
                },
            ),
            AvatarGroupItem::new(
                grace,
                Avatar {
                    name: Some("Grace Hopper".into()),
                    ..Default::default()
                },
            ),
        ],
        semantics_label: Some("Project team".into()),
        ..Default::default()
    });

    let row = fission_core::internal::widget_as_row(&widget).expect("avatar group row");
    let semantics = row.semantics.as_ref().expect("group semantics");
    assert_eq!(semantics.role, Role::Group);
    assert_eq!(semantics.label.as_deref(), Some("Project team"));
    assert_eq!(row.children.len(), 2);
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut lowering = InternalLoweringCx::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.ir.set_root(root);
    assert!(lowering.ir.nodes.contains_key(&ada));
    assert!(lowering.ir.nodes.contains_key(&grace));
}

#[test]
fn avatar_group_applies_overlap_and_adds_a_stable_overflow_surface() {
    let group_id = WidgetId::explicit("team");
    let avatars = ["ada", "grace", "katherine", "margaret"]
        .into_iter()
        .map(|name| {
            AvatarGroupItem::new(
                WidgetId::explicit(name),
                Avatar {
                    name: Some(name.into()),
                    ..Default::default()
                },
            )
        })
        .collect();
    let widget = build_avatar_group(AvatarGroup {
        id: Some(group_id),
        avatars,
        max_visible: Some(2),
        overlap: Some(12.0),
        size: Some(36.0),
        ..Default::default()
    });

    let row = fission_core::internal::widget_as_row(&widget).expect("avatar group row");
    assert_eq!(row.gap, Some(0.0));
    assert_eq!(row.children.len(), 3);

    let rebuilt = build_avatar_group(AvatarGroup {
        id: Some(group_id),
        avatars: ["ada", "grace", "katherine", "margaret"]
            .into_iter()
            .map(|name| AvatarGroupItem::new(WidgetId::explicit(name), Avatar::default()))
            .collect(),
        max_visible: Some(2),
        ..Default::default()
    });
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut first_lowering = InternalLoweringCx::new(&env, &runtime, None, None);
    let first_root = fission_core::internal::lower_widget(&widget, &mut first_lowering);
    first_lowering.ir.set_root(first_root);
    let mut rebuilt_lowering = InternalLoweringCx::new(&env, &runtime, None, None);
    let rebuilt_root = fission_core::internal::lower_widget(&rebuilt, &mut rebuilt_lowering);
    rebuilt_lowering.ir.set_root(rebuilt_root);
    let overflow_id = WidgetId::derived(group_id.as_u128(), &[0x4f56_464c]);
    assert!(first_lowering.ir.nodes.contains_key(&overflow_id));
    assert!(rebuilt_lowering.ir.nodes.contains_key(&overflow_id));
}

#[test]
fn avatar_group_overlap_is_reflected_in_ltr_and_rtl_layout() {
    for direction in [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft] {
        let first = WidgetId::explicit("overlap.first");
        let second = WidgetId::explicit("overlap.second");
        let mut env = Env::default();
        env.layout_direction = direction;
        let widget = build_avatar_group_with_env(
            AvatarGroup {
                avatars: vec![
                    AvatarGroupItem::new(first, Avatar::default()),
                    AvatarGroupItem::new(second, Avatar::default()),
                ],
                size: Some(36.0),
                overlap: Some(12.0),
                ..Default::default()
            },
            &env,
        );
        let runtime = RuntimeState::default();
        let mut lowering = InternalLoweringCx::new(&env, &runtime, None, None);
        let root = fission_core::internal::lower_widget(&widget, &mut lowering);
        lowering.ir.set_root(root);
        let input = build_layout_tree(&lowering.ir, &env);
        let mut engine = LayoutEngine::new().with_layout_direction(direction);
        let layout = engine
            .compute_layout(&input, root, LayoutSize::new(200.0, 80.0), &|_| 0.0)
            .expect("avatar group layout");
        let first_rect = layout.get_node_rect(first).expect("first avatar layout");
        let second_rect = layout.get_node_rect(second).expect("second avatar layout");
        let overlap = (first_rect.x() + first_rect.width())
            .min(second_rect.x() + second_rect.width())
            - first_rect.x().max(second_rect.x());

        assert_eq!(overlap, 12.0, "{direction:?} must preserve logical overlap");
    }
}
