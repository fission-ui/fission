use fission::compose::identity::BindingProbe;
use fission::prelude::*;
use fission_core::{self as core, ActionInput};

#[derive(Default, Debug, Clone, PartialEq)]
struct CounterState {
    count: i32,
}

impl GlobalState for CounterState {}

// This is the header and reducer shape emitted by the 0.16.0 CLI starter.
fission_component! {
    Counter<CounterState> |_, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } => Column(
        gap: tokens.spacing.l,
        children: [
            Text(format!("Count: {}", state.count)),
            Button(
                child: Text("Increment"),
                key: "increment",
                on_press: |_, state, _cx| state.count += 1,
            ),
        ],
    )
}

fission_component! {
    IgnoredNamed<()> view(_, _, _) => Text("Ignored named scopes")
}

fission_component! {
    IgnoredLegacy<()> |_, _, _| => Text("Ignored legacy scopes")
}

#[test]
fn generated_starter_header_builds_and_its_reducer_updates_current_state() {
    let mut state = CounterState::default();
    let runtime = core::RuntimeState::default();
    let env = core::Env::default();
    let mut build = core::internal::BuildCtx::<CounterState>::new();
    let view = core::View::new(&state, &runtime, &env, None);
    let probe = BindingProbe::default();
    let _: Widget = core::build::enter(&mut build, &view, || {
        core::build::provide(probe.clone(), || Counter.into())
    });
    let binding = probe
        .records()
        .into_iter()
        .next()
        .expect("increment action");
    build
        .registry
        .dispatch_with_input(
            &mut state,
            &binding.envelope,
            binding.node,
            &ActionInput::None,
        )
        .unwrap();
    assert_eq!(state.count, 1);
}

#[test]
fn all_ignored_scopes_build_without_colliding() {
    let runtime = core::RuntimeState::default();
    let env = core::Env::default();
    let view = core::View::new(&(), &runtime, &env, None);
    let mut build = core::internal::BuildCtx::<()>::new();
    core::build::enter(&mut build, &view, || {
        let _: Widget = IgnoredNamed.into();
        let _: Widget = IgnoredLegacy.into();
    });
}
