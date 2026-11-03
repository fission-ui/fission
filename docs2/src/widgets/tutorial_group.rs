use crate::content::GoalMeta;
use crate::state::DocsState;
use crate::widgets::{copy, TutorialListItem};
use fission::prelude::*;

const TUTORIAL_CARD_MIN_WIDTH: f32 = 320.0;

fission_component! {
    pub TutorialGroup<DocsState> {
        props {
            title_key: &'static str,
            description_key: &'static str,
            tutorials: Vec<GoalMeta>,
        }
    }
    view(group, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let heading_identifier = format!(
            "site-heading-2:{}",
            group.title_key.replace('.', "-")
        );
    } =>
    SemanticsRegion(
        identifier: Some("tutorial-group".to_string()),
        label: Some(cx.view.env().tr(group.title_key)),
        child: Column(gap: tokens.spacing.l, children: [
            Column(gap: tokens.spacing.xs, children: [
                SemanticsRegion(
                    copy(group.title_key)
                        .size(tokens.typography.heading2_size)
                        .weight(tokens.typography.font_weight_bold),
                    identifier: Some(heading_identifier),
                ),
                copy(group.description_key).color(tokens.colors.text_secondary),
            ]),
            SemanticsRegion(
                identifier: Some("tutorial-group-grid".to_string()),
                label: Some(cx.view.env().tr(group.title_key)),
                child: Grid(
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(TUTORIAL_CARD_MIN_WIDTH),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: tokens.spacing.m,
                    row_gap: tokens.spacing.m,
                    children: [
                        for tutorial in &group.tutorials {
                            TutorialListItem(
                                tutorial.clone(),
                                key: tutorial.path.clone(),
                            )
                        },
                    ],
                ),
            ),
        ]),
    )
}
