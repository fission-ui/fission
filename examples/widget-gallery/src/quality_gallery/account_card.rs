use fission::prelude::*;

use crate::GalleryState;

#[fission_component]
#[derive(Clone)]
pub(super) struct AccountDetailsCard {
    pub compact: bool,

    #[local_state(default = String::from("Avery Stone"))]
    display_name: String,

    #[local_state(default = String::from("avery@"))]
    recovery_email: String,
}

#[fission_reducer(UpdateQualityDisplayName)]
fn update_display_name(value: &mut String, ctx: &mut ReducerContext<String>) {
    let Some(change) = ctx.input.text_change() else {
        return;
    };
    *value = change.new_text.clone();
}

#[fission_reducer(UpdateQualityRecoveryEmail)]
fn update_recovery_email(value: &mut String, ctx: &mut ReducerContext<String>) {
    let Some(change) = ctx.input.text_change() else {
        return;
    };
    *value = change.new_text.clone();
}

#[fission_reducer(ToggleQualitySelect)]
fn toggle_quality_select(state: &mut GalleryState) {
    state.select_open = !state.select_open;
    state.menu_open = false;
    state.modal_open = false;
}

#[fission_reducer(SelectQualityRole)]
fn select_quality_role(state: &mut GalleryState, role: String) {
    state.select_value = Some(role);
    state.select_open = false;
}

#[fission_reducer(QualityProfileAction)]
fn quality_profile_action(_state: &mut GalleryState) {}

impl From<AccountDetailsCard> for Widget {
    fn from(card: AccountDetailsCard) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let selected_role = state
            .select_value
            .clone()
            .unwrap_or_else(|| "Editor".into());
        let display_name = card.display_name();
        let recovery_email = card.recovery_email();
        let update_display_name = ctx.bind_local(
            UpdateQualityDisplayName,
            display_name.clone(),
            reduce!(update_display_name),
        );
        let update_recovery_email = ctx.bind_local(
            UpdateQualityRecoveryEmail,
            recovery_email.clone(),
            reduce!(update_recovery_email),
        );
        let profile_action = with_reducer!(ctx, QualityProfileAction, quality_profile_action);

