# RFC: Terminal Shell Target

Status: proposal  
Audience: Fission runtime, renderer, shell, compiler, and widget authors  
Scope: rendering and interacting with a Fission application inside a terminal session

## 1. Summary

Fission should support a terminal shell target that runs a Fission application as a real terminal UI. The target must not open an operating-system window. It should lower the same Fission app model into a terminal cell buffer, render it with terminal control sequences, and map terminal keyboard, mouse, resize, focus, and scroll events back into normal Fission input events.

The target is not a visual clone of desktop, web, Android, or iOS output. A terminal is a character-cell environment with limited color, no pixel-accurate painting, and inconsistent support across terminal emulators. The terminal target is therefore a first-class platform with its own renderer, verifier, and acceptance rules.

Unsupported widgets or render operations must fail during the terminal target build wherever they can be detected statically. This must be based on the lowered Fission Core IR and Semantics model, not on a hand-maintained list of widget names.

## 2. Goals

- Add a terminal platform target that can be selected with normal Fission CLI commands.
- Render Fission UI into a deterministic terminal cell buffer.
- Support keyboard-first interaction, focus traversal, selection, text input, scroll, dialogs, menus, buttons, forms, tables, lists, and progress indicators.
- Fail the terminal build for unsupported lowered operations in declared routes/screens/states.
- Determine supportability from Core IR operations plus Semantics, not from widget-type allowlists.
- Preserve normal Fission app structure, reducers, actions, commands, jobs, services, and environment handling.
- Reuse Fission semantics and test infrastructure so terminal apps can be tested headlessly.
- Provide precise diagnostics with source provenance when unsupported UI is used.

## 3. Non-goals

- Pixel-perfect parity with graphical shells.
- Rendering arbitrary images, video, web views, 3D scenes, shaders, blur, or arbitrary vector graphics directly in the terminal.
- Replacing graphical shells.
- Forcing every public widget to declare terminal support manually.
- Making terminal support depend on a hosted service.
- Hiding unsupported UI behind runtime no-op fallbacks.

## 4. Command model

```text
fission add-target terminal --project-dir .
fission doctor terminal --project-dir .
fission build --target terminal --project-dir .
fission run --target terminal --project-dir .
fission test --target terminal --project-dir .
```

`fission build --target terminal` must run the terminal target verifier after app lowering and before producing the terminal executable/package. Verifier failure is a build failure.

## 5. Crate layout

```text
crates/shell/fission-shell-terminal
crates/render/fission-render-terminal
crates/platform/fission-platform-terminal
```

Responsibilities:

- `fission-render-terminal`: converts supported Core IR/display-list operations into terminal cells and style spans.
- `fission-shell-terminal`: owns terminal lifecycle, alternate screen mode, input event collection, resize handling, panic restoration, and app runtime integration.
- `fission-platform-terminal`: exposes target detection, terminal capability probing, and CLI integration.

The renderer must not depend on authoring widget names. It consumes lowered Core IR, layout snapshots, display lists, and semantics.

## 6. Terminal rendering model

The terminal renderer outputs a double-buffered grid of cells.

```rust
struct TerminalFrame {
    width_cols: u16,
    height_rows: u16,
    cells: Vec<TerminalCell>,
    cursor: Option<TerminalCursor>,
    focused_node: Option<NodeId>,
}

struct TerminalCell {
    grapheme: SmallString,
    style: TerminalStyle,
    node: Option<NodeId>,
}
```

A `TerminalCell` represents one terminal column position. A rendered grapheme may occupy one or more columns. The renderer must use Unicode grapheme segmentation for user-perceived characters and terminal width calculation that accounts for East Asian width and emoji/combining cases. Unicode Standard Annex #29 defines grapheme segmentation, and Unicode Standard Annex #11 defines East Asian Width as an input into fixed-width text handling [T2][T3].

The output backend should apply diffs between the previous frame and the new frame rather than repainting the whole terminal every tick. ECMA-48 defines common terminal control functions and escape/control sequences used by terminal emulators [T1]. The Fission shell should use a Rust terminal control abstraction internally, but this RFC does not mandate a specific crate.

## 7. Coordinate and layout rules

Fission layout remains logical and device-independent until terminal finalization. The terminal renderer converts layout units into terminal cells at the final rendering boundary.

Rules:

- logical x/y/width/height are quantized to columns and rows;
- text wrapping occurs on grapheme boundaries;
- focus, hit testing, and scroll bounds use the quantized cell grid;
- borders snap to cell lines;
- sub-cell transforms are unsupported;
- fractional positions must use deterministic rounding;
- overflow must clip or scroll deterministically;
- resize invalidates layout and repaints the frame.

The terminal target must expose a `TerminalViewport` to the runtime:

