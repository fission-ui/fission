use super::section::SectionHeading;
use crate::model::settings::{
    set_auto_advance_enabled, set_offline_enabled, set_smart_compose_enabled,
};
use crate::model::{InboxState, SetAutoAdvanceEnabled, SetOfflineEnabled, SetSmartComposeEnabled};
use fission::core::ui::widgets::Spacer;
use fission::core::ui::{Text, TextContent, Widget};
use fission::core::{reduce_with, ActionEnvelope};
use fission::icons::material;
use fission::widgets::{Card, HStack, Icon, Switch, VStack};

/// Experimental features the reader can switch on.
pub(super) struct LabsSettings;

impl From<LabsSettings> for Widget {
    fn from(_: LabsSettings) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;

        VStack {
            spacing: Some(tokens.spacing.m),
            children: vec![
                SectionHeading {
                    key: "settings.labs.title",
                }
                .into(),
                Card {
                    child: VStack {
                        spacing: Some(tokens.spacing.s),
                        children: vec![
                            LabToggle {
                                icon: Icon::svg(material::action::check_circle::regular()),
                                label_key: "settings.labs.smart_compose",
                                checked: state.smart_compose_enabled,
                                on_toggle: ctx.bind(
                                    SetSmartComposeEnabled(!state.smart_compose_enabled),
                                    reduce_with!(set_smart_compose_enabled),
                                ),
                            }
                            .into(),
                            LabToggle {
                                icon: Icon::svg(material::action::report_problem::regular()),
                                label_key: "settings.labs.offline",
                                checked: state.offline_enabled,
                                on_toggle: ctx.bind(
                                    SetOfflineEnabled(!state.offline_enabled),
                                    reduce_with!(set_offline_enabled),
                                ),
                            }
                            .into(),
                            LabToggle {
                                icon: Icon::svg(material::action::info::regular()),
                                label_key: "settings.labs.auto_advance",
                                checked: state.auto_advance_enabled,
                                on_toggle: ctx.bind(
                                    SetAutoAdvanceEnabled(!state.auto_advance_enabled),
                                    reduce_with!(set_auto_advance_enabled),
                                ),
                            }
                            .into(),
                        ],
                    }
                    .into(),
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into()
    }
}

/// One experimental feature with its switch.
struct LabToggle {
    icon: Icon,
    label_key: &'static str,
    checked: bool,
    on_toggle: ActionEnvelope,
}

impl From<LabToggle> for Widget {
    fn from(toggle: LabToggle) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;

        HStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                toggle.icon.size(tokens.typography.font_size_lg).into(),
                Text::new(TextContent::Key(toggle.label_key.into())).into(),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Switch {
                    checked: toggle.checked,
                    on_toggle: Some(toggle.on_toggle),
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into()
    }
}
