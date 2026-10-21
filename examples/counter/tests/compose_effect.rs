use fission::prelude::*;
use fission_test::{op_text, TestHarness};

#[fission_effect]
async fn increment_remote(current: i32) -> Result<i32, String> {
    Ok(current + 1)
}

fission_component! {
    RemoteCounter<()> {
        #[local_state(default = 0)]
        count: i32,

        #[local_state(default = false)]
        loading: bool,

        #[local_state(default = None)]
        error: Option<String>,
    } |counter, _state, _cx| =>
    Column(children: [
        Text(format!("Count: {}", counter.count)),
        Button(
            child: Text("Increment remotely"),
            key: "remote-increment",
            disabled: counter.loading,
            on_press: effect increment_remote(counter.count) {
                start {
                    counter.loading = true;
                    counter.error = None;
                }
                Ok(count) {
                    counter.count = count;
                    counter.loading = false;
                }
                Err(error) {
                    counter.error = Some(error);
                    counter.loading = false;
                }
            },
        ),
        if let Some(error) = &counter.error {
            Text(format!("Request failed: {error}"))
        },
    ])
}

#[test]
fn component_effect_syntax_builds_a_real_widget_tree() {
    let mut harness = TestHarness::new(()).with_root_widget(RemoteCounter::new());
    harness.pump().expect("pump remote counter");

    let ir = harness.last_ir.as_ref().expect("rendered IR");
    assert!(ir
        .nodes
        .values()
        .any(|node| op_text(&node.op).as_deref() == Some("Increment remotely")));
}
