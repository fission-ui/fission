use anyhow::Result;
use fission_core::action::GlobalState as CoreGlobalState;
use fission_core::authoring::BuildCtx;
use fission_core::authoring::LoweringContext;
use fission_core::env::Env;
use fission_core::ui::{Button, Column, Row, Scroll, Spacer, Text, TextInput, Widget};
use fission_core::{
    reduce_with, InputEvent, LayoutPoint, ReducerContext, Runtime, View, WidgetIdExt,
};
use fission_ir::{FlexDirection, Role};
use fission_layout::{LayoutEngine, LayoutSize, LineMetric, TextMeasurer};
use fission_render::{DisplayList, RenderScene, Renderer};
use fission_widgets::{Checkbox, Portal};
use std::sync::{Arc, Mutex};

use fission_shell_winit::Pipeline;

#[derive(Debug, Default, Clone)]
struct GlobalState {
    text: String,
    checked: bool,
    show_portal: bool,
}
impl CoreGlobalState for GlobalState {}

#[fission_macros::fission_action]
struct Toggle;
fn on_toggle(state: &mut GlobalState, _a: Toggle, _ctx: &mut ReducerContext<GlobalState>) {
    state.checked = !state.checked;
}

#[fission_macros::fission_action]
struct UpdateText;
fn on_update(state: &mut GlobalState, _a: UpdateText, ctx: &mut ReducerContext<GlobalState>) {
    if let Some(change) = ctx.input.text_change() {
        state.text = change.new_text.clone();
    }
}

#[derive(Clone, Default)]
struct MockRenderer(pub Arc<Mutex<Option<DisplayList>>>);
impl Renderer for MockRenderer {
    fn render_scene(&mut self, scene: &RenderScene) -> Result<()> {
        *self.0.lock().unwrap() = Some(scene.flatten());
        Ok(())
    }
}

struct MockMeasurer;
impl TextMeasurer for MockMeasurer {
    fn measure(&self, _text: &str, _font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        (0.0, 0.0)
    }
    fn hit_test(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        _x: f32,
        _y: f32,
    ) -> usize {
        0
    }
    fn get_line_metrics(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        vec![]
    }
}

