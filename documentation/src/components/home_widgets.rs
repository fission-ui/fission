use super::state::DocsState;
use fission::ir::{Role, Semantics};
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent, TextAlign};
use fission::prelude::*;

pub(super) fn site_semantics(identifier: impl Into<String>) -> Semantics {
    Semantics {
        role: Role::Generic,
        identifier: Some(identifier.into()),
        ..Semantics::default()
    }
}

pub(super) fn semantic_column(
    identifier: impl Into<String>,
    children: Vec<Node>,
    gap: Option<f32>,
    align_items: AlignItems,
) -> Node {
    Column {
        children,
        gap,
        align_items,
        semantics: Some(site_semantics(identifier)),
        ..Default::default()
    }
    .into_node()
}

pub(super) fn semantic_row(
    identifier: impl Into<String>,
    children: Vec<Node>,
    gap: Option<f32>,
    wrap: FlexWrap,
    align_items: AlignItems,
    justify_content: JustifyContent,
) -> Node {
    Row {
        children,
        gap,
        wrap,
        align_items,
        justify_content,
        semantics: Some(site_semantics(identifier)),
        ..Default::default()
    }
    .into_node()
}

#[derive(Clone, Debug)]
pub(super) struct CenteredSection {
    eyebrow: &'static str,
    title: &'static str,
    body: &'static str,
    cards: Vec<Node>,
}

impl CenteredSection {
    pub(super) fn new(
        eyebrow: &'static str,
        title: &'static str,
        body: &'static str,
        cards: Vec<Node>,
    ) -> Self {
        Self {
            eyebrow,
            title,
            body,
            cards,
        }
    }
}

impl Widget<DocsState> for CenteredSection {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Column {
                children: vec![
                    Text::new(self.eyebrow)
                        .size(tokens.typography.font_size_sm)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.secondary)
                        .into_node(),
                    Text::new(self.title)
                        .size(tokens.typography.heading2_size)
                        .family(tokens.typography.font_family_serif.clone())
                        .line_height(
                            tokens.typography.heading2_size * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .max_width(headline_width(tokens))
                        .text_align(TextAlign::Center)
                        .flex_shrink(1.0)
                        .into_node(),
                    Text::new(self.body)
                        .size(tokens.typography.body_large_size)
                        .line_height(
                            tokens.typography.body_large_size
                                * tokens.typography.line_height_relaxed,
                        )
                        .color(tokens.colors.text_secondary)
                        .max_width(prose_width(tokens))
                        .text_align(TextAlign::Center)
                        .flex_shrink(1.0)
                        .into_node(),
                    Row {
                        children: self.cards.clone(),
                        gap: Some(tokens.spacing.m),
                        wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    }
                    .into_node(),
                ],
                gap: Some(tokens.spacing.l),
                align_items: AlignItems::Center,
                ..Default::default()
            }
            .into_node(),
        )
        .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct SectionHeader {
    kicker: &'static str,
    title: &'static str,
    body: &'static str,
}

impl SectionHeader {
    pub(super) fn new(kicker: &'static str, title: &'static str, body: &'static str) -> Self {
        Self {
            kicker,
            title,
            body,
        }
    }
}

