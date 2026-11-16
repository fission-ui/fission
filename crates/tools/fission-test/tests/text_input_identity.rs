//! A text field keeps its focus while content built before it, but placed after it, changes as the user types.

use fission_core::ui::{Button, Column, Text, TextInput, Widget};
use fission_core::{GlobalState, ReducerContext};
use fission_ir::semantics::Role;
use fission_ir::WidgetId;
use fission_layout::{LayoutSize, ResolvedParagraphLayout, TextMeasurer};
use fission_test::{TestDriver, TestHarness};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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
struct SearchPage;

impl From<SearchPage> for Widget {
    fn from(_: SearchPage) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        // The results are built before the field, as a page component often is, but placed after
        // it. They change as the user types: a button only appears once there is a query.
        let results: Widget = if view.state().text.is_empty() {
            Text::new("No query").into()
        } else {
            Button {
                child: Some(Text::new("Clear").into()),
                ..Default::default()
            }
            .into()
        };
        let field: Widget = TextInput {
            value: view.state().text.clone(),
            on_input: Some(ctx.bind(
                UpdateText,
                update_text as fn(&mut State, UpdateText, &mut ReducerContext<State>),
            )),
            width: Some(300.0),
            ..Default::default()
        }
        .into();
        Column {
            children: vec![field, results],
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn a_field_without_an_id_keeps_focus_when_content_built_before_it_changes() {
    let mut driver =
        TestDriver::new(TestHarness::new(State::default()).with_root_widget(SearchPage));
    driver.pump().expect("first frame");

    let field = driver
        .find_role(Role::TextInput)
        .into_iter()
        .next()
        .expect("text field")
        .bounds;
    driver
        .tap_point(field.x() + 20.0, field.y() + field.height() / 2.0)
        .expect("focus the field");
    let focused = driver.harness.runtime.runtime_state.interaction.focused;
    assert!(focused.is_some(), "tapping the field focuses it");

    driver.type_text("ab").expect("type");
    driver.pump().expect("pump after typing");

    assert_eq!(
        driver.harness.runtime.runtime_state.interaction.focused, focused,
        "the field keeps its identity and focus while the results change"
    );
    assert!(
        driver.find_text("ab").is_some(),
        "both characters reach the field"
    );
}

struct CountingTextMeasurer(Arc<AtomicUsize>);

impl TextMeasurer for CountingTextMeasurer {
    fn measure(&self, text: &str, font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let width = text.chars().count() as f32 * font_size * 0.55;
        (
            width.min(available_width.unwrap_or(f32::INFINITY)),
            font_size * 1.4,
        )
    }

    fn resolve_rich_text(
        &self,
        runs: &[fission_ir::op::TextRun],
        available_width: Option<f32>,
    ) -> ResolvedParagraphLayout {
        self.0.fetch_add(1, Ordering::Relaxed);
        let info = self.layout_rich_text(runs, available_width);
        ResolvedParagraphLayout {
            constraint_width: available_width,
            size: LayoutSize::new(info.width, info.height),
            lines: Vec::new(),
            inline_boxes: info.inline_boxes,
            clusters: Vec::new(),
            glyphs: Vec::new(),
            caret_stops: Vec::new(),
            selection_boxes: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct FocusTransferPage;

impl From<FocusTransferPage> for Widget {
    fn from(_: FocusTransferPage) -> Self {
        Column {
            gap: Some(12.0),
            children: vec![
                TextInput {
                    id: Some(WidgetId::explicit("paragraph-cache.first")),
                    value: "First report paragraph stays the same while focus moves.".into(),
                    width: Some(360.0),
                    height: Some(44.0),
                    ..Default::default()
                }
                .into(),
                TextInput {
                    id: Some(WidgetId::explicit("paragraph-cache.second")),
                    value: "Second report paragraph stays the same while focus moves.".into(),
                    width: Some(360.0),
                    height: Some(44.0),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

fn rich_text_paint_id(driver: &TestDriver<State>, text: &str) -> WidgetId {
    driver
        .find_text(text)
        .and_then(|matched| matched.node_id)
        .expect("rich text paint node")
}

#[test]
fn focusing_text_inputs_preserves_paragraph_identity_and_reuses_layout() {
    const FIRST: &str = "First report paragraph stays the same while focus moves.";
    const SECOND: &str = "Second report paragraph stays the same while focus moves.";

    let resolutions = Arc::new(AtomicUsize::new(0));
    let measurer = Arc::new(CountingTextMeasurer(resolutions.clone()));
    let harness = TestHarness::new_with_measurer(State::default(), measurer)
        .with_root_widget(FocusTransferPage);
    let mut driver = TestDriver::new(harness);
    driver.pump().expect("initial frame");

    let first_id = rich_text_paint_id(&driver, FIRST);
    let second_id = rich_text_paint_id(&driver, SECOND);
    assert_eq!(resolutions.load(Ordering::Relaxed), 2);

    let first_field = driver.find_role(Role::TextInput)[0].bounds;
    driver
        .tap_point(
            first_field.x() + 20.0,
            first_field.y() + first_field.height() / 2.0,
        )
        .expect("focus the first field");

    assert_eq!(rich_text_paint_id(&driver, FIRST), first_id);
    assert_eq!(rich_text_paint_id(&driver, SECOND), second_id);
    assert_eq!(
        resolutions.load(Ordering::Relaxed),
        2,
        "focus decoration changes should reuse unchanged paragraph geometry"
    );

    let second_field = driver.find_role(Role::TextInput)[1].bounds;
    driver
        .tap_point(
            second_field.x() + 20.0,
            second_field.y() + second_field.height() / 2.0,
        )
        .expect("transfer focus to the second field");

    assert_eq!(rich_text_paint_id(&driver, FIRST), first_id);
    assert_eq!(rich_text_paint_id(&driver, SECOND), second_id);
    assert_eq!(
        resolutions.load(Ordering::Relaxed),
        2,
        "moving focus between fields should not resolve either paragraph again"
    );
}
