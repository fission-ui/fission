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
    fn build(
        &self,
        ctx: &mut BuildCtx<CounterState>,
        view: &View<CounterState>,
    ) -> impl fission::IntoWidget<CounterState> {
        {
            let decrement = with_reducer!(ctx, Decrement, decrement);
            let increment = with_reducer!(ctx, Increment, increment);

            Container::new(
                Column::new()
                    .gap(Some(20.0))
                    .child(Text::new("Counter").size(32.0))
                    .child(Text::new(format!("{}", view.state.count)).size(56.0))
                    .child(
                        Row::new()
                            .gap(12.0)
                            .child(Button::new(Text::new("Decrement")).on_press(decrement))
                            .child(Button::new(Text::new("Increment")).on_press(increment)),
                    ),
            )
            .padding_all(32.0)
        }
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::new(CounterApp).run()
}
