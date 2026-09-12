# Make Fission a framework you can build any UI on, without opening the IR

Closes #224.

## The problem this solves

The closed IR vocabulary is Fission's central bet. Nine targets exist — macOS,
Windows, Linux, Web, Android, iOS, Terminal, static site, SSR — because a
backend implements a finite, known set of operations rather than an open-ended
one. Flutter cannot add a terminal backend; we can, and that is downstream of
the vocabulary being closed.

The complaint behind #224 is real, but it is not a complaint about the
vocabulary. It is that everything *above* the vocabulary was closed too:

- The lowering machinery was `#[doc(hidden)]`, so nobody outside the repo could
  write a widget that composes IR. The vocabulary was closed **and** private.
- Design authority lived in per-widget theme structs. Adding one meant six
  coordinated edits across the codegen, the theme crate, the DSP schema and five
  design-system JSON files — which is why roughly 25 widgets resolved no
  component theme at all and hardcoded their own colours and geometry.
- Accessibility was opt-in per widget, and most widgets had not opted in.
- The box model was physical-only, so RTL was every widget author's problem.
- Keyboard behaviour was a hand-written match over three roles.

None of those required an open vocabulary to fix. This PR fixes them by widening
what you can *compose*, while the set of operations a backend must implement
stays fixed.

The one place the vocabulary itself grew is where a concept genuinely had no
representation: twelve accessibility roles, a key-press trigger, a blend mode,
and two backdrop filters. All appended, all with serde defaults, so serialized
IR from before this change still loads.

## What changed

### 1. A public authoring API

`fission_core::authoring` is new, documented, and covered by semver. It exports
the lowering machinery that first-party widgets use, and nothing else:

```rust
use fission_core::authoring::{IrBuilder, Lower, LoweringCx};

impl Lower for AspectRatio {
    fn lower(&self, cx: &mut LoweringCx) -> WidgetId {
        let id = cx.next_node_id();
        let child = self.child.lower(cx);
        IrBuilder::new(id, Op::Layout(LayoutOp::StyledBox { .. }))
            .child(child)
            .build(cx)
    }
}
```

`fission_core::internal` still exists but is now what its name claims: shell,
renderer and test helpers. Every first-party widget was moved onto the public
API, so the API is used by roughly a hundred call sites in-tree rather than
being a façade nobody exercises.

The traits were renamed as part of this (`LowerWidget` → `Lower`,
`LoweringContext` → `LoweringCx`, etc.), and their documentation rewritten for
an external reader rather than for someone who already knows the codebase.

**Breaking:** the renamed traits, and `internal` no longer re-exporting lowering.

### 2. Generic component recipes

This is the largest change and the one that unblocks the rest.

Before, each themed component had a bespoke struct — `ButtonTheme`,
`MenuTheme`, `SliderTheme` — and adding a new one meant editing the codegen, the
theme crate, the DSP schema, and every design system's JSON. The cost of adding
design authority was high enough that most widgets simply didn't.

Now there is one shape:

```rust
pub struct ComponentRecipe {
    pub base: ResolvedComponentStyle,
    pub parts: BTreeMap<String, ResolvedComponentStyle>,
    pub sizes: BTreeMap<ComponentSize, ResolvedComponentStyle>,
    pub states: ComponentStateStyles,
    pub scalars: BTreeMap<String, f32>,
}
```

reached by name:

```rust
let recipe = theme.recipe("dropdown");
let indicator = recipe.part("indicator");
```

Adding design authority is now two steps: write the recipe in the DSP JSON, read
it in the widget. The codegen enforces completeness — a supplied design system
that omits a required recipe fails the build rather than silently falling back
to hardcoded values.

All five design systems (default, Material 3, Fluent 2, Cupertino, Liquid Glass)
went from 15–18 recipes to **43**. The ~25 widgets that previously hardcoded
colours and geometry now resolve them from the active design system, so
switching design systems actually switches their appearance.

**Breaking:** per-component theme structs are replaced by `ComponentRecipe`.

### 3. Accessibility across every widget

Roughly thirty widgets gained semantics they did not have. Twelve roles were
appended to the IR for concepts that had no representation: `Tree`, `TreeItem`,
`Toolbar`, `RadioGroup`, `Table`, `TableRow`, `TableCell`, `ColumnHeader`,
`ProgressBar`, `Tooltip`, `Status`, `SpinButton`.

Concretely: the data table exposes table/row/cell structure instead of a wall of
generic boxes; toasts, progress indicators and loading states announce
themselves; disclosure controls report expanded state; the date picker trigger
says it opens a popup and whether it is open; close controls have accessible
names.

These reach AccessKit on desktop, web and mobile through the winit shell, and
ARIA on the site and SSR shells, without per-target work.

