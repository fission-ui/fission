use fission_core::op::Color;
use fission_core::ui::*;
use fission_core::*;
#[cfg(target_os = "android")]
use fission_shell_mobile::AndroidApp;
use fission_shell_mobile::MobileApp;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SmokeState {
    taps: u32,
}

impl AppState for SmokeState {}

#[derive(fission_macros::Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Increment;

fn on_increment(state: &mut SmokeState, _action: Increment, _ctx: &mut ReducerContext<SmokeState>) {
    state.taps += 1;
}

struct MobileSmokeApp;

impl Widget<SmokeState> for MobileSmokeApp {
    fn build(&self, ctx: &mut BuildCtx<SmokeState>, view: &View<SmokeState>) -> Node {
        let increment = ctx.bind(Increment, on_increment as Handler<SmokeState, Increment>);
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

        Container::new(
            Column {
                gap: Some(16.0),
                children: vec![
                    Text::new("Fission mobile smoke")
                        .size(28.0)
                        .color(Color::WHITE)
                        .into_node(),
                    Text::new(
                        "This exercises the shared winit + Vello shell path for mobile targets.",
                    )
                    .size(16.0)
                    .color(body)
                    .into_node(),
                    Text::new(format!("Taps: {}", view.state.taps))
                        .size(22.0)
                        .color(accent)
                        .into_node(),
                    Button {
                        on_press: Some(increment),
                        child: Some(Box::new(Text::new("Increment").into_node())),
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(24.0)
        .bg(background)
        .into_node()
    }
}

fn app() -> MobileApp<SmokeState, MobileSmokeApp> {
    MobileApp::new(MobileSmokeApp).with_title("Fission Mobile Smoke")
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app_handle: AndroidApp) {
    let _ = app().run_with_android_app(app_handle);
}

#[cfg(target_os = "android")]
fn main() {}

#[cfg(not(target_os = "android"))]
fn main() -> anyhow::Result<()> {
    app().run()
}