        CardLayout::new()
            .header(
                CardHeader::new(CardTitle::new("Account details"))
                    .description(CardDescription::new(
                        "Information shown to people in your workspace.",
                    ))
                    .action(
                        Button {
                            variant: ButtonVariant::TertiaryGray,
                            size: ComponentSize::Sm,
                            width: Some(28.0),
                            height: Some(28.0),
                            child: Some(
                                Icon::svg(material::navigation::more_horiz::regular())
                                    .size(16.0)
                                    .color(tokens.colors.text_primary)
                                    .into(),
                            ),
                            on_press: Some(profile_action.clone()),
                            ..Default::default()
                        }
                        .semantics_identifier("quality-gallery.profile.options"),
                    ),
            )
            .content(CardContent::new(Column {
                gap: Some(tokens.spacing.m + tokens.spacing.xs),
                children: widgets![
                    Row {
                        gap: Some(tokens.spacing.s + tokens.spacing.xs),
                        align_items: fission::op::AlignItems::Start,
                        children: widgets![
                            Container::new(FormControl {
                                id: Some(WidgetId::explicit(if card.compact {
                                    "quality-gallery.display-name.mobile"
                                } else {
                                    "quality-gallery.display-name.desktop"
                                })),
                                label: Some("Display name".into()),
                                child: TextInput {
                                    id: Some(WidgetId::explicit(if card.compact {
                                        "quality-gallery.display-name.input.mobile"
                                    } else {
                                        "quality-gallery.display-name.input.desktop"
                                    })),
                                    semantics_identifier: Some(
                                        "quality-gallery.input.display-name".into(),
                                    ),
                                    value: display_name.get(),
                                    on_input: Some(update_display_name),
                                    size: ComponentSize::Md,
                                    ..Default::default()
                                }
                                .into(),
                                error: None,
                                helper: (!card.compact).then(|| "Visible to your team.".into()),
                                required: false,
                            })
                            .width_length(Length::percent(50.0))
                            .flex_grow(1.0),
                            Container::new(FormControl {
                                id: Some(WidgetId::explicit(if card.compact {
                                    "quality-gallery.role.mobile"
                                } else {
                                    "quality-gallery.role.desktop"
                                })),
                                label: Some("Workspace role".into()),
                                child: SelectLayout::new(
                                    WidgetId::explicit(if card.compact {
                                        "quality-gallery.role.select.mobile"
                                    } else {
                                        "quality-gallery.role.select.desktop"
                                    }),
                                    SelectTrigger::new(selected_role.clone())
                                        .size(ComponentSize::Md)
                                        .semantics_identifier("quality-gallery.select.trigger",),
                                    SelectContent::new(vec![SelectGroup::new(vec![
                                        SelectOption::option("Viewer", selected_role == "Viewer",)
                                            .on_select(with_reducer!(
                                                ctx,
                                                SelectQualityRole("Viewer".into()),
                                                select_quality_role
                                            ))
                                            .semantics_identifier("quality-gallery.role.viewer",)
                                            .into(),
                                        SelectOption::option("Editor", selected_role == "Editor",)
                                            .on_select(with_reducer!(
                                                ctx,
                                                SelectQualityRole("Editor".into()),
                                                select_quality_role
                                            ))
                                            .semantics_identifier("quality-gallery.role.editor",)
                                            .into(),
                                        SelectSeparator::new().into(),
                                        SelectOption::option("Owner", selected_role == "Owner",)
                                            .on_select(with_reducer!(
                                                ctx,
                                                SelectQualityRole("Owner".into()),
                                                select_quality_role
                                            ))
                                            .semantics_identifier("quality-gallery.role.owner",)
                                            .into(),
                                    ])
                                    .label("Access level")
                                    .into(),]),
                                )
                                .open(state.select_open)
                                .on_toggle(with_reducer!(
                                    ctx,
                                    ToggleQualitySelect,
                                    toggle_quality_select
                                ))
                                .into(),
                                error: None,
                                helper: None,
                                required: false,
                            })
                            .width_length(Length::percent(50.0))
                            .flex_grow(1.0),
                        ],
                        ..Default::default()
                    },
                    FormControl {
                        id: Some(WidgetId::explicit(if card.compact {
                            "quality-gallery.recovery-email.mobile"
                        } else {
                            "quality-gallery.recovery-email.desktop"
                        })),
                        label: Some("Recovery email".into()),
                        child: TextInput {
                            id: Some(WidgetId::explicit(if card.compact {
                                "quality-gallery.recovery-email.input.mobile"
                            } else {
                                "quality-gallery.recovery-email.input.desktop"
                            })),
                            semantics_identifier: Some(
                                "quality-gallery.input.recovery-email".into(),
                            ),
                            value: recovery_email.get(),
                            on_input: Some(update_recovery_email),
                            size: ComponentSize::Md,
                            ..Default::default()
                        }
                        .into(),
                        error: Some("Enter a complete email address.".into()),
                        helper: None,
                        required: false,
                    },
                    AlertLayout::new(
                        AlertKind::Info,
                        AlertContent::new(widgets![
                            AlertTitle::new("Your account is protected"),
                            AlertDescription::new(
                                "Two-step verification is active on this account.",
                            ),
                        ]),
                    )
                    .leading(
                        Icon::svg(material::action::verified_user::regular())
                            .size(16.0)
                            .color(tokens.colors.text_primary),
                    ),
                ],
                ..Default::default()
            }))
            .footer(CardFooter::new(widgets![
                Button {
                    variant: ButtonVariant::TertiaryGray,
                    size: ComponentSize::Md,
                    content: Some(ButtonContent::new("Cancel")),
                    on_press: Some(profile_action.clone()),
                    ..Default::default()
                }
                .semantics_identifier("quality-gallery.profile.cancel"),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Primary,
                    size: ComponentSize::Md,
                    content: Some(
                        ButtonContent::new("Save changes")
                            .leading_icon(Icon::svg(material::content::save::regular(),)),
                    ),
                    on_press: Some(profile_action),
                    ..Default::default()
                }
                .semantics_identifier("quality-gallery.profile.save"),
            ]))
            .pattern(CardPattern::Plain)
            .into()
    }
}
