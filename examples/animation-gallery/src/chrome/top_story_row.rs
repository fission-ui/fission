use super::*;

pub(super) struct TopStoryRow<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<TopStoryRow<'_>> for Widget {
    fn from(row: TopStoryRow<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(view.env().theme.tokens.spacing.m),
            children: vec![
                StoryLabel {
                    step: "1",
                    title: "Widgets",
                    caption: "Browse available widget motions",
                }
                .into(),
                StoryLabel {
                    step: "2",
                    title: "Widget Detail",
                    caption: "Live preview, policies, APIs, and inspection",
                }
                .into(),
                StoryLabel {
                    step: "3",
                    title: "Properties",
                    caption: "Explore a single property in depth",
                }
                .into(),
                StoryLabel {
                    step: "4",
                    title: "Composition Workbench",
                    caption: "Compose effects with clarity and determinism",
                }
                .into(),
                PolicyControl {
                    ctx: row.ctx,
                    state: row.state,
                }
                .into(),
            ],
        }
        .into()
    }
}
