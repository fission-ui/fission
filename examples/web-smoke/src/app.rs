use fission::prelude::*;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CounterState {
    pub count: i32,
}

impl AppState for CounterState {}

#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}

pub struct CounterApp;

impl Widget<CounterState> for CounterApp {
    fn build(&self, ctx: &mut BuildCtx<CounterState>, view: &View<CounterState>) -> Node {
        let increment = with_reducer!(ctx, Increment, on_increment);
        let background = Color {
            r: 20,
            g: 23,
            b: 31,
            a: 255,
        };
        let body = Color {
            r: 184,
            g: 194,
            b: 209,
            a: 255,
        };
        let accent = Color {
            r: 145,
            g: 224,
            b: 196,
            a: 255,
        };

        Container::new(
            Column {
                gap: Some(16.0),
                children: vec![
                    Text::new("Fission web smoke")
                        .size(28.0)
                        .color(Color::WHITE)
                        .into_node(),
                    Text::new(
                        "This exercises the shared Fission runtime through the browser shell.",
                    )
                    .size(16.0)
                    .color(body)
                    .into_node(),
                    Text::new(format!("Count: {}", view.state.count))
                        .size(22.0)
                        .color(accent)
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
        )
        .padding_all(24.0)
        .bg(background)
        .into_node()
    }
}
