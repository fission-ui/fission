# Web LiveTest bridge WebAssembly bounds trap

Status: bridge mitigation and a focused regression are pushed on
`feature/docs2-goal-oriented-site`; the Zoffice long-running editor-open
reproduction still needs to be rerun against the patch.

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
- Completed responses are moved out of the pending map instead of cloned. The
  map is capped at 1,024 pending or uncollected requests; overflow returns an
  actionable error and does not enqueue a command.
- The browser regression performs 128 repeated asynchronous text queries,
  fills and drains the pending-response cap, and confirms the bridge still
  answers afterward.

## Verification and remaining evidence

The regression passed in Chromium against `examples/web-smoke`, built with
`FISSION_WEB_TEST_CONTROL=1`. The Web shell and test driver compiled for their
respective targets, and the test-driver library suite passed. This synthetic
browser run does not reproduce Zoffice's long-running PersonalDB editor-open
flow, so the reported app-level trap is not considered resolved until that
exact LiveTest passes against the pushed bridge change.
