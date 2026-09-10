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

pub(super) struct QualityFooter {
    pub compact: bool,
}

impl From<QualityFooter> for Widget {
    fn from(footer: QualityFooter) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let close_modal = with_reducer!(ctx, CloseQualityModal, close_quality_modal);

        Column {
            gap: Some(tokens.spacing.s + tokens.spacing.xs),
            children: widgets![
                Divider::default(),
                Row {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        Icon::svg(material::social::notifications_none::regular())
                            .size(14.0)
                            .color(tokens.colors.text_secondary),
                        Text::new("Product updates enabled")
                            .size(tokens.typography.font_size_xs)
                            .line_height(16.0)
                            .color(tokens.colors.text_secondary),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ComponentSize::Sm,
                            child: Some(Text::new("Review session").into()),
                            on_press: Some(with_reducer!(
                                ctx,
                                OpenQualityModal,
                                open_quality_modal
                            )),
                            ..Default::default()
                        }
                        .semantics_identifier("quality-gallery.modal.open"),
                        ModalLayout {
                            id: WidgetId::explicit(if footer.compact {
                                "quality-gallery.modal.mobile"
                            } else {
                                "quality-gallery.modal.desktop"
                            }),
                            header: ModalHeader::new("Review active session").description(
                                "This device has access to your account. End the session if you no longer recognize it.",
                            ),
                            content: ModalContent::new(SessionDeviceSummary),
                            footer: Some(ModalFooter::new(vec![
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
                            ])),
                            is_open: state.modal_open,
                            on_dismiss: Some(close_modal),
                            backdrop_semantics_identifier: Some(
                                "quality-gallery.modal.backdrop".into(),
                            ),
                            close_semantics_identifier: Some("quality-gallery.modal.close".into(),),
                            close_label: None,
                            surface_semantics_identifier: Some(
                                "quality-gallery.modal.surface".into(),
                            ),
                            width: None,
                            motion: Some(ModalMotion::Default),
                        },
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}

struct SessionDeviceSummary;

impl From<SessionDeviceSummary> for Widget {
    fn from(_summary: SessionDeviceSummary) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;

        Container::new(Row {
            gap: Some(tokens.spacing.s + tokens.spacing.xs),
            children: widgets![
                Container::new(
                    Icon::svg(material::hardware::laptop::regular())
                        .size(16.0)
                        .color(tokens.colors.text_primary),
                )
                .size(32.0, 32.0)
                .align_child(BoxAlignment::Center)
                .bg(tokens.colors.surface)
                .border(tokens.colors.text_primary.with_alpha(26), 1.0)
                .border_radius(tokens.radii.medium),
                Column {
                    gap: Some(0.0),
                    children: widgets![
                        Text::new("Chrome on Linux")
                            .size(tokens.typography.font_size_base)
                            .line_height(20.0)
                            .weight(tokens.typography.font_weight_medium)
                            .color(tokens.colors.text_primary),
                        Text::new("Active now · London")
                            .size(tokens.typography.font_size_xs)
                            .line_height(16.0)
                            .color(tokens.colors.text_secondary),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .bg(tokens.colors.surface_sunken.with_alpha(128))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.medium)
        .padding_all(tokens.spacing.s + tokens.spacing.xs + 1.0)
        .into()
    }
}
