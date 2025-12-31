use fission_core::{AppState, BuildCtx, View, Widget, Node};
use fission_widgets::hero::Hero;
use fission_core::ui::widgets::image::Image;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    show_detail: bool,
}
impl AppState for TestState {}

#[test]
fn test_hero_compilation() {
    let hero = Hero {
        tag: "avatar".into(),
        child: Box::new(Image { source: "test.png".into(), ..Default::default() }.into_node()),
    };
    assert!(true);
}
