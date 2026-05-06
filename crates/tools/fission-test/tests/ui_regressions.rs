use fission_core::op::Color;
use fission_core::ui::{Checkbox, Column, Container, Node, Row, Text, TextInput, ZStack};
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct State;
impl AppState for State {}

#[test]
fn test_modal_layout_cramping() {
    // Reproduces the "Contacts" modal cramping issue.
    // Structure: Container -> Column -> Row(Header) + Row(Item)

    struct ContactsModal;
    impl Widget<State> for ContactsModal {
        fn build(&self, _ctx: &mut BuildCtx<State>, _view: &View<State>) -> Node {
            Container::new(
                Column::default()
                    .children(vec![
                        // Header
                        Row::default()
                            .children(vec![
                                Container::new(Checkbox::default().into_node())
                                    .width(40.0)
                                    .into_node(),
                                Container::new(Text::new("Name").into_node())
                                    .width(150.0)
                                    .into_node(),
                                Container::new(Text::new("Email").into_node())
                                    .width(250.0)
                                    .into_node(),
                            ])
                            .into_node(),
                        // Item
                        Row::default()
                            .children(vec![
                                Container::new(Checkbox::default().into_node())
                                    .width(40.0)
                                    .into_node(),
                                Container::new(Text::new("Alice").into_node())
                                    .width(150.0)
                                    .into_node(),
                                Container::new(Text::new("alice@example.com").into_node())
                                    .width(250.0)
                                    .into_node(),
                            ])
                            .into_node(),
                    ])
                    .into_node(),
            )
            .width(400.0)
            .padding_all(16.0)
            .into_node()
        }
    }

    let mut h = TestHarness::new(State);
    h = h.with_root_widget(ContactsModal);
    h.pump().unwrap();

    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();

    // Helper to find container rect by text (text paint -> text layout -> container).
    let find_container_rect = |text: &str| -> fission_layout::LayoutRect {
        for (_id, node) in &ir.nodes {
            if let fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text: t, .. }) = &node.op {
                if t == text {
                    let text_layout_id = node.parent.expect("text layout parent");
                    let container_id = ir
                        .nodes
                        .get(&text_layout_id)
                        .and_then(|n| n.parent)
                        .expect("text container parent");
                    return snap.get_node_geometry(container_id).unwrap().rect;
                }
            }
        }
        panic!("Text '{}' not found", text);
    };

    let name_header = find_container_rect("Name");
    let email_header = find_container_rect("Email");
    let name_item = find_container_rect("Alice");
    let email_item = find_container_rect("alice@example.com");

    println!("Name Header: {:?}", name_header);
    println!("Email Header: {:?}", email_header);
    println!("Name Item: {:?}", name_item);
    println!("Email Item: {:?}", email_item);

    // Assert Alignment
    // Name Item should start where Name Header starts (approx)
    assert!(
        (name_header.x() - name_item.x()).abs() < 5.0,
        "Name column misaligned"
    );
    assert!(
        (email_header.x() - email_item.x()).abs() < 5.0,
        "Email column misaligned"
    );

    // Assert Spacing/Cramping
    // Name width should be substantial (not 0 or tiny)
    assert!(
        name_header.width() > 120.0,
        "Name column too narrow: {}",
        name_header.width()
    );

    // Check overlap between checkbox and text?
    // We can assume if x positions differ significantly, they don't overlap.
    // Checkbox is approx 20px?
    // Name Item X should be > Checkbox Width.
    assert!(
        name_item.x() > 20.0,
        "Name item too close to left edge (checkbox overlap?)"
    );
}

