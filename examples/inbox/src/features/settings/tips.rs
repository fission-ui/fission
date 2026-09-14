use crate::model::settings::set_quick_tip_open;
use crate::model::{InboxState, SetQuickTipOpen};
use fission::core::reduce_with;
use fission::core::ui::widgets::GestureDetector;
use fission::core::ui::{Text, TextContent, Widget};
use fission::icons::material;
use fission::widgets::{Badge, FormControl, HStack, Icon, NumberInput, VStack};

/// The quick tip toggle and page size.
pub(super) struct TipsSettings;

impl From<TipsSettings> for Widget {
    fn from(_: TipsSettings) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;

        VStack {
            spacing: Some(tokens.spacing.m),
            children: vec![
                HStack {
                    spacing: Some(tokens.spacing.xs),
                    children: vec![
                        GestureDetector {
                            on_tap: Some(
                                ctx.bind(SetQuickTipOpen(true), reduce_with!(set_quick_tip_open)),
                            ),
                            child: HStack {
                                spacing: Some(tokens.spacing.xs),
                                children: vec![
                                    Icon::svg(material::action::info::regular())
                                        .size(tokens.typography.font_size_base)
                                        .into(),
                                    Text::new(TextContent::Key("settings.tips.show".into()))
                                        .size(tokens.typography.font_size_xs)
                                        .into(),
                                ],
                            }
                            .into(),
                            ..Default::default()
                        }
                        .into(),
                        Badge {
                            text: view.tr("settings.beta"),
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
                FormControl {
                    id: None,
                    label: Some(view.tr("settings.page_size.label")),
                    required: false,
                    error: None,
                    helper: Some(view.tr("settings.page_size.helper")),
                    child: NumberInput {
                        id: None,
                        label: None,
                        value: 50.0,
                        min: Some(10.0),
                        max: Some(100.0),
                        step: 10.0,
                        on_increment: None,
                        on_decrement: None,
                        on_input: None,
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
            ],
        }
        .into()
    }
}
