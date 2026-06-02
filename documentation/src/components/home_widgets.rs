use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent, TextAlign};
use fission::prelude::*;
use fission::{Role, Semantics};

pub(super) fn site_semantics(identifier: impl Into<String>) -> Semantics {
    Semantics {
        role: Role::Generic,
        identifier: Some(identifier.into()),
        ..Semantics::default()
    }
}

pub(super) fn semantic_column(
    identifier: impl Into<String>,
    children: Vec<Widget>,
    gap: Option<f32>,
    align_items: AlignItems,
) -> Widget {
    Column {
        children,
        gap,
        align_items,
        semantics: Some(site_semantics(identifier)),
        ..Default::default()
    }
    .into()
}

pub(super) fn semantic_row(
    identifier: impl Into<String>,
    children: Vec<Widget>,
    gap: Option<f32>,
    wrap: FlexWrap,
    align_items: AlignItems,
    justify_content: JustifyContent,
) -> Widget {
    Row {
        children,
        gap,
        wrap,
        align_items,
        justify_content,
        semantics: Some(site_semantics(identifier)),
        ..Default::default()
    }
    .into()
}

#[derive(Clone, Debug)]
pub(super) struct CenteredSection {
    eyebrow: &'static str,
    title: &'static str,
    body: &'static str,
    cards: Vec<Widget>,
}

impl CenteredSection {
    pub(super) fn new(
        eyebrow: &'static str,
        title: &'static str,
        body: &'static str,
        cards: Vec<Widget>,
    ) -> Self {
        Self {
            eyebrow,
            title,
            body,
            cards,
        }
    }
}

impl From<CenteredSection> for Widget {
    fn from(component: CenteredSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Text::new(component.eyebrow)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.secondary)
                    .into(),
                Text::new(component.title)
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
                    .into(),
                Text::new(component.body)
                    .size(tokens.typography.body_large_size)
                    .line_height(
                        tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .max_width(prose_width(tokens))
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into(),
                Row {
                    children: component.cards.clone(),
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.l),
            align_items: AlignItems::Center,
            ..Default::default()
        })
        .into()
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

impl From<SectionHeader> for Widget {
    fn from(component: SectionHeader) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        semantic_column(
            "site-home-section-header",
            vec![
                Text::new(component.kicker)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.secondary)
                    .into(),
                Text::new(component.title)
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
                    .into(),
                Text::new(component.body)
                    .size(tokens.typography.body_large_size)
                    .line_height(
                        tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .max_width(prose_width(tokens))
                    .text_align(TextAlign::Center)
                    .flex_shrink(1.0)
                    .into(),
            ],
            Some(tokens.spacing.m),
            AlignItems::Center,
        )
    }
}
#[derive(Clone, Debug)]
pub(super) struct ShellSection {
    child: Widget,
}

impl ShellSection {
    pub(super) fn new(child: Widget) -> Self {
        Self { child }
    }
}

impl From<ShellSection> for Widget {
    fn from(component: ShellSection) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(component.child.clone())
            .padding_all(tokens.spacing.xl)
            .bg_fill(Fill::Solid(tokens.colors.surface))
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.xxl)
            .into()
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

