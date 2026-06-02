use fission_core::ui::widgets::image::Image;
use fission_core::GlobalState;
use fission_widgets::hero::Hero;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
struct TestState {
    show_detail: bool,
}
impl GlobalState for TestState {}

#[test]
fn test_hero_compilation() {
    let _hero = Hero {
        tag: "avatar".into(),
        child: Image::asset("test.png").into(),
    };
    assert!(true);
}
