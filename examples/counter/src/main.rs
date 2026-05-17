use fission::prelude::*;

#[derive(Default, Debug, Clone, PartialEq)]
struct CounterState {
    count: i32,
}

impl AppState for CounterState {}

#[fission_reducer(Increment)]
fn increment(state: &mut CounterState) {
    state.count += 1;
}

#[fission_reducer(Decrement)]
fn decrement(state: &mut CounterState) {
    state.count -= 1;
}

struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        let decrement = with_reducer!(ctx, Decrement, decrement);
        let increment = with_reducer!(ctx, Increment, increment);

        Container::new(
            Column {
                gap: Some(20.0),
                children: vec![
                    Text::new("Counter").size(32.0).into_node(),
                    Text::new(format!("{}", view.state.count))
                        .size(56.0)
                        .into_node(),
                    Row {
                        gap: Some(12.0),
                        children: vec![
                            Button {
                                on_press: Some(decrement),
                                child: Some(Box::new(Text::new("Decrement").into_node())),
                                ..Default::default()
                            }
                            .into_node(),
                            Button {
                                on_press: Some(increment),
                                child: Some(Box::new(Text::new("Increment").into_node())),
                                ..Default::default()
                            }
                            .into_node(),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(32.0)
        .into_node()
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::new(CounterApp).run()
}
