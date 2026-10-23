use fission::prelude::*;

use crate::GalleryState;

#[fission_reducer(QualityHierarchyAction)]
fn quality_hierarchy_action(_state: &mut GalleryState) {}

fission_component! {
    ActionHierarchyContent<GalleryState> { action: ActionEnvelope }
    |content, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Wrap(
            direction: FlexDirection::Row,
            spacing: tokens.spacing.s,
            run_spacing: None,
            children: [
                Button(
                    variant: ButtonVariant::Primary,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("Primary"),
                    on_press: content.action.clone(),
                )
                .semantics_identifier("quality-gallery.button.primary"),
                Button(
                    variant: ButtonVariant::SecondaryGray,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("Secondary"),
                    on_press: content.action.clone(),
                )
                .semantics_identifier("quality-gallery.button.secondary"),
                Button(
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("Outline"),
                    on_press: content.action.clone(),
                )
                .semantics_identifier("quality-gallery.button.outline"),
                Button(
                    variant: ButtonVariant::TertiaryGray,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("Ghost"),
                    on_press: content.action.clone(),
                )
                .semantics_identifier("quality-gallery.button.ghost"),
                Button(
                    variant: ButtonVariant::Destructive,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("Delete"),
                    on_press: content.action.clone(),
                )
                .semantics_identifier("quality-gallery.button.destructive"),
                Button(
                    variant: ButtonVariant::Primary,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("Disabled"),
                    disabled: true,
                )
                .semantics_identifier("quality-gallery.button.disabled"),
            ],
        )
}

fission_component! {
    pub(super) ActionHierarchyCard<GalleryState> { compact: bool }
    |card, _state, cx| {
        let action = with_reducer!(cx.build, QualityHierarchyAction, quality_hierarchy_action);
        let mut header = CardHeader::new(CardTitle::new("Action hierarchy"));
        if !card.compact {
            header = header.description(CardDescription::new("Default variants and states."));
        }

    } =>
        CardLayout::new()
            .header(header)
            .content(CardContent::new(ActionHierarchyContent::new(action)))
            .separated(false)
            .size(ComponentSize::Sm)
            .pattern(CardPattern::Plain)
}
