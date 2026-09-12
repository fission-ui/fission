//! Compiles the Rust in the advanced widget authoring guide.
//!
//! Each module below holds one of the guide's code blocks verbatim, and
//! `the_guide_matches_this_file` fails when the two drift apart, so the guide
//! cannot show code that does not build. Edit the guide and this file together.

const GUIDE: &str =
    include_str!("../../../../documentation/content/docs/guides/advanced-widget-authoring.mdx");
const THIS_FILE: &str = include_str!("advanced_widget_authoring_guide.rs");

#[test]
fn the_guide_matches_this_file() {
    let mut blocks = 0;
    let mut rest = GUIDE;
    while let Some(start) = rest.find("```rust\n") {
        let body = &rest[start + "```rust\n".len()..];
        let end = body
            .find("```")
            .expect("unterminated code block in the guide");
        let block = &body[..end];
        assert!(
            THIS_FILE.contains(block),
            "this guide code block is not compiled by this test:\n{block}"
        );
        blocks += 1;
        rest = &body[end + 3..];
    }
    assert_eq!(blocks, 3, "a guide code block was added or removed");
}

#[rustfmt::skip]
#[allow(dead_code)]
mod price_tag {
use fission_core::ui::{Row, Text, Widget};
use fission_core::widgets;

pub struct PriceTag {
    pub currency: String,
    pub amount: String,
}

impl From<PriceTag> for Widget {
    fn from(tag: PriceTag) -> Widget {
        Row {
            gap: Some(4.0),
            children: widgets![
                Text::new(tag.currency).size(12.0),
                Text::new(tag.amount).size(20.0),
            ],
            ..Default::default()
        }
        .into()
    }
}
}

#[rustfmt::skip]
mod aspect_ratio {
use fission_core::authoring::{custom_widget, lower_widget, IrBuilder, LowerWidget, LoweringContext};
use fission_core::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};

/// Constrains one child to a width-to-height ratio.
#[derive(Clone, Debug)]
pub struct AspectRatio {
    pub ratio: f32,
    pub child: Widget,
}

impl From<AspectRatio> for Widget {
    fn from(component: AspectRatio) -> Widget {
        custom_widget(
            "AspectRatio",
            AspectRatioLowerer {
                ratio: component.ratio,
                child: component.child,
            },
        )
    }
}

#[derive(Debug)]
struct AspectRatioLowerer {
    ratio: f32,
    child: Widget,
}

impl LowerWidget for AspectRatioLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let child_id = lower_widget(&self.child, cx);
        let id = cx.next_node_id();

        let mut builder = IrBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: Some(self.ratio),
            }),
        );
        builder.add_child(child_id);
        builder.build(cx)
    }

    fn stable_key(&self) -> u64 {
        self.ratio.to_bits() as u64
    }
}
}

#[rustfmt::skip]
mod testing {
use super::aspect_ratio::AspectRatio;

use fission_core::authoring::{lower_widget_to_ir, BuildCtx};
use fission_core::ui::Text;
use fission_core::{build, Env, GlobalState, RuntimeState, View, Widget};
use fission_ir::{CoreIR, LayoutOp, Op};

#[derive(Default, Debug)]
struct TestState;
impl GlobalState for TestState {}

fn lower(env: &Env, build_widget: impl FnOnce() -> Widget) -> CoreIR {
    let state = TestState;
    let runtime = RuntimeState::default();
    let view = View::new(&state, &runtime, env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    let widget = build::enter(&mut ctx, &view, build_widget);
    lower_widget_to_ir(&widget)
}

#[test]
fn aspect_ratio_constrains_its_child() {
    let ir = lower(&Env::default(), || {
        AspectRatio { ratio: 16.0 / 9.0, child: Text::new("hi").into() }.into()
    });

    let ratio = ir.nodes.values().find_map(|node| match &node.op {
        Op::Layout(LayoutOp::Box { aspect_ratio, .. }) => *aspect_ratio,
        _ => None,
    });
    assert_eq!(ratio, Some(16.0 / 9.0));
}
}
