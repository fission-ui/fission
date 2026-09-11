use fission_core::authoring::BuildCtx;
use fission_core::ui::{Button, ButtonVariant, Text, Widget};
use fission_core::{build, ActionEnvelope, ActionId, Env, GlobalState, LayoutSize, View, WidgetId};
use fission_ir::op::{Fill, Length};
use fission_ir::{CoreIR, FlexDirection, LayoutOp, Op, PaintOp, Semantics};
use fission_theme::{ButtonHierarchy, ComponentSize, ComponentState};
use fission_widgets::{
    Modal, ModalAction, ModalContent, ModalDescription, ModalFooter, ModalFooterAction,
    ModalHeader, ModalLayout, ModalTitle,
};

#[derive(Default, Clone, Debug)]
struct State;

impl GlobalState for State {}

fn action(name: &str) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: b"null".to_vec(),
    }
}

fn build_modal(
    viewport_width: f32,
    actions: Vec<ModalAction>,
    on_dismiss: Option<ActionEnvelope>,
) -> (CoreIR, Env) {
    let mut env = Env::default();
    env.viewport_size = LayoutSize::new(viewport_width, 844.0);
    build_modal_with_env(env, actions, on_dismiss)
}

fn build_modal_with_env(
    env: Env,
    actions: Vec<ModalAction>,
    on_dismiss: Option<ActionEnvelope>,
) -> (CoreIR, Env) {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );
    let mut ctx = BuildCtx::<State>::new();
    let _: Widget = build::enter(&mut ctx, &view, || {
        Modal {
            id: WidgetId::explicit("modal.quality"),
            title: "Review active session".into(),
            content: Text::new("Body").size(14.0).line_height(20.0).into(),
            is_open: true,
            on_dismiss,
            backdrop_semantics_identifier: Some("modal.backdrop".into()),
            close_semantics_identifier: Some("modal.close".into()),
            surface_semantics_identifier: Some("modal.surface".into()),
            actions,
            width: None,
            motion: None,
        }
        .into()
    });

    let portals = ctx.take_portals();
    assert_eq!(portals.len(), 1);
    (
        fission_core::internal::lower_widget_to_ir(&portals[0].1),
        env,
    )
}

fn build_modal_layout(viewport_width: f32, actions: Vec<ModalFooterAction>) -> (CoreIR, Env) {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();
    let mut env = Env::default();
    env.viewport_size = LayoutSize::new(viewport_width, 844.0);
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );
    let mut ctx = BuildCtx::<State>::new();
    let _: Widget = build::enter(&mut ctx, &view, || {
        ModalLayout {
            id: WidgetId::explicit("modal.quality"),
            header: ModalHeader::new("Review active session"),
            content: ModalContent::new(Text::new("Body")),
            footer: Some(ModalFooter::new(actions)),
            is_open: true,
            surface_semantics_identifier: Some("modal.surface".into()),
            ..Default::default()
        }
        .into()
    });

    let portals = ctx.take_portals();
    assert_eq!(portals.len(), 1);
    (
        fission_core::internal::lower_widget_to_ir(&portals[0].1),
        env,
    )
}

fn semantics_entry<'a>(ir: &'a CoreIR, identifier: &str) -> (WidgetId, &'a Semantics) {
    ir.nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.identifier.as_deref() == Some(identifier) => {
                Some((node.id, semantics))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing semantics node {identifier:?}"))
}

fn first_flex_parent(ir: &CoreIR, child: WidgetId) -> (WidgetId, FlexDirection) {
    let mut current = child;
    loop {
        let parent = ir
            .nodes
            .values()
            .find(|node| node.children.contains(&current))
            .unwrap_or_else(|| panic!("missing parent for {current}"));
        if let Op::Layout(LayoutOp::Flex { direction, .. }) = &parent.op {
            return (parent.id, *direction);
        }
        current = parent.id;
    }
}

fn descendants_contain(ir: &CoreIR, root: WidgetId, expected: &Fill) -> bool {
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        let node = &ir.nodes[&id];
        if matches!(
            &node.op,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(fill),
                ..
            }) if fill == expected
        ) {
            return true;
        }
        pending.extend(node.children.iter().copied());
    }
    false
}

