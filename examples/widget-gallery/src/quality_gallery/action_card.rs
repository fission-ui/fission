use fission::prelude::*;

use crate::GalleryState;

#[fission_reducer(QualityHierarchyAction)]
fn quality_hierarchy_action(_state: &mut GalleryState) {}

pub(super) struct ActionHierarchyCard {
    pub compact: bool,
}

impl From<ActionHierarchyCard> for Widget {
    fn from(card: ActionHierarchyCard) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let action = with_reducer!(ctx, QualityHierarchyAction, quality_hierarchy_action);
        let mut header = CardHeader::new(CardTitle::new("Action hierarchy"));
        if !card.compact {
            header = header.description(CardDescription::new("Default variants and states."));
        }

        CardLayout::new()
            .header(header)
            .content(CardContent::new(Wrap {
                direction: FlexDirection::Row,
                spacing: Some(tokens.spacing.s),
                children: widgets![
                    Button {
                        variant: ButtonVariant::Primary,
                        size: ComponentSize::Md,
                        content: Some(ButtonContent::new("Primary")),
                        on_press: Some(action.clone()),
                        ..Default::default()
                    }
                    .semantics_identifier("quality-gallery.button.primary"),
                    Button {
                        variant: ButtonVariant::SecondaryGray,
                        size: ComponentSize::Md,
                        content: Some(ButtonContent::new("Secondary")),
                        on_press: Some(action.clone()),
                        ..Default::default()
                    }
                    .semantics_identifier("quality-gallery.button.secondary"),
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ComponentSize::Md,
                        content: Some(ButtonContent::new("Outline")),
                        on_press: Some(action.clone()),
                        ..Default::default()
                    }
                    .semantics_identifier("quality-gallery.button.outline"),
                    Button {
                        variant: ButtonVariant::TertiaryGray,
                        size: ComponentSize::Md,
                        content: Some(ButtonContent::new("Ghost")),
                        on_press: Some(action.clone()),
                        ..Default::default()
                    }
                    .semantics_identifier("quality-gallery.button.ghost"),
                    Button {
                        variant: ButtonVariant::Destructive,
                        size: ComponentSize::Md,
                        content: Some(ButtonContent::new("Delete")),
                        on_press: Some(action),
                        ..Default::default()
                    }
                    .semantics_identifier("quality-gallery.button.destructive"),
                    Button {
                        variant: ButtonVariant::Primary,
                        size: ComponentSize::Md,
                        content: Some(ButtonContent::new("Disabled")),
                        disabled: true,
                        ..Default::default()
                    }
                    .semantics_identifier("quality-gallery.button.disabled"),
                ],
            }))
            .separated(false)
            .size(ComponentSize::Sm)
            .pattern(CardPattern::Plain)
            .into()
    }
}