impl From<ExternalNavLink> for Widget {
    fn from(component: ExternalNavLink) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Text::new(component.label)
            .size(tokens.typography.label_large_size)
            .weight(tokens.typography.font_weight_semibold)
            .color(tokens.colors.text_secondary)
            .semantics_identifier(format!("markdown-link:{}", component.href))
            .into()
    }
}
impl From<NavLink> for Widget {
    fn from(component: NavLink) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Text::new(component.label)
            .size(tokens.typography.label_large_size)
            .weight(tokens.typography.font_weight_semibold)
            .color(tokens.colors.text_link)
            .semantics_identifier(format!("site-route:{}", component.href))
            .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct ThemeToggle;

impl From<ThemeToggle> for Widget {
    fn from(_component: ThemeToggle) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Text::new("Theme")
            .size(tokens.typography.label_large_size)
            .weight(tokens.typography.font_weight_semibold)
            .color(tokens.colors.text_link)
            .semantics_identifier("site-theme-toggle")
            .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct SearchPill;

impl From<SearchPill> for Widget {
    fn from(_component: SearchPill) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        semantic_row(
            "site-search-trigger",
            vec![
                Text::new("Search")
                    .size(tokens.typography.label_large_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
                Container::new(
                    Text::new("Cmd K")
                        .size(tokens.typography.font_size_xs)
                        .family(tokens.typography.font_family_mono.clone())
                        .color(tokens.colors.text_muted),
                )
                .padding([tokens.spacing.s, tokens.spacing.s, 2.0, 2.0])
                .border(tokens.colors.border_strong, 1.0)
                .border_radius(tokens.radii.medium)
                .into(),
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

impl From<Cta> for Widget {
    fn from(component: Cta) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let (background, foreground, border) = if component.primary {
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
            Text::new(component.label)
                .size(tokens.typography.label_large_size)
                .weight(tokens.typography.font_weight_bold)
                .color(foreground)
                .semantics_identifier(format!("site-route:{}", component.href)),
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
        .into()
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

impl From<StatusText> for Widget {
    fn from(component: StatusText) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Text::new(component.label)
            .size(tokens.typography.font_size_sm)
            .family(tokens.typography.font_family_mono.clone())
            .color(tokens.colors.text_muted)
            .into()
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

impl From<Pill> for Widget {
    fn from(component: Pill) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(component.label)
                .size(tokens.typography.font_size_sm)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.primary),
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
        .into()
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

impl From<CodeCard> for Widget {
    fn from(component: CodeCard) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Text::new(component.label)
                    .size(tokens.typography.font_size_xs)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_muted)
                    .into(),
                Row {
                    children: vec![
                        Text::new("$")
                            .size(tokens.typography.font_size_sm)
                            .family(tokens.typography.font_family_mono.clone())
                            .color(tokens.colors.secondary)
                            .into(),
                        Text::new(component.command)
                            .size(tokens.typography.font_size_sm)
                            .line_height(
                                tokens.typography.font_size_sm * tokens.typography.line_height_snug,
                            )
                            .family(tokens.typography.font_family_mono.clone())
                            .color(tokens.colors.text_primary)
                            .into(),
                    ],
                    gap: Some(tokens.spacing.s),
                    align_items: AlignItems::Center,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.s),
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border_strong, 1.0)
        .border_radius(tokens.radii.xl)
        .into()
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

impl From<LinkCard> for Widget {
    fn from(component: LinkCard) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Card::new(
            vec![
                Container::new(
                    Text::new(component.eyebrow)
                        .size(tokens.typography.font_size_xs)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.primary),
                )
                .padding_all(tokens.spacing.s)
                .bg_fill(Fill::Solid(tokens.colors.primary_subtle))
                .border_radius(tokens.radii.medium)
                .into(),
                Text::new(component.title)
                    .size(tokens.typography.font_size_lg)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
                Paragraph::new(component.body).into(),
                NavLink::new(component.link, component.href).into(),
            ],
            compact_tile_width(tokens),
        )
        .into()
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

impl From<TargetRowCard> for Widget {
    fn from(component: TargetRowCard) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Row {
            children: vec![
                Column {
                    children: vec![
                        Row {
                            children: vec![
                                Text::new(component.name)
                                    .size(tokens.typography.font_size_lg)
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(tokens.colors.heading)
                                    .into(),
                                Text::new(component.status)
                                    .size(tokens.typography.font_size_xs)
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(tokens.colors.primary)
                                    .into(),
                                Text::new(component.platforms)
                                    .size(tokens.typography.font_size_sm)
                                    .color(tokens.colors.text_muted)
                                    .into(),
                            ],
                            gap: Some(tokens.spacing.s),
                            wrap: FlexWrap::Wrap,
                            align_items: AlignItems::Center,
                            ..Default::default()
                        }
                        .into(),
                        Paragraph::new(component.body).into(),
                    ],
                    gap: Some(tokens.spacing.s),
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Text::new(component.command)
                    .size(tokens.typography.font_size_sm)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.text_primary)
                    .into(),
                NavLink::new(component.cta, component.href).into(),
            ],
            gap: Some(tokens.spacing.l),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..Default::default()
        })
        .padding_all(tokens.spacing.l)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xl)
        .into()
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

impl From<ChartImageCard> for Widget {
    fn from(component: ChartImageCard) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let mut image_children = vec![Image::asset(component.image)
            .size(
                chart_tile_width(tokens) - tokens.spacing.l,
                tokens.spacing.xxxxl * 1.15,
            )
            .into()];
        if let Some(badge) = component.badge {
            image_children.push(
                Text::new(badge)
                    .size(tokens.typography.font_size_xs)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_primary)
                    .into(),
            );
        }
        Card::new(
            vec![
                Container::new(Column {
                    children: image_children,
                    gap: Some(tokens.spacing.xs),
                    align_items: AlignItems::Center,
                    ..Default::default()
                })
                .bg_fill(Fill::Solid(tokens.colors.on_surface.with_alpha(245)))
                .border_radius(tokens.radii.xl)
                .into(),
                Text::new(component.title)
                    .size(tokens.typography.label_large_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .semantics_identifier("site-route:/docs/charts/catalog/")
                    .into(),
            ],
            chart_tile_width(tokens),
        )
        .into()
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

impl From<Chip> for Widget {
    fn from(component: Chip) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(component.label)
                .size(tokens.typography.font_size_xs)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.surface_sunken),
        )
        .padding([tokens.spacing.s, tokens.spacing.s, 2.0, 2.0])
        .bg_fill(Fill::Solid(tokens.colors.on_surface))
        .border_radius(tokens.radii.full)
        .into()
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

impl From<ExampleCard> for Widget {
    fn from(component: ExampleCard) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Card::new(
            vec![
                Container::new(Column {
                    children: vec![
                        Text::new(component.tag)
                            .size(tokens.typography.font_size_xs)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.primary)
                            .into(),
                        Text::new(component.title)
                            .size(tokens.typography.heading_size)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    align_items: AlignItems::Center,
                    ..Default::default()
                })
                .padding_all(tokens.spacing.l)
                .bg_fill(Fill::Solid(tokens.colors.surface))
                .border_radius(tokens.radii.xl)
                .into(),
                Text::new(component.command)
                    .size(tokens.typography.font_size_sm)
                    .family(tokens.typography.font_family_mono.clone())
                    .color(tokens.colors.text_primary)
                    .into(),
                Paragraph::new(component.body).into(),
                Paragraph::new(component.feature_a).into(),
                Paragraph::new(component.feature_b).into(),
                Row {
                    children: vec![
                        Cta::new("Open guide", component.guide, true).into(),
                        Cta::new("Reference", component.reference, false).into(),
                    ],
                    gap: Some(tokens.spacing.s),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
            ],
            tile_width(tokens),
        )
        .into()
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

impl From<Paragraph> for Widget {
    fn from(component: Paragraph) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Text::new(component.body)
            .size(tokens.typography.body_medium_size)
            .line_height(tokens.typography.body_medium_size * tokens.typography.line_height_normal)
            .color(tokens.colors.text_secondary)
            .flex_shrink(1.0)
            .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct Card {
    children: Vec<Widget>,
    width: f32,
}

impl Card {
    pub(super) fn new(children: Vec<Widget>, width: f32) -> Self {
        Self { children, width }
    }
}

impl From<Card> for Widget {
    fn from(component: Card) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: component.children.clone(),
            gap: Some(tokens.spacing.m),
            ..Default::default()
        })
        .width(component.width)
        .flex_shrink(1.0)
        .padding_all(tokens.spacing.l)
        .bg_fill(Fill::Solid(tokens.colors.surface_raised))
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xl)
        .into()
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
