use fission_core::authoring::BuildCtx;
use fission_core::{build, GlobalState, View, WidgetId};
use fission_widgets::DateRangePicker;

#[derive(Clone, Debug, Default)]
struct State;

impl GlobalState for State {}

#[test]
fn date_range_spacing_comes_from_the_active_design_system() {
    let mut env = fission_core::Env::default();
    env.theme.tokens.spacing.s = 13.0;
    let runtime = fission_core::RuntimeState::default();
    let state = State;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let widget: fission_core::Widget = build::enter(&mut ctx, &view, || {
        DateRangePicker {
            id_start: WidgetId::explicit("start"),
            id_end: WidgetId::explicit("end"),
            start: None,
            end: None,
            is_start_open: false,
            is_end_open: false,
            on_change: None,
            on_toggle_start: None,
            on_toggle_end: None,
            on_close_start: None,
            on_close_end: None,
        }
        .into()
    });
    let fission_core::ui::WidgetKind::SemanticsRegion(region) = widget.kind() else {
        panic!("date range should expose a semantic group");
    };
    let row =
        fission_core::internal::widget_as_row(region.child.as_ref().expect("date range content"))
            .expect("date range uses a wrapping row");
    assert_eq!(row.gap, Some(13.0));
    assert_eq!(row.line_gap, Some(13.0));
}
