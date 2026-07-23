use fission::prelude::*;

const CONTENT_MIN_WIDTH: f32 = 240.0;
const CONTENT_MAX_WIDTH: f32 = 420.0;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CounterState {
    pub count: i32,
}

impl GlobalState for CounterState {}

#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}

#[derive(Clone)]
pub struct CounterApp;

impl From<CounterApp> for Widget {
    fn from(_component: CounterApp) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let tokens = &view.env().theme.tokens;
        let increment = with_reducer!(ctx, Increment, on_increment);

        let content = Container::new(Column {
            gap: Some(tokens.spacing.m),
            children: vec![
                Text::new("Fission web smoke")
                    .size(tokens.typography.heading_size)
                    .color(tokens.colors.text_primary)
                    .into(),
                Text::new("This exercises the shared Fission runtime through the browser shell.")
                    .size(tokens.typography.body_large_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
                Text::new(format!("Count: {}", view.state().count))
                    .size(tokens.typography.font_size_xl)
                    .color(tokens.colors.primary)
                    .into(),
                Button {
                    on_press: Some(increment),
                    child: Some(Text::new("Increment").into()),
                    ..Default::default()
                }
                .semantics_identifier("web-smoke.increment")
                .into(),
            ],
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(CONTENT_MIN_WIDTH),
            Length::percent(100.0),
            Length::points(CONTENT_MAX_WIDTH),
        ))
        .into();

        Container::new(Column {
            gap: Some(tokens.spacing.none),
            children: vec![
                content,
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        })
        .width_length(Length::vw(100.0))
        .height_length(Length::vh(100.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
        .bg(tokens.colors.background)
        .into()
    }
}
