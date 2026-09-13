//! Sync status, calendar, quick actions, meeting controls, mailbox stats and setup.

mod cards;
mod density;

use crate::model::quick::set_calendar_selected;
use crate::model::{InboxState, SetCalendarSelected};
use cards::{MailboxStatsCard, MeetCard, QuickActionsCard, SetupCard, SyncStatusCard};
use chrono::{Datelike, Local};
use density::SidebarDensity;
use fission::core::op::FlexDirection;
use fission::core::ui::{Container, Scroll, Widget};
use fission::core::{reduce_with, ActionEnvelope};
use fission::widgets::{Calendar, VStack};
use std::sync::Arc;

pub struct RightSidebar;

impl From<RightSidebar> for Widget {
    fn from(_component: RightSidebar) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let density = SidebarDensity::for_height(view.viewport_size().height);
        let today = Local::now().date_naive();
        // A calendar reports whichever date was chosen, so its action cannot be
        // bound ahead of time; bind once for the id and fill in the date on select.
        let calendar_action = ctx
            .bind(
                SetCalendarSelected(today),
                reduce_with!(set_calendar_selected),
            )
            .id;

        Container::new(Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            child: Some(
                VStack {
                    spacing: Some(density.spacing()),
                    children: vec![
                        SyncStatusCard.into(),
                        Calendar {
                            year: today.year(),
                            month: today.month(),
                            selected_date: view.state().calendar_selected.or(Some(today)),
                            on_select: Some(Arc::new(move |date| ActionEnvelope {
                                id: calendar_action,
                                payload: serde_json::to_vec(&SetCalendarSelected(date))
                                    .expect("a date always serializes"),
                            })),
                            on_navigate: None,
                            cell_size: Some(density.calendar_cell_size()),
                            padding: Some(density.calendar_padding()),
                        }
                        .into(),
                        QuickActionsCard { density }.into(),
                        MeetCard.into(),
                        MailboxStatsCard { density }.into(),
                        SetupCard.into(),
                    ],
                }
                .into(),
            ),
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .bg(tokens.colors.surface)
        .into()
    }
}
