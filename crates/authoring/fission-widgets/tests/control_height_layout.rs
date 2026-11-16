//! A select laid out beside a button is exactly as tall as the button, in
//! every design system and at every density.

use fission_core::action::GlobalState;
use fission_core::authoring::{BuildCtx, LoweringContext};
use fission_core::env::{Env, RuntimeState};
use fission_core::internal::build_layout_tree;
use fission_core::ui::{Button, ButtonVariant, Row, Text};
use fission_core::{View, Widget};
use fission_ir::{CoreIR, Op, Role, WidgetId};
use fission_layout::{LayoutEngine, LayoutSize, TextMeasurer};
use fission_theme::{
    Density, DesignMode, DesignSystem, FissionCupertinoDesignSystem, FissionDefaultDesignSystem,
    FissionEmberDesignSystem, FissionFluent2DesignSystem, FissionGraphiteDesignSystem,
    FissionLiquidGlassDesignSystem, FissionMaterialDesign3DesignSystem, Theme,
};
use fission_widgets::Select;
use std::sync::Arc;

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

struct Measurer;

impl TextMeasurer for Measurer {
    fn measure(&self, text: &str, font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        (text.len() as f32 * font_size * 0.5, font_size * 1.25)
    }

    fn measure_rich_text(
        &self,
        runs: &[fission_ir::op::TextRun],
        available_width: Option<f32>,
    ) -> (f32, f32) {
        let text: String = runs.iter().map(|run| run.text.as_str()).collect();
        let size = runs.first().map_or(14.0, |run| run.style.font_size);
        self.measure(&text, size, available_width)
    }
}

fn height_of_role(ir: &CoreIR, snapshot: &fission_layout::LayoutSnapshot, role: Role) -> f32 {
    let (id, node) = ir
        .nodes
        .iter()
        .find(|(_, node)| matches!(&node.op, Op::Semantics(semantics) if semantics.role == role))
        .unwrap_or_else(|| panic!("{role:?} semantics node"));
    let layout: WidgetId = node.children.first().copied().unwrap_or(*id);
    snapshot
        .get_node_rect(layout)
        .unwrap_or_else(|| panic!("{role:?} layout rect"))
        .height()
}

fn button_height(ir: &CoreIR, snapshot: &fission_layout::LayoutSnapshot, id: WidgetId) -> f32 {
    let layout = match &ir.nodes[&id].op {
        Op::Semantics(_) => ir.nodes[&id].children[0],
        _ => id,
    };
    snapshot
        .get_node_rect(layout)
        .expect("button layout rect")
        .height()
}

fn laid_out_heights(theme: Theme) -> (f32, f32) {
    let mut env = Env::default();
    env.theme = theme;
    let runtime = RuntimeState::default();
    let state = TestState;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::new();
    let widget = fission_core::build::enter(&mut ctx, &view, || -> Widget {
        Row {
            children: vec![
                Select {
                    selected_label: Some("Newest".into()),
                    ..Default::default()
                }
                .into(),
                Button {
                    id: Some(WidgetId::explicit("filters")),
                    variant: ButtonVariant::Outline,
                    child: Some(Text::new("Filters").into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    });

    let measurer: Arc<dyn TextMeasurer> = Arc::new(Measurer);
    let mut lower = LoweringContext::new(&env, &runtime, Some(&measurer), None);
    let root = fission_core::internal::lower_widget(&widget, &mut lower);
    lower.set_root(root);
    let ir = lower.into_ir();

    let nodes = build_layout_tree(&ir, &env);
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(Measurer));
    engine.rebuild(&nodes).unwrap();
    let snapshot = engine
        .compute_layout(&nodes, root, LayoutSize::new(800.0, 400.0), &|_| 0.0)
        .unwrap();

    (
        height_of_role(&ir, &snapshot, Role::ComboBox),
        button_height(&ir, &snapshot, WidgetId::explicit("filters")),
    )
}

#[test]
fn a_select_beside_a_button_matches_its_height_in_every_preset_and_density() {
    let presets: [(&str, fn(DesignMode) -> Theme); 7] = [
        ("Tidewater", FissionDefaultDesignSystem::theme),
        ("Graphite", FissionGraphiteDesignSystem::theme),
        ("Ember", FissionEmberDesignSystem::theme),
        ("Material 3", FissionMaterialDesign3DesignSystem::theme),
        ("Fluent 2", FissionFluent2DesignSystem::theme),
        ("Cupertino", FissionCupertinoDesignSystem::theme),
        ("Liquid Glass", FissionLiquidGlassDesignSystem::theme),
    ];
    for (name, theme) in presets {
        for density in [Density::Compact, Density::Comfortable, Density::Spacious] {
            let (select, button) = laid_out_heights(theme(DesignMode::Light).with_density(density));
            assert!(
                (select - button).abs() < 0.5,
                "{name} {density:?}: select {select} vs button {button}"
            );
        }
    }
}
