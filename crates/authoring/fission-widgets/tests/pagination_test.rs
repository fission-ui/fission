use fission_core::internal::BuildCtx;
use fission_core::{build, GlobalState, LayoutDirection, View, Widget};
use fission_ir::op::{Color, Fill};
use fission_widgets::pagination::Pagination;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    page: usize,
}
impl GlobalState for TestState {}

#[test]
fn test_pagination_structure() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let pagination = Pagination {
        current_page: 1,
        total_pages: 5,
        on_change: None,
    };

    let node = build::enter(&mut ctx, &view, || pagination.into());
    assert_eq!(fission_core::internal::widget_kind_name(&node), "Row"); // It builds a Row (HStack)
}

fn build_pagination(env: &fission_core::Env, current_page: usize, total_pages: usize) -> Widget {
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    build::enter(&mut ctx, &view, || {
        Pagination {
            current_page,
            total_pages,
            on_change: None,
        }
        .into()
    })
}

#[test]
fn pagination_geometry_and_current_page_come_from_component_recipe() {
    let mut env = fission_core::Env::default();
    let selected = Color {
        r: 23,
        g: 45,
        b: 67,
        a: 255,
    };
    env.theme.components.pagination.item_style.width = Some(31.0);
    env.theme.components.pagination.item_style.height = Some(29.0);
    env.theme.components.pagination.selected_style.background = Some(Fill::Solid(selected));

    let widget = build_pagination(&env, 1, 5);
    let row = fission_core::internal::widget_as_row(&widget).expect("pagination row");
    let current =
        fission_core::internal::widget_as_button(&row.children[1]).expect("current page button");
    let style = current.style.as_ref().expect("pagination recipe override");

    assert_eq!(style.width, Some(31.0));
    assert_eq!(style.height, Some(29.0));
    assert_eq!(style.background_fill, Some(Fill::Solid(selected)));
    assert_eq!(
        current
            .semantics
            .as_ref()
            .and_then(|semantics| semantics.selected),
        Some(true)
    );
    assert_eq!(
        current
            .semantics
            .as_ref()
            .and_then(|semantics| semantics.label.as_deref()),
        Some("1")
    );
}

#[test]
fn pagination_directional_controls_follow_layout_direction() {
    let ltr = build_pagination(&fission_core::Env::default(), 2, 5);
    let mut rtl_env = fission_core::Env::default();
    rtl_env.layout_direction = LayoutDirection::RightToLeft;
    let rtl = build_pagination(&rtl_env, 2, 5);

    let first_icon = |widget: &Widget| {
        let row = fission_core::internal::widget_as_row(widget).expect("pagination row");
        let button = fission_core::internal::widget_as_button(&row.children[0])
            .expect("previous page button");
        match button.child.as_ref().map(Widget::kind) {
            Some(fission_core::ui::WidgetKind::Icon(icon)) => format!("{:?}", icon.source),
            other => panic!("expected directional icon, got {other:?}"),
        }
    };

    assert_ne!(first_icon(&ltr), first_icon(&rtl));
}
