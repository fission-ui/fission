use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use motion_memory_repro_example::MotionMemoryReproApp;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MotionMemoryExample;

impl From<MotionMemoryExample> for Widget {
    fn from(_component: MotionMemoryExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let root = MotionMemoryReproApp::from_env()
            .expect("the checked-in motion memory fixture must be available");
        MountedExample::<(), _>::new(
            "showcase.example.motion-memory-repro",
            view.state().preview_generation,
            root,
        )
        .into()
    }
}
