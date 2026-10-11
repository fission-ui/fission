//! Replace a freshly initialized scaffold's src/app.rs after reading its guidance.
use fission::prelude::*;
use std::sync::Arc;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CounterState {
    pub count: i32,
    pub route: String,
}
impl GlobalState for CounterState {}

#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}
#[derive(Clone)]
pub struct CounterApp;
impl From<CounterApp> for Widget {
    fn from(_: CounterApp) -> Widget {
        let (ctx, view) = fission::build::current::<CounterState>();
        let tokens = &view.env().theme.tokens;
        let router = Router::<CounterState> {
            current_path: if view.state().route.is_empty() {
                "/".into()
            } else {
                view.state().route.clone()
            },
            routes: vec![
                Route {
                    path: "/".into(),
                    builder: Arc::new(|_, _, _| Text::new("Home route").into()),
                },
                Route {
                    path: "/about/details".into(),
                    builder: Arc::new(|_, _, _| Text::new("Details route").into()),
                },
            ],
            not_found: Some(Arc::new(|_, _, _| Text::new("Unknown route").into())),
        };
        Container::new(Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                Text::new("Viewport resize fixture").size(tokens.typography.heading2_size),
                Responsive::new(Text::new("Wide layout").semantics_label("Wide layout")).case(
                    ResponsiveCase::max_width(
                        600.0,
                        Text::new("Narrow layout").semantics_label("Narrow layout")
                    )
                ),
                Link::to("Details", "/about/details"),
                router,
                Text::new(format!("Count: {}", view.state().count)),
                Button {
                    on_press: Some(with_reducer!(ctx, Increment, on_increment)),
                    child: Some(Text::new("Increment").into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.l)
        .bg(tokens.colors.background)
        .into()
    }
}
