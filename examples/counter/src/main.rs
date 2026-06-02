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
        let (ctx, _) = fission::build::current::<()>();
        let count = counter.count();
        let decrement = ctx.bind_local(Decrement, count.clone(), reduce!(decrement));
        let increment = ctx.bind_local(Increment, count.clone(), reduce!(increment));

        Container::new(Column {
            gap: Some(20.0),
            children: widgets![
                Text::new("Counter").size(32.0),
                Text::new(format!("{}", count.get())).size(56.0),
                Row {
                    gap: Some(12.0),
                    children: widgets![
                        Button {
                            on_press: Some(decrement),
                            child: Some(Text::new("Decrement").into()),
                            ..Default::default()
                        },
                        Button {
                            on_press: Some(increment),
                            child: Some(Text::new("Increment").into()),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding_all(32.0)
        .into()
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::<(), _>::new(CounterApp {}).run()
}
