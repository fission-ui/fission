//! Layout building blocks shared by the marketing pages. Content carries the page: sections are
//! open, peer items sit in columns rather than cards, and only real content such as code or a
//! screenshot gets its own surface.

use super::home_widgets::{site_semantics, Cta, NavLink, SemanticColumn, SemanticRow};
use super::landing::{centred_band, landing_width};
use super::state::DocsState;
use fission::op::{AlignItems, BoxShadow, Fill, FlexWrap, JustifyContent, TextAlign};
use fission::prelude::*;

/// How a section's heading block lines up with its content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum HeadingAlign {
    Start,
    Center,
}

/// A full-width page section: optional heading block, then content, on the page width.
pub(super) struct Section {
    pub(super) identifier: &'static str,
    pub(super) anchor: Option<&'static str>,
    pub(super) eyebrow: &'static str,
    pub(super) title: &'static str,
    pub(super) lead: &'static str,
    pub(super) align: HeadingAlign,
    pub(super) tinted: bool,
    pub(super) content: Vec<Widget>,
}

impl From<Section> for Widget {
    fn from(section: Section) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let center = section.align == HeadingAlign::Center;
        let mut children = Vec::new();
        if !section.title.is_empty() {
            children.push(heading_block(
                tokens,
                section.anchor,
                section.eyebrow,
                section.title,
                section.lead,
                center,
            ));
        }
        children.extend(section.content);
        let column = Container::new(SemanticColumn::new(
            section.identifier,
            children,
            Some(tokens.spacing.xl),
            if center {
                AlignItems::Center
            } else {
                AlignItems::Stretch
            },
        ))
        .width_length(Length::percent(100.0))
        .max_width(landing_width(tokens));
        let mut band = Container::new(centred_band("site-kit-band", column.into()))
            .padding([
                tokens.spacing.xl,
                tokens.spacing.xl,
                tokens.spacing.xxxl,
                tokens.spacing.xxxl,
            ])
            .width_length(Length::percent(100.0));
        if section.tinted {
            band = band.bg_fill(Fill::Solid(tokens.colors.surface_sunken));
        }
        band.into()
    }
}

/// Eyebrow, title and lead paragraph.
pub(super) fn heading_block(
    tokens: &Tokens,
    anchor: Option<&'static str>,
    eyebrow: &'static str,
    title: &'static str,
    lead: &'static str,
    center: bool,
) -> Widget {
    let align = if center {
        TextAlign::Center
    } else {
        TextAlign::Start
    };
    let mut children = Vec::new();
    if !eyebrow.is_empty() {
        let mut label = Text::new(eyebrow.to_uppercase())
            .size(tokens.typography.font_size_xs)
            .weight(tokens.typography.font_weight_bold)
            .color(tokens.colors.primary)
            .text_align(align);
        if let Some(anchor) = anchor {
            label = label.semantics_identifier(format!("site-anchor:{anchor}"));
        }
        children.push(label.into());
    }
    children.push(
        Text::new(title)
            .size(tokens.typography.heading2_size)
            .line_height(tokens.typography.heading2_size * tokens.typography.line_height_heading)
            .weight(tokens.typography.font_weight_bold)
            .color(tokens.colors.heading)
            .text_align(align)
            .max_width(760.0)
            .into(),
    );
    if !lead.is_empty() {
        children.push(
            Text::new(lead)
                .size(tokens.typography.body_large_size)
                .line_height(
                    tokens.typography.body_large_size * tokens.typography.line_height_relaxed,
                )
                .color(tokens.colors.text_secondary)
                .text_align(align)
                .max_width(680.0)
                .into(),
        );
    }
    Column {
        children,
        gap: Some(tokens.spacing.m),
        align_items: if center {
            AlignItems::Center
        } else {
            AlignItems::Start
        },
        ..Default::default()
    }
    .into()
}

/// One open feature: icon, heading and a short explanation.
#[derive(Clone, Copy, Debug)]
pub(super) struct Feature {
    pub(super) icon: fn() -> &'static str,
    pub(super) title: &'static str,
    pub(super) body: &'static str,
}

/// Features laid out in columns with no card around them.
pub(super) fn feature_columns(
    tokens: &Tokens,
    identifier: &'static str,
    features: &[Feature],
) -> Widget {
    SemanticRow::new(
        identifier,
        features
            .iter()
            .map(|feature| {
                Column {
                    children: vec![
                        Icon::svg((feature.icon)())
                            .size(tokens.spacing.xl)
                            .color(tokens.colors.primary)
                            .into(),
                        Text::new(feature.title)
                            .size(tokens.typography.font_size_lg)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        body_text(tokens, feature.body),
                    ],
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into()
            })
            .collect(),
        Some(tokens.spacing.xl),
        FlexWrap::Wrap,
        AlignItems::Start,
        JustifyContent::SpaceBetween,
    )
    .into()
}

/// Secondary body copy at the reading size.
pub(super) fn body_text(tokens: &Tokens, body: impl Into<TextContent>) -> Widget {
    Text::new(body)
        .size(tokens.typography.font_size_base)
        .line_height(tokens.typography.font_size_base * tokens.typography.line_height_relaxed)
        .color(tokens.colors.text_secondary)
        .into()
}

/// Two columns side by side, stacking on narrow screens.
pub(super) fn split(
    tokens: &Tokens,
    identifier: &'static str,
    first: Widget,
    second: Widget,
) -> Widget {
    SemanticRow::new(
        identifier,
        vec![first, second],
        Some(tokens.spacing.xxl),
        FlexWrap::Wrap,
        AlignItems::Center,
        JustifyContent::SpaceBetween,
    )
    .into()
}

/// Numbered steps in a row, each with a short accent bar instead of a box.
pub(super) fn steps(
    tokens: &Tokens,
    identifier: &'static str,
    items: &[(&'static str, &'static str)],
) -> Widget {
    SemanticRow::new(
        identifier,
        items
            .iter()
            .enumerate()
            .map(|(index, (title, body))| {
                Column {
                    children: vec![
                        Container::new(Spacer::default())
                            .width(tokens.spacing.xl)
                            .height(3.0)
                            .bg_fill(Fill::Solid(tokens.colors.primary))
                            .border_radius(tokens.radii.full)
                            .into(),
                        Text::new(format!("{:02}", index + 1))
                            .size(tokens.typography.font_size_sm)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.primary)
                            .into(),
                        Text::new(*title)
                            .size(tokens.typography.font_size_lg)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.heading)
                            .into(),
                        body_text(tokens, *body),
                    ],
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into()
            })
            .collect(),
        Some(tokens.spacing.xl),
        FlexWrap::Wrap,
        AlignItems::Start,
        JustifyContent::SpaceBetween,
    )
    .into()
}