#[test]
fn modal_uses_recipe_width_and_preserves_a_sixteen_point_mobile_gutter() {
    let (desktop, desktop_env) = build_modal(1280.0, Vec::new(), None);
    let (desktop_surface_id, _) = semantics_entry(&desktop, "modal.surface");
    let desktop_surface = &desktop.nodes[&desktop.nodes[&desktop_surface_id].children[0]];
    let Op::Layout(LayoutOp::StyledBox { style, .. }) = &desktop_surface.op else {
        panic!("modal surface should lower to a styled box");
    };
    assert_eq!(
        &style.width,
        &Some(Length::Points(
            desktop_env
                .theme
                .components
                .modal
                .container_style
                .max_width
                .unwrap()
        ))
    );

    let (mobile, _) = build_modal(390.0, Vec::new(), None);
    let (mobile_surface_id, _) = semantics_entry(&mobile, "modal.surface");
    let mobile_surface = &mobile.nodes[&mobile.nodes[&mobile_surface_id].children[0]];
    let Op::Layout(LayoutOp::StyledBox { style, .. }) = &mobile_surface.op else {
        panic!("modal surface should lower to a styled box");
    };
    assert_eq!(&style.width, &Some(Length::Points(358.0)));
    assert_eq!(
        &style.max_height,
        &Some(Length::Points(
            844.0 - 2.0 * desktop_env.theme.components.modal.viewport_margin
        ))
    );
}

#[test]
fn modal_title_and_close_control_use_dialog_recipe_typography_and_geometry() {
    let (ir, env) = build_modal(1280.0, Vec::new(), Some(action("modal.dismiss")));
    let (_, surface_semantics) = semantics_entry(&ir, "modal.surface");
    assert_eq!(surface_semantics.labelled_by.len(), 1);
    assert!(matches!(
        &ir.nodes[&surface_semantics.labelled_by[0]].op,
        Op::Semantics(Semantics {
            role: fission_ir::Role::Text,
            ..
        })
    ));
    let title_run = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Paint(PaintOp::DrawRichText { runs, .. })
                if runs.iter().map(|run| run.text.as_str()).collect::<String>()
                    == "Review active session" =>
            {
                runs.first()
            }
            _ => None,
        })
        .expect("modal title rich-text run");
    assert_eq!(
        title_run.style.font_size,
        env.theme.components.modal.title_style.font_size.unwrap()
    );
    assert_eq!(
        title_run.style.font_weight,
        env.theme.components.modal.title_style.font_weight.unwrap()
    );
    assert_eq!(
        title_run.style.line_height,
        env.theme.components.modal.title_style.line_height
    );

    let (close_id, close_semantics) = semantics_entry(&ir, "modal.close");
    assert_eq!(close_semantics.label.as_deref(), Some("Close"));
    let close_box = &ir.nodes[&ir.nodes[&close_id].children[0]];
    let Op::Layout(LayoutOp::Box {
        width,
        height,
        padding,
        ..
    }) = &close_box.op
    else {
        panic!("close control should lower to a fixed button box");
    };
    let close_style = &env.theme.components.modal.close_button_style;
    assert_eq!(*width, close_style.width);
    assert_eq!(*height, close_style.height);

    // Layout padding is the recipe padding inflated by the widest border the
    // control can draw, so the box does not resize when a focus or hover state
    // thickens its outline. Assert that relationship rather than the sum, which
    // would re-encode the state recipes here.
    let recipe_padding = close_style.padding.unwrap();
    let inset: Vec<f32> = padding
        .iter()
        .zip(recipe_padding.iter())
        .map(|(actual, recipe)| actual - recipe)
        .collect();
    assert!(
        inset.iter().all(|value| *value >= 0.0),
        "close control padding {padding:?} should contain its recipe padding {recipe_padding:?}"
    );
    assert!(
        inset
            .windows(2)
            .all(|pair| (pair[0] - pair[1]).abs() < 1e-4),
        "border inset should be uniform on every side, got {inset:?}"
    );
}

