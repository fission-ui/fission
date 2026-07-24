use crate::components::protected_notes::ProtectedNotes;
use crate::components::section_header::SectionHeader;
use crate::components::ui::{ActionButton, Metric, PanelCard, ResponsiveGrid};
use crate::model::{
    on_authenticate_passkey, on_register_passkey, on_secure_unlock, AuthenticatePasskey,
    FieldInspectorState, RegisterPasskey, SecureUnlock,
};
use fission::prelude::*;

pub struct SecurityPanel;

impl From<SecurityPanel> for Widget {
    fn from(_: SecurityPanel) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let unlock = with_reducer!(ctx, SecureUnlock, on_secure_unlock);
        let register = with_reducer!(ctx, RegisterPasskey, on_register_passkey);
        let authenticate = with_reducer!(ctx, AuthenticatePasskey, on_authenticate_passkey);
        let spacing = &view.env().theme.tokens.spacing;

        PanelCard::new(Column {
            gap: Some(spacing.m),
            children: widgets![
                SectionHeader {
                    title: "Unlock protected site data",
                    body: "Biometrics verify the local user. Passkeys produce credential data that a backend would verify before granting account access.",
                },
                Row {
                    gap: Some(spacing.s),
                    wrap: ir_op::FlexWrap::Wrap,
                    children: widgets![
                        ActionButton::new(
                            "field-inspector.security.biometric",
                            "Biometric unlock",
                            unlock,
                            ButtonVariant::Primary,
                        ),
                        ActionButton::new(
                            "field-inspector.security.passkey-register",
                            "Register passkey",
                            register,
                            ButtonVariant::SecondaryColor,
                        ),
                        ActionButton::new(
                            "field-inspector.security.passkey-authenticate",
                            "Authenticate passkey",
                            authenticate,
                            ButtonVariant::SecondaryGray,
                        ),
                    ],
                    ..Default::default()
                },
                ResponsiveGrid::new(widgets![
                    Metric::new(
                        "Protected notes",
                        if view.state().sensitive_unlocked {
                            "Unlocked"
                        } else {
                            "Locked"
                        },
                    ),
                    Metric::new(
                        "Account proof",
                        if view.state().passkey_verified {
                            "Passkey verified"
                        } else {
                            "Pending"
                        },
                    ),
                ]),
                ProtectedNotes,
            ],
            ..Default::default()
        })
        .into()
    }
}
