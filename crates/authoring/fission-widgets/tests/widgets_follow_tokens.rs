//! Widgets size themselves from the active design tokens rather than from
//! literals, so a design system (or a density change) reaches every widget.
//!
//! Each check lowers a widget twice: once with the default tokens and once with
//! a token set to an unusual sentinel value. The sentinel must appear in the
//! lowered IR only when the token carries it.

use fission_core::authoring::BuildCtx;
use fission_core::ui::{Switch, Widget};
use fission_core::{build, GlobalState, View, WidgetId};
use fission_theme::Tokens;
use fission_widgets::{Timeline, TimelineItem, TreeItem, TreeView};

#[derive(Default, Clone, Debug)]
struct State;

impl GlobalState for State {}

fn lowered_ir(adjust: impl FnOnce(&mut Tokens), widget: impl FnOnce() -> Widget) -> String {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();
    let mut ctx = BuildCtx::<State>::new();
    let mut env = fission_core::Env::default();
    adjust(&mut env.theme.tokens);
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );
    let widget = build::enter(&mut ctx, &view, widget);
    // Lower against the same environment: core widgets read tokens while lowering.
    format!(
        "{:?}",
        fission_core::internal::lower_widget_to_ir_in(&env, &widget, WidgetId::app_root())
    )
}

fn assert_follows(
    what: &str,
    sentinel: f32,
    adjust: impl Fn(&mut Tokens, f32),
    widget: impl Fn() -> Widget,
) {
    let needle = format!("{sentinel:?}");
    let default_ir = lowered_ir(|_| {}, &widget);
    assert!(
        !default_ir.contains(&needle),
        "{what}: sentinel {needle} already appears with default tokens"
    );
    let adjusted_ir = lowered_ir(|tokens| adjust(tokens, sentinel), &widget);
    assert!(
        adjusted_ir.contains(&needle),
        "{what}: widget ignored the token set to {needle}"
    );
}

fn tree() -> Widget {
    TreeView {
        items: vec![TreeItem {
            id: "root".into(),
            icon: None,
            label: "Root".into(),
            children: Vec::new(),
            on_toggle: None,
            on_select: None,
        }],
        expanded_ids: Default::default(),
        selected_id: None,
    }
    .into()
}

fn timeline() -> Widget {
    Timeline {
        items: vec![TimelineItem {
            title: "Shipped".into(),
            description: Some("Release notes".into()),
            timestamp: Some("Today".into()),
        }],
    }
    .into()
}

#[test]
fn tree_rows_follow_the_large_control_height() {
    assert_follows(
        "tree row height",
        57.25,
        |tokens, value| tokens.sizing.control_lg = value,
        tree,
    );
}

#[test]
fn tree_rows_follow_the_spacing_scale() {
    assert_follows(
        "tree row padding",
        7.125,
        |tokens, value| tokens.spacing.s = value,
        tree,
    );
}

#[test]
fn switches_follow_the_icon_size() {
    assert_follows(
        "switch track height",
        21.375,
        |tokens, value| tokens.sizing.icon_md = value,
        || Switch::default().into(),
    );
}

#[test]
fn timelines_follow_spacing_and_type_tokens() {
    assert_follows(
        "timeline text gap",
        3.375,
        |tokens, value| tokens.spacing.xs = value,
        timeline,
    );
    assert_follows(
        "timeline timestamp size",
        11.625,
        |tokens, value| tokens.typography.font_size_xs = value,
        timeline,
    );
}
