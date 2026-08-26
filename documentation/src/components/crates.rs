use super::home_nav::HomePageNav;
use super::home_sections::PlatformAtlas;
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
        let crate_count = page.crates.len();
        let navigation: Widget = if view.env().locale.0 == "es-ES" {
            LocalizedNav.into()
        } else {
            HomePageNav.into()
        };
        Container::new(Column {
            children: vec![
                navigation,
                CrateDirectoryHero { crate_count }.into(),
                PlatformLegend.into(),
                Container::new(Column {
                    children: vec![
                        CrateDirectoryToolbar { crate_count }.into(),
                        CrateResults {
                            crates: page.crates,
                        }
                        .into(),
                    ],
                    gap: Some(tokens.spacing.l),
                    semantics: Some(site_semantics("crate-results")),
                    ..Default::default()
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
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(SemanticRow::new(
            "crate-directory-hero",
            vec![
                Container::new(CrateHeroContent::new(hero.crate_count, false))
                    .flex_grow(1.0)
                    .flex_shrink(1.0)
                    .into(),
                Container::new(PlatformAtlas)
                    .width_length(Length::clamp(
                        Length::points(360.0),
                        Length::percent(42.0),
                        Length::points(520.0),
                    ))
                    .flex_shrink(1.0)
                    .into(),
            ],
            Some(tokens.spacing.xxxl),
            FlexWrap::NoWrap,
            AlignItems::Center,
            JustifyContent::SpaceBetween,
        ))
        .width_length(Length::clamp(
            Length::points(280.0),
            Length::percent(100.0),
            Length::points(1400.0),
        ))
        .padding_lengths([
            Length::points(tokens.spacing.xxxl),
            Length::points(tokens.spacing.xl),
            Length::points(tokens.spacing.xxl),
            Length::points(tokens.spacing.xl),
        ])
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateHeroPhone {
    crate_count: usize,
}

impl From<CrateHeroPhone> for Widget {
    fn from(hero: CrateHeroPhone) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Container::new(CrateHeroContent::new(hero.crate_count, true))
            .width_length(Length::percent(100.0))
            .padding_lengths([
                Length::points(tokens.spacing.xl),
                Length::points(tokens.spacing.l),
                Length::points(tokens.spacing.xxxl),
                Length::points(tokens.spacing.l),
            ])
            .into()
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
        let indexed_count = if view.env().locale.0 == "es-ES" {
            format!("{} crates indexados", hero.crate_count)
        } else {
            format!("{} indexed crates", hero.crate_count)
        };
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
                CrateSearchBox.into(),
                Text::new(indexed_count)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.secondary)
                    .into(),
            ],
            gap: Some(tokens.spacing.m),
            semantics: Some(site_semantics("crate-directory-copy")),
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateSearchBox;

impl From<CrateSearchBox> for Widget {
    fn from(_search: CrateSearchBox) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let placeholder = if view.env().locale.0 == "es-ES" {
            "Buscar crates de Fission"
        } else {
            "Search Fission crates"
        };

        Container::new(Row {
            children: vec![
                Text::new("⌕")
                    .size(20.0)
                    .color(tokens.colors.primary)
                    .into(),
                Container::new(TextInput {
                    semantics_identifier: Some("crate-search-input".into()),
                    value: String::new(),
                    placeholder: Some(placeholder.into()),
                    borderless: true,
                    ..Default::default()
                })
                .flex_grow(1.0)
                .flex_shrink(1.0)
                .into(),
                Container::new(Text::new("/").size(12.0).color(tokens.colors.text_muted))
                    .padding_all(tokens.spacing.s)
                    .bg(tokens.colors.primary_subtle)
                    .border(tokens.colors.border, 1.0)
                    .border_radius(tokens.radii.small)
                    .into(),
            ],
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            semantics: Some(site_semantics("crate-searchbox")),
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .max_width(650.0)
        .min_height_length(Length::points(64.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
        .bg(tokens.colors.surface)
        .border(tokens.colors.primary, 2.0)
        .border_radius(tokens.radii.large)
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateDirectoryToolbar {
    crate_count: usize,
}

impl From<CrateDirectoryToolbar> for Widget {
    fn from(toolbar: CrateDirectoryToolbar) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let recently_updated = if view.env().locale.0 == "es-ES" {
            "Actualizados recientemente"
        } else {
            "Recently updated"
        };
        Row {
            children: vec![
                Text::new(format!("{} crates", toolbar.crate_count))
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_secondary)
                    .semantics_identifier("crate-results-count")
                    .into(),
                Text::new(recently_updated)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_medium)
                    .color(tokens.colors.text_secondary)
                    .semantics_identifier("crate-sort")
                    .into(),
            ],
            gap: Some(tokens.spacing.m),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            semantics: Some(site_semantics("crate-results-toolbar")),
            ..Default::default()
        }
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
            let (title, body) = if view.env().locale.0 == "es-ES" {
                (
                    "El registro se está preparando.",
                    "Los crates aparecerán cuando el indexador programado encuentre paquetes publicados con la palabra clave fission-framework y verifique una dependencia directa de Fission.",
                )
            } else {
                (
                    "The registry is warming up.",
                    "Crates will appear after the scheduled indexer finds published packages using the fission-framework keyword and verifies a direct Fission dependency.",
                )
            };
            return Container::new(Column {
                children: vec![
                    Text::new(title)
                        .size(tokens.typography.heading_size)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.heading)
                        .into(),
                    Text::new(body)
                        .size(tokens.typography.body_large_size)
                        .color(tokens.colors.text_secondary)
                        .max_width(700.0)
                        .text_align(TextAlign::Center)
                        .into(),
                ],
                gap: Some(tokens.spacing.m),
                align_items: AlignItems::Center,
                semantics: Some(site_semantics("crate-empty-registry")),
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
        CrateGrid {
            crates: grid.crates,
            columns: 2,
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateGridSingle {
    crates: Vec<RegistryCrate>,
}

impl From<CrateGridSingle> for Widget {
    fn from(grid: CrateGridSingle) -> Widget {
        CrateGrid {
            crates: grid.crates,
            columns: 1,
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateGrid {
    crates: Vec<RegistryCrate>,
    columns: usize,
}

impl From<CrateGrid> for Widget {
    fn from(grid: CrateGrid) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Column {
            children: grid
                .crates
                .chunks(grid.columns)
                .map(|items| {
                    CrateGridRow {
                        items: items.to_vec(),
                    }
                    .into()
                })
                .collect(),
            gap: Some(tokens.spacing.l),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateGridRow {
    items: Vec<RegistryCrate>,
}

impl From<CrateGridRow> for Widget {
    fn from(row: CrateGridRow) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Row {
            children: row
                .items
                .into_iter()
                .map(|item| Container::new(CrateCard { item }).flex_grow(1.0).into())
                .collect(),
            gap: Some(tokens.spacing.l),
            align_items: AlignItems::Stretch,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateCard {
    item: RegistryCrate,
}

impl From<CrateCard> for Widget {
    fn from(card: CrateCard) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let item = card.item;
        let href = format!("/crates/{}/", item.name);
        let platforms = item.platforms.join(",");
        let version = if item.is_prerelease() {
            if view.env().locale.0 == "es-ES" {
                format!("v{} · prelanzamiento", item.version)
            } else {
                format!("v{} · prerelease", item.version)
            }
        } else {
            format!("v{}", item.version)
        };
        let downloads = if view.env().locale.0 == "es-ES" {
            format!("{} descargas", item.downloads)
        } else {
            format!("{} downloads", item.downloads)
        };
        let updated = if view.env().locale.0 == "es-ES" {
            format!("Actualizado {}", item.updated_at)
        } else {
            format!("Updated {}", item.updated_at)
        };
        Container::new(Column {
            children: vec![
                SemanticRow::new(
                    format!("site-route:{href}"),
                    vec![
                        Text::new(item.name.clone())
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
                Text::new(item.description.clone())
                    .size(tokens.typography.body_medium_size)
                    .line_height(
                        tokens.typography.body_medium_size * tokens.typography.line_height_relaxed,
                    )
                    .color(tokens.colors.text_secondary)
                    .max_width(560.0)
                    .into(),
                SemanticRow::new(
                    "crate-card-stats",
                    vec![
                        Text::new(downloads)
                            .size(tokens.typography.font_size_sm)
                            .color(tokens.colors.text_muted)
                            .semantics_identifier(format!(
                                "crate-card-downloads:{}",
                                item.downloads
                            ))
                            .into(),
                        Text::new(updated)
                            .size(tokens.typography.font_size_sm)
                            .color(tokens.colors.text_muted)
                            .semantics_identifier(format!("crate-card-updated:{}", item.updated_at))
                            .into(),
                    ],
                    Some(tokens.spacing.m),
                    FlexWrap::Wrap,
                    AlignItems::Center,
                    JustifyContent::Start,
                )
                .into(),
                Row {
                    children: item
                        .platforms
                        .iter()
                        .filter_map(|id| PLATFORMS.iter().find(|(_, candidate)| candidate == id))
                        .map(|(label, id)| PlatformPill::new(*label, *id, false).into())
                        .collect(),
                    gap: Some(tokens.spacing.xs),
                    wrap: FlexWrap::Wrap,
                    semantics: Some(site_semantics(format!("crate-card-platforms:{platforms}"))),
                    ..Default::default()
                }
                .into(),
                CrateTaxonomyPreview {
                    keywords: item.keywords.clone(),
                    categories: item.categories.clone(),
                }
                .into(),
            ],
            gap: Some(tokens.spacing.m),
            semantics: Some(site_semantics(format!("crate-card:{}", item.name))),
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
struct CrateTaxonomyPreview {
    keywords: Vec<String>,
    categories: Vec<String>,
}

impl From<CrateTaxonomyPreview> for Widget {
    fn from(taxonomy: CrateTaxonomyPreview) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let labels = taxonomy
            .categories
            .iter()
            .chain(taxonomy.keywords.iter())
            .filter(|label| label.as_str() != "fission-framework")
            .take(5)
            .cloned()
            .collect::<Vec<_>>();

        if labels.is_empty() {
            return Column::default().into();
        }

        Row {
            children: labels
                .into_iter()
                .map(|label| {
                    Container::new(
                        Text::new(label)
                            .size(11.0)
                            .color(tokens.colors.text_secondary),
                    )
                    .padding([4.0, 7.0, 4.0, 7.0])
                    .bg(tokens.colors.surface_sunken)
                    .border(tokens.colors.border, 1.0)
                    .border_radius(tokens.radii.small)
                    .into()
                })
                .collect(),
            gap: Some(tokens.spacing.xs),
            wrap: FlexWrap::Wrap,
            semantics: Some(site_semantics("crate-card-taxonomy")),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct PlatformPill {
    label: String,
    id: String,
    large: bool,
}

impl PlatformPill {
    fn new(label: impl Into<String>, id: impl Into<String>, large: bool) -> Self {
        Self {
            label: label.into(),
            id: id.into(),
            large,
        }
    }
}

impl From<PlatformPill> for Widget {
    fn from(pill: PlatformPill) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let identifier = if pill.large {
            format!("crate-platform-filter:{}", pill.id)
        } else {
            format!("crate-platform:{}", pill.id)
        };
        Container::new(
            Text::new(pill.label)
                .size(if pill.large {
                    tokens.typography.font_size_sm
                } else {
                    11.0
                })
                .weight(tokens.typography.font_weight_medium)
                .color(tokens.colors.primary)
                .semantics_identifier(identifier),
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
        let navigation: Widget = if view.env().locale.0 == "es-ES" {
            LocalizedNav.into()
        } else {
            HomePageNav.into()
        };
        Container::new(Column {
            children: vec![
                navigation,
                Container::new(CrateDetailContent { item: page.item })
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
            semantics: Some(site_semantics("crate-detail-page")),
            ..Default::default()
        })
        .bg_fill(page_fill(tokens))
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateDetailContent {
    item: RegistryCrate,
}

impl From<CrateDetailContent> for Widget {
    fn from(content: CrateDetailContent) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let crates_href = if view.env().locale.0 == "es-ES" {
            "/es/crates/"
        } else {
            "/crates/"
        };
        let item = content.item;
        Column {
            children: vec![
                SemanticRow::new(
                    "crate-breadcrumb",
                    vec![
                        Text::new("Crates")
                            .size(tokens.typography.font_size_sm)
                            .weight(tokens.typography.font_weight_medium)
                            .color(tokens.colors.primary)
                            .semantics_identifier(format!("site-route:{crates_href}"))
                            .into(),
                        Text::new("/")
                            .size(tokens.typography.font_size_sm)
                            .color(tokens.colors.text_muted)
                            .into(),
                        Text::new(item.name.clone())
                            .size(tokens.typography.font_size_sm)
                            .color(tokens.colors.text_muted)
                            .into(),
                    ],
                    Some(tokens.spacing.s),
                    FlexWrap::Wrap,
                    AlignItems::Center,
                    JustifyContent::Start,
                )
                .into(),
                CrateDetailHeader { item: item.clone() }.into(),
                InstallCommand {
                    crate_name: item.name.clone(),
                }
                .into(),
                PlatformSupport { item: item.clone() }.into(),
                CrateDetailBody { item }.into(),
            ],
            gap: Some(tokens.spacing.xxl),
            ..Default::default()
        }
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
            semantics: Some(site_semantics("crate-detail-header")),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct InstallCommand {
    crate_name: String,
}

impl From<InstallCommand> for Widget {
    fn from(install: InstallCommand) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let command = format!("cargo add {}", install.crate_name);
        Container::new(Column {
            children: vec![
                Text::new("Install")
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_secondary)
                    .into(),
                Row {
                    children: vec![
                        Text::new(command)
                            .size(tokens.typography.body_medium_size)
                            .family(tokens.typography.font_family_mono.clone())
                            .color(tokens.colors.heading)
                            .flex_grow(1.0)
                            .semantics_identifier(format!(
                                "crate-install-command:{}",
                                install.crate_name
                            ))
                            .into(),
                        Text::new("Copy")
                            .size(tokens.typography.font_size_sm)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.primary)
                            .semantics_identifier("crate-copy-install")
                            .into(),
                    ],
                    gap: Some(tokens.spacing.m),
                    wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.s),
            semantics: Some(site_semantics("crate-install")),
            ..Default::default()
        })
        .padding_all(tokens.spacing.l)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.large)
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
                Text::new(
                    "Declared by the crate author and captured when this directory was generated.",
                )
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
                                semantics: Some(site_semantics(format!(
                                    "crate-platform-status:{}:{}",
                                    id,
                                    if declared {
                                        "supported"
                                    } else {
                                        "not-declared"
                                    }
                                ))),
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
            semantics: Some(site_semantics("crate-platform-support")),
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
struct CrateDetailBody {
    item: RegistryCrate,
}

impl From<CrateDetailBody> for Widget {
    fn from(body: CrateDetailBody) -> Widget {
        Responsive::new(CrateDetailBodyDesktop {
            item: body.item.clone(),
        })
        .id(WidgetId::explicit("crates.detail.body.responsive"))
        .case(ResponsiveCase::max_width(
            DESKTOP_BREAKPOINT,
            CrateDetailBodySingle { item: body.item },
        ))
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateDetailBodyDesktop {
    item: RegistryCrate,
}

impl From<CrateDetailBodyDesktop> for Widget {
    fn from(body: CrateDetailBodyDesktop) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Row {
            children: vec![
                Container::new(CrateReadme {
                    item: body.item.clone(),
                })
                .flex_grow(1.0)
                .flex_shrink(1.0)
                .into(),
                Container::new(CrateMetadata { item: body.item })
                    .width_length(Length::clamp(
                        Length::points(260.0),
                        Length::percent(30.0),
                        Length::points(340.0),
                    ))
                    .flex_shrink(0.0)
                    .into(),
            ],
            gap: Some(tokens.spacing.xxl),
            align_items: AlignItems::Start,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateDetailBodySingle {
    item: RegistryCrate,
}

impl From<CrateDetailBodySingle> for Widget {
    fn from(body: CrateDetailBodySingle) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Column {
            children: vec![
                CrateMetadata {
                    item: body.item.clone(),
                }
                .into(),
                CrateReadme { item: body.item }.into(),
            ],
            gap: Some(tokens.spacing.xxl),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateMetadata {
    item: RegistryCrate,
}

impl From<CrateMetadata> for Widget {
    fn from(metadata: CrateMetadata) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let item = metadata.item;
        let mut children: Vec<Widget> = vec![
            Text::new("Package metadata")
                .size(tokens.typography.heading_size)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .into(),
            MetadataLinkField::new(
                "crates.io",
                "View package",
                format!("https://crates.io/crates/{}", item.name),
            )
            .into(),
        ];

        children.push(
            MetadataOptionalLinkField::new("Documentation", item.documentation.clone()).into(),
        );
        children.push(MetadataOptionalLinkField::new("Repository", item.repository.clone()).into());
        children.push(
            MetadataTextField::new(
                "License",
                item.license
                    .clone()
                    .filter(|license| !license.trim().is_empty())
                    .unwrap_or_else(|| "Not provided".to_string()),
                true,
            )
            .into(),
        );
        children.push(MetadataTextField::new("Latest version", item.version.clone(), true).into());
        children.push(MetadataTextField::new("Downloads", item.downloads.to_string(), true).into());
        children.push(
            MetadataTextField::new(
                "Last updated",
                if item.updated_at.trim().is_empty() {
                    "Unknown".to_string()
                } else {
                    item.updated_at.clone()
                },
                true,
            )
            .into(),
        );
        children.push(MetadataPillField::new("Categories", item.categories.clone(), false).into());
        children.push(MetadataPillField::new("Keywords", item.keywords.clone(), false).into());
        children
            .push(MetadataPillField::new("Published versions", item.versions.clone(), true).into());
        children.push(
            MetadataTextField::new(
                "Dependencies",
                "Dependency metadata is not indexed yet.",
                false,
            )
            .into(),
        );

        Container::new(Column {
            children,
            gap: Some(tokens.spacing.l),
            semantics: Some(site_semantics("crate-metadata")),
            ..Default::default()
        })
        .padding_all(tokens.spacing.l)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.large)
        .into()
    }
}

#[derive(Clone, Debug)]
struct MetadataTextField {
    label: String,
    value: String,
    mono: bool,
}

impl MetadataTextField {
    fn new(label: impl Into<String>, value: impl Into<String>, mono: bool) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            mono,
        }
    }
}

impl From<MetadataTextField> for Widget {
    fn from(field: MetadataTextField) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let mut value = Text::new(field.value)
            .size(tokens.typography.font_size_sm)
            .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_relaxed)
            .color(tokens.colors.text_secondary);
        if field.mono {
            value = value.family(tokens.typography.font_family_mono.clone());
        }
        Column {
            children: vec![
                Text::new(field.label)
                    .size(11.0)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_muted)
                    .into(),
                value.into(),
            ],
            gap: Some(tokens.spacing.xs),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct MetadataLinkField {
    label: String,
    link_label: String,
    href: String,
}

impl MetadataLinkField {
    fn new(
        label: impl Into<String>,
        link_label: impl Into<String>,
        href: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            link_label: link_label.into(),
            href: href.into(),
        }
    }
}

impl From<MetadataLinkField> for Widget {
    fn from(field: MetadataLinkField) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        Column {
            children: vec![
                Text::new(field.label)
                    .size(11.0)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_muted)
                    .into(),
                Text::new(field.link_label)
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_medium)
                    .color(tokens.colors.primary)
                    .semantics_identifier(format!("markdown-link:{}", field.href))
                    .into(),
            ],
            gap: Some(tokens.spacing.xs),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct MetadataOptionalLinkField {
    label: String,
    href: Option<String>,
}

impl MetadataOptionalLinkField {
    fn new(label: impl Into<String>, href: Option<String>) -> Self {
        Self {
            label: label.into(),
            href,
        }
    }
}

impl From<MetadataOptionalLinkField> for Widget {
    fn from(field: MetadataOptionalLinkField) -> Widget {
        match field.href.as_deref().and_then(safe_external_url) {
            Some(href) => MetadataLinkField::new(field.label, "Open link", href).into(),
            None => MetadataTextField::new(field.label, "Not provided", false).into(),
        }
    }
}

fn safe_external_url(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return None;
    }
    let (scheme, remainder) = value.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    (!authority.is_empty()).then(|| value.to_string())
}

#[derive(Clone, Debug)]
struct MetadataPillField {
    label: String,
    values: Vec<String>,
    mono: bool,
}

impl MetadataPillField {
    fn new(label: impl Into<String>, values: Vec<String>, mono: bool) -> Self {
        Self {
            label: label.into(),
            values,
            mono,
        }
    }
}

impl From<MetadataPillField> for Widget {
    fn from(field: MetadataPillField) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let label = field.label;
        let mono = field.mono;
        let values = field.values.into_iter().take(12).collect::<Vec<_>>();
        let body: Widget = if values.is_empty() {
            Text::new("None declared")
                .size(tokens.typography.font_size_sm)
                .color(tokens.colors.text_muted)
                .into()
        } else {
            Row {
                children: values
                    .into_iter()
                    .map(|value| {
                        let mut text = Text::new(value)
                            .size(11.0)
                            .color(tokens.colors.text_secondary);
                        if mono {
                            text = text.family(tokens.typography.font_family_mono.clone());
                        }
                        Container::new(text)
                            .padding([5.0, 8.0, 5.0, 8.0])
                            .bg(tokens.colors.surface_sunken)
                            .border(tokens.colors.border, 1.0)
                            .border_radius(tokens.radii.small)
                            .into()
                    })
                    .collect(),
                gap: Some(tokens.spacing.xs),
                wrap: FlexWrap::Wrap,
                ..Default::default()
            }
            .into()
        };
        Column {
            children: vec![
                Text::new(label)
                    .size(11.0)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_muted)
                    .into(),
                body,
            ],
            gap: Some(tokens.spacing.xs),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct CrateReadme {
    item: RegistryCrate,
}

impl From<CrateReadme> for Widget {
    fn from(readme: CrateReadme) -> Widget {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let markdown = if readme.item.readme_markdown.trim().is_empty() {
            "## README unavailable\n\nThis release did not include a README.".to_string()
        } else {
            readme.item.readme_markdown
        };
        Container::new(Column {
            children: vec![
                Text::new("README")
                    .size(tokens.typography.font_size_sm)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_muted)
                    .into(),
                MarkdownViewer::new(markdown).into(),
            ],
            gap: Some(tokens.spacing.l),
            semantics: Some(site_semantics("crate-readme")),
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::safe_external_url;

    #[test]
    fn crate_metadata_links_allow_only_http_urls_with_an_authority() {
        assert_eq!(
            safe_external_url(" https://docs.example.test/crate "),
            Some("https://docs.example.test/crate".to_string())
        );
        assert_eq!(
            safe_external_url("HTTP://example.test"),
            Some("HTTP://example.test".to_string())
        );
        for unsafe_url in [
            "javascript:alert(1)",
            "data:text/html,hello",
            "file:///tmp/crate",
            "https:///missing-authority",
            "https://example.test\nscript",
            "",
        ] {
            assert_eq!(safe_external_url(unsafe_url), None, "{unsafe_url:?}");
        }
    }
}