#[test]
fn test_compose_form_spacing() {
    // Reproduces the "Compose" window gap issue.
    // Structure: Column(gap 16) -> Row(Label+Input) -> Row(Label+Input).

    struct ComposeForm;
    impl Widget<State> for ComposeForm {
        fn build(&self, _ctx: &mut BuildCtx<State>, _view: &View<State>) -> Node {
            Container::new(
                Column::default()
                    .gap(Some(16.0))
                    .children(vec![
                        // Row 1: To
                        Row::default()
                            .children(vec![
                                Text::new("To").width(50.0).into_node(),
                                TextInput::default().value("Alice").into_node(),
                            ])
                            .into_node(),
                        // Row 2: Subject
                        Row::default()
                            .children(vec![
                                Text::new("Subject").width(50.0).into_node(),
                                TextInput::default().value("Hello").into_node(),
                            ])
                            .into_node(),
                    ])
                    .into_node(),
            )
            .width(400.0)
            .into_node()
        }
    }

    let mut h = TestHarness::new(State);
    h = h.with_root_widget(ComposeForm);
    h.pump().unwrap();

    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();

    let find_y = |text: &str| -> f32 {
        for (id, node) in &ir.nodes {
            if let fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text: t, .. }) = &node.op {
                if t == text {
                    return snap.get_node_geometry(*id).unwrap().rect.y();
                }
            }
        }
        panic!("Text '{}' not found", text);
    };

    let to_y = find_y("To");
    let subj_y = find_y("Subject");

    println!("To Y: {}, Subject Y: {}", to_y, subj_y);

    let gap = subj_y - to_y;
    // Expected: LineHeight (20) + Gap (16) = 36.
    // If TextInput height is 40 (default), then 40 + 16 = 56.
    // Allow up to 60.
    assert!(gap < 60.0, "Excessive gap between fields: {}", gap);
}

#[test]
fn test_multi_modal_stacking() {
    // Reproduces Z-order/Transparency issue.
    // Structure: ZStack -> Content, Overlay1, Overlay2.

    struct MultiModal;
    impl Widget<State> for MultiModal {
        fn build(&self, _ctx: &mut BuildCtx<State>, _view: &View<State>) -> Node {
            // Simulate manually stacking overlays as the App would do
            ZStack::default()
                .children(vec![
                    // App Content
                    Container::new(Text::new("App Content").into_node())
                        .bg(Color {
                            r: 255,
                            g: 255,
                            b: 255,
                            a: 255,
                        })
                        .into_node(),
                    // Modal 1 (Contacts)
                    Container::new(
                        Container::new(Text::new("Modal 1").into_node())
                            .width(300.0)
                            .height(300.0)
                            .bg(Color {
                                r: 200,
                                g: 200,
                                b: 200,
                                a: 255,
                            })
                            .into_node(),
                    )
                    // Backdrop 1
                    .bg(Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 128,
                    })
                    .into_node(), // Implicitly stretches to fill due to ZStack behavior?
                    // No, usually requires AbsoluteFill.
                    // Container lowers to Box.
                    // We should use Overlay logic or AbsoluteFill.
                    // But here checking if ZStack renders in order.

                    // Modal 2 (Settings)
                    Container::new(
                        Container::new(Text::new("Modal 2").into_node())
                            .width(200.0)
                            .height(200.0)
                            .bg(Color {
                                r: 220,
                                g: 220,
                                b: 220,
                                a: 255,
                            })
                            .into_node(),
                    )
                    // Backdrop 2
                    .bg(Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 128,
                    })
                    .into_node(),
                ])
                .into_node()
        }
    }

    let mut h = TestHarness::new(State);
    h = h.with_root_widget(MultiModal);
    h.pump().unwrap();

    let dl = h.get_last_display_list().expect("No display list found");

    // Scan display list order
    // Expected: Content -> Backdrop1 -> Modal1 -> Backdrop2 -> Modal2

    let mut order = Vec::new();
    for op in dl.ops {
        match op {
            fission_render::DisplayOp::DrawText { text, .. } => order.push(text),
            _ => {}
        }
    }

    println!("Draw Order: {:?}", order);

    let idx1 = order
        .iter()
        .position(|s| s == "Modal 1")
        .expect("Modal 1 missing");
    let idx2 = order
        .iter()
        .position(|s| s == "Modal 2")
        .expect("Modal 2 missing");

    assert!(idx2 > idx1, "Modal 2 should be drawn AFTER Modal 1");
}
