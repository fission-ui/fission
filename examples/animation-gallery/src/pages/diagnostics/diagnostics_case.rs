use crate::widgets::common::GalleryCase;

#[derive(Clone, Copy)]
pub(super) struct DiagnosticsCase {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub primary_title: &'static str,
    pub primary_source: &'static str,
    pub secondary_title: &'static str,
    pub secondary_source: &'static str,
    pub diagnostic: &'static str,
}

pub(super) fn diagnostics_case(path: &str) -> DiagnosticsCase {
    match path {
        "/diagnostics/expressions" => DiagnosticsCase {
            title: "Lowered MotionExpr",
            subtitle: "Inspect the expression graph that ergonomic widget motion lowers into.",
            primary_title: "MotionExpr graph",
            primary_source: EXPRESSIONS_SOURCE,
            secondary_title: "Current value sampling",
            secondary_source: TIMELINE_SOURCE,
            diagnostic: "Expression diagnostics prove widget presets lower to deterministic native motion data.",
        },
        "/diagnostics/timeline" => DiagnosticsCase {
            title: "Timeline Values",
            subtitle: "Scrub frame time and inspect current resolved values for each track.",
            primary_title: "Timeline samples",
            primary_source: TIMELINE_SOURCE,
            secondary_title: "Rest criteria",
            secondary_source: REST_SOURCE,
            diagnostic: "Timeline diagnostics make motion review mechanical instead of eyeballed.",
        },
        "/diagnostics/tests" => DiagnosticsCase {
            title: "Test Harness Examples",
            subtitle: "Use Fission LiveTests to drive real apps, capture screenshots, and assert behavior.",
            primary_title: "LiveTest pattern",
            primary_source: TEST_HARNESS_SOURCE,
            secondary_title: "Deterministic motion assertion",
            secondary_source: crate::widgets::common::TEST_SOURCE,
            diagnostic: "Tests should interact with the real widget and then assert visible text, screenshots, or motion values.",
        },
        _ => DiagnosticsCase {
            title: "Lowered MotionDeclaration",
            subtitle: "Inspect the native declaration emitted by a widget-owned motion enum.",
            primary_title: "Lowered MotionDeclaration",
            primary_source: DECLARATION_SOURCE,
            secondary_title: "Deterministic test",
            secondary_source: crate::widgets::common::TEST_SOURCE,
            diagnostic: "Motion declarations are deterministic for a fixed policy, route, and frame time.",
        },
    }
}

pub(super) fn gallery_case(diagnostics: DiagnosticsCase) -> GalleryCase {
    GalleryCase {
        title: diagnostics.title,
        description: diagnostics.subtitle,
        motions: crate::widgets::common::MODAL_MOTIONS,
        slots: &["backdrop", "surface"],
        tracks: &[
            "surface.translate_y",
            "surface.opacity",
            "surface.scale",
            "backdrop.opacity",
        ],
        exprs: &[
            "MotionExpr::Px",
            "MotionExpr::Scalar",
            "MotionTransition::Spring",
        ],
        ergonomic_source: "ModalMotion::FromTop + ModalMotion::Fade + ModalMotion::Scale",
        native_source: crate::widgets::common::GENERIC_NATIVE_SOURCE,
        declaration_source: diagnostics.primary_source,
        test_source: diagnostics.secondary_source,
        diagnostic: diagnostics.diagnostic,
    }
}

const DECLARATION_SOURCE: &str = r#"MotionDeclaration {
    id: WidgetId::derived(gallery_modal.as_u128(), &[SLOT_SURFACE]),
    kind: MotionDeclarationKind::Presence {
        visible: true,
        enter: vec![
            MotionTrack::composite(TranslateY, Explicit(px(-24.0)), px(0.0)),
            MotionTrack::composite(Opacity, Explicit(scalar(0.0)), scalar(1.0)),
            MotionTrack::composite(Scale, Explicit(scalar(0.96)), scalar(1.0)),
        ],
        exit: reverse_tracks_for_exit(&enter),
        keep_rendered: false,
        inert_while_exiting: true,
    },
}"#;

const EXPRESSIONS_SOURCE: &str = r#"MotionExpr::If {
    predicate: MotionPredicate::PresenceEntering(gallery_modal.surface),
    then_expr: Box::new(MotionExpr::Px(-24.0)),
    else_expr: Box::new(MotionExpr::Px(0.0)),
}

MotionExpr::Scalar(0.96) -> MotionExpr::Scalar(1.0)"#;

const TIMELINE_SOURCE: &str = r#"0ms:   translate_y = -24px, opacity = 0.00, scale = 0.96
150ms: translate_y = -10px, opacity = 0.67, scale = 0.98
300ms: translate_y =   0px, opacity = 1.00, scale = 1.00"#;

const REST_SOURCE: &str = r#"Motion is resting when every active track is within rest_delta
and no spawned ripple/presence exit remains active.

The test harness should use pump_until_rest() before final assertions."#;

const TEST_HARNESS_SOURCE: &str = r#"let client = LiveTestClient::connect(port);
client.wait_for_ready(15_000)?;
client.tap_text("Modal")?;
client.tap_text("Play")?;
client.assert_text_visible("Archive thread")?;
client.screenshot("modal-open.png")?;
client.tap_text("Confirm")?;"#;