impl Widget<DocsState> for SectionHeader {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        semantic_column(
            "site-home-section-header",
            vec![
                Text::new(self.kicker)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.secondary)
                    .into_node(),
                Text::new(self.title)
                    .size(tokens.typography.heading2_size)
                    .family(tokens.typography.font_family_serif.clone())
                    .line_height(
                        tokens.typography.heading2_size * tokens.typography.line_height_heading,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .max_width(headline_width(tokens))
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into_node(),
                Text::new(self.body)
                    .size(tokens.typography.body_large_size)
                    .line_height(
                        tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .max_width(prose_width(tokens))
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into_node(),
            ],
            Some(tokens.spacing.m),
            AlignItems::Center,
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct ShellSection {
    child: Node,
}

impl ShellSection {
    pub(super) fn new(child: Node) -> Self {
        Self { child }
    }
}

impl Widget<DocsState> for ShellSection {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(self.child.clone())
            .padding_all(tokens.spacing.xl)
            .bg_fill(Fill::Solid(tokens.colors.surface))
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.xxl)
            .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct NavLink {
    label: &'static str,
    href: &'static str,
}

impl NavLink {
    pub(super) fn new(label: &'static str, href: &'static str) -> Self {
        Self { label, href }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ExternalNavLink {
    label: &'static str,
    href: &'static str,
}

impl ExternalNavLink {
    pub(super) fn new(label: &'static str, href: &'static str) -> Self {
        Self { label, href }
    }
}

impl Widget<DocsState> for ExternalNavLink {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Text::new(self.label)
            .size(tokens.typography.label_large_size)
            .weight(tokens.typography.font_weight_semibold)
            .color(tokens.colors.text_secondary)
            .semantics_identifier(format!("markdown-link:{}", self.href))
            .into_node()
    }
}

impl Widget<DocsState> for NavLink {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Text::new(self.label)
            .size(tokens.typography.label_large_size)
            .weight(tokens.typography.font_weight_semibold)
            .color(tokens.colors.text_link)
            .semantics_identifier(format!("site-route:{}", self.href))
            .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct ThemeToggle;

impl Widget<DocsState> for ThemeToggle {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Text::new("Theme")
            .size(tokens.typography.label_large_size)
            .weight(tokens.typography.font_weight_semibold)
            .color(tokens.colors.text_link)
            .semantics_identifier("site-theme-toggle")
            .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct SearchPill;

impl Widget<DocsState> for SearchPill {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        semantic_row(
            "site-search-trigger",
            vec![
                Text::new("Search")
                    .size(tokens.typography.label_large_size)
                    .color(tokens.colors.text_secondary)
                    .into_node(),
                Container::new(
                    Text::new("Cmd K")
                        .size(tokens.typography.font_size_xs)
                        .family(tokens.typography.font_family_mono.clone())
                        .color(tokens.colors.text_muted)
                        .into_node(),
                )
                .padding([tokens.spacing.s, tokens.spacing.s, 2.0, 2.0])
                .border(tokens.colors.border_strong, 1.0)
                .border_radius(tokens.radii.medium)
                .into_node(),
            ],
            Some(tokens.spacing.s),
            FlexWrap::NoWrap,
            AlignItems::Center,
            JustifyContent::Start,
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct Cta {
    label: &'static str,
    href: &'static str,
    primary: bool,
}

impl Cta {
    pub(super) fn new(label: &'static str, href: &'static str, primary: bool) -> Self {
        Self {
            label,
            href,
            primary,
        }
    }
}

impl Widget<DocsState> for Cta {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let (background, foreground, border) = if self.primary {
            (
                tokens.colors.primary,
                tokens.colors.on_primary,
                tokens.colors.primary,
            )
        } else {
            (
                tokens.colors.surface_raised,
                tokens.colors.text_primary,
                tokens.colors.border,
            )
        };
        Container::new(
            Text::new(self.label)
                .size(tokens.typography.label_large_size)
                .weight(tokens.typography.font_weight_bold)
                .color(foreground)
                .semantics_identifier(format!("site-route:{}", self.href))
                .into_node(),
        )
        .padding([
            tokens.spacing.l,
            tokens.spacing.l,
            tokens.spacing.m,
            tokens.spacing.m,
        ])
        .bg_fill(Fill::Solid(background))
        .border(border, 1.0)
        .border_radius(tokens.radii.full)
        .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct StatusText {
    label: &'static str,
}

impl StatusText {
    pub(super) fn new(label: &'static str) -> Self {
        Self { label }
    }
}

impl Widget<DocsState> for StatusText {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Text::new(self.label)
            .size(tokens.typography.font_size_sm)
            .family(tokens.typography.font_family_mono.clone())
            .color(tokens.colors.text_muted)
            .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct Pill {
    label: &'static str,
}

impl Pill {
    pub(super) fn new(label: &'static str) -> Self {
        Self { label }
    }
}

impl Widget<DocsState> for Pill {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Text::new(self.label)
                .size(tokens.typography.font_size_sm)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.primary)
                .into_node(),
        )
        .padding([
            tokens.spacing.m,
            tokens.spacing.m,
            tokens.spacing.s,
            tokens.spacing.s,
        ])
        .bg_fill(Fill::Solid(tokens.colors.primary_subtle))
        .border(tokens.colors.focus_ring, 1.0)
        .border_radius(tokens.radii.full)
        .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct CodeCard {
    label: &'static str,
    command: &'static str,
}

impl CodeCard {
    pub(super) fn new(label: &'static str, command: &'static str) -> Self {
        Self { label, command }
    }
}

impl Widget<DocsState> for CodeCard {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Column {
                children: vec![
                    Text::new(self.label)
                        .size(tokens.typography.font_size_xs)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.text_muted)
                        .into_node(),
                    Row {
                        children: vec![
                            Text::new("$")
                                .size(tokens.typography.font_size_sm)
                                .family(tokens.typography.font_family_mono.clone())
                                .color(tokens.colors.secondary)
                                .into_node(),
                            Text::new(self.command)
                                .size(tokens.typography.font_size_sm)
                                .line_height(
                                    tokens.typography.font_size_sm
                                        * tokens.typography.line_height_snug,
                                )
                                .family(tokens.typography.font_family_mono.clone())
                                .color(tokens.colors.text_primary)
                                .into_node(),
                        ],
                        gap: Some(tokens.spacing.s),
                        align_items: AlignItems::Center,
                        ..Default::default()
                    }
                    .into_node(),
                ],
                gap: Some(tokens.spacing.s),
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(tokens.spacing.m)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border_strong, 1.0)
        .border_radius(tokens.radii.xl)
        .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct LinkCard {
    eyebrow: &'static str,
    title: &'static str,
    body: &'static str,
    link: &'static str,
    href: &'static str,
}

impl LinkCard {
    pub(super) fn new(
        eyebrow: &'static str,
        title: &'static str,
        body: &'static str,
        link: &'static str,
        href: &'static str,
    ) -> Self {
        Self {
            eyebrow,
            title,
            body,
            link,
            href,
        }
    }
}

impl Widget<DocsState> for LinkCard {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Card::new(
            vec![
                Container::new(
                    Text::new(self.eyebrow)
                        .size(tokens.typography.font_size_xs)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.primary)
                        .into_node(),
                )
                .padding_all(tokens.spacing.s)
                .bg_fill(Fill::Solid(tokens.colors.primary_subtle))
                .border_radius(tokens.radii.medium)
                .into_node(),
                Text::new(self.title)
                    .size(tokens.typography.font_size_lg)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into_node(),
                Paragraph::new(self.body).build(ctx, view),
                NavLink::new(self.link, self.href).build(ctx, view),
            ],
            compact_tile_width(tokens),
        )
        .build(ctx, view)
    }
}

#[derive(Clone, Debug)]
pub(super) struct TargetRowCard {
    name: &'static str,
    status: &'static str,
    platforms: &'static str,
    command: &'static str,
    body: &'static str,
    href: &'static str,
    cta: &'static str,
}

impl TargetRowCard {
    pub(super) fn new(
        name: &'static str,
        status: &'static str,
        platforms: &'static str,
        command: &'static str,
        body: &'static str,
        href: &'static str,
        cta: &'static str,
    ) -> Self {
        Self {
            name,
            status,
            platforms,
            command,
            body,
            href,
            cta,
        }
    }
}

impl Widget<DocsState> for TargetRowCard {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Row {
                children: vec![
                    Column {
                        children: vec![
                            Row {
                                children: vec![
                                    Text::new(self.name)
                                        .size(tokens.typography.font_size_lg)
                                        .weight(tokens.typography.font_weight_bold)
                                        .color(tokens.colors.heading)
                                        .into_node(),
                                    Text::new(self.status)
                                        .size(tokens.typography.font_size_xs)
                                        .weight(tokens.typography.font_weight_bold)
                                        .color(tokens.colors.primary)
                                        .into_node(),
                                    Text::new(self.platforms)
                                        .size(tokens.typography.font_size_sm)
                                        .color(tokens.colors.text_muted)
                                        .into_node(),
                                ],
                                gap: Some(tokens.spacing.s),
                                wrap: FlexWrap::Wrap,
                                align_items: AlignItems::Center,
                                ..Default::default()
                            }
                            .into_node(),
                            Paragraph::new(self.body).build(ctx, view),
                        ],
                        gap: Some(tokens.spacing.s),
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                    Text::new(self.command)
                        .size(tokens.typography.font_size_sm)
                        .family(tokens.typography.font_family_mono.clone())
                        .color(tokens.colors.text_primary)
                        .into_node(),
                    NavLink::new(self.cta, self.href).build(ctx, view),
                ],
                gap: Some(tokens.spacing.l),
                wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(tokens.spacing.l)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xl)
        .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct ChartImageCard {
    title: &'static str,
    image: &'static str,
    badge: Option<&'static str>,
}

impl ChartImageCard {
    pub(super) fn new(title: &'static str, image: &'static str) -> Self {
        Self {
            title,
            image,
            badge: None,
        }
    }

    pub(super) fn with_badge(mut self, badge: &'static str) -> Self {
        self.badge = Some(badge);
        self
    }
}

impl Widget<DocsState> for ChartImageCard {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let mut image_children = vec![Image::asset(self.image)
            .size(
                chart_tile_width(tokens) - tokens.spacing.l,
                tokens.spacing.xxxxl * 1.15,
            )
            .into_node()];
        if let Some(badge) = self.badge {
            image_children.push(
                Text::new(badge)
                    .size(tokens.typography.font_size_xs)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_primary)
                    .into_node(),
            );
        }
        Card::new(
            vec![
                Container::new(
                    Column {
                        children: image_children,
                        gap: Some(tokens.spacing.xs),
                        align_items: AlignItems::Center,
                        ..Default::default()
                    }
                    .into_node(),
                )
                .bg_fill(Fill::Solid(tokens.colors.on_surface.with_alpha(245)))
                .border_radius(tokens.radii.xl)
                .into_node(),
                Text::new(self.title)
                    .size(tokens.typography.label_large_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .semantics_identifier("site-route:/docs/charts/catalog/")
                    .into_node(),
            ],
            chart_tile_width(tokens),
        )
        .build(_ctx, view)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Chip {
    label: &'static str,
}

impl Chip {
    pub(super) fn new(label: &'static str) -> Self {
        Self { label }
    }
}

impl Widget<DocsState> for Chip {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Text::new(self.label)
                .size(tokens.typography.font_size_xs)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.surface_sunken)
                .into_node(),
        )
        .padding([tokens.spacing.s, tokens.spacing.s, 2.0, 2.0])
        .bg_fill(Fill::Solid(tokens.colors.on_surface))
        .border_radius(tokens.radii.full)
        .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct ExampleCard {
    tag: &'static str,
    title: &'static str,
    command: &'static str,
    body: &'static str,
    feature_a: &'static str,
    feature_b: &'static str,
    guide: &'static str,
    reference: &'static str,
}

impl ExampleCard {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        tag: &'static str,
        title: &'static str,
        command: &'static str,
        body: &'static str,
        feature_a: &'static str,
        feature_b: &'static str,
        guide: &'static str,
        reference: &'static str,
    ) -> Self {
        Self {
            tag,
            title,
            command,
            body,
            feature_a,
            feature_b,
            guide,
            reference,
        }
    }
}

impl Widget<DocsState> for ExampleCard {
    fn build(&self, ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Card::new(
            vec![
                Container::new(
                    Column {
                        children: vec![
                            Text::new(self.tag)
                                .size(tokens.typography.font_size_xs)
                                .weight(tokens.typography.font_weight_bold)
                                .color(tokens.colors.primary)
                                .into_node(),
                            Text::new(self.title)
                                .size(tokens.typography.heading_size)
                                .weight(tokens.typography.font_weight_bold)
                                .color(tokens.colors.heading)
                                .into_node(),
                        ],
                        gap: Some(tokens.spacing.m),
                        align_items: AlignItems::Center,
                        ..Default::default()
                    }
                    .into_node(),
                )
                .padding_all(tokens.spacing.l)
                .bg_fill(Fill::Solid(tokens.colors.surface))
                .border_radius(tokens.radii.xl)
                .into_node(),
                Text::new(self.command)
                    .size(tokens.typography.font_size_sm)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.text_primary)
                    .into_node(),
                Paragraph::new(self.body).build(ctx, view),
                Paragraph::new(self.feature_a).build(ctx, view),
                Paragraph::new(self.feature_b).build(ctx, view),
                Row {
                    children: vec![
                        Cta::new("Open guide", self.guide, true).build(ctx, view),
                        Cta::new("Reference", self.reference, false).build(ctx, view),
                    ],
                    gap: Some(tokens.spacing.s),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into_node(),
            ],
            tile_width(tokens),
        )
        .build(ctx, view)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Paragraph {
    body: &'static str,
}

impl Paragraph {
    pub(super) fn new(body: &'static str) -> Self {
        Self { body }
    }
}

impl Widget<DocsState> for Paragraph {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Text::new(self.body)
            .size(tokens.typography.body_medium_size)
            .line_height(tokens.typography.body_medium_size * tokens.typography.line_height_normal)
            .color(tokens.colors.text_secondary)
            .flex_shrink(1.0)
            .into_node()
    }
}

#[derive(Clone, Debug)]
pub(super) struct Card {
    children: Vec<Node>,
    width: f32,
}

impl Card {
    pub(super) fn new(children: Vec<Node>, width: f32) -> Self {
        Self { children, width }
    }
}

impl Widget<DocsState> for Card {
    fn build(&self, _ctx: &mut BuildCtx<DocsState>, view: &View<DocsState>) -> Node {
        let tokens = &view.env.theme.tokens;
        Container::new(
            Column {
                children: self.children.clone(),
                gap: Some(tokens.spacing.m),
                ..Default::default()
            }
            .into_node(),
        )
        .width(self.width)
        .flex_shrink(1.0)
        .padding_all(tokens.spacing.l)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xl)
        .into_node()
    }
}

pub(super) fn page_fill(tokens: &Tokens) -> Fill {
    Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 1.0),
        stops: vec![
            (0.0, tokens.colors.background),
            (0.6, tokens.colors.surface_sunken),
            (1.0, tokens.colors.surface),
        ],
    }
}

pub(super) fn hero_text_width(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl * 8.25
}

pub(super) fn prose_width(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl * 9.0
}

pub(super) fn headline_width(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl * 6.5
}

pub(super) fn tile_width(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl * 3.5
}

pub(super) fn compact_tile_width(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl * 2.8
}

pub(super) fn content_width(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl * 11.75
}

pub(super) fn nav_inset(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl
}

fn chart_tile_width(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl * 2.05
}
