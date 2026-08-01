use super::home_nav::HomePageNav;
use super::home_widgets::{page_fill, site_semantics, SemanticRow};
use super::localized::LocalizedNav;
use super::state::DocsState;
use crate::registry::{RegistryCrate, PLATFORMS};
use fission::op::{AlignItems, FlexWrap, JustifyContent, TextAlign};
use fission::prelude::*;

const PHONE_BREAKPOINT: f32 = 720.0;
const DESKTOP_BREAKPOINT: f32 = 1_080.0;

#[derive(Clone, Debug)]
pub(crate) struct CrateDirectoryPage {
    crates: Vec<RegistryCrate>,
}

impl CrateDirectoryPage {
    pub(crate) fn new(crates: Vec<RegistryCrate>) -> Self {
        Self { crates }
    }
}

impl From<CrateDirectoryPage> for Widget {
    fn from(page: CrateDirectoryPage) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let navigation: Widget = if view.env().locale.0 == "es-ES" {
            LocalizedNav.into()
        } else {
            HomePageNav.into()
        };
        Container::new(Column {
            children: vec![
                navigation,
                CrateDirectoryHero {
                    crate_count: page.crates.len(),
                }
                .into(),
                PlatformLegend.into(),
                Container::new(CrateResults {
                    crates: page.crates,
                })
                .width_length(Length::clamp(
                    Length::points(280.0),
                    Length::percent(100.0),
                    Length::points(1304.0),
                ))
                .padding_lengths([
                    Length::points(tokens.spacing.xxl),
                    Length::points(tokens.spacing.xl),
                    Length::points(tokens.spacing.xxxxl),
                    Length::points(tokens.spacing.xl),
                ])
                .into(),
            ],
            gap: Some(0.0),
            semantics: Some(site_semantics("crate-directory-page")),
            ..Default::default()
        })
        .min_height_length(Length::vh(100.0))
        .bg_fill(page_fill(tokens))
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateDirectoryHero {
    crate_count: usize,
}

impl From<CrateDirectoryHero> for Widget {
    fn from(hero: CrateDirectoryHero) -> Widget {
        Responsive::new(CrateHeroDesktop {
            crate_count: hero.crate_count,
        })
        .id(WidgetId::explicit("crates.hero.responsive"))
        .case(ResponsiveCase::max_width(
            PHONE_BREAKPOINT,
            CrateHeroPhone {
                crate_count: hero.crate_count,
            },
        ))
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateHeroDesktop {
    crate_count: usize,
}

impl From<CrateHeroDesktop> for Widget {
    fn from(hero: CrateHeroDesktop) -> Widget {
        CrateHeroContent::new(hero.crate_count, false).into()
    }
}

#[derive(Clone, Debug)]
struct CrateHeroPhone {
    crate_count: usize,
}

impl From<CrateHeroPhone> for Widget {
    fn from(hero: CrateHeroPhone) -> Widget {
        CrateHeroContent::new(hero.crate_count, true).into()
    }
}

#[derive(Clone, Debug)]
struct CrateHeroContent {
    crate_count: usize,
    compact: bool,
}

impl CrateHeroContent {
    fn new(crate_count: usize, compact: bool) -> Self {
        Self {
            crate_count,
            compact,
        }
    }
}

impl From<CrateHeroContent> for Widget {
    fn from(hero: CrateHeroContent) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Text::new(TextContent::Key("crates.eyebrow".into()))
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(TextContent::Key("crates.title".into()))
                    .size(if hero.compact { 46.0 } else { 68.0 })
                    .line_height(if hero.compact { 48.0 } else { 70.0 })
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .max_width(760.0)
                    .into(),
                Text::new(TextContent::Key("crates.body".into()))
                    .size(if hero.compact { 17.0 } else { 20.0 })
                    .line_height(if hero.compact { 27.0 } else { 32.0 })
                    .color(tokens.colors.text_secondary)
                    .max_width(700.0)
                    .into(),
                Text::new(format!("{} indexed crates", hero.crate_count))
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.secondary)
                    .into(),
            ],
            gap: Some(tokens.spacing.m),
            semantics: Some(site_semantics("crate-directory-hero")),
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(280.0),
            Length::percent(100.0),
            Length::points(1304.0),
        ))
        .padding_lengths(if hero.compact {
            [
                Length::points(tokens.spacing.xl),
                Length::points(tokens.spacing.l),
                Length::points(tokens.spacing.xxl),
                Length::points(tokens.spacing.l),
            ]
        } else {
            [
                Length::points(tokens.spacing.xxxl),
                Length::points(tokens.spacing.xl),
                Length::points(tokens.spacing.xxxl),
                Length::points(tokens.spacing.xl),
            ]
        })
        .into()
    }
}

#[derive(Clone, Debug)]
struct PlatformLegend;

