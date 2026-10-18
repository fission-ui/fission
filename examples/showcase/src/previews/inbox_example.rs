use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use inbox_example::InboxApp;

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(crate) InboxExample<ShowcaseState> {}
    |_component, state, _cx| {} =>
        MountedExample::new(
            "showcase.example.inbox",
            state.preview_generation,
            InboxApp,
        )
        // The showcase header already names the example, so the inbox drops its brand.
        .with_state(inbox_example::embedded_state)
        .with_env(configure_inbox_env)
}

fn configure_inbox_env(state: &inbox_example::InboxState, env: &mut Env) {
    env.i18n = inbox_example::create_env().i18n;
    inbox_example::configure_embedded_env(state, env);
}
