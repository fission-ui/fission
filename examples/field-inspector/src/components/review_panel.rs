use crate::components::activity_log::ActivityLog;
use crate::components::section_header::SectionHeader;
use crate::components::ui::{ActionButton, BodyText, PanelCard, SmallButton, SoftPanel};
use crate::model::{
    on_adjust_alert_volume, on_copy_report_summary, on_deep_link_received, on_schedule_reminder,
    on_submit_report, AdjustAlertVolume, CopyReportSummary, FieldInspectorState, ScheduleReminder,
    SubmitReport,
};
use fission::prelude::*;

pub struct ReviewPanel;

impl From<ReviewPanel> for Widget {
    fn from(_: ReviewPanel) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let copy = with_reducer!(ctx, CopyReportSummary, on_copy_report_summary);
        let reminder = with_reducer!(ctx, ScheduleReminder, on_schedule_reminder);
        let volume_down = with_reducer!(
            ctx,
            AdjustAlertVolume(VolumeAdjustDirection::Down),
            on_adjust_alert_volume
        );
        let volume_up = with_reducer!(
            ctx,
            AdjustAlertVolume(VolumeAdjustDirection::Up),
            on_adjust_alert_volume
        );
        let submit = with_reducer!(ctx, SubmitReport, on_submit_report);
        let deep_link = ctx.bind(
            DeepLinkReceived {
                link: DeepLink::new(format!(
                    "field-inspector://work-orders/{}",
                    view.state().selected_order().id
                ))
                .source(DeepLinkSource::CustomScheme),
            },
            reduce_with!(on_deep_link_received),
        );
        let spacing = &view.env().theme.tokens.spacing;

        PanelCard::new(Column {
            gap: Some(spacing.m),
            children: widgets![
                SectionHeader {
                    title: "Review and submit",
                    body: "The report gathers host-provided context into a plain summary that can be copied, linked from notifications, or submitted.",
                },
                SoftPanel::new(BodyText::new(view.state().report_summary())),
                Row {
                    gap: Some(spacing.s),
                    wrap: ir_op::FlexWrap::Wrap,
                    children: widgets![
                        ActionButton::new(
                            "field-inspector.review.copy",
                            "Copy summary",
                            copy,
                            ButtonVariant::SecondaryGray,
                        ),
                        ActionButton::new(
                            "field-inspector.review.reminder",
                            "Schedule reminder",
                            reminder,
                            ButtonVariant::SecondaryColor,
                        ),
                        ActionButton::new(
                            "field-inspector.review.deep-link",
                            "Open deep link",
                            deep_link,
                            ButtonVariant::Ghost,
                        ),
                        SmallButton::new(
                            "field-inspector.review.volume-down",
                            "Volume -",
                            volume_down,
                            ButtonVariant::Ghost,
                        ),
                        SmallButton::new(
                            "field-inspector.review.volume-up",
                            "Volume +",
                            volume_up,
                            ButtonVariant::Ghost,
                        ),
                        ActionButton::new(
                            "field-inspector.review.submit",
                            if view.state().report_submitted {
                                "Submitted"
                            } else {
                                "Submit report"
                            },
                            submit,
                            ButtonVariant::Primary,
                        ),
                    ],
                    ..Default::default()
                },
                ActivityLog,
            ],
            ..Default::default()
        })
        .into()
    }
}
