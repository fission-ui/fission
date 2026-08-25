use anyhow::{Context, Result};
use fission_test_driver::{BrowserTestOptions, LiveTestClient, SelectorQuery};
use serde_json::Value;

fn browser_url() -> Option<String> {
    std::env::var("FISSION_WEB_SMOKE_URL").ok()
}

fn active_control(client: &LiveTestClient) -> Result<Value> {
    client.browser_evaluate_json(
        r#"(() => {
            const element = document.activeElement;
            return {
                tag: element?.tagName ?? null,
                type: element?.getAttribute("type"),
                value: element?.value ?? null,
                selectionStart: element?.selectionStart ?? null,
                selectionEnd: element?.selectionEnd ?? null,
                selectionDirection: element?.selectionDirection ?? null,
                name: element?.getAttribute("name"),
                autocomplete: element?.getAttribute("autocomplete"),
                inputmode: element?.getAttribute("inputmode"),
                ariaLabel: element?.getAttribute("aria-label"),
                ariaRequired: element?.getAttribute("aria-required"),
            };
        })()"#,
    )
}

fn apply_browser_edit(
    client: &LiveTestClient,
    value: &str,
    selection_start: u32,
    selection_end: u32,
    input_type: &str,
    data: Option<&str>,
    cancelable: bool,
    composing: bool,
) -> Result<()> {
    let arguments = serde_json::json!({
        "value": value,
        "selectionStart": selection_start,
        "selectionEnd": selection_end,
        "inputType": input_type,
        "data": data,
        "cancelable": cancelable,
        "composing": composing,
    });
    let expression = format!(
        r#"(() => {{
            const args = {arguments};
            const element = document.activeElement;
            if (!(element instanceof HTMLTextAreaElement || element instanceof HTMLInputElement)) {{
                throw new Error("Fission browser text adapter is not focused");
            }}
            element.dispatchEvent(new InputEvent("beforeinput", {{
                bubbles: true,
                cancelable: args.cancelable,
                inputType: args.inputType,
                data: args.data,
                isComposing: args.composing,
            }}));
            element.value = args.value;
            element.setSelectionRange(args.selectionStart, args.selectionEnd, "none");
            element.dispatchEvent(new InputEvent("input", {{
                bubbles: true,
                cancelable: false,
                inputType: args.inputType,
                data: args.data,
                isComposing: args.composing,
            }}));
            return true;
        }})()"#
    );
    client.browser_evaluate_json(&expression)?;
    client.pump()?;
    Ok(())
}

