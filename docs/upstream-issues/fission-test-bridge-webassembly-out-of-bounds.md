# Web LiveTest bridge WebAssembly bounds trap

Status: bridge commands, responses, and frame snapshots now cross the Wasm
boundary as JSON strings, avoiding `JsValue` payload allocation. Focused
Chromium coverage passes repeated text and selector queries; the Zoffice
editor-open reproduction still needs to be rerun against the patch.

## Reproduction evidence

Zoffice reported repeated Chrome LiveTest runs that trapped while waiting for
`a11y.document_body_editor`. One trace recorded 563 pages near a trap; a later
trace recorded 276 pages at startup, 371 after the first screenshot, and 586
after workspace creation, with no growth around the last successful poll.
Failures occurred in both `pollValue` and `submitValue` while allocating
externref handles. This evidence points to externref-table pressure rather than
linear-memory exhaustion.

## Bridge changes

- LiveTest uses `submit(json)` and `poll(id)` for all commands and responses.
  `poll` returns JSON text `null` while work is pending. The frame getter also
  returns a JSON string, parsed by the native browser driver after the Wasm call.
  No bridge payload uses a JavaScript object across the Wasm boundary.
- Completed responses are moved out of the pending map instead of cloned. The
  map is capped at 1,024 pending or uncollected requests; overflow returns an
  actionable error and does not enqueue a command.
- The browser regression performs 128 repeated asynchronous text queries,
  requests a semantic tree, fills and drains the pending-response cap, and
  confirms the bridge still answers afterward.

## Verification and remaining evidence

The regression passed in Chromium against a fresh `examples/web-smoke` build
with `FISSION_WEB_TEST_CONTROL=1`, including repeated `ResolveSelector` and
semantic-tree response paths. The Web shell compiled for wasm32, the
browser-driver integration target compiled, and the focused browser test passed.
This synthetic browser run does not reproduce Zoffice's PersonalDB editor-open
flow, so the app-level trap is not considered resolved until that exact LiveTest
passes against the pushed bridge change.
