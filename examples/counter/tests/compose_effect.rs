use fission::prelude::*;
use fission_test::{op_text, TestHarness};

#[fission_effect]
async fn increment_remote(_endpoint: String, current: i32) -> Result<i32, String> {
    Ok(current + 1)
}

fission_component! {
    RemoteCounter<()> {
        props { endpoint: &'static str }
        state {
            count: i32 = 0,
            loading: bool = false,
            error: Option<String> = None,
        }
    }
    view(counter, _state, _cx) =>
    Column(children: [
        Text(format!("Count: {}", counter.count)),
        Button(
            child: Text("Increment remotely"),
            key: "remote-increment",
            disabled: counter.loading,
            on_press: effect increment_remote(counter.endpoint.to_owned(), counter.count) {
                on_start {
                    counter.loading = true;
                    counter.error = None;
                }
                on_ok(count) {
                    counter.count = count;
                    counter.loading = false;
                }
                on_err(error) {
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
    let mut harness =
        TestHarness::new(()).with_root_widget(RemoteCounter::new("https://example.com/counter"));
    harness.pump().expect("pump remote counter");

    let ir = harness.last_ir.as_ref().expect("rendered IR");
    assert!(ir
        .nodes
        .values()
        .any(|node| op_text(&node.op).as_deref() == Some("Increment remotely")));
}
