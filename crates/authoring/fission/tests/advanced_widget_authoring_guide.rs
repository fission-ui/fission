//! Compiles the Rust in the advanced widget authoring guide.
//!
//! Each module below holds one of the guide's code blocks verbatim, and
//! `the_guide_matches_this_file` fails when the two drift apart, so the guide
//! cannot show code that does not build. Edit the guide and this file together.

const GUIDE: &str =
    include_str!("../../../../docs2/content/legacy/docs/guides/advanced-widget-authoring.mdx");
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
use fission::prelude::*;

fission_component! {
    pub PriceTag<()> {
        props { formatted_price: String }
    }
    view(tag, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Text(tag.formatted_price.clone(), font_size: tokens.typography.heading2_size)
}
}

#[rustfmt::skip]
#[allow(dead_code)]
mod aspect_frame {
use fission_core::authoring::{custom_widget, lower_widget, IrBuilder, LowerWidget, LoweringContext};
use fission_core::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};
use fission::prelude::fission_component;

/// Constrains one child to a width-to-height ratio.
fission_component! {
    pub ExampleAspectFrame<()> {
        props {
            ratio: f32,
            child: Widget,
        }
    }
    view(frame, _state, _cx) =>
    custom_widget("ExampleAspectFrame", AspectFrameLowerer {
        ratio: frame.ratio,
        child: frame.child.clone(),
    })
}

#[derive(Debug)]
struct AspectFrameLowerer {
    ratio: f32,
    child: Widget,
}

impl LowerWidget for AspectFrameLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        assert!(self.ratio.is_finite() && self.ratio > 0.0, "aspect ratio must be finite and positive");
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

    fn children(&self) -> Vec<&Widget> {
        vec![&self.child]
    }
}
}

#[rustfmt::skip]
#[allow(dead_code)]
mod testing {
use super::aspect_frame::ExampleAspectFrame;

use fission_core::authoring::{lower_widget_to_ir_in, BuildCtx};
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
    lower_widget_to_ir_in(env, &widget)
}

#[test]
fn aspect_ratio_constrains_its_child() {
    let ir = lower(&Env::default(), || {
        ExampleAspectFrame::new(16.0 / 9.0, Text::new("Preview").into()).into()
    });

    let ratio = ir.nodes.values().find_map(|node| match &node.op {
        Op::Layout(LayoutOp::Box { aspect_ratio, .. }) => *aspect_ratio,
        _ => None,
    });
    assert_eq!(ratio, Some(16.0 / 9.0));
}
}
