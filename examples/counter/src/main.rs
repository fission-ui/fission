use fission_widgets::{Button, Text, Row, TextContent, Node, Widget, View, BuildCtx};
use fission_core::{Action, AppState, ActionEnvelope, ActionId, op::Color as IrColor}; 
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use serde::{Serialize, Deserialize};
use lazy_static::lazy_static; 
use anyhow; 

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CounterState {
    value: i32,
}

impl AppState for CounterState {}

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Increment;

// The handler function.
fn on_increment(state: &mut CounterState, _action: Increment) {
    state.value += 1;
    println!("Counter incremented to: {}", state.value);
}

struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        Row {
            children: vec![
                Text { 
                    content: TextContent::Literal(format!("Count: {}", view.state.value)), 
                    width: Some(150.0), 
                    height: Some(50.0), 
                    font_size: Some(20.0),
                    color: Some(IrColor::BLACK),
                    ..Default::default() 
                }.into(),
                Button { 
                    on_press: Some(ctx.bind(Increment, on_increment)), 
                    child: Some(Box::new(Text { 
                        content: TextContent::Literal("Inc".into()), 
                        width: Some(80.0), 
                        height: Some(40.0),
                        font_size: Some(20.0),
                        color: Some(IrColor::WHITE),
                        ..Default::default() 
                    }.into())),
                    width: Some(100.0), 
                    height: Some(60.0),
                    ..Default::default() 
                }.into(),
            ],
            ..Default::default()
        }.into()
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(CounterApp);
    app.run()
}
