# Fission 0.16.0 preparation and migration notes

Fission 0.16.0 is being prepared. This document is not a release announcement:
no crates.io publication, Git tag or GitHub release is authorised by this work.
Version alignment does not establish package availability or platform
qualification. The registry dependency blocker below must be resolved before
publication can proceed.

## Application migrations

### Give lazy rows their identity before constructing them

`LazyColumn::with_keyed_item_builder` now takes a separate key selector and row
builder. The previous callback returned `(WidgetId, Widget)`, which supplied the
identity too late: the row component had already been constructed. Its retained
local state and reducer registrations could therefore use the wrong identity.

Before:

```rust,ignore
LazyColumn::with_keyed_item_builder(messages.len(), 64.0, move |index| {
    (
        WidgetId::explicit(&messages[index].id),
        MessageRow { message: messages[index].clone() }.into(),
    )
})
```

After, with `messages` held in an `Arc<Vec<Message>>`:

```rust,ignore
let message_keys = Arc::clone(&messages);

LazyColumn::with_keyed_item_builder(
    messages.len(),
    64.0,
    move |index| WidgetId::explicit(&message_keys[index].id),
    move |index| MessageRow { message: messages[index].clone() }.into(),
)
.id(WidgetId::explicit("inbox-messages"))
```

`MessageRow` is the application's named row component; its conversion can use
Compose normally. The framework establishes the row's identity before invoking
that conversion, inside the ordinary component build scope. Only visible rows
and the one-row overscan buffer are constructed. Keep the same data-derived key
when a row moves, and use distinct keys for distinct rows.

`with_item_builder(item_count, item_height, builder)` still has its existing
signature. It uses row positions as identities, so reserve it for lists whose
items do not change identity through insertion, removal, filtering or reordering.

### Construct and clone Web SQLite providers explicitly

`WebSqliteStore` now carries an optional database name. It is no longer a unit
struct and no longer implements `Copy`. Replace a bare `WebSqliteStore` value
with `WebSqliteStore::new()` or `WebSqliteStore::default()`, and use `.clone()`
when multiple owners need the same provider configuration.

The default constructor retains the application's default database. Use
`WebSqliteStore::open_database(name)?` only when the application intentionally
needs a separate named database. Call `close()` after outstanding work completes;
it disconnects the calling browser context and cancels that context's open
transaction. Later requests can reopen the same persistent database.

### Authorise an intentional PRAGMA explicitly

For worker-side transaction authorizer policies, `WebSqliteAuthorizerAction::Read`
covers SQLite reads, selections and function calls. It does **not** authorise
`Pragma`. SQLite uses that action for both inspection and changes such as
`PRAGMA user_version = ...` and `PRAGMA writable_schema = ON`. Treating every
PRAGMA as a read would allow persistent changes through a read policy.

If a deny-by-default policy previously relied on `Read` to allow a PRAGMA, add a
specific `Pragma` rule for the intended operation and constrain its arguments.
For example, an inspection rule for `user_version` must also distinguish it from
an assignment; allowing that name alone allows both forms. Rules use the first
match, so put narrow exceptions before broader denials. Keep unrelated PRAGMAs
denied rather than adding a blanket allow to restore old behaviour.

SQLite Session tracking may request `table_xinfo` when recording the first
change to a captured table. Add an explicit `Pragma` allow rule with
`first_argument: "table_xinfo"`, `second_argument` equal to the captured table,
and `database: "main"`. This grants only read-only metadata for that table, not
blanket PRAGMA access. Denial can surface later as `SQLITE_AUTH` during changeset
capture even if the INSERT succeeds.

An allowed SQL function can itself have side effects. Applications registering
custom functions must enforce their own side-effect policy. These connection
rules do not replace server authentication or authorisation.

### Upgrade the Android capability bridge with the runtime

Use the matching Fission CLI when rebuilding an existing Android project. Android
packaging now checks the generated Java capability bridge before building.
Known, unmodified generated bridges are refreshed automatically. Custom Java is
preserved: an obsolete custom bridge stops packaging and gets an adjacent
`.java.fission-new` candidate for a deliberate merge.

Merge the implementation and required method signatures, not just the bridge API
version marker. In particular, the PDF print bridge now receives a private
staged-file path rather than an entire PDF byte array. Keeping an old helper
beside a new Rust runtime would otherwise defer a missing-method failure until a
user invokes a capability.

### Keep document destinations and print results typed