/// A row in a divided list: heading, body and an optional link. The divider comes from CSS.
pub(super) fn list_row(
    tokens: &Tokens,
    eyebrow: &'static str,
    title: &'static str,
    body: &'static str,
    link: Option<(&'static str, &'static str)>,
) -> Widget {
    let mut children = Vec::new();
    if !eyebrow.is_empty() {
        children.push(
            Text::new(eyebrow.to_uppercase())
                .size(tokens.typography.font_size_xs)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.primary)
                .into(),
        );
    }
    children.extend([
        Text::new(title)
            .size(tokens.typography.font_size_lg)
            .weight(tokens.typography.font_weight_bold)
            .color(tokens.colors.heading)
            .into(),
        body_text(tokens, body),
    ]);
    if let Some((label, href)) = link {
        children.push(arrow_link(label, href));
    }
    Column {
        children,
        gap: Some(tokens.spacing.xs),
        semantics: Some(site_semantics("site-kit-list-row")),
        ..Default::default()
    }
    .into()
}

/// Rows stacked with dividers between them.
pub(super) fn divided_list(tokens: &Tokens, identifier: &'static str, rows: Vec<Widget>) -> Widget {
    SemanticColumn::new(
        identifier,
        rows,
        Some(tokens.spacing.none),
        AlignItems::Stretch,
    )
    .into()
}

/// A text link with a trailing arrow.
pub(super) fn arrow_link(label: &'static str, href: &'static str) -> Widget {
    NavLink::new(label, href).into()
}

/// Monospaced code on a dark surface, the same in both themes.
pub(super) fn code_block(tokens: &Tokens, identifier: &'static str, code: &'static str) -> Widget {
    Container::new(
        Text::new(code)
            .size(tokens.typography.font_size_sm)
            .line_height(tokens.typography.font_size_sm * 1.65)
            .family(tokens.typography.font_family_mono.clone())
            .color(Color {
                r: 226,
                g: 228,
                b: 255,
                a: 255,
            })
            .semantics_identifier(identifier),
    )
    .padding_all(tokens.spacing.xl)
    .bg_fill(Fill::Solid(Color {
        r: 22,
        g: 22,
        b: 36,
        a: 255,
    }))
    .border_radius(tokens.radii.xl)
    .into()
}

/// A screenshot with rounded corners and a soft shadow, and a caption under it.
pub(super) fn screenshot(
    tokens: &Tokens,
    src: &'static str,
    width: f32,
    height: f32,
    caption: &'static str,
) -> Widget {
    let mut children = vec![Container::new(Image::asset(src).size(width, height))
        .border_radius(tokens.radii.large)
        .shadow(BoxShadow {
            color: tokens.colors.text_primary.with_alpha(30),
            blur_radius: 30.0,
            spread_radius: 0.0,
            offset: (0.0, 14.0),
            inset: false,
        })
        .into()];
    if !caption.is_empty() {
        children.push(
            Text::new(caption)
                .size(tokens.typography.font_size_sm)
                .color(tokens.colors.text_secondary)
                .into(),
        );
    }
    Column {
        children,
        gap: Some(tokens.spacing.m),
        semantics: Some(site_semantics("site-kit-screenshot")),
        ..Default::default()
    }
    .into()
}

/// The closing call to action: centred text and buttons on a tinted band.
pub(super) struct CallToAction {
    pub(super) title: &'static str,
    pub(super) body: &'static str,
    pub(super) primary: (&'static str, &'static str),
    pub(super) secondary: Option<(&'static str, &'static str)>,
}

impl From<CallToAction> for Widget {
    fn from(cta: CallToAction) -> Self {
        let (_ctx, view) = fission::build::current::<DocsState>();
        let tokens = &view.env().theme.tokens;
        let mut buttons = vec![Cta::new(cta.primary.0, cta.primary.1, true).into()];
        if let Some((label, href)) = cta.secondary {
            buttons.push(Cta::new(label, href, false).into());
        }
        Section {
            identifier: "site-kit-cta",
            anchor: None,
            eyebrow: "",
            title: cta.title,
            lead: cta.body,
            align: HeadingAlign::Center,
            tinted: true,
            content: vec![Row {
                children: buttons,
                gap: Some(tokens.spacing.m),
                wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            }
            .into()],
        }
        .into()
    }
}
