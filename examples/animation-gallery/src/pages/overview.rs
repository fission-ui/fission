use crate::state::{navigate_to, AnimationGalleryState, NavigateTo};
use crate::style::*;
use crate::ui;
use crate::widgets;
use fission::build::BuildCtxHandle;
use fission::{Button, ButtonVariant, Column, Container, Row, Text, Widget};

pub struct OverviewPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<OverviewPage<'_>> for Widget {
    fn from(page: OverviewPage<'_>) -> Self {
        let _state = page.state;
        Column {
            gap: Some(16.0),
            children: vec![
                ui::PageHeader {
                    title: "Animation Gallery",
                    subtitle: "A calm workbench for real widget motion, property inspection, composition, policy, and tests.",
                }
                .into(),
                HeroPanel.into(),
                OverviewGrid {
                    ctx: &page.ctx,
                }
                .into(),
                MatrixCard {
                    title: "All animated widgets",
                    content: WIDGET_MATRIX,
                }
                .into(),
                MatrixCard {
                    title: "Motion properties",
                    content: PROPERTY_MATRIX,
                }
                .into(),
                ui::PageNote {
                    title: "Core idea",
                    body: "Animated UI in Fission is explicit, deterministic, inspectable, and optional. Common behavior is selected ergonomically; everything lowers to MotionExpr and can be tested.",
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct HeroPanel;

impl From<HeroPanel> for Widget {
    fn from(_: HeroPanel) -> Self {
        Container::new(Row {
            gap: Some(14.0),
            children: vec![
                HeroMetric {
                    value: "12",
                    label: "widget demos",
                }
                .into(),
                HeroMetric {
                    value: "9",
                    label: "motion properties",
                }
                .into(),
                HeroMetric {
                    value: "4",
                    label: "composition routes",
                }
                .into(),
                HeroMetric {
                    value: "9",
                    label: "LiveTests",
                }
                .into(),
            ],
            ..Default::default()
        })
        .padding_all(16.0)
        .border(BORDER, 1.0)
        .border_radius(18.0)
        .bg(color(244, 248, 255, 255))
        .into()
    }
}

struct HeroMetric<'a> {
    value: &'a str,
    label: &'a str,
}

impl From<HeroMetric<'_>> for Widget {
    fn from(metric: HeroMetric<'_>) -> Self {
        Container::new(Column {
            gap: Some(4.0),
            children: vec![
                Text::new(metric.value).size(24.0).color(BLUE).into(),
                Text::new(metric.label).size(11.0).color(MUTED).into(),
            ],
            ..Default::default()
        })
        .width(140.0)
        .padding_all(12.0)
        .border(BORDER, 1.0)
        .border_radius(14.0)
        .bg(SURFACE)
        .into()
    }
}

struct OverviewGrid<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
}

impl From<OverviewGrid<'_>> for Widget {
    fn from(grid: OverviewGrid<'_>) -> Self {
        let summaries = widgets::summaries();
        let mut rows = Vec::new();
        for chunk in summaries.chunks(4) {
            rows.push(
                Row {
                    gap: Some(14.0),
                    children: chunk
                        .iter()
                        .copied()
                        .map(|summary| {
                            OverviewCard {
                                ctx: grid.ctx,
                                summary,
                            }
                            .into()
                        })
                        .collect(),
                    ..Default::default()
                }
                .into(),
            );
        }

        Column {
            gap: Some(14.0),
            children: rows,
            ..Default::default()
        }
        .into()
    }
}

struct OverviewCard<'a> {
    ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    summary: widgets::common::WidgetSummary,
}

impl From<OverviewCard<'_>> for Widget {
    fn from(card: OverviewCard<'_>) -> Self {
        Button {
            variant: ButtonVariant::Ghost,
            on_press: Some(card.ctx.bind(
                NavigateTo(card.summary.path.to_string()),
                fission::reduce_with!(navigate_to),
            )),
            child: Some(
                Container::new(Column {
                    gap: Some(10.0),
                    children: vec![
                        Container::new(Text::new(card.summary.glyph).size(16.0).color(BLUE))
                            .width(82.0)
                            .height(64.0)
                            .padding_all(18.0)
                            .border_radius(12.0)
                            .bg(card.summary.tint)
                            .border(color(190, 204, 244, 255), 1.0)
                            .into(),
                        Text::new(card.summary.title).size(14.0).color(INK).into(),
                        Text::new(card.summary.subtitle)
                            .size(11.0)
                            .color(MUTED)
                            .into(),
                        ui::ColorDots {
                            colors: &[TEAL, VIOLET, BLUE, CYAN],
                        }
                        .into(),
                    ],
                    ..Default::default()
                })
                .padding_all(12.0)
                .width(146.0)
                .border(BORDER, 1.0)
                .border_radius(14.0)
                .bg(SURFACE)
                .into(),
            ),
            ..Default::default()
        }
        .into()
    }
}

struct MatrixCard<'a> {
    title: &'a str,
    content: &'a str,
}

impl From<MatrixCard<'_>> for Widget {
    fn from(card: MatrixCard<'_>) -> Self {
        Container::new(Column {
            gap: Some(8.0),
            children: vec![
                ui::SectionTitle { title: card.title }.into(),
                ui::CodeBlock {
                    source: card.content,
                }
                .into(),
            ],
            ..Default::default()
        })
        .padding_all(14.0)
        .border(BORDER, 1.0)
        .border_radius(16.0)
        .bg(SURFACE)
        .into()
    }
}

const WIDGET_MATRIX: &str = r#"Widget      None  Default  Composition  Native  Reduced  Disabled  Tests
Modal       yes   yes      yes          yes     yes      yes       yes
Drawer      yes   yes      yes          yes     yes      yes       yes
Accordion   yes   yes      yes          yes     yes      yes       yes
Tabs        yes   yes      yes          yes     yes      yes       yes
Button      yes   yes      yes          yes     yes      yes       yes
Toast       yes   yes      yes          yes     yes      yes       yes
Popover     yes   yes      yes          yes     yes      yes       yes
Tooltip     yes   yes      yes          yes     yes      yes       yes
Checkbox    yes   n/a      n/a          yes     yes      yes       yes
Switch      yes   n/a      n/a          yes     yes      yes       yes"#;

const PROPERTY_MATRIX: &str = r#"Property          Composite  Layout  Paint  Native  Reduced Policy  Tests
Opacity           yes        no      yes    yes     yes             yes
TranslateX/Y      yes        no      no     yes     yes             yes
Scale             yes        no      no     yes     partial         yes
Rotation          yes        no      no     yes     partial         yes
Width/Height      no         yes     no     yes     partial         yes
BackgroundColor   no         no      yes    yes     yes             yes
CornerRadius      no         no      yes    yes     yes             yes"#;
