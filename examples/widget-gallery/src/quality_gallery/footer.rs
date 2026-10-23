use fission::prelude::*;

use crate::GalleryState;

#[fission_reducer(OpenQualityModal)]
fn open_quality_modal(state: &mut GalleryState) {
    state.modal_open = true;
    state.menu_open = false;
    state.select_open = false;
}

#[fission_reducer(CloseQualityModal)]
fn close_quality_modal(state: &mut GalleryState) {
    state.modal_open = false;
}

fission_component! {
    pub(super) QualityFooter<GalleryState> { compact: bool }
    |footer, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let close_modal = with_reducer!(cx.build, CloseQualityModal, close_quality_modal);
    } =>
        Column(
            gap: tokens.spacing.s + tokens.spacing.xs,
            children: [
                Divider::default(),
                Row(
                    gap: tokens.spacing.s,
                    children: [
                        Icon::svg(material::social::notifications_none::regular())
                            .size(14.0)
                            .color(tokens.colors.text_secondary),
                        Text("Product updates enabled")
                            .size(tokens.typography.font_size_xs)
                            .line_height(16.0)
                            .color(tokens.colors.text_secondary),
                        Spacer(flex_grow: 1.0_f32),
                        Button(
                            variant: ButtonVariant::Outline,
                            size: ComponentSize::Sm,
                            child: Text("Review session"),
                            on_press: with_reducer!(
                                cx.build,
                                OpenQualityModal,
                                open_quality_modal
                            ),
                        )
                        .semantics_identifier("quality-gallery.modal.open"),
                        ModalLayout(
                            id: WidgetId::explicit(if footer.compact {
                                "quality-gallery.modal.mobile"
                            } else {
                                "quality-gallery.modal.desktop"
                            }),
                            header: ModalHeader::new("Review active session").description(
                                "This device has access to your account. End the session if you no longer recognize it.",
                            ),
                            content: ModalContent::new(SessionDeviceSummary::new()),
                            footer: ModalFooter::new(vec![
                                ModalFooterAction {
                                    label: "Cancel".into(),
                                    on_press: Some(close_modal.clone()),
                                    variant: ButtonVariant::Outline,
                                    semantics_identifier: Some(
                                        "quality-gallery.modal.cancel".into(),
                                    ),
                                },
                                ModalFooterAction {
                                    label: "End session".into(),
                                    on_press: Some(close_modal.clone()),
                                    variant: ButtonVariant::Destructive,
                                    semantics_identifier: Some(
                                        "quality-gallery.modal.end-session".into(),
                                    ),
                                },
                            ]),
                            is_open: state.modal_open,
                            on_dismiss: close_modal,
                            backdrop_semantics_identifier: "quality-gallery.modal.backdrop".to_string(),
                            close_semantics_identifier: "quality-gallery.modal.close".to_string(),
                            close_label: None,
                            surface_semantics_identifier: "quality-gallery.modal.surface".to_string(),
                            width: None,
                            motion: ModalMotion::Default,
                        ),
                    ],
                ),
            ],
        )
}

fission_component! {
    SessionDeviceSummary<()> {}
    |_summary, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Container(Row(
            gap: tokens.spacing.s + tokens.spacing.xs,
            children: [
                Container(
                    Icon::svg(material::hardware::laptop::regular())
                        .size(16.0)
                        .color(tokens.colors.text_primary),
                )
                .size(32.0, 32.0)
                .align_child(BoxAlignment::Center)
                .bg(tokens.colors.surface)
                .border(tokens.colors.text_primary.with_alpha(26), 1.0)
                .border_radius(tokens.radii.medium),
                Column(
                    gap: 0.0_f32,
                    children: [
                        Text("Chrome on Linux")
                            .size(tokens.typography.font_size_base)
                            .line_height(20.0)
                            .weight(tokens.typography.font_weight_medium)
                            .color(tokens.colors.text_primary),
                        Text("Active now · London")
                            .size(tokens.typography.font_size_xs)
                            .line_height(16.0)
                            .color(tokens.colors.text_secondary),
                    ],
                ),
            ],
        ))
        .bg(tokens.colors.surface_sunken.with_alpha(128))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.medium)
        .padding_all(tokens.spacing.s + tokens.spacing.xs + 1.0)
}
