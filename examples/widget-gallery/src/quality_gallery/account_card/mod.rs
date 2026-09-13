//! The account details card: two fields side by side, a recovery email with a
//! validation error, and a protected-account notice.

mod display_name_field;
mod profile_footer;
mod recovery_email_field;
mod role_field;

use crate::GalleryState;
use display_name_field::DisplayNameField;
use fission::prelude::*;
use profile_footer::ProfileFooter;
use recovery_email_field::RecoveryEmailField;
use role_field::RoleField;

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
    if let Some(change) = ctx.input.text_change() {
        *value = change.new_text.clone();
    }
}

#[fission_reducer(UpdateQualityRecoveryEmail)]
fn update_recovery_email(value: &mut String, ctx: &mut ReducerContext<String>) {
    if let Some(change) = ctx.input.text_change() {
        *value = change.new_text.clone();
    }
}

/// The card's buttons are a visual fixture: they need an action to render
/// enabled, and pressing them changes nothing.
#[fission_reducer(QualityProfileAction)]
fn quality_profile_action(_state: &mut GalleryState) {}

/// Pairs a stable widget id with the layout it belongs to, so the mobile and
/// desktop copies of a field never share an identity.
fn layout_id(base: &str, compact: bool) -> WidgetId {
    let layout = if compact { "mobile" } else { "desktop" };
    WidgetId::explicit(&format!("{base}.{layout}"))
}

impl From<AccountDetailsCard> for Widget {
    fn from(card: AccountDetailsCard) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let display_name = card.display_name();
        let recovery_email = card.recovery_email();
        let profile_action = with_reducer!(ctx, QualityProfileAction, quality_profile_action);

        CardLayout::new()
            .header(
                CardHeader::new(CardTitle::new("Account details"))
                    .description(CardDescription::new(
                        "Information shown to people in your workspace.",
                    ))
                    .action(ProfileOptionsButton {
                        action: profile_action.clone(),
                    }),
            )
            .content(CardContent::new(Column {
                gap: Some(tokens.spacing.m + tokens.spacing.xs),
                children: widgets![
                    Row {
                        gap: Some(tokens.spacing.s + tokens.spacing.xs),
                        align_items: fission::op::AlignItems::Start,
                        children: widgets![
                            DisplayNameField {
                                compact: card.compact,
                                value: display_name.get(),
                                on_input: ctx.bind_local(
                                    UpdateQualityDisplayName,
                                    display_name.clone(),
                                    reduce!(update_display_name),
                                ),
                            },
                            RoleField {
                                compact: card.compact,
                            },
                        ],
                        ..Default::default()
                    },
                    RecoveryEmailField {
                        compact: card.compact,
                        value: recovery_email.get(),
                        on_input: ctx.bind_local(
                            UpdateQualityRecoveryEmail,
                            recovery_email.clone(),
                            reduce!(update_recovery_email),
                        ),
                    },
                    ProtectedAccountNotice,
                ],
                ..Default::default()
            }))
            .footer(
                ProfileFooter {
                    action: profile_action,
                }
                .into(),
            )
            .pattern(CardPattern::Plain)
            .into()
    }
}

struct ProfileOptionsButton {
    action: ActionEnvelope,
}

impl From<ProfileOptionsButton> for Widget {
    fn from(button: ProfileOptionsButton) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
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
            on_press: Some(button.action),
            ..Default::default()
        }
        .semantics_identifier("quality-gallery.profile.options")
        .into()
    }
}

struct ProtectedAccountNotice;

impl From<ProtectedAccountNotice> for Widget {
    fn from(_notice: ProtectedAccountNotice) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        AlertLayout::new(
            AlertKind::Info,
            AlertContent::new(widgets![
                AlertTitle::new("Your account is protected"),
                AlertDescription::new("Two-step verification is active on this account."),
            ]),
        )
        .leading(
            Icon::svg(material::action::verified_user::regular())
                .size(16.0)
                .color(tokens.colors.text_primary),
        )
        .into()
    }
}
