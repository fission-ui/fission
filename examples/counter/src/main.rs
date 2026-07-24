use fission::prelude::*;

#[fission_component]
#[derive(Clone)]
struct CounterApp {
    #[local_state(default = 0)]
    count: i32,
}

#[fission_reducer(Increment)]
fn increment(count: &mut i32) {
    *count += 1;
}

#[fission_reducer(Decrement)]
fn decrement(count: &mut i32) {
    *count -= 1;
}

impl From<CounterApp> for Widget {
    fn from(counter: CounterApp) -> Self {
        let (ctx, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let spacing = &tokens.spacing;
        let typography = &tokens.typography;
        let count = counter.count();
        let decrement = ctx.bind_local(Decrement, count.clone(), reduce!(decrement));
        let increment = ctx.bind_local(Increment, count.clone(), reduce!(increment));

        Container::new(Column {
            gap: Some(spacing.l),
            children: widgets![
                Text::new("Counter")
                    .size(typography.heading1_size)
                    .color(tokens.colors.text_primary),
                Text::new(format!("{}", count.get()))
                    .size(typography.display_md_size)
                    .color(tokens.colors.primary),
                Row {
                    gap: Some(spacing.m),
                    children: widgets![
                        Button {
                            on_press: Some(decrement),
                            child: Some(Text::new("Decrement").into()),
                            ..Default::default()
                        }
                        .semantics_identifier("counter.decrement"),
                        Button {
                            on_press: Some(increment),
                            child: Some(Text::new("Increment").into()),
                            ..Default::default()
                        }
                        .semantics_identifier("counter.increment"),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding_all(spacing.xl)
        .bg(tokens.colors.background)
        .into()
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::<(), _>::new(CounterApp {}).run()
}
