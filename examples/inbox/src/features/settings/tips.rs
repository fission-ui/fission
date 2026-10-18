use crate::model::settings::set_quick_tip_open;
use crate::model::{InboxState, SetQuickTipOpen};
use fission::prelude::*;

fission_component! {
/// The quick tip toggle and page size.
    pub(super) TipsSettings<InboxState> |_settings, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        VStack(
            spacing: tokens.spacing.m,
            children: [
                HStack(
                    spacing: tokens.spacing.xs,
                    children: [
                        GestureDetector(
                            on_tap: cx.bind(
                                SetQuickTipOpen(true),
                                reduce_with!(set_quick_tip_open),
                            ),
                            child: HStack(
                                spacing: tokens.spacing.xs,
                                children: [
                                    Icon::svg(material::action::info::regular())
                                        .size(tokens.typography.font_size_base),
                                    Text(TextContent::Key("settings.tips.show".into()))
                                        .size(tokens.typography.font_size_xs),
                                ],
                            ),
                        ),
                        Badge(text: cx.tr("settings.beta")),
                    ],
                ),
                FormControl(
                    label: Some(cx.tr("settings.page_size.label")),
                    helper: Some(cx.tr("settings.page_size.helper")),
                    child: NumberInput(
                        value: 50.0,
                        min: Some(10.0),
                        max: Some(100.0),
                        step: 10.0,
                    ),
                ),
            ],
        )
}
