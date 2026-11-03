use crate::content::GoalMeta;
use crate::state::DocsState;
use fission::prelude::*;

fission_component! {
    pub TutorialListItem<DocsState> {
        props {
            tutorial: GoalMeta,
        }
    }
    view(item, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let heading_identifier = format!(
            "site-heading-3:{}",
            item.tutorial.app
        );
    } =>
    SemanticsRegion(
        identifier: Some(format!("tutorial-list-{}", item.tutorial.app)),
        label: Some(item.tutorial.title.clone()),
        child: Column(gap: tokens.spacing.xs, children: [
            SemanticsRegion(
                Text(item.tutorial.title.clone())
                    .color(tokens.colors.primary)
                    .weight(tokens.typography.font_weight_bold),
                identifier: Some(heading_identifier),
            ),
            Text(item.tutorial.description.clone()).color(tokens.colors.text_secondary),
            Row(gap: tokens.spacing.s, children: [
                Badge(text: item.tutorial.difficulty.clone()),
                Badge(text: item.tutorial.duration.clone()),
            ]),
        ]),
    )
    .hyperlink(Hyperlink::new(item.tutorial.path.clone()))
}
