use fission_core::{AppState, BuildCtx, Node, View, Widget};
use fission_widgets::timeline::{Timeline, TimelineItem};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState;
impl AppState for TestState {}

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

    let node = fission_core::view::lower_widget_to_node(&timeline, &mut ctx, &view);
    assert!(matches!(node, Node::Column(_)));
}
