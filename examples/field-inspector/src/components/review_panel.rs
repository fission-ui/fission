use crate::components::activity_log::ActivityLog;
use crate::components::section_header::SectionHeader;
use crate::components::ui::{ActionButton, BodyText, PanelCard, SmallButton, SoftPanel};
use crate::model::{
    on_adjust_alert_volume, on_copy_report_summary, on_deep_link_received, on_schedule_reminder,
    on_submit_report, AdjustAlertVolume, CopyReportSummary, FieldInspectorState, ScheduleReminder,
    SubmitReport,
};
use fission::prelude::*;
fission_component! {
    pub ReviewPanel<FieldInspectorState> {}
    |_panel, state, cx| {
        let copy = with_reducer!(cx.build, CopyReportSummary, on_copy_report_summary);
        let reminder = with_reducer!(cx.build, ScheduleReminder, on_schedule_reminder);
        let volume_down = with_reducer!(cx.build, AdjustAlertVolume(VolumeAdjustDirection::Down), on_adjust_alert_volume);
        let volume_up = with_reducer!(cx.build, AdjustAlertVolume(VolumeAdjustDirection::Up), on_adjust_alert_volume);
        let submit = with_reducer!(cx.build, SubmitReport, on_submit_report);
        let deep_link = cx.bind(DeepLinkReceived {
            link: DeepLink::new(format!("field-inspector://work-orders/{}", state.selected_order().id)).source(DeepLinkSource::CustomScheme),
        }, reduce_with!(on_deep_link_received));
        let spacing = &cx.view.env().theme.tokens.spacing;
    } =>
        PanelCard(Column(
            gap: spacing.m,
            children: [
                SectionHeader("Review and submit", "The report gathers host-provided context into a plain summary that can be copied, linked from notifications, or submitted."),
                SoftPanel(BodyText(state.report_summary())),
                Row(gap: spacing.s, wrap: ir_op::FlexWrap::Wrap, children: [
                    ActionButton("field-inspector.review.copy".to_string(), "Copy summary".to_string(), copy, ButtonVariant::SecondaryGray),
                    ActionButton("field-inspector.review.reminder".to_string(), "Schedule reminder".to_string(), reminder, ButtonVariant::SecondaryColor),
                    ActionButton("field-inspector.review.deep-link".to_string(), "Open deep link".to_string(), deep_link, ButtonVariant::Ghost),
                    SmallButton("field-inspector.review.volume-down".to_string(), "Volume -".to_string(), volume_down, ButtonVariant::Ghost),
                    SmallButton("field-inspector.review.volume-up".to_string(), "Volume +".to_string(), volume_up, ButtonVariant::Ghost),
                    ActionButton("field-inspector.review.submit".to_string(), if state.report_submitted { "Submitted" } else { "Submit report" }.to_string(), submit, ButtonVariant::Primary),
                ]),
                ActivityLog(),
            ],
        ))
}
