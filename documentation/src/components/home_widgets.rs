use super::state::DocsState;
use fission::op::{AlignItems, Fill, FlexWrap, JustifyContent};
use fission::prelude::*;
use fission::{Role, Semantics};

pub(super) fn site_semantics(identifier: impl Into<String>) -> Semantics {
    let identifier = identifier.into();
    let identifier = if identifier.starts_with("value-home-") {
        format!("site-{identifier}")
    } else {
        identifier
    };
    Semantics {
        role: Role::Generic,
        identifier: Some(identifier),
        ..Semantics::default()
    }
}

#[derive(Debug)]
pub(super) struct SemanticColumn {
    identifier: String,
    children: Vec<Widget>,
    gap: Option<f32>,
    align_items: AlignItems,
}

impl SemanticColumn {
    pub(super) fn new(
        identifier: impl Into<String>,
        children: Vec<Widget>,
        gap: Option<f32>,
        align_items: AlignItems,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            children,
            gap,
            align_items,
        }
    }
}

impl From<SemanticColumn> for Widget {
    fn from(column: SemanticColumn) -> Self {
        Column {
            children: column.children,
            gap: column.gap,
            align_items: column.align_items,
            semantics: Some(site_semantics(column.identifier)),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Debug)]
pub(super) struct SemanticRow {
    identifier: String,
    children: Vec<Widget>,
    gap: Option<f32>,
    wrap: FlexWrap,
    align_items: AlignItems,
    justify_content: JustifyContent,
}

impl SemanticRow {
    pub(super) fn new(
        identifier: impl Into<String>,
        children: Vec<Widget>,
        gap: Option<f32>,
        wrap: FlexWrap,
        align_items: AlignItems,
        justify_content: JustifyContent,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            children,
            gap,
            wrap,
            align_items,
            justify_content,
        }
    }
}

impl From<SemanticRow> for Widget {
    fn from(row: SemanticRow) -> Self {
        Row {
            children: row.children,
            gap: row.gap,
            wrap: row.wrap,
            align_items: row.align_items,
            justify_content: row.justify_content,
            semantics: Some(site_semantics(row.identifier)),
            ..Default::default()
        }
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
        Row {
            children: vec![
                Column {
                    children: vec![Center {
                        child: Icon::svg(material::device::dark_mode::regular())
                            .size(tokens.spacing.m)
                            .color(tokens.colors.text_link)
                            .into(),
                    }
                    .into()],
                    semantics: Some(site_semantics("site-theme-icon:dark")),
                    ..Default::default()
                }
                .into(),
                Column {
                    children: vec![Center {
                        child: Icon::svg(material::device::light_mode::regular())
                            .size(tokens.spacing.m)
                            .color(tokens.colors.text_link)
                            .into(),
                    }
                    .into()],
                    semantics: Some(site_semantics("site-theme-icon:light")),
                    ..Default::default()
                }
                .into(),
            ],
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            semantics: Some(site_semantics("site-theme-toggle")),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(super) struct LocaleSwitcher;

impl From<LocaleSwitcher> for Widget {
    fn from(_component: LocaleSwitcher) -> Self {
        Text::new("Language")
            .semantics_identifier("site-locale-switcher")
            .into()
    }
}
#[derive(Clone, Debug)]
pub(super) struct SearchPill;

impl From<SearchPill> for Widget {
    fn from(_component: SearchPill) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        SemanticRow::new(
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
                .bg_fill(Fill::Solid(tokens.colors.surface_sunken))
                .border_radius(tokens.radii.medium)
                .into(),
            ],
            Some(tokens.spacing.s),
            FlexWrap::NoWrap,
            AlignItems::Center,
            JustifyContent::Start,
        )
        .into()
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
        // Secondary buttons are a quiet tint rather than an outline.
        let (background, foreground) = if component.primary {
            (tokens.colors.primary, tokens.colors.on_primary)
        } else {
            (tokens.colors.surface_sunken, tokens.colors.text_primary)
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
        .border_radius(tokens.radii.full)
        .into()
    }
}
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(super) struct StatusText {
    label: &'static str,
}

#[allow(dead_code)]
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
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(super) struct CodeCard {
    label: &'static str,
    command: &'static str,
}

#[allow(dead_code)]
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
        .bg_fill(Fill::Solid(tokens.colors.surface_sunken))
        .border_radius(tokens.radii.xl)
        .into()
    }
}
pub(super) fn nav_inset(tokens: &Tokens) -> f32 {
    tokens.spacing.xxxxl
}