impl From<PlatformLegend> for Widget {
    fn from(_legend: PlatformLegend) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Row {
            children: PLATFORMS
                .iter()
                .map(|(label, id)| PlatformPill::new(*label, *id, true).into())
                .collect(),
            gap: Some(tokens.spacing.s),
            wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            semantics: Some(site_semantics("crate-platform-legend")),
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(280.0),
            Length::percent(100.0),
            Length::points(1304.0),
        ))
        .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.large)
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateResults {
    crates: Vec<RegistryCrate>,
}

impl From<CrateResults> for Widget {
    fn from(results: CrateResults) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        if results.crates.is_empty() {
            return Container::new(Column {
                children: vec![
                    Text::new("The registry is warming up.")
                        .size(tokens.typography.heading_size)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .into(),
                    Text::new("Crates will appear after the scheduled indexer finds published packages using the fission keyword and verifies a direct Fission dependency.")
                        .size(tokens.typography.body_large_size)
                        .color(tokens.colors.text_secondary)
                        .max_width(700.0)
                        .text_align(TextAlign::Center)
                        .into(),
                ],
                gap: Some(tokens.spacing.m),
                align_items: AlignItems::Center,
                ..Default::default()
            })
            .padding_all(tokens.spacing.xxxl)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.large)
            .into();
        }

        Responsive::new(CrateGridDesktop {
            crates: results.crates.clone(),
        })
        .id(WidgetId::explicit("crates.results.responsive"))
        .case(ResponsiveCase::max_width(
            DESKTOP_BREAKPOINT,
            CrateGridSingle {
                crates: results.crates,
            },
        ))
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateGridDesktop {
    crates: Vec<RegistryCrate>,
}

impl From<CrateGridDesktop> for Widget {
    fn from(grid: CrateGridDesktop) -> Widget {
        crate_grid(grid.crates, 2)
    }
}

#[derive(Clone, Debug)]
struct CrateGridSingle {
    crates: Vec<RegistryCrate>,
}

impl From<CrateGridSingle> for Widget {
    fn from(grid: CrateGridSingle) -> Widget {
        crate_grid(grid.crates, 1)
    }
}

fn crate_grid(crates: Vec<RegistryCrate>, columns: usize) -> Widget {
    let (_ctx, view) = fission::build::current::<DocsState>();
    let tokens = &view.env().theme.tokens;
    let rows = crates
        .chunks(columns)
        .map(|crates| {
            Row {
                children: crates
                    .iter()
                    .cloned()
                    .map(|item| Container::new(CrateCard { item }).flex_grow(1.0).into())
                    .collect(),
                gap: Some(tokens.spacing.l),
                align_items: AlignItems::Stretch,
                ..Default::default()
            }
            .into()
        })
        .collect();
    Column {
        children: rows,
        gap: Some(tokens.spacing.l),
        ..Default::default()
    }
    .into()
}

#[derive(Clone, Debug)]
struct CrateCard {
    item: RegistryCrate,
}

impl From<CrateCard> for Widget {
    fn from(card: CrateCard) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let href = format!("/crates/{}/", card.item.name);
        let version = if card.item.is_prerelease() {
            format!("v{} · prerelease", card.item.version)
        } else {
            format!("v{}", card.item.version)
        };
        Container::new(Column {
            children: vec![
                SemanticRow::new(
                    format!("site-route:{href}"),
                    vec![
                        Text::new(card.item.name.clone())
                            .size(tokens.typography.heading_size)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .semantics_identifier(format!("site-route:{href}"))
                            .into(),
                        Text::new(version)
                            .size(tokens.typography.font_size_sm)
                            .family(tokens.typography.font_family_mono.clone())
                            .color(tokens.colors.primary)
                            .into(),
                    ],
                    Some(tokens.spacing.s),
                    FlexWrap::Wrap,
                    AlignItems::Center,
                    JustifyContent::SpaceBetween,
                )
                .into(),
                Text::new(card.item.description.clone())
                    .size(tokens.typography.body_medium_size)
                    .line_height(
                        tokens.typography.body_medium_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .max_width(560.0)
                    .into(),
                Text::new(format!("{} downloads", card.item.downloads))
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_muted)
                    .into(),
                Row {
                    children: card
                        .item
                        .platforms
                        .iter()
                        .filter_map(|id| PLATFORMS.iter().find(|(_, candidate)| candidate == id))
                        .map(|(label, id)| PlatformPill::new(*label, *id, false).into())
                        .collect(),
                    gap: Some(tokens.spacing.xs),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.m),
            semantics: Some(site_semantics("crate-card")),
            ..Default::default()
        })
        .padding_all(tokens.spacing.l)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.large)
        .min_height(260.0)
        .into()
    }
}

#[derive(Clone, Debug)]
struct PlatformPill {
    label: String,
    large: bool,
}

