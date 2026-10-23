use crate::components::protected_notes::ProtectedNotes;
use crate::components::section_header::SectionHeader;
use crate::components::ui::{
    ActionButton, Metric, PanelCard, ResponsiveGrid, DEFAULT_ITEM_MIN_WIDTH,
};
use crate::model::{
    on_authenticate_passkey, on_register_passkey, on_secure_unlock, AuthenticatePasskey,
    FieldInspectorState, RegisterPasskey, SecureUnlock,
};
use fission::prelude::*;
fission_component! {
    pub SecurityPanel<FieldInspectorState> {}
    |_panel, state, cx| {
        let unlock = with_reducer!(cx.build, SecureUnlock, on_secure_unlock);
        let register = with_reducer!(cx.build, RegisterPasskey, on_register_passkey);
        let authenticate = with_reducer!(cx.build, AuthenticatePasskey, on_authenticate_passkey);
        let spacing = &cx.view.env().theme.tokens.spacing;
    } =>
        PanelCard(Column(
            gap: spacing.m,
            children: [
                SectionHeader("Unlock protected site data", "Biometrics verify the local user. Passkeys produce credential data that a backend would verify before granting account access."),
                Row(gap: spacing.s, wrap: ir_op::FlexWrap::Wrap, children: [
                    ActionButton("field-inspector.security.biometric".to_string(), "Biometric unlock".to_string(), unlock, ButtonVariant::Primary),
                    ActionButton("field-inspector.security.passkey-register".to_string(), "Register passkey".to_string(), register, ButtonVariant::SecondaryColor),
                    ActionButton("field-inspector.security.passkey-authenticate".to_string(), "Authenticate passkey".to_string(), authenticate, ButtonVariant::SecondaryGray),
                ]),
                ResponsiveGrid(vec![
                    Metric::new("Protected notes".to_string(), if state.sensitive_unlocked { "Unlocked" } else { "Locked" }.to_string()).into(),
                    Metric::new("Account proof".to_string(), if state.passkey_verified { "Passkey verified" } else { "Pending" }.to_string()).into(),
                ], DEFAULT_ITEM_MIN_WIDTH),
                ProtectedNotes(),
            ],
        ))
}
