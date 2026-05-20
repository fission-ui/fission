use fission_core::ui::{Container, Text};
use fission_core::{AppState, BuildCtx, Node, View, Widget};
use fission_ir::op::Color;
use fission_shell_terminal::{verify_terminal_ir, TerminalApp};

#[derive(Default, Debug, Clone, PartialEq)]
struct State;
impl AppState for State {}

struct HelloApp;

impl Widget<State> for HelloApp {
    fn build(&self, _ctx: &mut BuildCtx<State>, _view: &View<State>) -> Node {
        Container::new(Text::new("Hello terminal").color(Color::BLACK).into_node())
            .width(24.0)
            .height(3.0)
            .padding([1.0, 1.0, 1.0, 1.0])
            .bg(Color::WHITE)
            .border(Color::BLACK, 1.0)
            .into_node()
    }
}

#[test]
fn terminal_app_renders_real_fission_widget_tree_to_cells() {
    let mut app = TerminalApp::<State, _>::new(HelloApp);
    let frame = app.render_frame(40, 10).expect("render terminal frame");
    assert!(frame.as_plain_text().contains("Hello terminal"));
}

#[test]
fn terminal_verifier_rejects_graphical_only_paint() {
    let mut ir = fission_ir::CoreIR::new();
    let id = fission_ir::NodeId::from_u128(1);
    ir.add_node(
        id,
        fission_ir::Op::Paint(fission_ir::PaintOp::DrawImage {
            source: "image.png".to_string(),
            fit: fission_ir::op::ImageFit::Contain,
        }),
        Vec::new(),
    );
    ir.set_root(id);
    assert!(verify_terminal_ir(&ir).is_err());
}
