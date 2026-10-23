use fission::prelude::*;

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
}

impl GlobalState for SmokeState {}

fission_component! {
    pub MobileSmokeApp<SmokeState>
    |_app, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let logo = Image::memory(FISSION_LOGO_PNG.to_vec())
            .size(IMAGE_SIZE, IMAGE_SIZE)
            .fit(fission::core::op::ImageFit::Contain)
            .semantic_label("Fission logo image probe");
    } =>
    Container(
        Column(
            gap: tokens.spacing.none,
            children: [
                Container(
                    Column(
                        gap: tokens.spacing.m,
                        children: [
                            Text("Mobile smoke")
                                .size(tokens.typography.font_size_xl)
                                .color(tokens.colors.text_primary),
                            Text("Fission shell on mobile targets.")
                                .size(tokens.typography.body_large_size)
                                .color(tokens.colors.text_secondary),
                            Text("Image probe")
                                .size(tokens.typography.font_size_base)
                                .color(tokens.colors.text_secondary),
                            Container(logo)
                                .width_length(Length::percent(100.0))
                                .height_length(Length::points(IMAGE_PROBE_HEIGHT))
                                .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
                                .bg(tokens.colors.surface)
                                .border(tokens.colors.primary, 1.0)
                                .border_radius(tokens.radii.xl),
                            Text(format!("Taps: {}", state.taps))
                                .size(tokens.typography.font_size_xl)
                                .color(tokens.colors.primary),
                            Button(
                                child: Text("Tap"),
                                key: "increment",
                                on_press: |_app, state, _cx| state.taps += 1,
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
                Spacer(flex_grow: 1.0_f32),
            ],
        ),
    )
    .height_length(Length::vh(100.0))
    .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
    .bg(tokens.colors.background)
}
#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<SmokeState, MobileSmokeApp> {
    let app = MobileApp::<SmokeState, _>::new(MobileSmokeApp).with_title("Fission Mobile Smoke");
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    DesktopApp::<SmokeState, _>::new(MobileSmokeApp).run()
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
