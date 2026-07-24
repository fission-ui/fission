use crate::gallery_section::GallerySection;
use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{
    Drawer, DrawerSide, HStack, Modal, ModalAction, Positioned, Select, SelectItem, Toast,
    ToastKind, Tooltip, VStack,
};

#[fission_reducer(ToggleSelect)]
fn toggle_select(state: &mut GalleryState) {
    state.select_open = !state.select_open;
}

#[fission_reducer(SelectValue)]
fn select_value(state: &mut GalleryState, value: String) {
    state.select_value = Some(value);
    state.select_open = false;
}

#[fission_reducer(ToggleModal)]
fn toggle_modal(state: &mut GalleryState) {
    state.modal_open = !state.modal_open;
}

#[fission_reducer(CloseModal)]
fn close_modal(state: &mut GalleryState) {
    state.modal_open = false;
}

#[fission_reducer(ToggleDrawer)]
fn toggle_drawer(state: &mut GalleryState) {
    state.drawer_open = !state.drawer_open;
}

#[fission_reducer(CloseDrawer)]
fn close_drawer(state: &mut GalleryState) {
    state.drawer_open = false;
}

#[fission_reducer(ShowToast)]
fn show_toast(state: &mut GalleryState) {
    state.show_toast = true;
}

#[fission_reducer(DismissToast)]
fn dismiss_toast(state: &mut GalleryState) {
    state.show_toast = false;
}

pub(crate) struct OverlaySection;

impl From<OverlaySection> for Widget {
    fn from(_section: OverlaySection) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let close_modal = with_reducer!(ctx, CloseModal, close_modal);
        let close_drawer = with_reducer!(ctx, CloseDrawer, close_drawer);
        let mut children = widgets![
            HStack {
                spacing: Some(tokens.spacing.s),
                children: widgets![
                    Button {
                        variant: ButtonVariant::Outline,
                        child: Some(Text::new("Open Modal").into()),
                        on_press: Some(with_reducer!(ctx, ToggleModal, toggle_modal)),
                        ..Default::default()
                    }
                    .semantics_identifier("gallery.modal.open"),
                    Button {
                        variant: ButtonVariant::Outline,
                        child: Some(Text::new("Open Drawer").into()),
                        on_press: Some(with_reducer!(ctx, ToggleDrawer, toggle_drawer)),
                        ..Default::default()
                    }
                    .semantics_identifier("gallery.drawer.open"),
                    Button {
                        variant: ButtonVariant::Outline,
                        child: Some(Text::new("Show Toast").into()),
                        on_press: Some(with_reducer!(ctx, ShowToast, show_toast)),
                        ..Default::default()
                    }
                    .semantics_identifier("gallery.toast.show"),
                ],
            },
            Tooltip {
                id: WidgetId::explicit("gallery_tooltip"),
                child: Text::new("Hover me for tooltip").into(),
                text: "This is a tooltip!".into(),
                is_visible: false,
                motion: None,
            },
            Select {
                id: WidgetId::explicit("gallery_select"),
                selected_label: state.select_value.clone(),
                items: vec![
                    SelectItem {
                        label: "Option A".into(),
                        icon: None,
                        on_select: with_reducer!(ctx, SelectValue("Option A".into()), select_value),
                    },
                    SelectItem {
                        label: "Option B".into(),
                        icon: None,
                        on_select: with_reducer!(ctx, SelectValue("Option B".into()), select_value),
                    },
                ],
                is_open: state.select_open,
                on_toggle: Some(with_reducer!(ctx, ToggleSelect, toggle_select)),
                placeholder: "Choose...".into(),
                width: None,
            },
        ];

        if state.modal_open {
            children.push(
                Modal {
                    id: WidgetId::explicit("gallery_modal"),
                    title: "Gallery Modal".into(),
                    content: Text::new("This is modal content.\nYou can put any widget here.")
                        .into(),
                    is_open: true,
                    on_dismiss: Some(close_modal.clone()),
                    actions: vec![
                        ModalAction {
                            label: "Cancel".into(),
                            on_press: Some(close_modal.clone()),
                            is_primary: false,
                        },
                        ModalAction {
                            label: "Confirm".into(),
                            on_press: Some(close_modal),
                            is_primary: true,
                        },
                    ],
                    width: None,
                    motion: None,
                }
                .into(),
            );
        }

        if state.drawer_open {
            children.push(
                Drawer {
                    id: WidgetId::explicit("gallery_drawer"),
                    side: DrawerSide::Right,
                    is_open: true,
                    on_dismiss: Some(close_drawer),
                    content: VStack {
                        spacing: Some(tokens.spacing.s),
                        children: widgets![
                            Text::new("Drawer Content")
                                .size(tokens.typography.font_size_lg)
                                .weight(tokens.typography.font_weight_bold)
                                .color(tokens.colors.text_primary),
                            Text::new("This slides in from the right.")
                                .color(tokens.colors.text_secondary),
                        ],
                    }
                    .into(),
                    width: None,
                    motion: None,
                }
                .into(),
            );
        }

        if state.show_toast {
            let toast: Widget = Toast {
                id: WidgetId::explicit("gallery_toast"),
                kind: ToastKind::Success,
                message: "Action completed!".into(),
                on_close: Some(with_reducer!(ctx, DismissToast, dismiss_toast)),
                motion: None,
            }
            .into();
            ctx.register_portal_with_layer(
                PortalLayer::Toast,
                Some(WidgetId::explicit("gallery_toast")),
                Positioned {
                    right: Some(tokens.spacing.m),
                    bottom: Some(tokens.spacing.m),
                    child: Some(toast),
                    ..Default::default()
                }
                .into(),
            );
        }

        GallerySection::new("Overlays", children).into()
    }
}
