use fission_core::internal::BuildCtx;
use fission_core::{build, GlobalState, View};
use fission_widgets::timeline::{Timeline, TimelineItem};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState;
impl GlobalState for TestState {}

#[test]
fn test_timeline_structure() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let timeline = Timeline {
        items: vec![
            TimelineItem {
                title: "Step 1".into(),
                description: None,
                timestamp: None,
            },
            TimelineItem {
                title: "Step 2".into(),
                description: None,
                timestamp: None,
            },
        ],
    };

    let node = build::enter(&mut ctx, &view, || timeline.into());
    assert_eq!(fission_core::internal::widget_kind_name(&node), "Column");
}
