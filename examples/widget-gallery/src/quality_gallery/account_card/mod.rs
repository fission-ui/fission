//! The account details card: two fields side by side, a recovery email with a
//! validation error, and a protected-account notice.

mod display_name_field;
mod profile_footer;
mod recovery_email_field;
mod role_field;

use crate::GalleryState;
use fission::prelude::*;
use profile_footer::ProfileFooter;
use role_field::RoleField;

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

fission_component! {
    #[derive(Clone)] pub(super) AccountDetailsCard<GalleryState> {
        compact: bool,
    }
    |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let profile_action = with_reducer!(cx.build, QualityProfileAction, quality_profile_action);
    } =>
        CardLayout::new()
            .header(
                CardHeader::new(CardTitle::new("Account details"))
                    .description(CardDescription::new(
                        "Information shown to people in your workspace.",
                    ))
                    .action(ProfileOptionsButton::new(profile_action.clone())),
            )
            .content(CardContent::new(AccountFields::new(card.compact)))
            .footer(CardFooter::new(widgets![ProfileFooter::new(profile_action)]))
            .pattern(CardPattern::Plain)
}

fission_component! {
    AccountFields<GalleryState> {
        compact: bool,
        #[local_state(default = String::from("Avery Stone"))]
        display_name: String,
        #[local_state(default = String::from("avery@"))]
        recovery_email: String,
    }
    |fields, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Column(
            gap: tokens.spacing.m + tokens.spacing.xs,
            children: [
                Row(
                    gap: tokens.spacing.s + tokens.spacing.xs,
                    align_items: fission::op::AlignItems::Start,
                    children: [
                        Container(FormControl(
                            id: layout_id("quality-gallery.display-name", fields.compact),
                            label: Some(String::from("Display name")),
                            child: TextInput(
                                id: layout_id("quality-gallery.display-name.input", fields.compact),
                                semantics_identifier: Some(String::from("quality-gallery.input.display-name")),
                                value: fields.display_name.clone(),
                                bind(on_input): to(fields.display_name),
                                size: ComponentSize::Md,
                            ),
                            helper: if fields.compact { None } else { Some(String::from("Visible to your team.")) },
                        ))
                        .width_length(Length::percent(50.0))
                        .flex_grow(1.0),
                        RoleField(fields.compact),
                    ],
                ),
                FormControl(
                    id: layout_id("quality-gallery.recovery-email", fields.compact),
                    label: Some(String::from("Recovery email")),
                    child: TextInput(
                        id: layout_id("quality-gallery.recovery-email.input", fields.compact),
                        semantics_identifier: Some(String::from("quality-gallery.input.recovery-email")),
                        value: fields.recovery_email.clone(),
                        bind(on_input): to(fields.recovery_email),
                        size: ComponentSize::Md,
                    ),
                    error: Some(String::from("Enter a complete email address.")),
                ),
                ProtectedAccountNotice(),
            ],
        )
}

fission_component! {
    ProfileOptionsButton<GalleryState> { action: ActionEnvelope }
    |button, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Button(
            variant: ButtonVariant::TertiaryGray,
            size: ComponentSize::Sm,
            width: 28.0_f32,
            height: 28.0_f32,
            child: Some(
                Icon::svg(material::navigation::more_horiz::regular())
                    .size(16.0)
                    .color(tokens.colors.text_primary)
                    .into()
            ),
            on_press: button.action.clone(),
        )
        .semantics_identifier("quality-gallery.profile.options")
}

fission_component! {
    ProtectedAccountNotice<GalleryState> {}
    |_notice, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
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
}
