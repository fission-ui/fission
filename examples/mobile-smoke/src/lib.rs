use fission::core::Length;
use fission::prelude::*;

const FISSION_LOGO_PNG: &[u8] = include_bytes!("../../../docs/fission_logo.png");

#[cfg(target_os = "android")]
const ANDROID_TEST_CONTROL_PORT: u16 = 48761;

#[derive(Default, Debug, Clone, PartialEq)]
struct SmokeState {
    taps: u32,
}

impl GlobalState for SmokeState {}

#[fission_reducer(Increment)]
fn on_increment(state: &mut SmokeState) {
    state.taps += 1;
}

#[derive(Clone)]
struct MobileSmokeApp;

impl From<MobileSmokeApp> for Widget {
    fn from(_component: MobileSmokeApp) -> Self {
        let (ctx, view) = fission::build::current::<SmokeState>();
        let tokens = &view.env().theme.tokens;
        let increment = with_reducer!(ctx, Increment, on_increment);

        let content = Container::new(Column {
            gap: Some(16.0),
            children: vec![
                Text::new("Mobile smoke")
                    .size(tokens.typography.font_size_xl)
                    .color(tokens.colors.text_primary)
                    .into(),
                Text::new("Fission shell on mobile targets.")
                    .size(tokens.typography.body_large_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
                Text::new("Image probe")
                    .size(tokens.typography.font_size_base)
                    .color(tokens.colors.text_secondary)
                    .into(),
                Container::new(
                    Image::memory(FISSION_LOGO_PNG.to_vec())
                        .size(144.0, 144.0)
                        .fit(fission::core::op::ImageFit::Contain)
                        .semantic_label("Fission logo image probe"),
                )
                .width_length(Length::percent(100.0))
                .height_length(Length::points(176.0))
                .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
                .bg(tokens.colors.surface)
                .border(tokens.colors.primary, 1.0)
                .border_radius(tokens.radii.xl)
                .into(),
                Text::new(format!("Taps: {}", view.state().taps))
                    .size(tokens.typography.font_size_xl)
                    .color(tokens.colors.primary)
                    .into(),
                Button {
                    on_press: Some(increment),
                    child: Some(Text::new("Tap").into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(240.0),
            Length::percent(100.0),
            Length::points(420.0),
        ))
        .into();

        Container::new(Column {
            gap: Some(0.0),
            children: vec![
                content,
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        })
        .height_length(Length::vh(100.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
        .bg(tokens.colors.background)
        .into()
    }
}
#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<SmokeState, MobileSmokeApp> {
    let app = MobileApp::<SmokeState, _>::new(MobileSmokeApp).with_title("Fission Mobile Smoke");
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
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
