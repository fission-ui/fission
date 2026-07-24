use super::PropertyCase;
use crate::state::AnimationGalleryState;
use crate::style::{BORDER, SURFACE};
use crate::ui;
use crate::widgets::common::CurrentValues;
use fission::prelude::*;

pub(super) struct PropertyInfoPanel<'a> {
    pub property: &'a PropertyCase,
    pub state: &'a AnimationGalleryState,
}

impl From<PropertyInfoPanel<'_>> for Widget {
    fn from(panel: PropertyInfoPanel<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                ui::SectionTitle {
                    title: "Property Info",
                },
                ui::LabelValue {
                    label: "Name",
                    value: panel.property.property_name,
                },
                ui::LabelValue {
                    label: "Type",
                    value: panel.property.value_type,
                },
                ui::LabelValue {
                    label: "Phase",
                    value: panel.property.phase,
                },
                ui::LabelValue {
                    label: "Layout",
                    value: panel.property.layout,
                },
                ui::LabelValue {
                    label: "Paint",
                    value: panel.property.paint,
                },
                ui::LabelValue {
                    label: "Reduced",
                    value: panel.property.reduced,
                },
                CurrentValues { state: panel.state },
                ui::PageNote {
                    title: "Notes",
                    body: panel.property.notes,
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