### 4. The composite keyboard contract is a registry

Arrow navigation, Home/End, typeahead, wrapping and disabled-skipping used to be
a three-arm match. It is now a table:

```rust
static COMPOSITE_CONTRACTS: &[CompositeContract] = &[ .. ];
```

A role gets the full contract by appearing in the table. This is why menus,
tablists, listboxes, radio groups, trees and toolbars all behave consistently
now — they share one implementation rather than three partial ones.

### 5. Declared key actions

The contract above covers keys a *role* implies. It cannot cover a key whose
meaning only the application knows. Previously an app that wanted `Ctrl+K` to
open a command palette had to reach around the framework for raw key events,
losing focus scoping and the shell's key routing with it.

`KeyCode`, the modifier bits and a new `KeyBinding` moved down into `fission-ir`
— a semantic node can now declare a key, so the IR has to be able to name one.
They are still re-exported from `fission_core::event`, so existing code is
unaffected.

```rust
Semantics {
    key_actions: vec![
        KeyAction::with_modifiers(KeyCode::Char('k'), MOD_CTRL, OpenPalette::static_id().as_u128()),
    ],
    ..Default::default()
}
```

The rules, all covered by tests:

- A binding fires when its node **or a descendant** holds focus, so a dialog can
  bind a key for its whole subtree.
- The innermost declaration wins.
- Modifiers match exactly — `Ctrl+K` is not `K`, and not `Ctrl+Shift+K`.
- A declared key beats built-in handling for that key. A node that explicitly
  asked for Enter meant it.
- Disabled nodes ignore their bindings, like every other interaction path.
- **Tab is never overridable.** Focus traversal is a property of the tree, and
  letting one widget claim Tab would let it trap focus for the whole app.

`ActionTrigger::Key` is appended after `Dismiss`, preserving every existing
discriminant.

### 6. A logical box model

`BoxStyle` gained `padding_directional` and `margin_directional` in
`[start, end, top, bottom]` order — the counterpart to Flutter's
`EdgeInsetsDirectional`. Reading order is resolved **once**, during layout,
instead of in each widget:

```rust
resolve_box_style(style, layout_direction)
```

`Positioned` gained `start`/`end`, and `Icon::svg_directional` picks between two
glyphs by reading direction (a back chevron should point the other way in RTL).

Widgets no longer branch on direction. That is what makes RTL correct by
default rather than per-widget.

### 7. GPU information pushed down to the backend

Two things the IR could not express, both of which a GPU backend can do:

`CompositeStyle::blend_mode` — the CSS `mix-blend-mode` set plus additive
`Plus`. The vello backend maps all but `Plus` onto `peniko::Mix`; the site
backend emits `mix-blend-mode`. Backends that cannot blend composite normally
rather than dropping the layer.

`BackdropFilter` gained `Saturate`, `Brightness` and `Chain`. Blur alone
desaturates whatever is behind it, which is why every platform's real glass
material pairs blur with a saturation boost — and Fission ships a Liquid Glass
design system that could not express it. The site backend emits the full CSS
filter list; the software renderer renders the blur and skips the colour
adjustment rather than dropping the surface.

Both are carried all the way through `fission-render`'s `LayerStyle` to the
backend even when the active backend cannot honour them, so the forthcoming
Skia backend needs no pipeline change to pick them up.

**Breaking:** `BackdropFilter` is no longer `Copy` (it now holds a `Vec` in the
`Chain` variant).

### 8. Bug fixes found along the way

- **Text painted two different ways.** `Text` emitted `DrawText` or
  `DrawRichText` depending on an unreachable condition — the font family is
  always `Some` via the theme fallback, so the `DrawText` branch was dead. `Text`
  now always paints through rich text, matching Flutter's single-`Paragraph`
  model. `Op::text()` and `PaintOp::text()` were added so callers stop matching
  a single variant and silently missing the other; every in-tree caller and test
  was migrated.
- **Caret lost across a model transform** in `TextInput`, where a
  `pending_model_transform` branch was unreachable. Selection is now reconciled
  on both the retained and fallback paths.
- **Enter and Space did nothing on most controls.** Keyboard activation built
  its envelope with `and_then` over `payload_data`, so a `Default` action
  carrying no payload produced `None` and was never dispatched — which is most
  actions. Clicking the same control worked. Accepting an editable combobox's
  active option had the identical defect. Found by a test written for the new
  key-action path, which is the point of writing them.
- **The data table painted rows white** regardless of theme.
- **Keycap and scrim colours were hardcoded** rather than themed.

### 9. `CustomRenderObject` no longer has a widget-specific hook

