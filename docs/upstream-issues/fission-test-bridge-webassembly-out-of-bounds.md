# Web LiveTest bridge WebAssembly bounds trap

Status: still open. Bridge commands, responses, and frame snapshots now cross
the Wasm boundary as JSON strings, avoiding `JsValue` payload allocation. The
focused Chromium regression passes the reported selector-and-pump sequence, but
the full Zoffice editor-open flow still traps under Fission `bcec7df5`.

## Reproduction evidence

Zoffice's first string-only retest trapped after 28.21 seconds while waiting for
`a11y.document_body_editor`. Request 51 was `ResolveSelector` for
`a11y.document_body_editor` with candidate and hidden-node collection disabled.
Requests 47 and 49 returned 270-byte `SelectorError` strings; requests 48 and 50
returned 15-byte `Pump` responses. The poll trap happened before JavaScript
received a value. Request 50 was the last successful poll.

WebAssembly memory was 276 pages at startup, 371 after the first screenshot, and
586 (38,404,096 bytes) after workspace creation. It stayed at 586 pages around
request 50 and after the trap. Earlier value-transport attempts also failed in
`__externref_table_alloc`; however, the current string-only bridge still traps,
so the underlying cause is not yet isolated and cannot be attributed only to
externref payload conversion.

## Bridge changes

- LiveTest uses `submit(json)` and `poll(id)` for all commands and responses.
  `poll` returns JSON text `null` while work is pending. The frame getter also
  returns a JSON string, parsed by the native browser driver after the Wasm call.
  No bridge payload uses a JavaScript object across the Wasm boundary.
- `pollDiagnostics()` exposes a numeric request/stage marker in test builds so a
  future reproduction can distinguish response lookup, JSON serialization, and
  the Wasm string return conversion after a trapped call.
- Completed responses are moved out of the pending map instead of cloned. The
  map is capped at 1,024 pending or uncollected requests; overflow returns an
  actionable error and does not enqueue a command.
- The browser regression performs 128 repeated asynchronous text queries,
  requests a semantic tree, resolves a selector, repeats 32 missing-selector
  responses followed by pumps, fills and drains the pending-response cap, and
  confirms the bridge still answers afterward.

## Verification and remaining evidence

The regression passes in Chromium against a fresh `examples/web-smoke` build
with `FISSION_WEB_TEST_CONTROL=1`. The Web shell compiles for wasm32 and the
focused browser test covers repeated queries, the selector-and-pump cadence,
and the response cap. In the Zoffice retest, Fission `bcec7df5` preserved the
custom worker, and PersonalDB completed workspace creation and schema reopen,
but request 51 still trapped in `poll`; a subsequent `submit` trapped in
`__wbindgen_malloc`. The app-level bridge issue remains open pending poll-stage
diagnostics and another full LiveTest run.
