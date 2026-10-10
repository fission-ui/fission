use fission::op::{AlignItems, JustifyContent};
use fission::prelude::*;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CounterState {
    pub count: i32,
}
impl GlobalState for CounterState {}

#[fission_action]
enum CounterActions {
    Increment,
}

fn do_increment(
    state: &mut CounterState,
    _action: CounterActions,
    _ctx: &mut ReducerContext<CounterState>,
) {
    state.count += 4;
}

#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}

#[fission_reducer(SetTitle)]
fn on_set_title(title: &mut String, ctx: &mut ReducerContext<String>) {
    let Some(change) = ctx.input.text_change() else {
        return;
    };
    *title = change.new_text.clone();
}

#[fission_component]
#[derive(Clone)]
pub struct CounterApp {
    #[local_state(default = "Counter App".to_string())]
    pub title: String,
}

impl From<CounterApp> for Widget {
    fn from(component: CounterApp) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let title = component.title();
        let on_title = ctx.bind_local(SetTitle, title.clone(), reduce!(on_set_title));
        // let increment = with_reducer!(ctx, Increment, on_increment);
        let increment = ctx.bind(Increment, reduce!(on_increment));
        let increment_4 = ctx.bind(CounterActions::Increment, reduce!(do_increment));
        Center {
            child: Column {
                gap: Some(16.0),
                // 1. Centers elements horizontally inside the Column bounds (Cross-axis)
                align_items: AlignItems::Center,
                // 2. Centers elements vertically inside the Column bounds (Main-axis)
                justify_content: JustifyContent::Center,
                children: widgets![
                    Text::new(title.get()).size(28.0),
                    TextInput {
                        // value: title.get(),
                        editing_value: Some(TextEditingValue::from_text(title.get())),
                        label: Some("Title".into()),
                        placeholder: Some("Enter a title".into()),
                        on_input: Some(on_title),
                        width: Some(320.0),
                        ..Default::default()
                    },
                    Text::new(format!("Count: {}", view.state().count)).size(22.0),
                    Button {
                        on_press: Some(increment),
                        child: Some(Text::new("Increment").into()),
                        ..Default::default()
                    },
                    Button {
                        on_press: Some(increment_4),
                        child: Some(Text::new("Increment by 4").into()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}