#[test]
fn modal_footer_stacks_actions_on_narrow_viewports_and_honors_explicit_variants() {
    let modal_actions = || {
        vec![
            ModalFooterAction {
                label: "Cancel".into(),
                on_press: None,
                variant: ButtonVariant::SecondaryGray,
                semantics_identifier: Some("modal.cancel".into()),
            },
            ModalFooterAction {
                label: "End session".into(),
                on_press: None,
                variant: ButtonVariant::Destructive,
                semantics_identifier: Some("modal.end-session".into()),
            },
        ]
    };

    let (desktop, _) = build_modal_layout(1280.0, modal_actions());
    let (desktop_cancel, _) = semantics_entry(&desktop, "modal.cancel");
    assert_eq!(
        first_flex_parent(&desktop, desktop_cancel).1,
        FlexDirection::Row
    );

    let (mobile, mobile_env) = build_modal_layout(390.0, modal_actions());
    let (mobile_cancel, _) = semantics_entry(&mobile, "modal.cancel");
    let (mobile_end, _) = semantics_entry(&mobile, "modal.end-session");
    let (footer_id, direction) = first_flex_parent(&mobile, mobile_cancel);
    assert_eq!(direction, FlexDirection::Column);
    let footer_children = &mobile.nodes[&footer_id].children;
    assert!(
        footer_children
            .iter()
            .position(|id| *id == mobile_end)
            .expect("destructive action in footer")
            < footer_children
                .iter()
                .position(|id| *id == mobile_cancel)
                .expect("cancel action in footer"),
        "the emphasized action should appear first in the stacked visual order"
    );

    let destructive = mobile_env.theme.components.button.resolve(
        ButtonHierarchy::Destructive,
        ComponentSize::Md,
        ComponentState::Default,
    );
    assert!(descendants_contain(
        &mobile,
        mobile_end,
        destructive
            .background
            .as_ref()
            .expect("destructive background")
    ));
    assert!(
        mobile_env
            .theme
            .components
            .modal
            .footer_style
            .background
            .is_some(),
        "the default footer uses the design system's subtle surface tint"
    );
    assert!(
        mobile_env
            .theme
            .components
            .modal
            .footer_style
            .border
            .is_some(),
        "the default footer has a full-width top boundary"
    );
}

#[test]
fn modal_footer_recipe_renders_one_full_width_top_boundary_without_a_box_border() {
    let mut env = Env::default();
    env.viewport_size = LayoutSize::new(1280.0, 844.0);
    let footer_tint = fission_ir::op::Color {
        r: 19,
        g: 43,
        b: 67,
        a: 255,
    };
    let boundary_tint = fission_ir::op::Color {
        r: 91,
        g: 117,
        b: 143,
        a: 255,
    };
    env.theme.components.modal.footer_style.background = Some(Fill::Solid(footer_tint));
    let boundary = env
        .theme
        .components
        .modal
        .footer_style
        .border
        .as_mut()
        .expect("modal footer boundary recipe");
    boundary.fill = Fill::Solid(boundary_tint);
    boundary.width = 3.0;

    let (ir, _) = build_modal_with_env(
        env,
        vec![ModalAction {
            label: "Save".into(),
            on_press: None,
            is_primary: true,
            semantics_identifier: Some("modal.save".into()),
        }],
        None,
    );

    assert!(ir.nodes.values().any(|node| match &node.op {
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(color)),
            ..
        }) => *color == footer_tint,
        _ => false,
    }));
    let boundary_paints: Vec<_> = ir
        .nodes
        .values()
        .filter(|node| match &node.op {
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(color)),
                stroke: None,
                corner_radius,
                shadow: None,
            }) => *color == boundary_tint && *corner_radius == 0.0,
            _ => false,
        })
        .collect();
    assert_eq!(boundary_paints.len(), 1);
    let boundary_box = ir
        .nodes
        .values()
        .find(|node| node.children.contains(&boundary_paints[0].id))
        .expect("layout box containing modal footer boundary");
    let Op::Layout(LayoutOp::StyledBox { style, .. }) = &boundary_box.op else {
        panic!("modal footer boundary should lower to a styled box");
    };
    assert_eq!(style.height, Some(Length::Points(3.0)));
    assert_eq!(
        style.padding, None,
        "the boundary must reach both surface edges"
    );
    assert!(
        !ir.nodes.values().any(|node| match &node.op {
            Op::Paint(PaintOp::DrawRect {
                stroke: Some(stroke),
                ..
            }) => stroke.fill == Fill::Solid(boundary_tint),
            _ => false,
        }),
        "the footer boundary must not stroke the footer's side and bottom edges"
    );
}

