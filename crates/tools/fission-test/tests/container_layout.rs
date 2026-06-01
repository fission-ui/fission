use fission_core::op::Color;
use fission_core::ui::Container;
use fission_core::IntoWidget;
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct State;
impl AppState for State {}

#[test]
fn test_container_background_fills_border_box() {
    // Regression test for "Toast border smaller than content".
    // Container with padding should paint its background/border on the full size (Border Box),
    // not the inner Padding Box.

    let padding = 10.0;
    let width = 100.0;
    let height = 100.0;

    #[derive(Clone)]
    struct Root {
        p: f32,
        _w: f32,
        _h: f32,
    }
    impl Widget<State> for Root {
        fn build(
            &self,
            ctx: &mut BuildCtx<State>,
            view: &View<State>,
        ) -> impl fission_core::IntoWidget<State> {
            fission_core::view::internal_node_widget({
                // Use Align to prevent stretching the test container
                fission_core::ui::widgets::align::Align::new(
                    Container::new(
                        // Content: Large box
                        Container::new(fission_core::ui::widgets::spacer::Spacer::default())
                            .width(200.0)
                            .height(200.0)
                            .bg(Color::BLUE),
                    )
                    // Auto size
                    .padding_all(self.p)
                    .bg(Color::RED) // The background
                    .build(ctx, view)
                    .into_widget()
                    .lower_to_node(ctx, view),
                )
                .into_node()
            })
        }
    }

    let mut h = TestHarness::new(State);
    h = h.with_root_widget(Root {
        p: padding,
        _w: width,
        _h: height,
    });
    h.pump().unwrap();

    let ir = h.last_ir.as_ref().unwrap();
    let snap = h.last_snapshot.as_ref().unwrap();

    let real_root_id = ir.root.unwrap();
    let root_node = ir.nodes.get(&real_root_id).unwrap();
    let container_id = root_node.children[0]; // Child of Align

    let container_geom = snap.get_node_geometry(container_id).unwrap();

    // Expected: 200 + 10 + 10 = 220
    println!("Container Rect: {:?}", container_geom.rect);
    assert_eq!(container_geom.rect.width(), 220.0);
    assert_eq!(container_geom.rect.height(), 220.0);

    let container_node = ir.nodes.get(&container_id).unwrap();
    let bg_id = container_node.children[0];
    let bg_geom = snap.get_node_geometry(bg_id).unwrap();

    println!("Background Rect: {:?}", bg_geom.rect);

    // If bug exists, background might be smaller (e.g. 200x200 if stuck in padding box)
    assert_eq!(bg_geom.rect.width(), 220.0);
    assert_eq!(bg_geom.rect.height(), 220.0);
}
