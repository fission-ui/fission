use crate::blog::BlogPostSummary;
use crate::state::DocsState;
use fission::prelude::*;

fission_component! {
    pub BlogPostCard<DocsState> {
        props { post: BlogPostSummary }
    }
    view(card, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let categories = &card.post.categories;
        let authors = card.post.authors.iter().map(|author| match author.as_str() {
            "fission" => "Fission Team",
            "courtney" | "courtney-robinson" => "Courtney Robinson",
            other => other,
        }).collect::<Vec<_>>().join(", ");
    } =>
    Container(
        SemanticsRegion(
            identifier: Some("blog-post-card".to_string()),
            label: Some(card.post.title.clone()),
            child: Column(gap: tokens.spacing.s, children: [
                Row(gap: tokens.spacing.s, wrap: ir_op::FlexWrap::Wrap, align_items: ir_op::AlignItems::Center, children: [
                    Text(card.post.published.clone()).color(tokens.colors.text_secondary),
                    if !authors.is_empty() {
                        Text(authors.clone()).color(tokens.colors.text_secondary)
                    },
                ]),
                Text(card.post.title.clone())
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary),
                Text(card.post.description.clone()).color(tokens.colors.text_secondary),
                if !categories.is_empty() {
                    Row(gap: tokens.spacing.xs, wrap: ir_op::FlexWrap::Wrap, children: [
                        for category in categories {
                            Badge(text: category.clone(), key: category.clone())
                        }
                    ])
                },
            ]),
        )
        .hyperlink(Hyperlink::new(card.post.path.clone()))
    )
    .padding_all(tokens.spacing.l)
    .bg(tokens.colors.surface)
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.large)
}