#[test]
fn retained_modal_anatomy_links_title_and_description_and_scrolls_only_the_body() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();
    let mut env = Env::default();
    env.viewport_size = LayoutSize::new(390.0, 320.0);
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );
    let mut ctx = BuildCtx::<State>::new();
    let _: Widget = build::enter(&mut ctx, &view, || {
        ModalLayout {
            id: WidgetId::explicit("modal.retained"),
            header: ModalHeader {
                leading: None,
                title: ModalTitle::custom(
                    "Rename workspace",
                    Text::new("Rename workspace").weight(600),
                )
                .semantics_identifier("modal.retained.title"),
                description: Some(
                    ModalDescription::new("Choose a name everyone will recognize.")
                        .semantics_identifier("modal.retained.description"),
                ),
                trailing: None,
            },
            content: ModalContent::new(Text::new("Body")),
            footer: Some(ModalFooter {
                children: vec![Button {
                    child: Some(Text::new("Advanced").into()),
                    ..Default::default()
                }
                .semantics_identifier("modal.retained.advanced")
                .into()],
                actions: vec![ModalFooterAction {
                    label: "Save".into(),
                    on_press: None,
                    variant: ButtonVariant::Primary,
                    semantics_identifier: Some("modal.retained.save".into()),
                }],
            }),
            is_open: true,
            on_dismiss: Some(action("modal.retained.dismiss")),
            backdrop_semantics_identifier: Some("modal.retained.backdrop".into()),
            close_semantics_identifier: Some("modal.retained.close".into()),
            close_label: None,
            surface_semantics_identifier: Some("modal.retained.surface".into()),
            width: None,
            motion: None,
        }
        .into()
    });

    let portals = ctx.take_portals();
    let ir = fission_core::internal::lower_widget_to_ir(&portals[0].1);
    let (title_id, title) = semantics_entry(&ir, "modal.retained.title");
    let (description_id, description) = semantics_entry(&ir, "modal.retained.description");
    let (_, surface) = semantics_entry(&ir, "modal.retained.surface");
    semantics_entry(&ir, "modal.retained.advanced");
    assert_eq!(title.role, fission_ir::Role::Text);
    assert_eq!(description.role, fission_ir::Role::Text);
    assert_eq!(surface.labelled_by, vec![title_id]);
    assert_eq!(surface.described_by, vec![description_id]);
    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            flex_shrink,
            ..
        }) if *flex_shrink == 1.0
    )));

    let (surface_id, _) = semantics_entry(&ir, "modal.retained.surface");
    let surface_box = &ir.nodes[&ir.nodes[&surface_id].children[0]];
    let Op::Layout(LayoutOp::StyledBox { style, .. }) = &surface_box.op else {
        panic!("modal surface should remain a styled box");
    };
    assert_eq!(style.max_height, Some(Length::Points(288.0)));
}
