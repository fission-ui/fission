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

#[test]
fn a_panicking_scope_restores_the_surrounding_lowering_context() {
    let host = env_with(FissionDefaultDesignSystem::theme(DesignMode::Light));
    let material = env_with(FissionMaterialDesign3DesignSystem::theme(DesignMode::Light));
    let runtime = RuntimeState::default();
    let mut cx = LoweringContext::new(&host, &runtime, None, None);

    let before_root = fission_core::internal::lower_widget(&Text::new("Before").into(), &mut cx);
    cx.set_root(before_root);
    let before_nodes = cx.ir().nodes.len();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cx.with_env(&material, |_cx| panic!("scoped lowering failed"));
    }));
    assert!(panic.is_err());

    assert_eq!(
        cx.ir().nodes.len(),
        before_nodes,
        "nodes lowered before the scope survive its panic"
    );
    let after_root = fission_core::internal::lower_widget(&Text::new("After").into(), &mut cx);
    assert_ne!(
        before_root, after_root,
        "lowering can continue after the panic"
    );
    assert!(
        cx.ir().nodes.len() > before_nodes,
        "subsequent lowering writes into the restored IR"
    );
    assert_eq!(
        cx.env().theme.tokens.colors.primary,
        host.theme.tokens.colors.primary,
        "the surrounding environment remains in force"
    );
}

#[test]
fn a_scope_rejects_a_different_layout_direction() {
    let host = env_with(FissionDefaultDesignSystem::theme(DesignMode::Light));
    let mut rtl = host.clone();
    rtl.layout_direction = fission_ir::LayoutDirection::RightToLeft;
    let runtime = RuntimeState::default();
    let mut cx = LoweringContext::new(&host, &runtime, None, None);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cx.with_env(&rtl, |_cx| ());
    }));

    assert!(panic.is_err());
    assert_eq!(cx.into_ir().layout_direction, host.layout_direction);
}
