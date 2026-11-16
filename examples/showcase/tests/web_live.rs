use fission_test_driver::{BrowserTestOptions, LiveTestClient, SelectorQuery};

const READY_TIMEOUT_MS: u64 = 30_000;
const ACTION_TIMEOUT_MS: u64 = 10_000;

fn browser() -> LiveTestClient {
    let url = std::env::var("FISSION_SHOWCASE_WEB_URL")
        .expect("set FISSION_SHOWCASE_WEB_URL to the running showcase app");
    let mut options = BrowserTestOptions::new(url).fission_canvas();
    options.viewport_width = 1440;
    options.viewport_height = 960;
    LiveTestClient::launch_browser(options).expect("launch showcase browser")
}

fn open_example(client: &LiveTestClient, slug: &str) {
    open_example_at(client, slug, 0.5);
}

fn open_example_at(client: &LiveTestClient, slug: &str, relative_x: f32) {
    client
        .touch_selector_at(
            SelectorQuery::semantic_identifier(format!("showcase.example.{slug}")),
            relative_x,
            0.5,
        )
        .unwrap_or_else(|error| panic!("open {slug} with real browser touch: {error}"));
    client.pump().expect("pump selected example");
    client
        .wait_for_visible(
            SelectorQuery::semantic_identifier(format!("showcase.source.{slug}")),
            ACTION_TIMEOUT_MS,
        )
        .unwrap_or_else(|error| panic!("{slug} did not become the active preview: {error}"));
}

fn install_pointer_trace(client: &LiveTestClient) {
    client
        .browser_evaluate_json(
            r#"(() => {
                const canvas = document.querySelector('canvas');
                if (!canvas) return false;
                globalThis.__FISSION_SHOWCASE_POINTER_TRACE = [];
                for (const type of ['pointerdown', 'pointerup', 'pointercancel', 'lostpointercapture']) {
                    canvas.addEventListener(type, event => {
                        globalThis.__FISSION_SHOWCASE_POINTER_TRACE.push({
                            type,
                            pointerId: event.pointerId,
                            pointerType: event.pointerType,
                            x: event.clientX,
                            y: event.clientY,
                            buttons: event.buttons,
                            captured: canvas.hasPointerCapture(event.pointerId),
                        });
                    });
                }
                return true;
            })()"#,
        )
        .expect("install browser pointer trace");
}

fn pointer_trace(client: &LiveTestClient) -> String {
    format!(
        "{}",
        client
            .browser_evaluate_json("globalThis.__FISSION_SHOWCASE_POINTER_TRACE ?? []",)
            .expect("read browser pointer trace")
    )
}

