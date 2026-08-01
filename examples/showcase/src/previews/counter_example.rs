use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CounterExample;

impl From<CounterExample> for Widget {
    fn from(_component: CounterExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<(), _>::new(
            "showcase.example.counter",
            view.state().preview_generation,
            counter_example::CounterApp {},
        )
        .into()
    }
}
