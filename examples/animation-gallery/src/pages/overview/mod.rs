mod hero_metric;
mod hero_panel;
mod matrix_card;
mod overview_card;
mod overview_grid;

use crate::state::AnimationGalleryState;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use hero_panel::HeroPanel;
use matrix_card::MatrixCard;
use overview_grid::OverviewGrid;

pub struct OverviewPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<OverviewPage<'_>> for Widget {
    fn from(page: OverviewPage<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let spacing = &view.env().theme.tokens.spacing;

        Column {
            gap: Some(spacing.m),
            children: widgets![
                ui::PageHeader {
                    title: "Animation Gallery",
                    subtitle: "A calm workbench for real widget motion, property inspection, composition, policy, and tests.",
                },
                HeroPanel,
                OverviewGrid { ctx: &page.ctx },
                MatrixCard {
                    title: "All animated widgets",
                    content: WIDGET_MATRIX,
                },
                MatrixCard {
                    title: "Motion properties",
                    content: PROPERTY_MATRIX,
                },
                ui::PageNote {
                    title: "Core idea",
                    body: "Animated UI in Fission is explicit, deterministic, inspectable, and optional. Common behavior is selected ergonomically; everything lowers to MotionExpr and can be tested.",
                },
            ],
            ..Default::default()
        }
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
Width/Height      no         yes     no      yes     partial         yes
BackgroundColor   no         no      yes    yes     yes             yes
CornerRadius      no         no      yes    yes     yes             yes"#;
