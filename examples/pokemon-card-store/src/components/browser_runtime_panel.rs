use crate::app::StoreState;
use crate::components::browser_cart_island::BrowserCartIsland;
use crate::components::layout::CARD_GRID_MIN_WIDTH;
use crate::components::palette::{BORDER, SURFACE, TEXT_BODY, TEXT_MUTED, TEXT_PRIMARY};
use crate::components::status_chip::StatusChip;
use fission::prelude::*;

pub struct BrowserRuntimePanel;

impl From<BrowserRuntimePanel> for Widget {
    fn from(_: BrowserRuntimePanel) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                Row {
                    gap: Some(tokens.spacing.s),
                    wrap: ir_op::FlexWrap::Wrap,
                    children: widgets![
                        StatusChip {
                            label: "Worker",
                            identifier: "worker-status:catalog-filters",
                            status: "Waiting for worker",
                        },
                        StatusChip {
                            label: "Island",
                            identifier: "island-status:cart-drawer",
                            status: "Waiting for island",
                        },
                    ],
                    ..Default::default()
                },
                Text::new("Browser bridge")
                    .size(typography.font_size_lg)
                    .line_height(
                        typography.font_size_lg * typography.line_height_heading
                    )
                    .weight(typography.font_weight_bold)
                    .color(TEXT_PRIMARY),
                Text::new("The page is server rendered first. The worker and island artifacts then load as small WASM modules and update only the semantic targets they own.")
                    .size(typography.font_size_base)
                    .line_height(
                        typography.font_size_base * typography.line_height_normal
                    )
                    .color(TEXT_MUTED),
                Grid {
                    columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                        GridTrack::Points(CARD_GRID_MIN_WIDTH),
                        GridTrack::Fr(1.0),
                    ))],
                    rows: vec![GridTrack::Auto],
                    column_gap: Some(tokens.spacing.m),
                    row_gap: Some(tokens.spacing.m),
                    children: widgets![
                        Column {
                            gap: Some(tokens.spacing.s),
                            children: widgets![
                                Text::new("Worker enhancement status pending")
                                    .size(typography.font_size_sm)
                                    .line_height(
                                        typography.font_size_sm
                                            * typography.line_height_snug
                                    )
                                    .color(TEXT_MUTED)
                                    .semantics_identifier("worker-filter-summary"),
                                Text::new("This side represents progressive enhancement. The browser worker runs off the main thread and reports when route-local catalogue behaviour is ready.")
                                    .size(typography.font_size_base)
                                    .line_height(
                                        typography.font_size_base
                                            * typography.line_height_normal
                                    )
                                    .color(TEXT_BODY),
                            ],
                            ..Default::default()
                        },
                        BrowserCartIsland,
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xxl)
        .bg(SURFACE)
        .into()
    }
}
