use fission::prelude::*;

mod document_probe;
use document_probe::DocumentProbe;

const FISSION_LOGO_PNG: &[u8] = include_bytes!("../../../docs/fission_logo.png");
const CONTENT_MIN_WIDTH: f32 = 240.0;
const CONTENT_MAX_WIDTH: f32 = 420.0;
const IMAGE_SIZE: f32 = 144.0;
const IMAGE_PROBE_HEIGHT: f32 = 176.0;

#[cfg(target_os = "android")]
const ANDROID_TEST_CONTROL_PORT: u16 = 48761;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct SmokeState {
    taps: u32,
    documents: document_probe::DocumentState,
}

impl GlobalState for SmokeState {}

fission_component! {
    pub MobileSmokeApp<SmokeState>
    view(_app, state, cx) {
        let tokens = &cx.view.env().theme.tokens;
        let logo = Image::memory(FISSION_LOGO_PNG.to_vec())
            .size(IMAGE_SIZE, IMAGE_SIZE)
            .fit(fission::core::op::ImageFit::Contain)
            .semantic_label(cx.view.env().i18n.get(&cx.view.env().locale, "smoke.logo").unwrap_or("Fission"));
    } =>
    SafeArea(child: Scroll(child: Container(
        Column(
            gap: tokens.spacing.l,
            children: [
                Container(
                    Column(
                        gap: tokens.spacing.m,
                        children: [
                            Text(TextContent::Key("smoke.title".into()))
                                .size(tokens.typography.font_size_xl)
                                .color(tokens.colors.text_primary),
                            Text(TextContent::Key("smoke.description".into()))
                                .size(tokens.typography.body_large_size)
                                .color(tokens.colors.text_secondary),
                            Text(TextContent::Key("smoke.image".into()))
                                .size(tokens.typography.font_size_base)
                                .color(tokens.colors.text_secondary),
                            Container(logo)
                                .width_length(Length::percent(100.0))
                                .height_length(Length::points(IMAGE_PROBE_HEIGHT))
                                .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
                                .bg(tokens.colors.surface)
                                .border(tokens.colors.primary, 1.0)
                                .border_radius(tokens.radii.xl),
                            Row(gap: tokens.spacing.s, children: [
                                Text(TextContent::Key("smoke.taps".into())),
                                Text(state.taps.to_string())
                                    .size(tokens.typography.font_size_xl)
                                    .color(tokens.colors.primary),
                            ]),
                            Button(
                                child: Text(TextContent::Key("smoke.tap".into())),
                                key: "increment",
                                on_press: reduce { state.taps += 1; },
                            )
                            .semantics_identifier("mobile-smoke.increment"),
                        ],
                    ),
                )
                .width_length(Length::clamp(
                    Length::points(CONTENT_MIN_WIDTH),
                    Length::percent(100.0),
                    Length::points(CONTENT_MAX_WIDTH),
                )),
                DocumentProbe(),
            ],
        ),
    )
    .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
    .bg(tokens.colors.background)))
}
#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<SmokeState, MobileSmokeApp> {
    let app = MobileApp::<SmokeState, _>::new(MobileSmokeApp)
        .with_title("Fission Mobile Smoke")
        .with_env(document_probe::create_env())
        .with_async(|asyncs| {
            asyncs.register_job(document_probe::READ_DOCUMENT, document_probe::read_document);
        });
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    DesktopApp::<SmokeState, _>::new(MobileSmokeApp)
        .with_env(document_probe::create_env())
        .with_async(|asyncs| {
            asyncs.register_job(document_probe::READ_DOCUMENT, document_probe::read_document);
        })
        .run()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn run_mobile() -> anyhow::Result<()> {
    mobile_app().run()
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app_handle: AndroidApp) {
    let _ = mobile_app().run_with_android_app(app_handle);
}