#[derive(Clone)]
struct Root;
impl From<Root> for Widget {
    fn from(_component: Root) -> Self {
        let (ctx, view) = fission_core::build::current::<GlobalState>();
        use fission_core::ui::Column;
        let mut children: Vec<Widget> = vec![
            Checkbox {
                checked: view.state().checked,
                on_toggle: Some(ctx.bind(Toggle, reduce_with!(on_toggle))),
                label: Some("check".into()),
                ..Default::default()
            }
            .into(),
            TextInput {
                value: view.state().text.clone(),
                placeholder: Some("type".into()),
                on_input: Some(ctx.bind(UpdateText, reduce_with!(on_update))),
                width: Some(200.0),
                height: Some(40.0),
                ..Default::default()
            }
            .into(),
        ];
        if view.state().show_portal {
            use fission_core::ui::{Overlay, Row as UIRow, ZStack};
            let overlay = Overlay {
                id: None,
                content: UIRow::default().into(),
                overlay: ZStack::default().into(),
            };
            children.push(
                Portal {
                    child: overlay.into(),
                }
                .into(),
            );
        }
        Column {
            children,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct VirtualizedRows;

impl From<VirtualizedRows> for Widget {
    fn from(_component: VirtualizedRows) -> Self {
        const SCROLL_ID: fission_ir::WidgetId = fission_ir::WidgetId::from_u128(70_001);
        const COLUMNS_ID: fission_ir::WidgetId = fission_ir::WidgetId::from_u128(70_002);
        const ROW_HEIGHT: f32 = 30.0;
        const ROW_COUNT: usize = 100;
        const COLUMN_WIDTH: f32 = 40.0;
        const COLUMN_COUNT: usize = 30;

        let (ctx, view) = fission_core::build::current::<GlobalState>();
        ctx.mark_scroll_build_dependency(SCROLL_ID);
        ctx.mark_scroll_build_dependency(COLUMNS_ID);

        let offset = view.runtime().scroll.get_offset(SCROLL_ID).max(0.0);
        let first_row = (offset / ROW_HEIGHT).floor() as usize;
        let end_row = (first_row + 4).min(ROW_COUNT);
        let column_offset = view.runtime().scroll.get_offset(COLUMNS_ID).max(0.0);
        let first_column = (column_offset / COLUMN_WIDTH).floor() as usize;
        let end_column = (first_column + 4).min(COLUMN_COUNT);
        let mut column_children = vec![Spacer {
            width: Some(first_column as f32 * COLUMN_WIDTH),
            ..Default::default()
        }
        .into()];
        for column in first_column..end_column {
            let label = format!("column-{column}");
            column_children.push(
                Button {
                    id: Some(fission_ir::WidgetId::from_u128(70_200 + column as u128)),
                    child: Some(Text::new(label.clone()).into()),
                    semantics: Some(fission_ir::Semantics {
                        role: Role::Button,
                        label: Some(label),
                        ..Default::default()
                    }),
                    width: Some(COLUMN_WIDTH),
                    height: Some(40.0),
                    ..Default::default()
                }
                .into(),
            );
        }
        column_children.push(
            Spacer {
                width: Some((COLUMN_COUNT - end_column) as f32 * COLUMN_WIDTH),
                ..Default::default()
            }
            .into(),
        );
        let mut children = vec![Spacer {
            height: Some(first_row as f32 * ROW_HEIGHT),
            ..Default::default()
        }
        .into()];
        for row in first_row..end_row {
            let label = format!("row-{row}");
            children.push(
                Button {
                    id: Some(fission_ir::WidgetId::from_u128(70_100 + row as u128)),
                    child: Some(Text::new(label.clone()).into()),
                    semantics: Some(fission_ir::Semantics {
                        role: Role::Button,
                        label: Some(label),
                        ..Default::default()
                    }),
                    width: Some(140.0),
                    height: Some(ROW_HEIGHT),
                    ..Default::default()
                }
                .into(),
            );
        }
        children.push(
            Spacer {
                height: Some((ROW_COUNT - end_row) as f32 * ROW_HEIGHT),
                ..Default::default()
            }
            .into(),
        );

        Column {
            children: vec![
                Scroll {
                    id: Some(COLUMNS_ID),
                    direction: FlexDirection::Row,
                    width: Some(160.0),
                    height: Some(40.0),
                    show_scrollbar: false,
                    child: Some(
                        Row {
                            children: column_children,
                            ..Default::default()
                        }
                        .into(),
                    ),
                    ..Default::default()
                }
                .into(),
                Scroll {
                    id: Some(SCROLL_ID),
                    width: Some(160.0),
                    height: Some(90.0),
                    show_scrollbar: false,
                    child: Some(
                        Column {
                            children,
                            ..Default::default()
                        }
                        .into(),
                    ),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

fn pump<W>(
    runtime: &mut Runtime,
    layout: &mut LayoutEngine,
    pipe: &mut Pipeline,
    env: &Env,
    root: &W,
) -> (fission_ir::CoreIR, fission_layout::LayoutSnapshot)
where
    W: Clone + Into<Widget>,
{
    // Build + wrap portals like desktop
    let (node_tree, scroll_build_dependencies) = {
        let state = runtime.get_global_state::<GlobalState>().unwrap();
        let view = View::new(
            state,
            &runtime.runtime_state,
            env,
            pipe.last_snapshot.as_ref(),
        );
        let mut ctx = BuildCtx::new();
        let mut tree = fission_core::build::enter(&mut ctx, &view, || (*root).clone().into());
        let scroll_build_dependencies = ctx.take_scroll_build_dependencies();
        runtime.clear_reducers();
        let motion = ctx.take_motion_declarations();
        runtime.sync_motion_declarations(&motion, pipe.last_snapshot.as_ref());
        let vids = ctx.take_video_registrations();
        runtime.sync_video_nodes(&vids);
        let portals_with_ids = ctx.take_portals();
        let portals = portals_with_ids
            .into_iter()
            .map(|(id, node)| {
                if let Some(id) = id {
                    fission_core::ui::Container::new(node).id(id).into()
                } else {
                    node
                }
            })
            .collect::<Vec<_>>();
        if !portals.is_empty() {
            use fission_core::ui::{Overlay, Row, ZStack};
            let mut children = Vec::with_capacity(1 + portals.len());
            children.push(tree);
            for p in portals {
                children.push(
                    Overlay {
                        id: None,
                        content: Row::default().into(),
                        overlay: p,
                    }
                    .into(),
                );
            }
            tree = ZStack { id: None, children }.into();
        }
        (tree, scroll_build_dependencies)
    };
    // Lower
    let mut cx = LoweringContext::new(
        env,
        &runtime.runtime_state,
        None,
        pipe.last_snapshot.as_ref(),
    );
    for scroll_id in scroll_build_dependencies {
        cx.mark_scroll_build_dependency(scroll_id);
    }
    let root_id = fission_core::internal::lower_widget(&node_tree, &mut cx);
    cx.set_root(root_id);
    let ir = cx.into_ir();
    // Render via pipeline (performing layout inside)
    let viewport = LayoutSize {
        width: 800.0,
        height: 600.0,
    };
    let mut renderer = MockRenderer::default();
    let env = Env::default();
    let _ = pipe
        .render(
            ir.clone(),
            viewport,
            layout,
            &runtime.runtime_state.scroll,
            &mut renderer,
            &runtime.runtime_state.video,
            &runtime.runtime_state.web,
            &env,
        )
        .expect("render ok");
    let snap = pipe.last_snapshot.clone().expect("snapshot");
    (ir, snap)
}

fn button_labels(ir: &fission_ir::CoreIR) -> Vec<String> {
    ir.nodes
        .values()
        .filter_map(|node| match &node.op {
            fission_ir::Op::Semantics(semantics) if semantics.role == Role::Button => {
                semantics.label.clone()
            }
            _ => None,
        })
        .collect()
}

#[test]
fn declared_scroll_dependency_rebuilds_custom_virtualized_rows() -> Result<()> {
    use fission_core::event::{PointerEvent, PointerPhase, ScrollDeltaMode};

    const SCROLL_ID: fission_ir::WidgetId = fission_ir::WidgetId::from_u128(70_001);
    const COLUMNS_ID: fission_ir::WidgetId = fission_ir::WidgetId::from_u128(70_002);
    let env = Env::default();
    let mut runtime = Runtime::default();
    runtime.add_global_state(Box::new(GlobalState::default()))?;
    let mut layout = LayoutEngine::new().with_measurer(Arc::new(MockMeasurer));
    let mut pipe = Pipeline::new();

    let (initial_ir, initial_layout) =
        pump(&mut runtime, &mut layout, &mut pipe, &env, &VirtualizedRows);
    assert!(initial_ir.scroll_build_dependencies.contains(&SCROLL_ID));
    assert!(initial_ir.scroll_build_dependencies.contains(&COLUMNS_ID));
    assert!(button_labels(&initial_ir).contains(&"row-0".to_owned()));
    assert!(button_labels(&initial_ir).contains(&"row-2".to_owned()));
    assert!(!button_labels(&initial_ir).contains(&"row-4".to_owned()));
    assert!(button_labels(&initial_ir).contains(&"column-0".to_owned()));
    assert!(button_labels(&initial_ir).contains(&"column-1".to_owned()));
    assert!(!button_labels(&initial_ir).contains(&"column-4".to_owned()));

    let before_offset = runtime.runtime_state.scroll.get_offset(SCROLL_ID);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Scroll {
            point: LayoutPoint::new(60.0, 80.0),
            delta: LayoutPoint::new(0.0, 60.0),
            delta_mode: ScrollDeltaMode::Pixel,
            phase: PointerPhase::Moved,
            modifiers: 0,
        }),
        &initial_ir,
        &initial_layout,
    )?;
    assert_eq!(runtime.runtime_state.scroll.get_offset(SCROLL_ID), 60.0);
    assert_ne!(
        runtime.runtime_state.scroll.get_offset(SCROLL_ID),
        before_offset
    );

    let (scrolled_ir, scrolled_layout) =
        pump(&mut runtime, &mut layout, &mut pipe, &env, &VirtualizedRows);
    assert!(scrolled_ir.scroll_build_dependencies.contains(&SCROLL_ID));
    assert!(button_labels(&scrolled_ir).contains(&"row-2".to_owned()));
    assert!(!button_labels(&scrolled_ir).contains(&"row-0".to_owned()));
    assert!(button_labels(&scrolled_ir).contains(&"row-5".to_owned()));

    let before_horizontal = runtime.runtime_state.scroll.get_offset(COLUMNS_ID);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Scroll {
            point: LayoutPoint::new(60.0, 20.0),
            delta: LayoutPoint::new(40.0, 0.0),
            delta_mode: ScrollDeltaMode::Pixel,
            phase: PointerPhase::Moved,
            modifiers: 0,
        }),
        &scrolled_ir,
        &scrolled_layout,
    )?;
    assert_eq!(runtime.runtime_state.scroll.get_offset(COLUMNS_ID), 40.0);
    assert_ne!(
        runtime.runtime_state.scroll.get_offset(COLUMNS_ID),
        before_horizontal
    );

    let (horizontally_scrolled_ir, _) =
        pump(&mut runtime, &mut layout, &mut pipe, &env, &VirtualizedRows);
    assert!(horizontally_scrolled_ir
        .scroll_build_dependencies
        .contains(&COLUMNS_ID));
    assert!(button_labels(&horizontally_scrolled_ir).contains(&"column-1".to_owned()));
    assert!(!button_labels(&horizontally_scrolled_ir).contains(&"column-0".to_owned()));
    assert!(button_labels(&horizontally_scrolled_ir).contains(&"column-4".to_owned()));
    Ok(())
}

#[test]
fn strict_incremental_checkbox_textinput_checkbox_sequence() -> Result<()> {
    // Strict incremental only
    std::env::set_var("FISSION_LAYOUT_STRICT", "1");
    std::env::remove_var("FISSION_ALLOW_FULL_REBUILD");

    let env = Env::default();
    let mut runtime = Runtime::default();
    runtime.add_global_state(Box::new(GlobalState::default()))?;
    let mut layout = LayoutEngine::new().with_measurer(Arc::new(MockMeasurer));
    let mut pipe = Pipeline::new();
    let root = Root;

    // Initial frame
    let (ir, snap) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // Toggle checkbox (down+up)
    let mut cb = None;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == Role::Checkbox {
                cb = Some(*id);
                break;
            }
        }
    }
    let cb = cb.expect("checkbox semantics");
    let r = snap.get_node_rect(cb).unwrap();
    let p = LayoutPoint::new(r.x() + r.width() / 2.0, r.y() + r.height() / 2.0);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: p,
            button: fission_core::event::PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &snap,
    )?;
    let (_ir2, _sn2) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: p,
            button: fission_core::event::PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &snap,
    )?;
    let (_ir3, _sn3) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // Focus text input
    let (ir4, sn4) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    let mut ti = None;
    for (id, node) in &ir4.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == Role::TextInput {
                ti = Some(*id);
                break;
            }
        }
    }
    let ti = ti.expect("text input semantics");
    let tr = sn4.get_node_rect(ti).unwrap();
    let tp = LayoutPoint::new(tr.x() + tr.width() / 2.0, tr.y() + tr.height() / 2.0);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: tp,
            button: fission_core::event::PointerButton::Primary,
            modifiers: 0,
        }),
        &ir4,
        &sn4,
    )?;
    let (_ir5, _sn5) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // Toggle checkbox again
    let (ir6, sn6) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: p,
            button: fission_core::event::PointerButton::Primary,
            modifiers: 0,
        }),
        &ir6,
        &sn6,
    )?;
    let (_ir7, _sn7) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: p,
            button: fission_core::event::PointerButton::Primary,
            modifiers: 0,
        }),
        &ir6,
        &sn6,
    )?;
    let (_ir8, _sn8) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // If we reached here without panic, strict incremental survived the sequence.
    Ok(())
}
