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

pub(crate) fn modal() -> Vec<Widget> {
    widgets![ModalDemo::new()]
}

fission_component! {
    ModalDemo<GalleryState> |_demo, state, cx| {
        let close = with_reducer!(cx.build, CloseModal, close_modal);
    } => Column(children: [
        HStack(spacing: None, children: [
            Button(
                variant: ButtonVariant::Outline,
                child: Text("Open Modal"),
                on_press: with_reducer!(cx.build, ToggleModal, toggle_modal),
            ).semantics_identifier("gallery.modal.open"),
        ]),
        if state.modal_open {
            Modal(
                id: WidgetId::explicit("gallery_modal"),
                title: "Gallery Modal",
                content: Text("This is modal content.\nYou can put any widget here."),
                is_open: true,
                on_dismiss: close.clone(),
                backdrop_semantics_identifier: Some("gallery.modal.backdrop".into()),
                close_semantics_identifier: Some("gallery.modal.close".into()),
                surface_semantics_identifier: Some("gallery.modal.surface".into()),
                actions: vec![
                    ModalAction {
                        label: "Cancel".into(),
                        on_press: Some(close.clone()),
                        is_primary: false,
                        semantics_identifier: Some("gallery.modal.cancel".into()),
                    },
                    ModalAction {
                        label: "Confirm".into(),
                        on_press: Some(close.clone()),
                        is_primary: true,
                        semantics_identifier: Some("gallery.modal.confirm".into()),
                    },
                ],
                width: None,
                motion: None,
            )
        }
    ])
}

pub(crate) fn drawer() -> Vec<Widget> {
    widgets![DrawerDemo::new()]
}

fission_component! {
    DrawerDemo<GalleryState> |_demo, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } => Column(children: [
        HStack(spacing: None, children: [
            Button(
                variant: ButtonVariant::Outline,
                child: Text("Open Drawer"),
                on_press: with_reducer!(cx.build, ToggleDrawer, toggle_drawer),
            ).semantics_identifier("gallery.drawer.open"),
        ]),
        if state.drawer_open {
            Drawer(
                id: WidgetId::explicit("gallery_drawer"),
                side: DrawerSide::Right,
                is_open: true,
                on_dismiss: with_reducer!(cx.build, CloseDrawer, close_drawer),
                dismiss_semantics_identifier: Some("gallery.drawer.backdrop".into()),
                content: Container(
                    VStack(spacing: tokens.spacing.s, children: [
                        Text("Drawer Content")
                            .size(tokens.typography.font_size_lg)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.text_primary),
                        Text("This slides in from the right.")
                            .color(tokens.colors.text_secondary),
                    ])
                )
                .padding_all(tokens.spacing.l)
                ,
                width: None,
                motion: None,
            )
        }
    ])
}

pub(crate) fn toast() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    if state.show_toast {
        ctx.register_portal_with_layer(
            PortalLayer::Toast,
            Some(WidgetId::explicit("gallery_toast")),
            ToastPortal::new(tokens.spacing.m).into(),
        );
    }
    widgets![ToastTrigger::new()]
}

fission_component! {
    ToastPortal<GalleryState> { inset: f32 } |portal, _state, cx| {} =>
    Positioned(
        right: portal.inset,
        bottom: portal.inset,
        child: Toast(
            id: WidgetId::explicit("gallery_toast"),
            kind: ToastKind::Success,
            message: "Action completed!",
            on_close: with_reducer!(cx.build, DismissToast, dismiss_toast),
            duration: fission::widgets::ToastDuration::Default,
            motion: None,
        ),
    )
}

fission_component! {
    ToastTrigger<GalleryState> |_trigger, _state, cx| {} =>
    HStack(spacing: None, children: [
        Button(
            variant: ButtonVariant::Outline,
            child: Text("Show Toast"),
            on_press: with_reducer!(cx.build, ShowToast, show_toast),
        ).semantics_identifier("gallery.toast.show"),
    ])
}

pub(crate) fn tooltip() -> Vec<Widget> {
    widgets![TooltipDemo::new()]
}

fission_component! {
    TooltipDemo<()> |_demo, _state, _cx| {} =>
    HStack(spacing: None, children: [
        Tooltip(
            id: WidgetId::explicit("gallery_tooltip"),
            child: Text("Hover me for tooltip"),
            text: "This is a tooltip!",
            is_visible: false,
            motion: None,
        ),
    ])
}

pub(crate) fn select() -> Vec<Widget> {
    widgets![SelectDemo::new()]
}

fission_component! {
    SelectDemo<GalleryState> |_demo, state, cx| {} =>
    HStack(spacing: None, children: [
        Select(
            id: WidgetId::explicit("gallery_select"),
            selected_label: state.select_value.clone(),
            items: vec![
                SelectItem {
                    label: "Option A".into(),
                    icon: None,
                    on_select: with_reducer!(cx.build, SelectValue("Option A".into()), select_value),
                    semantics_identifier: Some("gallery.select.a".into()),
                    form_value: None,
                },
                SelectItem {
                    label: "Option B".into(),
                    icon: None,
                    on_select: with_reducer!(cx.build, SelectValue("Option B".into()), select_value),
                    semantics_identifier: Some("gallery.select.b".into()),
                    form_value: None,
                },
            ],
            is_open: state.select_open,
            on_toggle: with_reducer!(cx.build, ToggleSelect, toggle_select),
            trigger_semantics_identifier: Some("gallery.select.trigger".into()),
            placeholder: "Choose...",
            width: None,
        ),
    ])
}
