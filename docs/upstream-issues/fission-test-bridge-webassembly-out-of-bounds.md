# Web LiveTest bridge WebAssembly bounds trap

Status: targeted bridge fix is on the branch; full application qualification is
pending. `poll` now reaches scalar Wasm exports through a JavaScript adapter
instead of the wasm-bindgen closure trampoline, and reads JSON response bytes
from linear memory. The focused Chromium regression passes this path. The full
application flow reproduced a closure-entry trap at request 49 before the latest
bridge change; that change still needs the full app retest.

## Reproduction evidence

The first application string-only retest trapped after 28.21 seconds while
waiting for an editor selector. Request 51 was `ResolveSelector` for that
selector with candidate and hidden-node collection disabled.
Requests 47 and 49 returned 270-byte `SelectorError` strings; requests 48 and 50
returned 15-byte `Pump` responses. The poll trap happened before JavaScript
received a value. Request 50 was the last successful poll.

WebAssembly memory was 276 pages at startup, 371 after the first screenshot, and
586 (38,404,096 bytes) after application setup. It stayed at 586 pages around
request 50 and after the trap. Earlier value-transport attempts also failed in
`__externref_table_alloc`; however, the current string-only bridge still traps,
so the underlying cause is not yet isolated and cannot be attributed only to
externref payload conversion.

In a subsequent retest, the raw linear-memory marker read as request 46, stage 3
(`(46 << 2) | 3 == 187`), before request 47's `ResolveSelector` poll trapped.
The poll closure's wasm-bindgen state remained stable (`a=1`, `b=5774816`,
`cnt=1`) before and after the trap, but its Rust body was not entered. This
narrows the failure to closure entry rather than response lookup or JSON
serialization. The latest change removes that closure from the poll path; its
full-app result is not yet verified.

## Bridge changes

- LiveTest uses `submit(json)` and `poll(id)` for commands and responses.
  `poll` calls scalar Wasm exports directly and decodes the response JSON from
  module memory; it returns JSON text `null` while work is pending. The frame
  getter also returns a JSON string, parsed by the native browser driver after
  the Wasm call. No bridge payload uses a JavaScript object across the Wasm
  boundary.
- `pollDiagnostics()` exposes a numeric request/stage marker in test builds. The
  same value is stored at a published byte offset in the test Wasm memory, so a
  harness can read it directly even when another callback traps. This can
  distinguish response lookup, JSON serialization, and string return conversion.
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
direct-memory diagnostics, and the response cap. Application retests preserved
the custom worker and completed database setup, but opening the editor still
trapped before entering the poll body. The direct-export poll path awaits a full
application LiveTest result.
