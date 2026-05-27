use fission_core::ui::widgets::image::Image;
use fission_core::AppState;
use fission_widgets::hero::Hero;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
struct TestState {
    show_detail: bool,
}
impl AppState for TestState {}

#[test]
fn test_hero_compilation() {
    let _hero = Hero {
        tag: "avatar".into(),
        child: Box::new(Image::asset("test.png").into_node()),
    };
    assert!(true);
}
