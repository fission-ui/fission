use anyhow::Result;
use fission_core::build;
use fission_core::env::Env;
use fission_core::internal::BuildCtx;
use fission_core::internal::InternalLoweringCx;
use fission_core::ui::{Grid, GridItem, TextInput, Widget};
use fission_core::Runtime;
use fission_core::{op::GridTrack, View, WidgetId, WidgetIdExt};
use fission_layout::{LayoutEngine, LineMetric, TextMeasurer};
use fission_render::{RenderScene, Renderer};
use fission_shell_desktop::Pipeline;
use fission_widgets::{HStack, MenuButton, MenuItem, VStack};
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
struct GlobalState {
    open: bool,
}
impl fission_core::action::GlobalState for GlobalState {}

struct MockRenderer;
impl Renderer for MockRenderer {
    fn render_scene(&mut self, _scene: &RenderScene) -> Result<()> {
        Ok(())
    }
}

struct MockMeasurer;
impl TextMeasurer for MockMeasurer {
    fn measure(&self, _text: &str, _font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        (80.0, 20.0)
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
        let (_ctx, _view) = fission_core::build::current::<GlobalState>();
        Grid {
            columns: vec![
                GridTrack::Points(220.0),
                GridTrack::Points(380.0),
                GridTrack::Fr(1.0),
            ],
            rows: vec![GridTrack::Fr(1.0)],
            children: vec![GridItem::new(VStack {
                spacing: Some(0.0),
                children: vec![HStack {
                    spacing: Some(8.0),
                    children: vec![
                        TextInput {
                            width: Some(200.0),
                            ..Default::default()
                        }
                        .into(),
                        MenuButton {
                            id: WidgetId::explicit("test_menu"),
                            label: "Filter".into(),
                            is_open: true,
                            on_toggle: None,
                            items: vec![
                                MenuItem {
                                    label: "All".into(),
                                    icon: None,
                                    on_select: None,
                                },
                                MenuItem {
                                    label: "Unread".into(),
                                    icon: None,
                                    on_select: None,
                                },
                            ],
                        }
                        .into(),
                    ],
                }
                .into()],
            })
            .cell(1, 2)
            .into()],
            ..Default::default()
        }
        .into()
    }
}
#[test]
fn menu_portal_position_near_anchor() -> Result<()> {
    let env = Env::default();
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(GlobalState { open: false }))?;
    let mut layout = LayoutEngine::new().with_measurer(Arc::new(MockMeasurer));
    let mut pipe = Pipeline::new();

    // Frame 1: closed (to capture anchor rect)
    let (node_tree, portals) = {
        let state = runtime.get_app_state::<GlobalState>().unwrap();
        let view = View::new(
            state,
            &runtime.runtime_state,
            &env,
            pipe.last_snapshot.as_ref(),
        );
        let mut ctx = BuildCtx::<GlobalState>::new();
        let node = build::enter(&mut ctx, &view, || Root.into());
        let portals_with_ids = ctx.take_portals();
        let portals: Vec<Widget> = portals_with_ids
            .into_iter()
            .map(|(id, node)| {
                if let Some(id) = id {
                    fission_core::ui::Container::new(node).id(id).into()
                } else {
                    node
                }
            })
            .collect();
        (node, portals)
    };

    // Compose overlay with portals
    let final_root = if portals.is_empty() {
        node_tree
    } else {
        use fission_core::ui::{Overlay, ZStack};
        Overlay {
            id: None,
            content: node_tree,
            overlay: ZStack {
                children: portals,
                ..Default::default()
            }
            .into(),
        }
        .into()
    };

    // InternalLower + layout
    let mut cx = InternalLoweringCx::new(
        &env,
        &runtime.runtime_state,
        None,
        pipe.last_snapshot.as_ref(),
    );
    let root_id = fission_core::internal::lower_widget(&final_root, &mut cx);
    cx.ir.root = Some(root_id);
    let ir = cx.ir;

    // Layout
    let viewport = fission_layout::LayoutSize {
        width: 1024.0,
        height: 768.0,
    };
    let env = fission_core::env::Env::default();
    let _ = pipe.render(
        ir.clone(),
        viewport,
        &mut layout,
        &runtime.runtime_state.scroll,
        &mut MockRenderer,
        &runtime.runtime_state.video,
        &runtime.runtime_state.web,
        &env,
    )?;

    let _snap = pipe.last_snapshot.clone().expect("snapshot");

    // Frame 2: open (should register a portal positioned by the previous snapshot rect)
    {
        // Mutate state
        if let Some(state) = runtime.get_app_state_mut::<GlobalState>() {
            state.open = true;
        }

        let (node_tree, portals) = {
            let state = runtime.get_app_state::<GlobalState>().unwrap();
            let view = View::new(
                state,
                &runtime.runtime_state,
                &env,
                pipe.last_snapshot.as_ref(),
            );
            let mut ctx = BuildCtx::<GlobalState>::new();
            let node = build::enter(&mut ctx, &view, || Root.into());
            let portals_with_ids = ctx.take_portals();
            let portals: Vec<Widget> = portals_with_ids
                .into_iter()
                .map(|(id, node)| {
                    if let Some(id) = id {
                        fission_core::ui::Container::new(node).id(id).into()
                    } else {
                        node
                    }
                })
                .collect();
            (node, portals)
        };

        let final_root = if portals.is_empty() {
            node_tree
        } else {
            use fission_core::ui::{Overlay, ZStack};
            Overlay {
                id: None,
                content: node_tree,
                overlay: ZStack {
                    children: portals,
                    ..Default::default()
                }
                .into(),
            }
            .into()
        };

        let mut cx = InternalLoweringCx::new(
            &env,
            &runtime.runtime_state,
            None,
            pipe.last_snapshot.as_ref(),
        );
        let root_id = fission_core::internal::lower_widget(&final_root, &mut cx);
        cx.ir.root = Some(root_id);
        let ir2 = cx.ir;

        let env = fission_core::env::Env::default();
        let _ = pipe.render(
            ir.clone(),
            viewport,
            &mut layout,
            &runtime.runtime_state.scroll,
            &mut MockRenderer,
            &runtime.runtime_state.video,
            &runtime.runtime_state.web,
            &env,
        )?;

        let snap2 = pipe.last_snapshot.clone().expect("snapshot2");

        let widget_id = WidgetId::explicit("test_menu");
        let anchor_node = WidgetId::derived(widget_id.as_u128(), &[]);
        let anchor_rect = snap2.get_node_rect(anchor_node).expect("anchor rect");

        // Find Flyout op and check its content's geometry
        let mut flyout_xs = Vec::new();
        let mut flyout_ys = Vec::new();
        for (_id, n) in &ir2.nodes {
            if let fission_ir::Op::Layout(fission_ir::LayoutOp::Flyout {
                anchor: _a,
                content,
            }) = n.op
            {
                if let Some(r) = snap2.get_node_rect(content) {
                    flyout_xs.push(r.x());
                    flyout_ys.push(r.y());
                }
            }
        }
        assert!(!flyout_xs.is_empty(), "no flyout nodes in IR (frame 2)");

        // X should match anchor's left within tolerance
        let ok_x = flyout_xs
            .iter()
            .any(|x| (*x - anchor_rect.x()).abs() < 20.0);
        if !ok_x {
            eprintln!("anchor_x={}, flyout_xs={:?}", anchor_rect.x(), flyout_xs);
        }
        assert!(ok_x, "no flyout content near anchor x");

        // Y should match anchor's bottom within tolerance (anchor.y + anchor.h)
        let anchor_bottom = anchor_rect.y() + anchor_rect.height();
        let ok_y = flyout_ys.iter().any(|y| (*y - anchor_bottom).abs() < 20.0);
        if !ok_y {
            eprintln!("anchor_bottom={}, flyout_ys={:?}", anchor_bottom, flyout_ys);
        }
        assert!(ok_y, "no flyout content near anchor y");
    }

    Ok(())
}
