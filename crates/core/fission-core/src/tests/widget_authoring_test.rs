use crate::{
    ui::{Node, Text, TextContent},
    AnyWidget, AppState, BuildCtx, Env, IntoWidget, RuntimeState, View, Widget,
};

#[derive(Debug, Default)]
struct State {
    label: String,
}

impl AppState for State {}

struct LabelText;

impl Widget<State> for LabelText {
    fn build(&self, _ctx: &mut BuildCtx<State>, view: &View<State>) -> impl IntoWidget<State> {
        Text::new(view.state.label.clone())
    }
}

struct LabelHost;

impl Widget<State> for LabelHost {
    fn build(&self, _ctx: &mut BuildCtx<State>, _view: &View<State>) -> impl IntoWidget<State> {
        LabelText
    }
}

#[test]
fn widget_build_can_return_another_widget_without_exposing_node() {
    let state = State {
        label: "Fission".to_string(),
    };
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::new();

    let node = LabelHost.build_node(&mut ctx, &view);

    match node {
        Node::Text(text) => assert_eq!(text.content, TextContent::Literal("Fission".into())),
        other => panic!("expected text node, got {other:?}"),
    }
}

#[test]
fn any_widget_erases_heterogeneous_widget_storage_at_the_framework_boundary() {
    let state = State {
        label: "Stored".to_string(),
    };
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::new();
    let stored = AnyWidget::new(LabelText);

    let node = stored.build_node(&mut ctx, &view);

    match node {
        Node::Text(text) => assert_eq!(text.content, TextContent::Literal("Stored".into())),
        other => panic!("expected text node, got {other:?}"),
    }
}
