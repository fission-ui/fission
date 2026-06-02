use fission::prelude::*;

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
        let increment = with_reducer!(ctx, Increment, on_increment);
        let viewport = view.viewport_size();
        let content_width = (viewport.width - 48.0).clamp(240.0, 420.0);
        let background = Color {
            r: 20,
            g: 23,
            b: 31,
            a: 255,
        };
        let body = Color {
            r: 184,
            g: 194,
            b: 209,
            a: 255,
        };
        let accent = Color {
            r: 145,
            g: 224,
            b: 196,
            a: 255,
        };

        let content = Container::new(Column {
            gap: Some(16.0),
            children: vec![
                Text::new("Mobile smoke")
                    .size(24.0)
                    .color(Color::WHITE)
                    .max_width(content_width)
                    .into(),
                Text::new("Fission shell on mobile targets.")
                    .size(16.0)
                    .color(body)
                    .max_width(content_width)
                    .into(),
                Text::new(format!("Taps: {}", view.state().taps))
                    .size(22.0)
                    .color(accent)
                    .into(),
                Button {
                    width: Some(content_width),
                    on_press: Some(increment),
                    child: Some(
                        Text::new("Tap")
                            .width((content_width - 96.0).max(120.0))
                            .into(),
                    ),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        })
        .width(content_width)
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
        .height(viewport.height.max(1.0))
        .padding_all(24.0)
        .bg(background)
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
