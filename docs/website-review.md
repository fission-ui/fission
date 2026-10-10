# Visually qualify a browser website with `fission test`

After `fission init`, immediately read the generated `AGENTS.md` and any
referenced `AGENTS.fission.md`. Keep generated `fission.toml`; add targets with
`fission add-target`. Author the application in Rust/Fission, build, review,
inspect the actual images and JSON, fix source, and review again.

```sh
fission test --visual-review --target static-site --project-dir my-site --output-dir review --json
fission test --visual-review --target web --project-dir my-app --route / --route /details/ \
  --mount /repository-name/ --output-dir review-web --json
fission test --visual-review --target web --output-dir review-phone --route / \
  --viewport 390x844 --case-timeout-seconds 90
```

Without `--visual-review`, `fission test` retains its existing target smoke-test
behavior. Visual mode requires an explicit Web or Static site target and an
output directory. Matrix/output/strict/JSON flags are limited to visual mode;
native variant and headless simulator flags are not used for it.

Visual mode freshly builds and owns a finite loopback serving session. Content static sites
select the root entry plus sorted routes from static metadata. Web and custom Rust site builders
have no route discovery contract: default coverage is `/` only; supply every
required app-relative route explicitly. Route paths exclude queries, fragments,
percent escapes, traversal and external URLs. A mount prefixes the selected
paths and never rewrites the app configuration. Web apps must configure their
navigation base path to match their hosting mount. Direct loads are reviewed;
interactive route transitions are outside this command.

Defaults are actual Chromium viewports **390×900, 800×900 and 1280×900 CSS
pixels**, device scale **1**, desktop emulation, reduced motion, and one case
at a time. A fresh disposable browser observes each case before navigation;
errors cannot carry across cases. The browser must acknowledge its inner
viewport. Web additionally requires development test control, a submitted
content frame at the same layout viewport, semantic tree access, and a browser
paint boundary before the full GPU-surface screenshot. A rejected viewport,
uniform capture, invalid PNG, or missing image is an incomplete case. Differing
pixels establish capture content only. They do **not** establish visual
correctness: human/agent inspection of every relevant PNG remains required,
even when the command exits 0. Reports set `image_inspection_required: true`.

`--output-dir` receives `review.json` and deterministic
`route-NNN-WIDTHxHEIGHT.png` files. Route indexes follow sorted selected paths.
Image paths in the report are relative to its output directory. Rerunning
replaces selected-case images and the report; unused older images are not
claimed by the new report. Avoid sharing one output directory between runs.
Stdout in JSON mode is one `fission.website-review.v1` record, also saved with
screenshots; diagnostic output uses stderr. No base64 image is emitted.

Console errors, exceptions and failed resources are recorded per case. All
failed resources are actionable unless they are Chromium `Other` requests for the exact same-origin
`/favicon.ico` or declared icon URL, or a POST to the exact same-origin
`/__fission/renderer` diagnostic endpoint. Failed
documents, scripts and WASM are never broadly suppressed.

DOM geometry identifies horizontal border boxes outside the viewport, excluding
intentional overflow/scroll/clip ancestors and wholly offscreen transformed fixed panels. Normal vertical document scrolling
is permitted. Canvas geometry identifies horizontal semantic-bound candidates
outside non-scroll ancestors. Canvas candidates are warnings because semantic
geometry cannot establish clipping intent. DOM border boxes are also warning
candidates: their bounds alone cannot prove intended clipping or visual failure.
Findings include bounds and selector
or semantic ID. Both checks are **partial**, with explicit limitations: no
exhaustive clipping, text-glyph, pseudo-element, iframe, shadow-root,
accessibility or SEO audit. A clean supported check is not a claim of no visual
clipping; inspect screenshots, especially warning candidates and nonsemantic
paint. An unsupported or failed geometry check is incomplete. Warning-only
matrices report `warning_candidates` and exit 0 by default. Pass `--strict` to
make geometry warnings exit 3; `strict_warnings` records that policy. Required
resource/runtime errors remain blocking under either policy.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | All selected cases captured; no confirmed errors. Warnings may remain; inspect images. |
| 1 | Review execution/build/configuration/report failure. |
| 2 | At least one incomplete, timed-out, unsupported or cancelled case. |
| 3 | Complete cases contain confirmed errors, or warning candidates under `--strict`. |

When defects and incomplete cases coexist, exit 2 takes precedence; the report
retains both. Warning status remains explicit even when strict policy makes it
fail. Argument parsing errors use Clap's exit 2 and do not emit a report.
Startup is bounded by `--startup-timeout-seconds` (default 300, max 3600), each
browser worker by `--case-timeout-seconds` (default 60, max 300). The matrix is
bounded to 64 routes, 16 viewports and 256 cases. Evidence and observations are
bounded; exceeding observation or DOM geometry limits makes a case incomplete.

Ctrl+C/SIGTERM, timeout and normal/error returns stop owned build/browser
process trees and the owned listener. Existing listeners are never adopted or
terminated. Uncatchable OS termination and descendants deliberately escaping
owned process groups are outside this lifecycle contract. A case failure gives
a stage and recovery instruction; correct source/assets/tools and rerun.

Web control and the frame acknowledgment exist only in the explicitly enabled
development test build. A normal Web build removes inherited control flags;
static production output acquires no test hooks. This command does not
publish, deploy, or change hosting/provider settings.

Project/output paths are made absolute lexically, preserving symlink and
mapped-drive spelling without filesystem canonicalization. The output directory
must still be writable. No public preview or separate review command is needed.
