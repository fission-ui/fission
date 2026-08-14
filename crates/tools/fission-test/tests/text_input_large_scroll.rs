use anyhow::Result;
use fission_core::event::{ImeEvent, InputEvent};
use fission_core::op::{Color, FlexDirection};
use fission_core::ui::{Column, Container, Positioned, Scroll, Spacer, TextInput, Widget, ZStack};
use fission_core::{GlobalState, ReducerContext};
use fission_ir::semantics::Role;
use fission_ir::{Op, PaintOp, WidgetId};
use fission_test::{TestDriver, TestHarness};

#[derive(Debug, Default, Clone)]
struct State {
    text: String,
}

impl GlobalState for State {}

#[fission_macros::fission_action]
struct UpdateText;

fn update_text(state: &mut State, _action: UpdateText, ctx: &mut ReducerContext<State>) {
    if let Some(change) = ctx.input.text_change() {
        state.text = change.new_text.clone();
    }
}

#[derive(Clone)]
struct LargeScrollEditor;

impl From<LargeScrollEditor> for Widget {
    fn from(_component: LargeScrollEditor) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        let paper = Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        let ink = Color {
            r: 15,
            g: 23,
            b: 42,
            a: 255,
        };
        let border = Color {
            r: 226,
            g: 232,
            b: 240,
            a: 255,
        };
        let text_input = TextInput {
            id: Some(WidgetId::explicit("large.editor.body")),
            value: view.state().text.clone(),
            on_input: Some(ctx.bind(
                UpdateText,
                update_text as fn(&mut State, UpdateText, &mut ReducerContext<State>),
            )),
            width: Some(672.0),
            height: Some(912.0),
            multiline: true,
            borderless: true,
            capture_tab: false,
            auto_indent: true,
            font_size: Some(14.5),
            line_height: Some(22.0),
            text_color: Some(ink),
            padding: Some([0.0, 0.0, 0.0, 0.0]),
            expands: true,
            scroll_padding: Some([16.0, 16.0, 48.0, 48.0]),
            selection_controls: fission_core::ui::widgets::text_input::TextSelectionControls {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        }
        .semantics_identifier("large.editor.body");

        let page: Widget = Container::new(ZStack {
            children: vec![
                Spacer {
                    width: Some(816.0),
                    height: Some(1056.0),
                    ..Default::default()
                }
                .into(),
                Positioned {
                    left: Some(72.0),
                    top: Some(72.0),
                    width: Some(672.0),
                    height: Some(912.0),
                    child: Some(text_input.into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        })
        .width(816.0)
        .height(1056.0)
        .bg(paper)
        .border(border, 1.0)
        .into();

        Container::new(Scroll {
            id: Some(WidgetId::explicit("large.editor.scroll")),
            child: Some(
                Container::new(Column {
                    children: vec![page],
                    ..Default::default()
                })
                .width(816.0)
                .height(1056.0)
                .into(),
            ),
            direction: FlexDirection::Column,
            width: Some(752.0),
            height: Some(400.0),
            show_scrollbar: true,
            ..Default::default()
        })
        .width(752.0)
        .height(400.0)
        .bg(Color {
            r: 248,
            g: 250,
            b: 252,
            a: 255,
        })
        .into()
    }
}

#[test]
fn large_multiline_text_input_paints_committed_text_inside_scrolled_page() -> Result<()> {
    let marker = "Fission large editor marker\nSaved through IME\n";
    let mut driver =
        TestDriver::new(TestHarness::new(State::default()).with_root_widget(LargeScrollEditor));
    driver.harness.env.viewport_size = fission_layout::LayoutSize::new(800.0, 600.0);
    driver.pump()?;

    let scroll_id = WidgetId::explicit("large.editor.scroll");
    driver
        .harness
        .runtime
        .runtime_state
        .scroll
        .set_offset(scroll_id, 72.0);
    driver.pump()?;

    let input = driver
        .find_role(Role::TextInput)
        .into_iter()
        .find(|record| record.node_id == WidgetId::explicit("large.editor.body"))
        .expect("large editor TextInput semantics");
    driver
        .harness
        .runtime
        .runtime_state
        .interaction
        .set_focused(Some(input.node_id));
    driver
        .harness
        .send_event(InputEvent::Ime(ImeEvent::Commit {
            text: marker.to_string(),
        }))?;
    driver.pump()?;

    let ir = driver.harness.last_ir.as_ref().expect("IR");
    let semantics = ir
        .nodes
        .get(&input.node_id)
        .and_then(|node| match &node.op {
            Op::Semantics(semantics) => Some(semantics),
            _ => None,
        })
        .expect("TextInput semantics after commit");
    assert_eq!(semantics.value.as_deref(), Some(marker));

    let display_text = driver
        .harness
        .get_last_display_list()
        .expect("display list")
        .ops
        .iter()
        .filter_map(|op| match op {
            fission_render::DisplayOp::DrawText { text, .. } => Some(text.clone()),
            fission_render::DisplayOp::DrawRichText { runs, .. } => {
                Some(runs.iter().map(|run| run.text.as_str()).collect::<String>())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        display_text.iter().any(|text| text.contains(marker)),
        "display list did not contain committed TextInput text; visible text={display_text:?}"
    );

    let paint_text = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Paint(PaintOp::DrawRichText { runs, .. }) => {
                Some(runs.iter().map(|run| run.text.as_str()).collect::<String>())
            }
            Op::Paint(PaintOp::DrawText { text, .. }) => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        paint_text.iter().any(|text| text.contains(marker)),
        "IR paint nodes did not contain committed TextInput text; paint text={paint_text:?}"
    );

    Ok(())
}
