use fission_core::internal::BuildCtx;
use fission_core::ui::{Button, Text, Widget};
use fission_core::{build, ActionEnvelope, ActionId, Env, GlobalState, View, WidgetId};
use fission_ir::op::Length;
use fission_ir::{
    CoreIR, FlyoutAlignment, FlyoutOptions, FlyoutPlacement, FlyoutWidth, LayoutOp, Op,
};
use fission_widgets::{
    flyout, flyout_with_options, Combobox, MenuButton, MenuItem, Popover, Select, SelectItem,
};

#[derive(Clone, Debug, Default)]
struct State;

impl GlobalState for State {}

fn action(name: &str) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: b"null".to_vec(),
    }
}

fn lower(widget: Widget) -> CoreIR {
    fission_core::internal::lower_widget_to_ir(&widget)
}

fn build_widget(build_widget: impl FnOnce() -> Widget) -> (CoreIR, Vec<CoreIR>) {
    let state = State;
    let runtime = fission_core::RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let widget = build::enter(&mut ctx, &view, build_widget);
    let root = lower(widget);
    let portals = ctx
        .take_portals()
        .into_iter()
        .map(|(_, portal)| lower(portal))
        .collect();
    (root, portals)
}

fn flyout_options(ir: &CoreIR) -> FlyoutOptions {
    let matches = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Layout(LayoutOp::Flyout { options, .. }) => Some(*options),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one retained flyout policy");
    matches[0]
}

fn has_fixed_box_width(ir: &CoreIR, expected: f32) -> bool {
    ir.nodes.values().any(|node| match &node.op {
        Op::Layout(LayoutOp::Box {
            width: Some(width), ..
        }) => *width == expected,
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            matches!(&style.width, Some(Length::Points(width)) if *width == expected)
        }
        _ => false,
    })
}

#[test]
fn low_level_flyout_preserves_defaults_and_explicit_policy() {
    let anchor = WidgetId::explicit("anchor");
    let default_ir = lower(flyout(anchor, Text::new("Default").into()));
    assert_eq!(flyout_options(&default_ir), FlyoutOptions::default());

    let policy = FlyoutOptions::default()
        .with_alignment(FlyoutAlignment::End)
        .with_placement(FlyoutPlacement::Above)
        .with_width(FlyoutWidth::AtLeastAnchor)
        .with_gap(6.0);
    let positioned_ir = lower(flyout_with_options(
        anchor,
        Text::new("Positioned").into(),
        policy,
    ));
    assert_eq!(flyout_options(&positioned_ir), policy);
}

#[test]
fn ordinary_popover_keeps_start_auto_content_policy() {
    let (_, portals) = build_widget(|| {
        Popover {
            id: WidgetId::explicit("details"),
            is_open: true,
            on_close: None,
            trigger: Button {
                child: Some(Text::new("Details").into()),
                ..Default::default()
            }
            .into(),
            content: Text::new("Content").into(),
            motion: None,
        }
        .into()
    });

    assert_eq!(portals.len(), 1);
    assert_eq!(flyout_options(&portals[0]), FlyoutOptions::default());
}

#[test]
fn menu_select_and_combobox_choose_control_specific_policies() {
    let (_, menu_portals) = build_widget(|| {
        MenuButton {
            id: WidgetId::explicit("actions"),
            label: "Actions".into(),
            items: vec![MenuItem {
                label: "Archive".into(),
                icon: None,
                on_select: Some(action("archive")),
                semantics_identifier: None,
            }],
            is_open: true,
            on_toggle: None,
            trigger_semantics_identifier: None,
        }
        .into()
    });
    let menu_policy = flyout_options(&menu_portals[0]);
    assert_eq!(menu_policy.alignment, FlyoutAlignment::End);
    assert_eq!(menu_policy.placement, FlyoutPlacement::Auto);
    assert_eq!(menu_policy.width, FlyoutWidth::Content);
    assert_eq!(menu_policy.gap, 4.0);
    assert!(has_fixed_box_width(&menu_portals[0], 208.0));

    let (_, select_portals) = build_widget(|| {
        Select {
            id: WidgetId::explicit("country"),
            selected_label: None,
            items: vec![SelectItem {
                label: "Canada".into(),
                icon: None,
                on_select: action("canada"),
                semantics_identifier: None,
            }],
            is_open: true,
            on_toggle: None,
            trigger_semantics_identifier: None,
            placeholder: "Choose a country".into(),
            // A fluid trigger must not cause the popup to fall back to the
            // standalone menu width.
            width: None,
        }
        .into()
    });
    let select_policy = flyout_options(&select_portals[0]);
    assert_eq!(select_policy.alignment, FlyoutAlignment::Start);
    assert_eq!(select_policy.width, FlyoutWidth::MatchAnchor);
    assert_eq!(select_policy.gap, 4.0);
    assert!(!has_fixed_box_width(&select_portals[0], 208.0));

    let (_, combobox_portals) = build_widget(|| {
        Combobox {
            id: WidgetId::explicit("assignee"),
            value: String::new(),
            items: vec!["Avery".into()],
            is_open: true,
            width: None,
            max_popup_height: None,
            on_input: None,
            on_select: None,
            on_toggle: None,
        }
        .into()
    });
    let combobox_policy = flyout_options(&combobox_portals[0]);
    assert_eq!(combobox_policy.alignment, FlyoutAlignment::Start);
    assert_eq!(combobox_policy.width, FlyoutWidth::MatchAnchor);
    assert_eq!(combobox_policy.gap, 4.0);
}
