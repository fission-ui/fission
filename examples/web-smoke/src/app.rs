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
    fn build(
        &self,
        ctx: &mut BuildCtx<CounterState>,
        view: &View<CounterState>,
    ) -> impl fission::IntoWidget<CounterState> {
        {
            let increment = with_reducer!(ctx, Increment, on_increment);
            let viewport = view.viewport_size();
            let content_width = (viewport.width - 48.0).clamp(240.0, 420.0);
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

            let content = Container::new(
                Column::new()
                    .gap(Some(16.0))
                    .child(
                        Text::new("Fission web smoke")
                            .size(28.0)
                            .color(Color::WHITE)
                            .max_width(content_width),
                    )
                    .child(
                        Text::new(
                            "This exercises the shared Fission runtime through the browser shell.",
                        )
                        .size(16.0)
                        .color(body)
                        .max_width(content_width),
                    )
                    .child(
                        Text::new(format!("Count: {}", view.state.count))
                            .size(22.0)
                            .color(accent),
                    )
                    .child(
                        Button::new(
                            Text::new("Increment").width((content_width - 96.0).max(120.0)),
                        )
                        .width(content_width)
                        .on_press(increment),
                    ),
            )
            .width(content_width);

            Container::new(Column::new().gap(Some(0.0)).child(content).child(Spacer {
                flex_grow: 1.0,
                ..Default::default()
            }))
            .width(viewport.width.max(1.0))
            .height(viewport.height.max(1.0))
            .padding_all(24.0)
            .bg(background)
        }
    }
}
