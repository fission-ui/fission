use crate::components::{RightSidebar, Sidebar};
use crate::inbox_router::InboxRouter;
use crate::model::InboxState;
use fission::op::AlignItems;
use fission::prelude::*;

const COMPACT_BREAKPOINT: f32 = 760.0;
const NAVIGATION_SPLIT_RATIO: f32 = 0.20;
const RIGHT_SIDEBAR_BREAKPOINT: f32 = 1_100.0;
const RIGHT_SIDEBAR_MIN_WIDTH: f32 = 232.0;
const RIGHT_SIDEBAR_MAX_WIDTH: f32 = 288.0;

fission_component! {
    pub(crate) InboxShell<InboxState> |_shell, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let width = cx.view.viewport_size().width;
    } =>
    Container(
        SafeArea(
            child: if width < COMPACT_BREAKPOINT {
                InboxRouter()
            } else {
                SplitView(
                    id: WidgetId::explicit("main_split"),
                    direction: SplitDirection::Horizontal,
                    split_ratio: NAVIGATION_SPLIT_RATIO,
                    first: Sidebar(),
                    second: Row(
                        align_items: AlignItems::Stretch,
                        children: [
                            Container(InboxRouter())
                                .flex_grow(1.0)
                                .min_width(0.0),
                            if width >= RIGHT_SIDEBAR_BREAKPOINT {
                                Container(RightSidebar())
                                    .width_length(Length::clamp(
                                        Length::points(RIGHT_SIDEBAR_MIN_WIDTH),
                                        Length::percent(22.0),
                                        Length::points(RIGHT_SIDEBAR_MAX_WIDTH),
                                    ))
                                    .flex_shrink(0.0)
                            },
                        ],
                    ),
                )
            },
        ),
    )
    .bg(tokens.colors.background)
    .flex_grow(1.0)
}
