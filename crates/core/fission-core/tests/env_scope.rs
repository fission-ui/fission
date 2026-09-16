//! A subtree wrapped in `EnvScope` lowers with the scoped theme's recipe values,
//! and the widgets around it keep the host's.

use fission_core::authoring::LoweringContext;
use fission_core::ui::{Button, Column, EnvScope, Text};
use fission_core::{Env, RuntimeState, Widget};
use fission_ir::{CoreIR, Op};
use fission_theme::{
    DesignMode, DesignSystem, FissionDefaultDesignSystem, FissionMaterialDesign3DesignSystem, Theme,
};

fn env_with(theme: Theme) -> Env {
    let mut env = Env::default();
    env.theme = theme;
    env
}

fn lower(widget: &Widget, env: &Env) -> CoreIR {
    let runtime = RuntimeState::default();
    let mut cx = LoweringContext::new(env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(widget, &mut cx);
    cx.set_root(root);
    cx.into_ir()
}

/// The layout and paint a tree lowers to, without node ids, in a stable order.
fn look(ir: &CoreIR) -> Vec<String> {
    let mut ops: Vec<String> = ir
        .nodes
        .values()
        .filter(|node| matches!(node.op, Op::Layout(_) | Op::Paint(_)))
        .map(|node| format!("{:?}", node.op))
        .collect();
    ops.sort();
    ops
}

fn button() -> Widget {
    Button {
        child: Some(Text::new("Save").into()),
        ..Default::default()
    }
    .into()
}

#[test]
fn a_scoped_subtree_lowers_with_the_scoped_theme() {
    let host = env_with(FissionDefaultDesignSystem::theme(DesignMode::Light));
    let material = env_with(FissionMaterialDesign3DesignSystem::theme(DesignMode::Light));
    assert_ne!(
        look(&lower(&button(), &host)),
        look(&lower(&button(), &material)),
        "the two design systems lower a button differently"
    );

    let scoped: Widget = EnvScope::new(material.clone(), button()).into();

    assert_eq!(
        look(&lower(&scoped, &host)),
        look(&lower(&button(), &material)),
        "a button inside the scope lowers as if the scoped theme were the host's"
    );
}

#[test]
fn widgets_beside_the_scope_keep_the_host_theme() {
    let host = env_with(FissionDefaultDesignSystem::theme(DesignMode::Light));
    let material = env_with(FissionMaterialDesign3DesignSystem::theme(DesignMode::Light));
    let tree: Widget = Column {
        children: vec![EnvScope::new(material.clone(), button()).into(), button()],
        ..Default::default()
    }
    .into();

    let tree_look = look(&lower(&tree, &host));

    for (theme, env) in [("scoped Material 3", &material), ("host Tidewater", &host)] {
        for op in look(&lower(&button(), env)) {
            assert!(
                tree_look.contains(&op),
                "the {theme} button is missing {op} from the mixed tree"
            );
        }
    }
}
