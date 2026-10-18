mod property_case;
mod property_info_panel;
mod property_layout;
mod property_preview;
mod property_workspace;

pub use property_case::PropertyCase;

use crate::state::AnimationGalleryState;
use crate::ui::*;
use fission::prelude::*;
use property_case::property_case;
use property_layout::PropertyLayout;

fission_component! {
    pub PropertiesPage<AnimationGalleryState> { path: String }
    |page, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let property = property_case(&page.path);
    } =>
        Column(
            gap: tokens.spacing.s,
            children: [
                PageHeader(property.title.to_string(), property.description.to_string()),
                PropertyLayout(property),
            ],
        )
}
