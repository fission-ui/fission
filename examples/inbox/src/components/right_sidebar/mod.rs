//! Sync status, calendar, quick actions, meeting controls, mailbox stats and setup.

mod cards;
mod density;

use crate::model::quick::set_calendar_selected;
use crate::model::{InboxState, SetCalendarSelected};
use cards::{MailboxStatsCard, MeetCard, QuickActionsCard, SetupCard, SyncStatusCard};
use chrono::{Datelike, Local};
use density::SidebarDensity;
use fission::prelude::*;
use std::sync::Arc;

fission_component! {
    pub RightSidebar<InboxState> |_sidebar, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let density = SidebarDensity::for_height(cx.view.viewport_size().height);
        let today = Local::now().date_naive();
        let calendar_action = cx.bind(
            SetCalendarSelected(today),
            reduce_with!(set_calendar_selected),
        );
    } =>
        Container(
            Scroll(
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            child: VStack(
                    spacing: density.spacing(),
                    children: [
                        SyncStatusCard(),
                        Calendar(
                            year: today.year(),
                            month: today.month(),
                            selected_date: state.calendar_selected.or(Some(today)),
                            on_select: Arc::new(move |date| {
                                calendar_action.with_action(&SetCalendarSelected(date))
                            }) as Arc<dyn Fn(chrono::NaiveDate) -> ActionEnvelope + Send + Sync>,
                            cell_size: density.calendar_cell_size(),
                            padding: density.calendar_padding(),
                        ),
                        QuickActionsCard(density),
                        MeetCard(),
                        MailboxStatsCard(density),
                        SetupCard(),
                    ],
            ),
            ),
        )
        .padding_all(tokens.spacing.s)
        .bg(tokens.colors.surface)
}