#[test]
#[ignore = "requires a Web test-control showcase build and Chrome"]
fn showcase_browser_interactions_remain_live_across_every_preview() {
    let client = browser();
    install_pointer_trace(&client);
    client
        .wait_for_visible(
            SelectorQuery::semantic_identifier("showcase.example.inbox"),
            READY_TIMEOUT_MS,
        )
        .expect("showcase becomes ready");

    client.assert_text_visible("Apps").expect("Apps group");
    client
        .assert_text_visible("Galleries")
        .expect("Galleries group");
    for excluded in [
        "Counter",
        "Platform",
        "Diagnostics",
        "3D Embed",
        "Web Smoke",
        "Mobile Smoke",
        "Motion Memory",
    ] {
        client
            .assert_text_not_visible(excluded)
            .unwrap_or_else(|error| panic!("excluded catalogue entry {excluded}: {error}"));
    }

    // Prove a mounted action works before introducing text focus or IME state.
    open_example(&client, "text-lab");
    client
        .touch_selector(SelectorQuery::semantic_identifier("text-lab.open-modal"))
        .expect("open Text Lab modal before editing");
    client.pump().expect("paint physical Text Lab modal action");
    client
        .wait_for_visible(
            SelectorQuery::semantic_identifier("text-lab.modal"),
            ACTION_TIMEOUT_MS,
        )
        .unwrap_or_else(|error| {
            panic!(
                "physical mounted action failed: {error}; browser pointer trace: {}",
                pointer_trace(&client),
            )
        });
    client
        .tap_semantic_identifier("text-lab.modal.cancel")
        .expect("close Text Lab modal before editing");
    client.pump().expect("paint first closed modal");

    // Use real browser touch input for the catalogue. This catches the Web
    // pointer translation and the rebuild between press and release.
    open_example(&client, "todo-design-system");
    open_example(&client, "inbox");
    open_example(&client, "text-lab");

    // Touch the left, centre and right portions of catalogue controls. This
    // catches holes or offsets inside their visible bounds, not merely whether
    // one coordinate happens to work.
    for relative_x in [0.2, 0.5, 0.8] {
        open_example_at(&client, "animation-gallery", relative_x);
        open_example_at(&client, "text-lab", relative_x);
    }

    client
        .touch_selector(SelectorQuery::semantic_identifier("text-lab.single-line"))
        .expect("focus single-line input with touch");
    let single_line = client
        .resolve_selector(SelectorQuery::semantic_identifier("text-lab.single-line"))
        .expect("single-line input remains in the live semantic tree after focus");
    assert!(single_line.focusable, "single-line input must be focusable");
    client
        .type_text("Focused")
        .expect("type into the touched single-line input");
    client.pump().expect("paint single-line edit");
    let single_line = client
        .resolve_selector(SelectorQuery::semantic_identifier("text-lab.single-line"))
        .expect("single-line input remains after editing");
    assert_eq!(
        single_line.value.as_deref(),
        Some("Focused"),
        "touching the single-line field must give it keyboard focus"
    );
    client.press_key("Tab", 0).expect("Tab to multiline input");
    let multiline_before_typing = client
        .resolve_selector(SelectorQuery::semantic_identifier("text-lab.multiline"))
        .expect("multiline input remains in the live semantic tree after Tab");
    assert!(
        multiline_before_typing.focusable && multiline_before_typing.text_editable,
        "multiline input must remain focusable and editable after Tab: {multiline_before_typing:?}"
    );
    client
        .type_text("Multiline focus survives")
        .expect("type into multiline input");
    client.pump().expect("paint multiline edit");
    let multiline_after_typing = client
        .resolve_selector(SelectorQuery::semantic_identifier("text-lab.multiline"))
        .expect("multiline input remains in the live semantic tree after editing");
    let single_line_after_tab = client
        .resolve_selector(SelectorQuery::semantic_identifier("text-lab.single-line"))
        .expect("single-line input remains after Tab");
    assert_eq!(
        (
            single_line_after_tab.value.as_deref(),
            multiline_after_typing.value.as_deref(),
        ),
        (Some("Focused"), Some("Multiline focus survives")),
        "Tab must transfer keyboard input from the single-line input to the multiline editor"
    );
    client
        .wait_for_value(
            SelectorQuery::semantic_identifier("text-lab.multiline"),
            "Multiline focus survives",
            ACTION_TIMEOUT_MS,
        )
        .expect("Tab focus and multiline editing remain active");

    client
        .touch_selector(SelectorQuery::semantic_identifier("text-lab.open-modal"))
        .expect("open Text Lab modal");
    client.pump().expect("paint Text Lab modal");
    client
        .wait_for_visible(
            SelectorQuery::semantic_identifier("text-lab.modal"),
            ACTION_TIMEOUT_MS,
        )
        .unwrap_or_else(|error| {
            panic!(
                "Text Lab modal becomes visible: {error}; browser pointer trace: {}",
                pointer_trace(&client)
            )
        });
    client.tap_text("Cancel").expect("close Text Lab modal");
    client.pump().expect("paint closed Text Lab modal");

    open_example(&client, "animation-gallery");
    let modal_navigation = client
        .resolve_selector(SelectorQuery::semantic_identifier(
            "animation-gallery.nav.widgets.modal",
        ))
        .expect("animation Modal navigation row");
    assert!(
        modal_navigation.logical_bounds.width > 180.0,
        "animation navigation must expose a full-row hit surface, got {:?}",
        modal_navigation.logical_bounds
    );
    client
        .touch_selector_at(
            SelectorQuery::semantic_identifier("animation-gallery.nav.widgets.modal"),
            0.85,
            0.5,
        )
        .expect("open animation Modal page from the row edge");
    client
        .wait_for_text("Open real modal", ACTION_TIMEOUT_MS)
        .expect("animation Modal page becomes ready");
    client
        .touch_selector(SelectorQuery::semantic_identifier(
            "animation-gallery.action.open-real-modal",
        ))
        .expect("open real animation modal");
    client
        .wait_for_visible(
            SelectorQuery::semantic_identifier("gallery.real.modal.surface"),
            ACTION_TIMEOUT_MS,
        )
        .expect("real animation modal becomes visible");
    client
        .touch_selector(SelectorQuery::semantic_identifier(
            "gallery.real.modal.cancel",
        ))
        .expect("close real animation modal");
    client.pump().expect("paint closed animation modal");

    client
        .touch_selector(SelectorQuery::semantic_identifier(
            "animation-gallery.nav.widgets.drawer",
        ))
        .expect("open animation Drawer page");
    client
        .wait_for_text("Open real drawer", ACTION_TIMEOUT_MS)
        .expect("animation Drawer page becomes ready");
    client
        .touch_selector(SelectorQuery::semantic_identifier(
            "animation-gallery.action.open-real-drawer",
        ))
        .expect("open real animation drawer");
    client
        .wait_for_visible(
            SelectorQuery::semantic_identifier("gallery.real.drawer.backdrop"),
            ACTION_TIMEOUT_MS,
        )
        .expect("real animation drawer becomes visible");
    client
        .touch_selector(SelectorQuery::semantic_identifier(
            "gallery.real.drawer.close",
        ))
        .expect("close animation drawer");
    client.pump().expect("paint closed animation drawer");

    client
        .touch_selector(SelectorQuery::semantic_identifier(
            "animation-gallery.nav.widgets.popover",
        ))
        .expect("open animation Popover page");
    client
        .wait_for_text("Profile actions", ACTION_TIMEOUT_MS)
        .expect("animation Popover page becomes ready");
    client
        .touch_selector(SelectorQuery::semantic_identifier(
            "gallery.real.popover.trigger",
        ))
        .expect("open real animation popover");
    client
        .wait_for_text("Invite teammate", ACTION_TIMEOUT_MS)
        .expect("real animation popover becomes visible");
    client
        .touch_selector(SelectorQuery::semantic_identifier(
            "gallery.real.popover.close",
        ))
        .expect("close animation popover");
    client.pump().expect("paint closed animation popover");

    // Repeatedly swap every retained preview. This guards against stale action
    // scopes and the lock-up previously seen after rapid catalogue changes.
    for _ in 0..2 {
        for slug in [
            "inbox",
            "todo-design-system",
            "text-lab",
            "widget-gallery",
            "animation-gallery",
            "chart-gallery",
            "icons_gallery",
        ] {
            open_example(&client, slug);
        }
    }

    // A final interaction proves that the app remains responsive after the
    // stress loop rather than merely rendering the last selected preview.
    open_example(&client, "text-lab");
    client
        .touch_selector(SelectorQuery::semantic_identifier("text-lab.open-modal"))
        .expect("open final modal");
    client
        .wait_for_visible(
            SelectorQuery::semantic_identifier("text-lab.modal"),
            ACTION_TIMEOUT_MS,
        )
        .expect("showcase remains interactive after repeated swaps");
}
