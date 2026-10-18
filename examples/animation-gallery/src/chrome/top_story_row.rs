use super::*;

fission_component! {
    pub(super) TopStoryRow<AnimationGalleryState> |_row, _state, cx| {} =>
        Wrap(
            direction: FlexDirection::Row,
            spacing: cx.view.env().theme.tokens.spacing.m,
            children: [
                StoryLabel("1", "Widgets", "Browse available widget motions"),
                StoryLabel("2", "Widget Detail", "Live preview, policies, APIs, and inspection"),
                StoryLabel("3", "Properties", "Explore a single property in depth"),
                StoryLabel("4", "Composition Workbench", "Compose effects with clarity and determinism"),
                PolicyControl(),
            ],
        )
}
