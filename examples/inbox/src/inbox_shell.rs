use crate::components::{RightSidebar, Sidebar};
use crate::inbox_router::InboxRouter;
use crate::model::InboxState;
use fission::core::ui::{Container, Row, Widget};
use fission::core::Length;
use fission::widgets::{SafeArea, SplitDirection, SplitView};

const COMPACT_BREAKPOINT: f32 = 760.0;
const NAVIGATION_SPLIT_RATIO: f32 = 0.20;
const RIGHT_SIDEBAR_BREAKPOINT: f32 = 1_100.0;
const RIGHT_SIDEBAR_MIN_WIDTH: f32 = 232.0;
const RIGHT_SIDEBAR_MAX_WIDTH: f32 = 288.0;

pub(crate) struct InboxShell;

impl From<InboxShell> for Widget {
    fn from(_shell: InboxShell) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let width = view.viewport_size().width;

        let content: Widget = if width < COMPACT_BREAKPOINT {
            InboxRouter.into()
        } else {
            let mut mail_children = vec![Container::new(InboxRouter)
                .flex_grow(1.0)
                .min_width(0.0)
                .into()];
            if width >= RIGHT_SIDEBAR_BREAKPOINT {
                mail_children.push(
                    Container::new(RightSidebar)
                        .width_length(Length::clamp(
                            Length::points(RIGHT_SIDEBAR_MIN_WIDTH),
                            Length::percent(22.0),
                            Length::points(RIGHT_SIDEBAR_MAX_WIDTH),
                        ))
                        .flex_shrink(0.0)
                        .into(),
                );
            }

            SplitView {
                id: fission::WidgetId::explicit("main_split"),
                direction: SplitDirection::Horizontal,
                split_ratio: NAVIGATION_SPLIT_RATIO,
                on_resize: None,
                first: Sidebar.into(),
                second: Row {
                    gap: None,
                    align_items: fission::op::AlignItems::Stretch,
                    children: mail_children,
                    ..Default::default()
                }
                .into(),
            }
            .into()
        };

        Container::new(SafeArea {
            id: None,
            child: content,
        })
        .bg(tokens.colors.background)
        .flex_grow(1.0)
        .into()
    }
}
