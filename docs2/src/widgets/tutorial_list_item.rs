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
    } =>
    SemanticsRegion(
        identifier: Some(format!("tutorial-list-{}", item.tutorial.app)),
        label: Some(item.tutorial.title.clone()),
        child: Column(gap: tokens.spacing.xs, children: [
            Link::to(item.tutorial.title.clone(), item.tutorial.path.clone()),
            Text(item.tutorial.description.clone()).color(tokens.colors.text_secondary),
            Row(gap: tokens.spacing.s, children: [
                Badge(text: item.tutorial.difficulty.clone()),
                Badge(text: item.tutorial.duration.clone()),
            ]),
        ]),
    )
}
