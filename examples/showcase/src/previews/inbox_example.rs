use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use inbox_example::InboxApp;

#[derive(Clone, Copy, Debug)]
pub(crate) struct InboxExample;

impl From<InboxExample> for Widget {
    fn from(_component: InboxExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::new(
            "showcase.example.inbox",
            view.state().preview_generation,
            InboxApp,
        )
        // The showcase header already names the example, so the inbox drops its brand.
        .with_state(inbox_example::embedded_state)
        .with_env(configure_inbox_env)
        .into()
    }
}

fn configure_inbox_env(state: &inbox_example::InboxState, env: &mut Env) {
    env.i18n = inbox_example::create_env().i18n;
    inbox_example::configure_embedded_env(state, env);
}