#[test]
fn canvas_text_adapter_reconciles_complete_browser_edits() -> Result<()> {
    let Some(url) = browser_url() else {
        eprintln!("set FISSION_WEB_SMOKE_URL to run the browser text-input conformance test");
        return Ok(());
    };
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())?;

    client.focus_selector(SelectorQuery::semantic_identifier("web-smoke.text.primary"))?;
    let initial = active_control(&client)?;
    assert_eq!(initial["tag"], "TEXTAREA");
    assert_eq!(initial["name"], "primary");
    assert_eq!(initial["ariaLabel"], "Primary field");
    assert_eq!(initial["ariaRequired"], "true");

    apply_browser_edit(&client, "A🙂B", 3, 3, "insertText", Some("🙂"), true, false)?;
    client.wait_for_text("Primary value: A🙂B (edits: 1)", 5_000)?;
    let unicode = active_control(&client)?;
    assert_eq!(unicode["value"], "A🙂B");
    assert_eq!(unicode["selectionStart"], 3);
    assert_eq!(unicode["selectionEnd"], 3);

    apply_browser_edit(
        &client,
        "AZB",
        2,
        2,
        "insertReplacementText",
        Some("Z"),
        false,
        false,
    )?;
    client.wait_for_text("Primary value: AZB (edits: 2)", 5_000)?;
    apply_browser_edit(
        &client,
        "AB",
        1,
        1,
        "deleteContentBackward",
        None,
        true,
        false,
    )?;
    client.wait_for_text("Primary value: AB (edits: 3)", 5_000)?;
    apply_browser_edit(
        &client,
        "Autofilled",
        10,
        10,
        "insertReplacementText",
        None,
        false,
        false,
    )?;
    client.wait_for_text("Primary value: Autofilled (edits: 4)", 5_000)?;

    client.focus_selector(SelectorQuery::semantic_identifier(
        "web-smoke.text.secondary",
    ))?;
    apply_browser_edit(
        &client,
        "second",
        6,
        6,
        "insertText",
        Some("second"),
        true,
        false,
    )?;
    client.wait_for_text("Secondary value: second (edits: 1)", 5_000)?;
    client.wait_for_text("Primary value: Autofilled (edits: 4)", 5_000)?;

    client.focus_selector(SelectorQuery::semantic_identifier("web-smoke.text.primary"))?;
    let restored = active_control(&client)?;
    assert_eq!(restored["value"], "Autofilled");
    assert_eq!(restored["selectionStart"], 10);
    assert_eq!(restored["selectionEnd"], 10);

    client.browser_evaluate_json(
        r#"(() => {
            const element = document.activeElement;
            element.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true, data: "" }));
            element.dispatchEvent(new CompositionEvent("compositionupdate", { bubbles: true, data: "世" }));
            return true;
        })()"#,
    )?;
    apply_browser_edit(
        &client,
        "Autofilled世界",
        12,
        12,
        "insertCompositionText",
        Some("世界"),
        false,
        true,
    )?;
    client.browser_evaluate_json(
        r#"(() => {
            document.activeElement.dispatchEvent(
                new CompositionEvent("compositionend", { bubbles: true, data: "世界" })
            );
            return true;
        })()"#,
    )?;
    apply_browser_edit(
        &client,
        "Autofilled世界",
        12,
        12,
        "insertFromComposition",
        Some("世界"),
        false,
        false,
    )?;
    client.wait_for_text("Primary value: Autofilled世界 (edits: 5)", 5_000)?;

    // CDP keyboard events are delivered through the browser's native event
    // path, so this covers Fission's selectively enabled clipboard defaults
    // rather than the deterministic test bridge's synthetic clipboard path.
    client.press_key("a", 4)?;
    let copied_selection = active_control(&client)?;
    assert_eq!(copied_selection["selectionStart"], 0);
    assert_eq!(copied_selection["selectionEnd"], 12);
    client.press_key("c", 4)?;
    client.focus_selector(SelectorQuery::semantic_identifier(
        "web-smoke.text.secondary",
    ))?;
    client.press_key("a", 4)?;
    client.press_key("v", 4)?;
    let pasted = active_control(&client)?;
    assert_eq!(pasted["value"], "Autofilled世界");
    assert_eq!(pasted["selectionStart"], 12);
    assert_eq!(pasted["selectionEnd"], 12);
    client.wait_for_text("Secondary value: Autofilled世界 (edits: 2)", 5_000)?;
    client.right_click_selector(SelectorQuery::semantic_identifier(
        "web-smoke.text.secondary",
    ))?;
    client.wait_for_text("Copy", 5_000)?;
    client.press_key("Escape", 0)?;

    client.focus_selector(SelectorQuery::semantic_identifier(
        "web-smoke.text.password",
    ))?;
    let password = active_control(&client)?;
    assert_eq!(password["tag"], "INPUT");
    assert_eq!(password["type"], "password");
    assert_eq!(password["autocomplete"], "current-password");
    assert_eq!(password["ariaLabel"], "Password field");
    apply_browser_edit(
        &client,
        "s3cret",
        6,
        6,
        "insertText",
        Some("s3cret"),
        true,
        false,
    )
    .context("password adapter edit")?;
    let inactive_value =
        client.browser_evaluate_json(r#"document.querySelector('textarea')?.value ?? null"#)?;
    assert_eq!(
        inactive_value, "",
        "secure text leaked into the inactive textarea adapter"
    );

    let semantic = client.resolve_selector(SelectorQuery::semantic_identifier(
        "web-smoke.text.password",
    ))?;
    assert!(semantic.masked);
    assert!(semantic.value_present);
    assert_eq!(semantic.value, None, "semantic tree leaked obscured text");
    Ok(())
}
