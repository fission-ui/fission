mod property_case;
mod property_info_panel;
mod property_layout;
mod property_preview;
mod property_workspace;

pub use property_case::PropertyCase;

use crate::state::AnimationGalleryState;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use property_case::property_case;
use property_layout::PropertyLayout;

pub struct PropertiesPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub path: String,
}

impl From<PropertiesPage<'_>> for Widget {
    fn from(page: PropertiesPage<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let property = property_case(&page.path);

        Column {
            gap: Some(view.env().theme.tokens.spacing.s),
            children: widgets![
                ui::PageHeader {
                    title: property.title,
                    subtitle: property.description,
                },
                PropertyLayout {
                    ctx: &page.ctx,
                    state: page.state,
                    property: &property,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
