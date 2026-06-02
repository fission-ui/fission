use fission_core::ui::{Column, Container, Scroll, Text, TextInput};
use fission_core::{FlexDirection, GlobalState, InputEvent, LayoutPoint, PointerEvent, Widget};
use fission_ir::op::{Color, Fill, LayoutOp, PaintOp};
use fission_shell_terminal::{verify_terminal_ir, TerminalApp};

#[derive(Default, Debug, Clone, PartialEq)]
struct State;
impl GlobalState for State {}

#[derive(Clone)]
struct HelloApp;

impl From<HelloApp> for Widget {
    fn from(_component: HelloApp) -> Self {
        let (_ctx, _view) = fission_core::build::current::<State>();
        Container::new(Text::new("Hello terminal").color(Color::BLACK))
            .width(24.0)
            .height(3.0)
            .padding([1.0, 1.0, 1.0, 1.0])
            .bg(Color::WHITE)
            .border(Color::BLACK, 1.0)
            .into()
    }
}
#[derive(Clone)]
struct ScrollApp;

impl From<ScrollApp> for Widget {
    fn from(_component: ScrollApp) -> Self {
        let (_ctx, _view) = fission_core::build::current::<State>();
        let children = (0..12)
            .map(|idx| {
                Text::new(format!("line-{idx:02}"))
                    .color(Color::BLACK)
                    .into()
            })
            .collect();
        Scroll {
            id: Some(fission_ir::WidgetId::explicit("terminal_scroll")),
            direction: FlexDirection::Column,
            width: Some(18.0),
            height: Some(8.0),
            show_scrollbar: true,
            child: Some(
                Column {
                    gap: Some(0.0),
                    children,
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        }
        .into()
    }
}
#[derive(Clone)]
struct TextInputApp;

impl From<TextInputApp> for Widget {
    fn from(_component: TextInputApp) -> Self {
        let (_ctx, _view) = fission_core::build::current::<State>();
        TextInput {
            id: Some(fission_ir::WidgetId::explicit("terminal_text_input")),
            value: "abc".to_string(),
            width: Some(12.0),
            height: Some(3.0),
            text_color: Some(Color::BLACK),
            background_fill: Some(Fill::Solid(Color::WHITE)),
            border_color: Some(Color::BLACK),
            focus_border_color: Some(Color::BLACK),
            ..Default::default()
        }
        .into()
    }
}
#[test]
fn terminal_app_renders_real_fission_widget_tree_to_cells() {
    let mut app = TerminalApp::<State, _>::new(HelloApp);
    let frame = app.render_frame(40, 10).expect("render terminal frame");
    assert!(frame.as_plain_text().contains("Hello terminal"));
}

#[test]
fn terminal_verifier_rejects_graphical_only_paint() {
    let mut ir = fission_ir::CoreIR::new();
    let id = fission_ir::WidgetId::from_u128(1);
    ir.add_node(
        id,
        fission_ir::Op::Paint(fission_ir::PaintOp::DrawImage {
            request: fission_ir::op::ImageRequest {
                source: fission_ir::op::ImageSource::Asset {
                    path: "image.png".to_string(),
                },
                ..Default::default()
            },
            fit: fission_ir::op::ImageFit::Contain,
            alignment: fission_ir::op::ImageAlignment::Center,
        }),
        Vec::new(),
    );
    ir.set_root(id);
    assert!(verify_terminal_ir(&ir).is_err());
}

#[test]
fn terminal_verifier_documents_supported_and_unsupported_ir_shapes() {
    let mut supported = fission_ir::CoreIR::new();
    let root = fission_ir::WidgetId::from_u128(10);
    let text = fission_ir::WidgetId::from_u128(11);
    supported.add_node(
        root,
        fission_ir::Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            width: Some(20.0),
            height: Some(4.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![text],
    );
    supported.add_node(
        text,
        fission_ir::Op::Paint(PaintOp::DrawText {
            text: "terminal text".to_string(),
            size: 12.0,
            color: Color::BLACK,
            underline: false,
            wrap: true,
            caret_index: Some(4),
            caret_color: Some(Color::BLACK),
            caret_width: Some(1.0),
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
        Vec::new(),
    );
    supported.set_root(root);
    assert!(verify_terminal_ir(&supported).is_ok());

    let mut gradient = fission_ir::CoreIR::new();
    let id = fission_ir::WidgetId::from_u128(12);
    gradient.add_node(
        id,
        fission_ir::Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::LinearGradient {
                start: (0.0, 0.0),
                end: (1.0, 1.0),
                stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)],
            }),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        Vec::new(),
    );
    gradient.set_root(id);
    assert!(verify_terminal_ir(&gradient).is_err());
}

#[test]
fn terminal_renderer_clips_and_offsets_scroll_content() {
    let mut app = TerminalApp::<State, _>::new(ScrollApp);
    let first = app.render_frame(24, 10).expect("initial render");
    assert!(first.as_plain_text().contains("line-00"));

    app.send_event(InputEvent::Pointer(PointerEvent::Scroll {
        point: LayoutPoint::new(1.0, 1.0),
        delta: LayoutPoint::new(0.0, 4.0),
        modifiers: 0,
    }))
    .expect("scroll event");

    let scrolled = app.render_frame(24, 10).expect("scrolled render");
    let plain = scrolled.as_plain_text();
    assert!(!plain.contains("line-00"));
    assert!(plain.contains("line-04") || plain.contains("line-05"));
    assert!(plain.contains('#'));
}

#[test]
fn terminal_renderer_shows_text_input_caret_when_focused() {
    let mut app = TerminalApp::<State, _>::new(TextInputApp);
    app.render_frame(24, 6).expect("initial render");
    app.send_event(InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint::new(2.0, 1.0),
        button: fission_core::PointerButton::Primary,
        modifiers: 0,
    }))
    .expect("focus input");
    let focused = app.render_frame(24, 6).expect("focused render");
    assert!(focused.as_plain_text().contains('|'));
}