```rust
struct TerminalViewport {
    cols: u16,
    rows: u16,
    color_mode: TerminalColorMode,
    mouse_mode: TerminalMouseMode,
    unicode_mode: TerminalUnicodeMode,
}
```

## 8. Semantics-driven support detection

Terminal support must be determined from the lowered app, not from widget names.

The verifier receives:

```text
Core tree
Layout snapshot
Display list
Semantics tree
Source provenance map
Declared terminal target profile
```

A node is terminal-lowerable when at least one of the following is true:

1. Its visual operations are directly supported by the terminal renderer.
2. Its Semantics role and value define a complete terminal representation.
3. It provides a semantics fallback that can be rendered as text, table, list, form control, progress, or placeholder.

A node is not terminal-lowerable when it depends on unsupported paint/embed/media/spatial operations and does not expose a complete semantic fallback.

### 8.1 Required Semantics extensions

Fission should extend Semantics with target-independent representation fields instead of adding terminal-specific support flags to every widget.

```rust
struct Semantics {
    role: SemanticsRole,
    label: Option<LocalizedText>,
    description: Option<LocalizedText>,
    value: Option<SemanticValue>,
    actions: SemanticActions,
    focus: FocusSemantics,
    input: Option<InputSemantics>,
    scroll: Option<ScrollSemantics>,
    range: Option<RangeSemantics>,
    table: Option<TableSemantics>,
    alternate: Option<SemanticAlternate>,
}

struct SemanticAlternate {
    text: Option<LocalizedText>,
    rows: Option<Vec<Vec<LocalizedText>>>,
    placeholder: Option<LocalizedText>,
    reason: Option<LocalizedText>,
}
```

This is not a terminal marker. It is a general semantic fallback that can also improve accessibility, tests, snapshots, low-bandwidth renderers, and debugging. The terminal target uses it to decide whether an otherwise visual node can be represented honestly in a cell UI.

### 8.2 Supported semantic roles

The terminal target should initially support these roles:

- text
- heading
- paragraph
- link
- button
- checkbox
- radio
- switch
- text field
- multiline text field
- password field
- select/listbox
- menu/menu item
- tab/tab panel
- dialog
- alert
- progress bar
- slider/range
- table/grid
- tree
- list/list item
- scroll region
- separator
- status

The role list belongs to Semantics/Core, not to individual widgets. Any widget that lowers to one of these semantic structures can be supported without being named by the terminal renderer.

### 8.3 Supported visual operations

Initial terminal visual support:

- text runs;
- foreground/background color mapped to the active terminal color profile;
- simple solid fills as cell backgrounds;
- borders using box-drawing characters where available;
- padding, margin, clipping, and scroll regions;
- focus indicators;
- selection highlights;
- progress/range visualizations;
- tables and lists;
- simple icons when represented by text, Unicode symbols, or configured ASCII fallbacks.

Unsupported without fallback:

- arbitrary vector paths;
- raster images;
- video;
- audio controls without semantic controls;
- web views;
- 3D scenes;
- shaders;
- complex transforms;
- blur and backdrop filters;
- pixel-dependent canvas/custom paint;
- drag/drop interactions that cannot be expressed by terminal input.

## 9. Build-time verification

The terminal target verifier must run during `fission build --target terminal` and `fission test --target terminal`.

```text
app source
  -> Rust compile
  -> Fission app lowering for declared surfaces
  -> Core IR/display-list/semantics
  -> terminal verifier
  -> terminal executable/package
```

Verifier failures must be reported as build errors:

```text
error[terminal.unsupported_op]: image node cannot be lowered to terminal UI
  --> src/screens/profile.rs:42:13
   |
42 |             Avatar::new(user.photo)
   |             ^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = reason: lowered node emits ImagePaint, which is not supported by the terminal renderer
   = fix: provide a semantic alternate such as initials, display name, or placeholder text
```

The verifier must not use `contains`, suffix matching, widget name tables, or manually maintained widget support maps. It must inspect the exact lowered operations and semantics.

## 10. Declared surface coverage

A Fission application can produce different UI for different routes, feature flags, user state, data state, platform state, and permissions. No static verifier can prove support for states that are never lowered.

Therefore terminal support requires declared surface coverage:

```toml
[terminal]
default_route = "/"
strict = true

[[terminal.surfaces]]
id = "home"
route = "/"
state = "tests/states/home.toml"

[[terminal.surfaces]]
id = "settings"
route = "/settings"
state = "tests/states/settings.toml"
```

Rules:

- In `strict = true`, the terminal build fails unless every configured route/surface has at least one verification state.
- If the app has a route registry, every route must be covered or explicitly excluded with a reason.
- If runtime state later emits unsupported IR not covered by declared states, the terminal runtime must fail closed with `terminal.unsupported_runtime_node`, record provenance, and render a fatal diagnostic screen.
- CI must run `fission test --target terminal` over declared states.