impl PlatformPill {
    fn new(label: impl Into<String>, _id: impl Into<String>, large: bool) -> Self {
        Self {
            label: label.into(),
            large,
        }
    }
}

impl From<PlatformPill> for Widget {
    fn from(pill: PlatformPill) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(pill.label)
                .size(if pill.large {
                    tokens.typography.font_size_sm
                } else {
                    11.0
                })
                .weight(tokens.typography.font_weight_medium)
                .color(tokens.colors.primary),
        )
        .padding(if pill.large {
            [10.0, 14.0, 10.0, 14.0]
        } else {
            [5.0, 8.0, 5.0, 8.0]
        })
        .bg(tokens.colors.primary_subtle)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.medium)
        .into()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CrateDetailPage {
    item: RegistryCrate,
}

impl CrateDetailPage {
    pub(crate) fn new(item: RegistryCrate) -> Self {
        Self { item }
    }
}

impl From<CrateDetailPage> for Widget {
    fn from(page: CrateDetailPage) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let item = page.item;
        Container::new(Column {
            children: vec![
                HomePageNav.into(),
                Container::new(Column {
                    children: vec![
                        Text::new("Crates  /  ".to_string() + &item.name)
                            .size(tokens.typography.font_size_sm)
                            .color(tokens.colors.text_muted)
                            .into(),
                        CrateDetailHeader { item: item.clone() }.into(),
                        PlatformSupport { item: item.clone() }.into(),
                        CrateReadme { item }.into(),
                    ],
                    gap: Some(tokens.spacing.xxl),
                    ..Default::default()
                })
                .max_width(1100.0)
                .width(1100.0)
                .padding([
                    tokens.spacing.xxl,
                    tokens.spacing.xl,
                    tokens.spacing.xxxxl,
                    tokens.spacing.xl,
                ])
                .into(),
            ],
            gap: Some(0.0),
            semantics: Some(site_semantics("crate-detail-page")),
            ..Default::default()
        })
        .min_height(800.0)
        .bg_fill(page_fill(tokens))
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateDetailHeader {
    item: RegistryCrate,
}

impl From<CrateDetailHeader> for Widget {
    fn from(header: CrateDetailHeader) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Column {
            children: vec![
                Text::new(header.item.name.clone())
                    .size(52.0)
                    .line_height(55.0)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
                Text::new(format!(
                    "v{}{}",
                    header.item.version,
                    if header.item.is_prerelease() {
                        " · prerelease"
                    } else {
                        ""
                    }
                ))
                .size(tokens.typography.font_size_sm)
                .family(tokens.typography.font_family_mono.clone())
                .color(tokens.colors.primary)
                .into(),
                Text::new(header.item.description)
                    .size(tokens.typography.body_large_size)
                    .line_height(
                        tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .max_width(800.0)
                    .into(),
            ],
            gap: Some(tokens.spacing.m),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct PlatformSupport {
    item: RegistryCrate,
}

impl From<PlatformSupport> for Widget {
    fn from(support: PlatformSupport) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            children: vec![
                Text::new("Platform support")
                    .size(tokens.typography.heading_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.heading)
                    .into(),
                Text::new("Declared by the crate author in package.metadata.fission.")
                    .size(tokens.typography.body_medium_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
                Row {
                    children: PLATFORMS
                        .iter()
                        .map(|(label, id)| {
                            let declared =
                                support.item.platforms.iter().any(|platform| platform == id);
                            Container::new(Column {
                                children: vec![
                                    Text::new(*label)
                                        .weight(tokens.typography.font_weight_bold)
                                        .into(),
                                    Text::new(if declared {
                                        "Supported"
                                    } else {
                                        "Not declared"
                                    })
                                    .size(tokens.typography.font_size_sm)
                                    .color(if declared {
                                        tokens.colors.secondary
                                    } else {
                                        tokens.colors.text_muted
                                    })
                                    .into(),
                                ],
                                gap: Some(tokens.spacing.xs),
                                align_items: AlignItems::Center,
                                ..Default::default()
                            })
                            .padding_all(tokens.spacing.m)
                            .bg(tokens.colors.surface)
                            .border(tokens.colors.border, 1.0)
                            .border_radius(tokens.radii.medium)
                            .min_width(120.0)
                            .into()
                        })
                        .collect(),
                    gap: Some(tokens.spacing.s),
                    wrap: FlexWrap::Wrap,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.m),
            ..Default::default()
        })
        .padding_all(tokens.spacing.l)
        .bg(tokens.colors.primary_subtle)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.large)
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateReadme {
    item: RegistryCrate,
}

impl From<CrateReadme> for Widget {
    fn from(readme: CrateReadme) -> Widget {
        let markdown = if readme.item.readme_markdown.trim().is_empty() {
            "## README unavailable\n\nThis release did not include a README.".to_string()
        } else {
            readme.item.readme_markdown
        };
        MarkdownViewer::new(markdown).into()
    }
}
