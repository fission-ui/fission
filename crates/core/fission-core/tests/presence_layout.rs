//! Wrapping content in `Presence` must not change how it is sized.

use fission_core::authoring::LoweringContext;
use fission_core::env::{Env, RuntimeState};
use fission_core::internal::build_layout_tree;
use fission_core::motion::Presence;
use fission_core::ui::{Align, Container, Positioned, Text, Widget, ZStack};
use fission_core::WidgetId;
use fission_layout::{LayoutEngine, LayoutSize};

fn card_height(wrap_in_presence: bool) -> f32 {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let card_id = WidgetId::explicit("presence.layout.card");
    let card: Widget = Container {
        id: Some(card_id),
        ..Container::new(Text::new("A short notification"))
            .width(240.0)
            .padding_all(8.0)
    }
    .into();
    let child: Widget = if wrap_in_presence {
        Presence {
            id: WidgetId::explicit("presence.layout.presence"),
            visible: true,
            child: card,
            ..Default::default()
        }
        .into()
    } else {
        card
    };
    let widget: Widget = Align::new(child).into();

    let mut lowering = LoweringContext::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.set_root(root);
    let input = build_layout_tree(lowering.ir(), &env);
    let snapshot = LayoutEngine::new()
        .compute_layout(&input, root, LayoutSize::new(800.0, 600.0), &|_| 0.0)
        .expect("layout");
    snapshot
        .get_node_rect(card_id)
        .expect("card rect")
        .size
        .height
}

#[test]
fn presence_keeps_an_aligned_card_at_its_content_height() {
    let bare = card_height(false);
    let wrapped = card_height(true);
    assert!(
        bare < 100.0,
        "the bare card should size to its text, got {bare}"
    );
    assert_eq!(
        wrapped, bare,
        "wrapping the card in Presence changed its height from {bare} to {wrapped}"
    );
}

#[test]
fn presence_keeps_an_edge_positioned_card_at_its_content_size() {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let card_id = WidgetId::explicit("presence.layout.toast");
    let card: Widget = Container {
        id: Some(card_id),
        ..Container::new(Text::new("Action completed")).padding_all(8.0)
    }
    .into();
    let widget: Widget = ZStack {
        children: vec![Positioned {
            right: Some(16.0),
            bottom: Some(16.0),
            child: Some(
                Presence {
                    id: WidgetId::explicit("presence.layout.toast.presence"),
                    visible: true,
                    child: card,
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        }
        .into()],
        ..Default::default()
    }
    .into();

    let mut lowering = LoweringContext::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.set_root(root);
    let input = build_layout_tree(lowering.ir(), &env);
    let snapshot = LayoutEngine::new()
        .compute_layout(&input, root, LayoutSize::new(800.0, 600.0), &|_| 0.0)
        .expect("layout");
    let rect = snapshot.get_node_rect(card_id).expect("toast rect");
    assert!(
        rect.size.height < 100.0 && rect.size.width < 400.0,
        "an edge-positioned card should size to its content, got {:?}",
        rect.size
    );
}