This is the practical way to satisfy build-time failure while acknowledging that arbitrary runtime state cannot be exhaustively proven without a declared state space.

## 11. Input mapping

Terminal input maps to normal Fission input events.

| Terminal input | Fission event |
| --- | --- |
| character input | text input / IME-lite text edit |
| Enter/Space | activate focused control |
| Tab/Shift-Tab | focus next/previous |
| arrows | directional navigation, caret movement, list selection, or scroll depending on focus role |
| Escape | cancel/close dialog/menu or propagate command |
| Ctrl/Alt shortcuts | command accelerators |
| mouse click | pointer down/up/click where terminal mouse reporting exists |
| mouse drag | selection, scroll thumb, or drag action if representable |
| mouse wheel | scroll event |
| resize | viewport resize and layout invalidation |

The terminal target must remain keyboard-complete. Any action available only through mouse input is a terminal target error unless it has an equivalent keyboard action in Semantics.

## 12. Focus and accessibility

The terminal shell must use the Semantics tree as its focus model. Focus order must be deterministic. Focusable controls must expose labels and actions. A terminal UI with unlabeled interactive controls is invalid.

Required checks:

- every focusable node has a role;
- every focusable node has a label unless the role/value makes it self-evident;
- every actionable node exposes at least one semantic action;
- every text input exposes edit semantics;
- every scrollable region exposes scroll semantics;
- every progress/range control exposes min/max/current where applicable.

These checks should also strengthen graphical accessibility, not only terminal support.

## 13. Styling model

Terminal styling is a projection of Fission style into terminal capabilities.

Supported style channels:

- foreground color;
- background color;
- bold;
- italic where supported;
- underline;
- dim;
- reverse;
- selected/focused state;
- disabled state;
- error state.

The renderer must support multiple terminal profiles:

```rust
enum TerminalColorMode {
    Mono,
    Ansi16,
    Ansi256,
    TrueColor,
}
```

The target should prefer design-system tokens, then map them through the active terminal profile. It must not assume truecolor is available.

## 14. Text editing

Terminal text editing is required for forms and developer tools.

Minimum support:

- single-line text fields;
- multiline text areas;
- password masking;
- cursor movement by grapheme;
- backspace/delete by grapheme;
- selection where terminal support permits it;
- clipboard integration where platform APIs permit it;
- validation messages;
- submit/cancel actions.

Full platform IME parity is not required in the first implementation, but text handling must be Unicode-safe at the grapheme level.

## 15. Testing

Required test layers:

- unit tests for cell buffer diffing;
- unit tests for Unicode width/grapheme handling;
- verifier tests for supported and unsupported Core IR operations;
- verifier tests for semantic fallbacks;
- golden tests for terminal frames;
- input simulation tests for keyboard, mouse, scroll, and resize;
- smoke tests that run sample terminal apps in a pseudo-terminal;
- CI checks that strict terminal apps fail when unsupported nodes are introduced.

Golden tests must store semantic/cell snapshots, not screenshots.

## 16. Diagnostics

Terminal verifier diagnostics must include:

- stable error ID;
- source span/provenance;
- node ID;
- unsupported Core IR operation or missing semantic field;
- target profile that rejected it;
- suggested semantic fallback or platform exclusion;
- route/surface/state that produced it.

Example IDs:

```text
terminal.unsupported_op
terminal.missing_semantic_role
terminal.missing_semantic_action
terminal.missing_text_fallback
terminal.pixel_only_custom_paint
terminal.surface_not_declared
terminal.unsupported_runtime_node
```

## 17. Acceptance criteria

The terminal shell target is accepted when:

- `fission add-target terminal` creates required target configuration without modifying app logic.
- `fission build --target terminal` produces a runnable terminal app for supported surfaces.
- unsupported lowered operations fail the terminal build for declared surfaces.
- support detection uses Core IR and Semantics, not widget-name tables.
- text, buttons, forms, lists, tables, dialogs, menus, scroll regions, and progress controls are interactive.
- keyboard-only usage covers every action.
- terminal frame golden tests are deterministic.
- runtime unsupported nodes fail closed with provenance instead of silently disappearing.
- graphical shells remain unaffected by terminal-only verifier rules.

## References

[T1] ECMA-48, Control Functions for Coded Character Sets: https://ecma-international.org/publications-and-standards/standards/ecma-48/  
[T2] Unicode Standard Annex #29, Unicode Text Segmentation: https://www.unicode.org/reports/tr29/  
[T3] Unicode Standard Annex #11, East Asian Width: https://www.unicode.org/reports/tr11/
