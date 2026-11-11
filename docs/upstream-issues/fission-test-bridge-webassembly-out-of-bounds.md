# Web LiveTest bridge WebAssembly bounds trap

Status: large Web LiveTest responses now cross the Wasm boundary as one JSON
string, avoiding per-field `JsValue` allocation. The focused Chromium regression
passes with a semantic-tree query. The Zoffice editor-open reproduction still
needs to be rerun against the patch.

## Reproduction evidence

Zoffice reported repeated Chrome LiveTest runs that trapped after about 150 s
while waiting for `a11y.document_body_editor`. The WebAssembly memory remained
at 36,896,768 bytes (563 pages) and did not grow near the trap. The earlier
failure occurred inside the Rust `poll` closure; a later run trapped in
`__wbindgen_malloc` while wasm-bindgen copied the next short JSON command string
into WebAssembly memory. This evidence does not point to linear-memory growth as
the cause.

## Bridge changes

- LiveTest now submits command objects and receives response objects through
  `submitValue` / `pollValue`, avoiding the extra JSON-string conversion on each
  bridge call. The original string methods remain for compatibility with older
  test drivers.
- Large semantic-tree and selector-error responses, plus text results above 128
  items, use one JSON string for the response. Small responses retain the direct
  JS-value path. The native browser driver accepts both formats. This avoids
  building a large nested graph of wasm-bindgen externref handles for apps with
  many rendered nodes.
- Completed responses are moved out of the pending map instead of cloned. The
  map is capped at 1,024 pending or uncollected requests; overflow returns an
  actionable error and does not enqueue a command.
- The browser regression performs 128 repeated asynchronous text queries,
  requests a semantic tree, fills and drains the pending-response cap, and
  confirms the bridge still answers afterward.

## Verification and remaining evidence

The regression passed in Chromium against a fresh `examples/web-smoke` build
with `FISSION_WEB_TEST_CONTROL=1`, including the semantic-tree response path.
The Web shell compiled for wasm32, the browser-driver decoder unit test passed,
and the test-driver integration target compiled. This synthetic browser run does
not reproduce Zoffice's PersonalDB editor-open flow, so the app-level trap is
not considered resolved until that exact LiveTest passes against the pushed
bridge change.
