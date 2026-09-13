//! Reducers for the secure unlock and passkey flows.

use super::*;

#[fission_reducer(SecureUnlock)]
pub fn on_secure_unlock(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Security;
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .biometrics()
        .authenticate(BiometricAuthenticateRequest {
            reason: format!("Unlock protected notes for {}", state.selected_order().site),
            title: Some("Unlock site data".into()),
            subtitle: Some(state.selected_order().asset.id.into()),
            fallback_title: Some("Use device credential".into()),
            cancel_title: Some("Cancel".into()),
            allow_device_credential: true,
            required_strength: BiometricStrength::Any,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(RegisterPasskey)]
pub fn on_register_passkey(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Security;
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .passkeys()
        .register(PasskeyRegistrationRequest {
            relying_party: PasskeyRelyingParty::new(PASSKEY_RELYING_PARTY_ID, "Field Inspector"),
            user: PasskeyUser::new(
                vec![7, 42],
                "technician@example.com",
                state.selected_order().assigned_to,
            ),
            challenge: b"demo-registration-challenge".to_vec(),
            pub_key_algorithms: vec![PasskeyAlgorithm::ES256, PasskeyAlgorithm::RS256],
            timeout_ms: Some(60_000),
            attestation: PasskeyAttestationConveyance::None,
            authenticator_selection: Some(PasskeyAuthenticatorSelection {
                attachment: Some(PasskeyAuthenticatorAttachment::Platform),
                resident_key: PasskeyResidentKeyRequirement::Required,
                user_verification: PasskeyUserVerification::Preferred,
            }),
            exclude_credentials: Vec::new(),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(AuthenticatePasskey)]
pub fn on_authenticate_passkey(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Security;
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .passkeys()
        .authenticate(PasskeyAuthenticationRequest {
            relying_party_id: PASSKEY_RELYING_PARTY_ID.into(),
            challenge: b"demo-authentication-challenge".to_vec(),
            allow_credentials: state.registered_passkey.iter().cloned().collect(),
            user_verification: PasskeyUserVerification::Preferred,
            mediation: PasskeyMediation::Required,
            timeout_ms: Some(60_000),
        })
        .on_ok(ok)
        .on_err(err);
}
