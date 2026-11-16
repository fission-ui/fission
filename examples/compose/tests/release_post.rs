//! Compiles the release post's examples and checks their retained-state behaviour.
//!
//! Keep the post and these verbatim examples together: the drift tests reject
//! code blocks that are no longer compiled here.

use fission::prelude::*;
use fission_core::authoring::{lower_widget_to_ir_in, BuildCtx};
use fission_core::{build, Runtime, View};

const POST: &str =
    include_str!("../../../docs2/content/blog/2026-11-16-fission-0-16-0-compose.mdx");
const THIS_FILE: &str = include_str!("release_post.rs");
const MAIN: &str = include_str!("fixtures/release_post/src/main.rs");
const ENGLISH: &str = include_str!("fixtures/release_post/i18n/en-US.json");
const SPANISH: &str = include_str!("fixtures/release_post/i18n/es-ES.json");

#[allow(dead_code)]
#[path = "fixtures/release_post/src/main.rs"]
mod shell;

fn fenced_blocks<'a>(source: &'a str, language: &str) -> Vec<&'a str> {
    let marker = format!("```{language}\n");
    let mut rest = source;
    let mut blocks = Vec::new();
    while let Some(start) = rest.find(&marker) {
        let body = &rest[start + marker.len()..];
        let end = body
            .find("```")
            .expect("unterminated release post code block");
        blocks.push(&body[..end]);
        rest = &body[end + 3..];
    }
    blocks
}

#[test]
fn every_rust_example_in_the_post_is_compiled_verbatim() {
    let blocks = fenced_blocks(POST, "rust");
    assert_eq!(
        blocks.len(),
        3,
        "a release post Rust block was added or removed"
    );
    for block in blocks {
        assert!(
            THIS_FILE.contains(block) || MAIN == block,
            "this release post code block is not compiled by this test:\n{block}"
        );
    }
}

#[test]
fn the_launcher_uses_the_posts_exact_translation_files() {
    let blocks = fenced_blocks(POST, "json");
    assert_eq!(blocks, vec![ENGLISH, SPANISH]);
    for source in blocks {
        let _: std::collections::HashMap<String, String> =
            serde_json::from_str(source).expect("the post's translation file must be valid JSON");
    }
}

fn environment() -> Env {
    let mut env = Env::default();
    for (locale, messages) in [("en-US", ENGLISH), ("es-ES", SPANISH)] {
        env.i18n.add_bundle(fission::i18n::TranslationBundle {
            locale: fission::i18n::Locale::from(locale),
            messages: serde_json::from_str(messages).unwrap(),
        });
    }
    env
}

fn assert_retained_counter(make_counter: impl Fn(TextContent) -> Widget) {
    let mut runtime = Runtime::default();
    runtime.add_global_state(Box::new(())).unwrap();
    let mut env = environment();

    // Include rebuilds without a press, and change both locale and props.
    // The count belongs to the retained component, not its current constructor.
    for (step, (expected, press)) in [(0, true), (1, false), (1, true), (2, true), (3, false)]
        .into_iter()
        .enumerate()
    {
        let (label, expected_label) = if step == 4 {
            (
                TextContent::Literal("Another arrival".into()),
                "Another arrival",
            )
        } else {
            env.locale = fission::i18n::Locale::from(if step < 2 { "en-US" } else { "es-ES" });
            (
                TextContent::Key("check_in.record_arrival".into()),
                if step < 2 {
                    "Record an arrival"
                } else {
                    "Registrar una llegada"
                },
            )
        };
        let mut context = BuildCtx::<()>::new();
        let view = View::new(&(), &runtime.runtime_state, &env, None);
        let widget = build::enter(&mut context, &view, || make_counter(label));
        let ir = lower_widget_to_ir_in(&env, &widget);

        assert!(
            fission_test::ir_has_text(&ir, &expected.to_string()),
            "the rebuilt counter must render {expected} at step {step}"
        );
        assert!(
            fission_test::ir_has_text(&ir, expected_label),
            "the current label prop and locale must be rendered at step {step}"
        );
        let actions = ir
            .nodes
            .iter()
            .filter_map(|(id, node)| match &node.op {
                fission::op::Op::Semantics(semantics) => semantics
                    .actions
                    .entries
                    .first()
                    .map(|entry| (*id, entry.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 1, "the counter has one actionable button");

        runtime.clear_reducers();
        runtime.absorb_registry(context.registry);
        if press {
            let (node, action) = &actions[0];
            runtime
                .dispatch(
                    ActionEnvelope {
                        id: ActionId::from_u128(action.action_id),
                        payload: action
                            .payload_data
                            .clone()
                            .unwrap_or_else(|| b"null".to_vec()),
                    },
                    *node,
                )
                .expect("the rendered button action must reach its reducer");
        }
    }
}

#[test]
fn struct_based_counter_increments_and_retains_state_across_rebuilds() {
    assert_retained_counter(|label| traditional::CheckInCounter { label }.into());
}

#[test]
fn compose_counter_increments_and_retains_state_across_rebuilds() {
    assert_retained_counter(|label| compose::CheckInCounter { label }.into());
}

#[rustfmt::skip]
#[allow(dead_code)]
mod traditional {
use fission::prelude::*;

#[with_local_state]
pub struct CheckInCounter {
    pub label: TextContent,
    #[local_state(default = 0)]
    count: usize,
}

#[fission_reducer(RecordArrival)]
fn record_arrival(count: &mut usize) {
    *count += 1;
}

impl From<CheckInCounter> for Widget {
    fn from(counter: CheckInCounter) -> Widget {
        let (cx, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let count = counter.count();
        let record = cx.bind_local(RecordArrival, count.clone(), reduce!(record_arrival));

        Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                Text::new(count.get().to_string()),
                Button {
                    child: Some(Text::new(counter.label).into()),
                    on_press: Some(record),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
}

#[rustfmt::skip]
#[allow(dead_code)]
mod compose {
use fission::prelude::*;

fission_component! {
    pub CheckInCounter<()> {
        props { label: TextContent }
        state { count: usize = 0 }
    }

    view(counter, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Column(
        gap: tokens.spacing.m,
        children: [
            Text(counter.count.to_string()),
            Button(
                key: "record-arrival",
                child: Text(counter.label.clone()),
                on_press: reduce {
                    counter.count += 1;
                },
            ),
        ],
    )
}
}
