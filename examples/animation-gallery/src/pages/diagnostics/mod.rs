mod diagnostics_case;
mod diagnostics_source_panel;
mod diagnostics_workspace;

use self::diagnostics_case::{diagnostics_case, gallery_case};
use self::diagnostics_workspace::DiagnosticsWorkspace;
use crate::state::AnimationGalleryState;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub DiagnosticsPage<AnimationGalleryState> { path: String } |page, _state, cx| {
        let diagnostics = diagnostics_case(&page.path);
        let gallery = gallery_case(diagnostics);
    } =>
        Column(
            gap: cx.view.env().theme.tokens.spacing.m,
            children: [
                PageHeader(
                    diagnostics.title.to_string(),
                    diagnostics.subtitle.to_string(),
                ),
                DiagnosticsWorkspace(diagnostics, gallery),
            ],
        )
}