`RangeSlider` recovered its configuration through a `range_slider_config()`
method on the `CustomRenderObject` trait — a first-party widget's name in a
public extension point. It now recovers config from the typed sidecar map via
`RangeSliderRuntimeConfig::from_sidecar`, and the widget participates in hit
testing as `Role::Group` with `draggable`, exactly as the built-in `Slider`
does. The hook and a dead `paint()` method are gone.

No `as_any` was added. The typed-sidecar mechanism (`AnyRenderObject`) already
existed and is what this uses.

### 10. Widgets with no text to fall back on

A button can be named by the text inside it. An image, an icon, a video and a
scroll view cannot — if they do not declare what they are, assistive technology
has nothing to work with. Four cases were silently broken:

- **`Scroll` never declared its axis.** Nothing in the repo set `scrollable_x`
  or `scrollable_y`, so the shells' scroll support — `Action::ScrollDown`,
  `set_scroll_y_max`, the offset reporting — was unreachable code. A screen
  reader user could not scroll a Fission scroll view on any target. `Scroll`
  now emits a semantic region declaring the axis it scrolls, plus an optional
  `semantic_label` for pages with several independent scroll regions. The region
  is `Role::Generic` rather than `Role::Group` — a scroll viewport is not a
  group of related items, and the shells already retain a generic node
  precisely when it declares itself scrollable.
- **`Image::semantic_label` reached the web and nowhere else.** The site shell
  reads it off the paint op to emit `alt`; AccessKit only walks semantic nodes
  and never saw `PaintOp::DrawImage`. The same image was announced in a browser
  and silent on macOS, Windows, Linux, Android and iOS. A labelled image now
  emits `Role::Image` semantics; an unlabelled one still emits none, so
  decorative images stay out of the accessibility tree.
- **`Icon` had no accessible name at all**, where Flutter's `Icon` has
  `semanticLabel`. Added, and deliberately opt-in: an icon inside a labelled
  button is decorative, and naming it would make the control announce twice.
- **`Video` had no semantics.** Unlike an image there is no decorative video, so
  it always emits semantics now — announcing "video" beats announcing nothing.

`Role::Video` was appended for this (discriminant 37), mapped to
`AccessRole::Video` on the winit shell and to a labelled `group` on the site
shell, since ARIA has no video role.

## Performance

Fixing the recipe mechanism turned out to fix a class of stack overflows.

`Theme` was **41,448 bytes** by value, on 2MB thread stacks. `MenuTheme` alone
was 11,224 of them. Several crates — `fission-charts`, `platform_api`,
`fission-shell-server`, `pokemon-card-store` — overflowed the stack in tests
purely from moving a theme around.

Three changes fixed it:

1. Generated recipes are emitted as successive `recipes.insert(..)` statements
   rather than one giant literal, so no enormous temporary is materialised.
2. The generated theme is built as a block binding `tokens`, `components` and
   `design_system` separately.
3. **`Theme.components` is now `Arc<ComponentTheme>`**, and
   `ComponentTheme.recipes` is `Arc<BTreeMap<String, ComponentRecipe>>`.

`Theme` is now roughly **1KB inline**. Cloning a theme — which SSR does once per
request — went from a 40KB memcpy to a refcount bump. The ~140KB of recipe data
exists once per design system rather than once per clone.

`serde`'s `rc` feature was enabled on `fission-theme` for `Arc<T>`
serialization.

## Verification

- Full workspace suite: **316 test binaries, 1959 tests, 0 failures, 0 stack
  overflows.**
- `cargo fmt --check` clean.
- `cargo clippy --workspace --all-targets` clean.

The baseline was established by checking out `main` in a worktree with only the
compile fix applied and diffing the failing sets, because `main` did not build
on its own and an empty failure list would otherwise have looked like success.

## Breaking changes, collected

| Change | Migration |
| --- | --- |
| `LowerWidget` → `Lower`, `LoweringContext` → `LoweringCx` | Rename; import from `fission_core::authoring` |
| `fission_core::internal` no longer re-exports lowering | Import from `fission_core::authoring` |
| Per-component theme structs → `ComponentRecipe` | `theme.recipe("name")`, `.part("name")` |
| `CustomRenderObject::range_slider_config` removed | Use the typed sidecar |
| `BackdropFilter` is no longer `Copy` | `.clone()` where it was copied |
| `Combobox` gained fields | It now derives `Default`; use `..Default::default()` |

Serialized IR compatibility is preserved: every new role, trigger and field is
appended with a serde default, and the discriminant tests assert it.

## What this does not do

It does not open the IR vocabulary. There is no `Custom` arm on `LayoutOp`,
`PaintOp` or `Role`. A widget that wants something new composes it from the
existing vocabulary, which is what keeps a tenth backend a tractable amount of
work.
