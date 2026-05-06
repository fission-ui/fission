use anyhow::Result;
use fission_core::env::Env;
use fission_core::lowering::LoweringContext;
use fission_core::ui::{Grid, GridItem, Node, TextInput};
use fission_core::Runtime;
use fission_core::{op::GridTrack, BuildCtx, NodeId, View, Widget, WidgetNodeId};
use fission_layout::{LayoutEngine, LineMetric, TextMeasurer};
use fission_render::{RenderScene, Renderer};
use fission_shell_desktop::Pipeline;
use fission_widgets::{HStack, MenuButton, MenuItem, VStack};
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
struct AppState {
    open: bool,
}
impl fission_core::action::AppState for AppState {}

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

struct Root;
impl Widget<AppState> for Root {
    fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> Node {
        Grid {
            columns: vec![
                GridTrack::Points(220.0),
                GridTrack::Points(380.0),
                GridTrack::Fr(1.0),
            ],
            rows: vec![GridTrack::Fr(1.0)],
            children: vec![GridItem::new(
                VStack {
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
                                id: WidgetNodeId::explicit("test_menu"),
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
                            .build(ctx, view),
                        ],
                    }
                    .build(ctx, view)],
                }
                .build(ctx, view),
            )
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
    runtime.add_app_state(Box::new(AppState { open: false }))?;
    let mut layout = LayoutEngine::new().with_measurer(Arc::new(MockMeasurer));
    let mut pipe = Pipeline::new();

    // Frame 1: closed (to capture anchor rect)
    let (node_tree, portals) = {
        let state = runtime.get_app_state::<AppState>().unwrap();
        let view = View::new(
            state,
            &runtime.runtime_state,
            &env,
            pipe.last_snapshot.as_ref(),
        );
        let mut ctx = BuildCtx::new();
        let node = Root.build(&mut ctx, &view);
        let portals_with_ids = ctx.take_portals();
        let portals: Vec<Node> = portals_with_ids
            .into_iter()
            .map(|(id, node)| {
                if let Some(id) = id {
                    fission_core::ui::Container::new(node)
                        .id(id.into())
                        .into_node()
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
        Node::Overlay(Overlay {
            id: None,
            content: Box::new(node_tree),
            overlay: Box::new(Node::ZStack(ZStack {
                children: portals,
                ..Default::default()
            })),
        })
    };

    // Lower + layout
    let mut cx = LoweringContext::new(
        &env,
        &runtime.runtime_state,
        None,
        pipe.last_snapshot.as_ref(),
    );
    let root_id = final_root.lower(&mut cx);
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
        if let Some(state) = runtime.get_app_state_mut::<AppState>() {
            state.open = true;
        }

        let (node_tree, portals) = {
            let state = runtime.get_app_state::<AppState>().unwrap();
            let view = View::new(
                state,
                &runtime.runtime_state,
                &env,
                pipe.last_snapshot.as_ref(),
            );
            let mut ctx = BuildCtx::new();
            let node = Root.build(&mut ctx, &view);
            let portals_with_ids = ctx.take_portals();
            let portals: Vec<Node> = portals_with_ids
                .into_iter()
                .map(|(id, node)| {
                    if let Some(id) = id {
                        fission_core::ui::Container::new(node)
                            .id(id.into())
                            .into_node()
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
            Node::Overlay(Overlay {
                id: None,
                content: Box::new(node_tree),
                overlay: Box::new(Node::ZStack(ZStack {
                    children: portals,
                    ..Default::default()
                })),
            })
        };

        let mut cx = LoweringContext::new(
            &env,
            &runtime.runtime_state,
            None,
            pipe.last_snapshot.as_ref(),
        );
        let root_id = final_root.lower(&mut cx);
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

        let widget_id = WidgetNodeId::explicit("test_menu");
        let anchor_node = NodeId::derived(widget_id.as_u128(), &[]);
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
