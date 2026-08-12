# 3.7 Design Fidelity Primitives

Fission exposes high-level typed primitives for translating conventional
HTML/CSS designs without viewport arithmetic, visually styled button hacks, or
wrapper-only spacing nodes.

## Packaged fonts

A Design System Package may declare multiple font faces in `dsp.json`:

```json
{
  "assets": {
    "fonts": [
      {
        "family": "Inter",
        "weight": 400,
        "style": "normal",
        "path": "fonts/Inter-Regular.ttf",
        "format": "truetype"
      },
      {
        "family": "Inter",
        "weight": 700,
        "style": "normal",
        "path": "fonts/Inter-Bold.ttf",
        "format": "truetype"
      }
    ]
  }
}
```

Code generation validates each file, embeds its bytes, and emits rerun hints.
`DesktopApp::with_design_system`, mobile/web hosts, `FissionSite::with_design_system`,
and `TestHarness::new_with_design_system` register the same family, weight,
style, and variation-axis metadata before the first frame. Application-owned
faces can be supplied with the host's `with_fonts` method.

## Typed boxes and responsive layout

`Length` represents points, percentages, viewport units, arithmetic,
min/max/clamp, and intrinsic sizing. `BoxStyle` combines those values with
padding, margin, overflow, alignment, positioning, aspect ratio, and flex/grid
participation. `Container` exposes concise point-based methods and typed
counterparts such as `width_length`, `padding_lengths`, `margin_lengths`, and
`positioned_lengths`.

`Responsive` keeps width-dependent alternatives in one local layout node. Its
cases can query either the viewport or the constraints of the containing box.
`GridTrack` supports `minmax`, `repeat`, `auto_fit`, and `auto_fill`; the same
declarations lower to native layout and CSS grid tracks.

## Neutral interaction surfaces

`Pressable` provides button, link, or menu-item semantics without implicit
padding, fill, border, elevation, or minimum size. Its base, hover, pressed,
focused, and disabled styles are partial overlays:

```rust,ignore
use fission::{op, Pressable, PressableRole, PressableStyle, Text};
use fission::motion::{MotionEasing, MotionTransition};

let card = Pressable::new(Text::new("24 Dahlia Road"))
    .label("Open property")
    .semantics_identifier("property.open")
    .role(PressableRole::Link)
    .on_press(open_property)
    .style(PressableStyle {
    background: Some(op::Fill::Solid(surface)),
    corner_radius: Some(16.0),
    padding: Some(op::Length::all(op::Length::points(16.0))),
    ..Default::default()
})
.hover(PressableStyle {
    background: Some(op::Fill::Solid(surface_hover)),
    scale: Some(1.01),
    ..Default::default()
})
.pressed(PressableStyle {
    scale: Some(0.985),
    ..Default::default()
})
.transition(MotionTransition::tween(160, MotionEasing::EaseOut));
```

Solid background and border colours, border width, corner radius, point-based
padding, opacity, and scale interpolate automatically. Gradients and ordered
shadow lists switch at the state boundary. Ripple feedback is opt-in.

## Shadows and backdrop filters

`BoxShadow` preserves offset, blur, spread, and inset through Core IR and the
display list. `Container::shadows` paints multiple layers in declaration order.
The Vello, Skia, reference-software, and static-site renderers preserve shadow
geometry. Production native software profiles rasterize through Skia; the
reference renderer remains available only for comparison and conformance.

`Container::backdrop_blur` emits a rounded, clipped backdrop-filter operation.
Static sites use CSS backdrop filters and Skia raster performs the production
software filter directly. GPU compositor execution is tracked separately;
until that pass is available, the Vello renderer preserves the operation but
does not apply the framebuffer blur.
