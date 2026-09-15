//! Overlays hug their content instead of filling the window.

use anyhow::Result;
use fission_core::ui::{Positioned, Text, Widget, ZStack};
use fission_core::{GlobalState, PortalLayer, Role, WidgetId};
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{Modal, ModalAction, Toast, ToastKind};

#[derive(Clone, Debug, Default)]
struct State;

impl GlobalState for State {}

#[derive(Clone)]
struct OverlayRoot;

impl From<OverlayRoot> for Widget {
    fn from(_root: OverlayRoot) -> Self {
        let (ctx, _) = fission_core::build::current::<State>();
        let toast: Widget = Toast {
            id: WidgetId::explicit("overlay-sizing.toast"),
            kind: ToastKind::Success,
            message: "Action completed".into(),
            on_close: None,
            duration: fission_widgets::ToastDuration::Default,
            motion: None,
        }
        .into();
        ctx.register_portal_with_layer(
            PortalLayer::Toast,
            Some(WidgetId::explicit("overlay-sizing.toast")),
            Positioned {
                right: Some(16.0),
                bottom: Some(16.0),
                child: Some(toast),
                ..Default::default()
            }
            .into(),
        );
        ZStack {
            children: vec![
                Text::new("Background").into(),
                Modal {
                    id: WidgetId::explicit("overlay-sizing.modal"),
                    title: "Short dialog".into(),
                    content: Text::new("One line of body text.").into(),
                    is_open: true,
                    surface_semantics_identifier: Some("overlay-sizing.surface".into()),
                    width: Some(360.0),
                    actions: vec![
                        ModalAction {
                            label: "Cancel".into(),
                            on_press: None,
                            is_primary: false,
                            semantics_identifier: None,
                        },
                        ModalAction {
                            label: "Confirm".into(),
                            on_press: None,
                            is_primary: true,
                            semantics_identifier: None,
                        },
                    ],
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn open_modal_and_toast_hug_their_content() -> Result<()> {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(OverlayRoot));
    driver.harness.env.viewport_size = fission_layout::LayoutSize::new(800.0, 600.0);
    driver.pump()?;
    driver.pump()?;

    let surface = driver
        .find_semantics_identifier("overlay-sizing.surface")
        .expect("modal surface semantics");
    assert!(
        surface.bounds.height() < 300.0,
        "a short modal should hug its content, got {:?}",
        surface.bounds
    );

    let toast = driver
        .find_role(Role::Alert)
        .into_iter()
        .next()
        .expect("toast alert semantics");
    assert!(
        toast.bounds.height() < 120.0 && toast.bounds.width() < 600.0,
        "a toast should size to its message, got {:?}",
        toast.bounds
    );
    Ok(())
}

#[derive(Clone)]
struct ToastInRoot;

impl From<ToastInRoot> for Widget {
    fn from(_root: ToastInRoot) -> Self {
        ZStack {
            children: vec![Positioned {
                right: Some(16.0),
                bottom: Some(16.0),
                child: Some(
                    Toast {
                        id: WidgetId::explicit("overlay-sizing.root-toast"),
                        kind: ToastKind::Success,
                        message: "Action completed".into(),
                        on_close: None,
                        duration: fission_widgets::ToastDuration::Default,
                        motion: None,
                    }
                    .into(),
                ),
                ..Default::default()
            }
            .into()],
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn toast_positioned_in_the_root_sizes_to_its_message() -> Result<()> {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(ToastInRoot));
    driver.harness.env.viewport_size = fission_layout::LayoutSize::new(800.0, 600.0);
    driver.pump()?;
    let toast = driver
        .find_role(Role::Alert)
        .into_iter()
        .next()
        .expect("toast alert semantics");
    assert!(
        toast.bounds.height() < 120.0 && toast.bounds.width() < 600.0,
        "a toast positioned in the root should size to its message, got {:?}",
        toast.bounds
    );
    Ok(())
}

#[derive(Clone)]
struct CardPortal;

impl From<CardPortal> for Widget {
    fn from(_root: CardPortal) -> Self {
        let (ctx, _) = fission_core::build::current::<State>();
        ctx.register_portal_with_layer(
            PortalLayer::Toast,
            Some(WidgetId::explicit("overlay-sizing.card-portal")),
            Positioned {
                right: Some(16.0),
                bottom: Some(16.0),
                child: Some(
                    fission_core::ui::SemanticsRegion::new(
                        fission_core::ui::Container::new(Text::new("Portal card")).padding_all(8.0),
                    )
                    .role(Role::Alert)
                    .into(),
                ),
                ..Default::default()
            }
            .into(),
        );
        Text::new("Background").into()
    }
}

#[test]
fn plain_card_in_a_portal_sizes_to_its_content() -> Result<()> {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(CardPortal));
    driver.harness.env.viewport_size = fission_layout::LayoutSize::new(800.0, 600.0);
    driver.pump()?;
    let card = driver
        .find_role(Role::Alert)
        .into_iter()
        .next()
        .expect("card alert semantics");
    assert!(
        card.bounds.height() < 120.0 && card.bounds.width() < 600.0,
        "a plain card in a portal should size to its content, got {:?}",
        card.bounds
    );
    Ok(())
}

#[derive(Clone)]
enum ToastShape {
    ToastWithoutMotion,
    RowWithGrowingText,
    RowWithGrowingTextAndButton,
}

#[derive(Clone)]
struct ToastShapeRoot(ToastShape);

impl From<ToastShapeRoot> for Widget {
    fn from(root: ToastShapeRoot) -> Self {
        let child: Widget = match root.0 {
            ToastShape::ToastWithoutMotion => Toast {
                id: WidgetId::explicit("overlay-sizing.shape-toast"),
                kind: ToastKind::Success,
                message: "Action completed".into(),
                on_close: None,
                duration: fission_widgets::ToastDuration::Default,
                motion: Some(fission_widgets::ToastMotion::None),
            }
            .into(),
            ToastShape::RowWithGrowingText | ToastShape::RowWithGrowingTextAndButton => {
                let mut children = vec![Text::new("Action completed").flex_grow(1.0).into()];
                if matches!(root.0, ToastShape::RowWithGrowingTextAndButton) {
                    children.push(
                        fission_core::ui::Button {
                            variant: fission_core::ui::ButtonVariant::Ghost,
                            child: Some(Text::new("x").into()),
                            ..Default::default()
                        }
                        .into(),
                    );
                }
                fission_core::ui::SemanticsRegion::new(
                    fission_core::ui::Container::new(fission_core::ui::Row {
                        children,
                        ..Default::default()
                    })
                    .padding_all(8.0),
                )
                .role(Role::Alert)
                .into()
            }
        };
        ZStack {
            children: vec![Positioned {
                right: Some(16.0),
                bottom: Some(16.0),
                child: Some(child),
                ..Default::default()
            }
            .into()],
            ..Default::default()
        }
        .into()
    }
}

fn toast_shape_bounds(shape: ToastShape) -> Result<fission_layout::LayoutRect> {
    let mut driver =
        TestDriver::new(TestHarness::new(State).with_root_widget(ToastShapeRoot(shape)));
    driver.harness.env.viewport_size = fission_layout::LayoutSize::new(800.0, 600.0);
    driver.pump()?;
    Ok(driver
        .find_role(Role::Alert)
        .into_iter()
        .next()
        .expect("alert semantics")
        .bounds)
}

#[test]
fn toast_without_motion_sizes_to_its_message() -> Result<()> {
    let bounds = toast_shape_bounds(ToastShape::ToastWithoutMotion)?;
    assert!(bounds.height() < 120.0, "toast without motion: {bounds:?}");
    Ok(())
}

#[test]
fn card_row_with_growing_text_sizes_to_its_content() -> Result<()> {
    let bounds = toast_shape_bounds(ToastShape::RowWithGrowingText)?;
    assert!(bounds.height() < 120.0, "row with growing text: {bounds:?}");
    Ok(())
}

#[test]
fn card_row_with_a_button_sizes_to_its_content() -> Result<()> {
    let bounds = toast_shape_bounds(ToastShape::RowWithGrowingTextAndButton)?;
    assert!(
        bounds.height() < 120.0,
        "row with growing text and button: {bounds:?}"
    );
    Ok(())
}

#[derive(Clone, Copy)]
struct CardDecor {
    elevated: bool,
    labelled_button: bool,
}

#[derive(Clone)]
struct DecorRoot(CardDecor);

impl From<DecorRoot> for Widget {
    fn from(root: DecorRoot) -> Self {
        let (_, view) = fission_core::build::current::<State>();
        let tokens = &view.env().theme.tokens;
        let mut children = vec![Text::new("Action completed").flex_grow(1.0).into()];
        if root.0.labelled_button {
            children.push(
                fission_core::ui::SemanticsRegion::new(fission_core::ui::Button {
                    variant: fission_core::ui::ButtonVariant::Ghost,
                    child: Some(Text::new("x").into()),
                    ..Default::default()
                })
                .label("Dismiss notification")
                .into(),
            );
        }
        let mut card = fission_core::ui::Container::new(fission_core::ui::Row {
            children,
            ..Default::default()
        })
        .padding_all(8.0);
        if root.0.elevated {
            card = card
                .bg_fill(fission_core::op::Fill::Solid(tokens.colors.surface))
                .border(tokens.colors.border, 1.0)
                .border_radius(tokens.radii.medium)
                .shadow(
                    tokens
                        .elevations
                        .level3
                        .unwrap_or(fission_core::op::BoxShadow {
                            spread_radius: 0.0,
                            inset: false,
                            color: fission_core::op::Color {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 60,
                            },
                            blur_radius: 12.0,
                            offset: (0.0, 6.0),
                        }),
                );
        }
        let alert: Widget = fission_core::ui::SemanticsRegion::new(card)
            .role(Role::Alert)
            .label("Action completed")
            .into();
        ZStack {
            children: vec![Positioned {
                right: Some(16.0),
                bottom: Some(16.0),
                child: Some(alert),
                ..Default::default()
            }
            .into()],
            ..Default::default()
        }
        .into()
    }
}

fn decor_bounds(decor: CardDecor) -> Result<fission_layout::LayoutRect> {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(DecorRoot(decor)));
    driver.harness.env.viewport_size = fission_layout::LayoutSize::new(800.0, 600.0);
    driver.pump()?;
    Ok(driver
        .find_role(Role::Alert)
        .into_iter()
        .next()
        .expect("decor alert semantics")
        .bounds)
}

#[test]
fn elevated_card_sizes_to_its_content() -> Result<()> {
    let bounds = decor_bounds(CardDecor {
        elevated: true,
        labelled_button: false,
    })?;
    assert!(bounds.height() < 120.0, "elevated card: {bounds:?}");
    Ok(())
}

#[test]
fn card_with_a_labelled_button_sizes_to_its_content() -> Result<()> {
    let bounds = decor_bounds(CardDecor {
        elevated: false,
        labelled_button: true,
    })?;
    assert!(bounds.height() < 120.0, "labelled button card: {bounds:?}");
    Ok(())
}

#[test]
fn elevated_card_with_a_labelled_button_sizes_to_its_content() -> Result<()> {
    let bounds = decor_bounds(CardDecor {
        elevated: true,
        labelled_button: true,
    })?;
    assert!(
        bounds.height() < 120.0,
        "elevated labelled button card: {bounds:?}"
    );
    Ok(())
}

#[derive(Clone)]
struct GalleryModalRoot;

impl From<GalleryModalRoot> for Widget {
    fn from(_root: GalleryModalRoot) -> Self {
        ZStack {
            children: vec![
                Text::new("Background").into(),
                Modal {
                    id: WidgetId::explicit("overlay-sizing.gallery-modal"),
                    title: "Gallery Modal".into(),
                    content: Text::new("This is modal content.\nYou can put any widget here.")
                        .into(),
                    is_open: true,
                    surface_semantics_identifier: Some("overlay-sizing.gallery-surface".into()),
                    actions: vec![
                        ModalAction {
                            label: "Cancel".into(),
                            on_press: None,
                            is_primary: false,
                            semantics_identifier: None,
                        },
                        ModalAction {
                            label: "Confirm".into(),
                            on_press: None,
                            is_primary: true,
                            semantics_identifier: None,
                        },
                    ],
                    width: None,
                    motion: None,
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

#[test]
fn modal_without_a_width_hugs_its_content() -> Result<()> {
    let mut driver = TestDriver::new(TestHarness::new(State).with_root_widget(GalleryModalRoot));
    driver.harness.env.viewport_size = fission_layout::LayoutSize::new(1200.0, 935.0);
    driver.pump()?;
    driver.pump()?;
    let surface = driver
        .find_semantics_identifier("overlay-sizing.gallery-surface")
        .expect("modal surface semantics");
    assert!(
        surface.bounds.height() < 300.0,
        "a modal without a width should hug its content, got {:?}",
        surface.bounds
    );
    Ok(())
}