Save dialogs return an optional `SaveFileDestination`. `None` means cancellation;
it is not a request to invent a fallback path. A `Location` uses the ordinary
filesystem operations; a `WritableFile` is an opaque host handle written through
`WRITE_PICKED_FILE`. Its display name is not a filesystem path. Browser handle
persistence and permission APIs are explicitly browser-specific; do not assume a
serialized handle is a portable document grant.

Android validates and claims a one-shot source stream before opening a selected
document for truncation. iOS save picking uses a move operation to obtain an
accessible destination, and import reads snapshot data inside file coordination
using the coordinator's supplied URL. These fixes do not promise atomic saves:
a later provider or I/O failure can still fail a write.

PDF printing is an asynchronous capability. `PrintOutcome::Presented` means a
print surface opened; `Submitted` means a host accepted a job, not that paper
was printed. Handle `Cancelled` and errors explicitly. Android copies staged
PDFs to the spooler on a bounded background writer with cancellation support.
The PDF remains intact and the system spooler applies the user's selected page
range. Application-supplied page-range and copy overrides remain unsupported on
Android; choose these in its system print sheet.

## Registry publication blocker: the required winit revision is not in `.6`

The shell currently depends on package `fission-winit` from
[`worka-ai/winit` at `1e99d748234f11a0dd49a00a1ffc3cca1bb0b203`](https://github.com/worka-ai/winit/commit/1e99d748234f11a0dd49a00a1ffc3cca1bb0b203).
That checkout declares version `0.30.13-fission.6`, but it is **not** the source
published under the same version on crates.io.

The independently downloaded
[`fission-winit-0.30.13-fission.6.crate`](https://static.crates.io/crates/fission-winit/fission-winit-0.30.13-fission.6.crate)
matches the [public sparse-index](https://index.crates.io/fi/ss/fission-winit)
SHA-256:

```text
6a190926b9870d28d221c6da25237ff3279a8b4fb2a3e7dcb2034102bfe406c1
```

Its `.cargo_vcs_info.json` identifies commit
`93fed5f0e1cbfdb20c8acf8528b8b1d583336e9b`, not the pinned Git revision above.
The source difference is consequential:

- The pinned revision adds `BrowserDefaults::KEYBOARD_SHORTCUTS` in
  `src/platform/web.rs`.
- It changes `src/platform_impl/web/web_sys/canvas.rs` so browser commands such
  as Find, Location and Reload can remain available alongside IME/text editing.
- Fission's `web_input.rs` and `ime.rs` already reference that new constant.
  The registry `.6` lacks it, so replacing Git with that registry version would
  break the Web build as well as discard the intended browser-command behaviour.

The pinned Git dependency is deliberately retained. Adding a registry version
that cannot supply the required API is not a publication fix. This also affects
the Android dependency declaration and the shared shell dependency used by the
vendored accessibility adapter; those consumers must stay aligned.

Before a future registry release, publish a **new, distinct** fork version
containing the required revision, update the affected manifests to that registry
package, and validate registry-only consumers, including Web and Android. Do not
attempt to replace the immutable `.6` archive. Publication of that prerequisite
is not authorised as part of the current preparation work.

## Registry publication blocker: the Vello GPU package lacks the upload fixes

The checked-out Vello fork is
[`9e2b85cb91316f7870097d12666804b1a50e1aed`](https://github.com/worka-ai/vello/commit/9e2b85cb91316f7870097d12666804b1a50e1aed).
Its `vello_gpu` package still declares `fission-vello-gpu` version
`0.2.0-fission.1`. The independently downloaded
[`0.2.0-fission.1` registry archive](https://static.crates.io/crates/fission-vello-gpu/fission-vello-gpu-0.2.0-fission.1.crate)
matches the [public sparse-index](https://index.crates.io/fi/ss/fission-vello-gpu)
SHA-256:

```text
13b25d5708d7af53970aa1d36612b960e1f3bbbaa2681ca855ddedc2973ef0bb
```

That archive's `.cargo_vcs_info.json` identifies
`9877cac65d10c878f1c9056d6f77d2873d90d7c7`. Its
`src/render/wgpu/mod.rs` matches that earlier Git revision exactly, rather than
the currently checked-out revision. The two intervening commits change that
one file:

- [`422504076716eaa9b70fcd4983ed4f88c3300a0b`](https://github.com/worka-ai/vello/commit/422504076716eaa9b70fcd4983ed4f88c3300a0b)
  replaces mapped transient instance-buffer initialisation with unmapped
  `COPY_DST` allocations populated through `Queue::write_buffer`.
- [`9e2b85cb91316f7870097d12666804b1a50e1aed`](https://github.com/worka-ai/vello/commit/9e2b85cb91316f7870097d12666804b1a50e1aed)
  reuses the existing layer-configuration buffer during intermediate-texture
  resizing and updates it through the queue instead of recreating a mapped
  buffer.

The checkout therefore contains fixes absent from the registry package. A
workspace build uses the local path dependency in `fission-render-vello`, but a
published consumer resolves the declared registry dependency. Keeping the old
package version would lose those fixes outside this checkout. This establishes
a source/package mismatch, not proof that either version has passed browser or
device qualification.

The other four fork archives were also downloaded and checked against their
public index checksums. Their `.cargo_vcs_info.json` files identify the same
earlier revision, but their packaged source and build scripts match the current
checkout:

| Package | Existing published version | Source/build files compared |
| --- | --- | ---: |
| `fission-vello-cpu` | `0.2.0-fission.1` | 40 |
| `fission-vello-common` | `0.2.0-fission.1` | 33 |
| `fission-vello-gpu-shaders` | `0.2.0-fission.1` | 9 |
| `fission-glifo` | `0.3.0-fission.1` | 11 |

There is no evidence from this comparison that those four unchanged packages
need replacement merely because the fork's Git revision advanced. The smallest
future publication fix is a distinct version of `fission-vello-gpu` containing
the upload corrections, corresponding dependency/version alignment, and
registry-only consumer qualification. Do not republish or substitute the
immutable existing archive. No fork version changes or publication are included
in this readiness check.

## Qualification gates before a release

Keep release evidence tied to the eventual candidate commit. Focused source or
unit checks do not establish device behaviour or registry installability.

- Exercise keyed lazy-row state and actions through scrolling and reordering.
- Keep the real-browser SQLite suite green, including explicit PRAGMA denial/allowance,
  transaction cancellation and connection teardown. The candidate fixture passes
  in Chromium, including the real Rust cancelled-BEGIN future and exact-table
  Session metadata permission; this does not establish device-provider behaviour.
- Test existing Android project upgrades with both generated and customised
  bridges, then exercise document import, save, cancellation and printing on a
  device or emulator. Verify the selected print range in actual output.
- Exercise iOS provider-backed import/save flows and cancellation on a simulator
  or device, including destinations outside the application's container.
- Resolve both fork dependency blockers before claiming crates.io readiness;
  validate the package contents and downstream installation separately from a
  workspace build.

The new documentation source is `docs2`; there is no need to restore the removed
legacy documentation project for migration or release qualification.

## Documentation cutover evidence and remaining gates

On 13 November 2026, the integrated candidate qualification compiled all 160
distinct extracted checkpoints: 15 actual Static sites and 145 Web applications.
It rendered 1,118 routes and passed internal-link, homepage SEO and showcase
output checks. Including focused tests, a release showcase rebuild and two real
browser showcase tests, this run completed in approximately 38 minutes using
the existing shared dependency cache. This is not a cold-CI timing claim.

The subsequent site-only qualification passed 35 site library tests and reused
all 160 complete preview packages without recompiling their applications. A
serial Chromium regression checked the homepage, documentation hub and login
tutorial at both 1,536px and 390px widths. It verified heading scale, no
horizontal page overflow, equal-height desktop task cards and a visible finished
login app selected from the last runnable checkpoint rather than closing prose.
Manual LiveTest screenshots and login interactions additionally verified empty
validation, retained field input, obscured passwords and feedback which does not
claim that a valid form has authenticated anyone.

These checks do not make the candidate merge-ready by themselves:

- Independently confirm the final pushed commit through GitHub's PR checks.
- Configure and verify production COOP `same-origin` and COEP `require-corp`
  response headers on containing pages and embedded applications. Local test
  server headers do not establish production cross-origin isolation.
- Finish native document import/save/update/cancellation and print-sheet checks
  on iOS and Android. iOS Metal/Vello rendering, a counter interaction and actual
  Files save-sheet presentation passed; completed provider operations did not.
- Resolve the registry dependency blockers above and coordinate availability of
  the matching 0.16.0 CLI/framework packages before deploying setup commands that
  require them. The site labels this as its documentation version, not latest
  stable. No package publication, tag or release is part of this preparation.

Responsive text regressions cover end-caret visibility, including replacement
by another responsive widget identity. They do not establish arbitrary mid-text
selection transfer between different widget identities, editable-combobox resize
behaviour or every multiline scroll policy.
