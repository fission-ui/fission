# fission-widgets

High-level, composable UI widgets for the Fission framework.

This crate provides a comprehensive widget library built on top of `fission-core` primitives. Each widget follows a declarative, data-driven pattern: you construct the widget struct with its configuration, then convert it with `Into<Widget>` to produce the authored widget tree.

## Architecture

Widgets in this crate do not own state. They receive all data (labels, open/closed flags, selected indices) through their struct fields and communicate user interactions back to the application via `ActionEnvelope` callbacks. This is analogous to "controlled components" in React or the state-management pattern in SwiftUI.

Widgets implement `From<Component> for Widget`, so they are state-agnostic at the type level and compose through ordinary `.into()` conversions. When a widget needs runtime inputs such as theme or animation state, it reads the current build context while converting.

## Widget catalog

### Layout

| Widget | Description |
|--------|-------------|
| `HStack` | Horizontal stack. Convenience wrapper around `Row` with a `spacing` parameter. |
| `VStack` | Vertical stack. Convenience wrapper around `Column` with a `spacing` parameter. |
| `Center` | Centers its child using `Align`. |
| `Wrap` | Flow layout that wraps children to the next line. Supports row and column directions. |
| `SplitView` | Resizable split pane. Uses `flex_grow` to divide space by `split_ratio`. |
| `Spacer` | Re-exported from `fission-core`. Flexible space that fills available room. |
| `Divider` | A 1px visual separator line. Supports horizontal and vertical orientations. |

### Overlays and popups

| Widget | Description |
|--------|-------------|
| `Modal` | Dialog with a dimmed backdrop, title bar, content area, and action buttons. Renders into the portal overlay layer at `PortalLayer::Modal`. |
| `Popover` | Anchor-relative popup. Positions content next to a trigger widget using the flyout layout system. |
| `Tooltip` | Hover-activated text label. Appears when the trigger widget is hovered. |
| `Drawer` | Slide-out panel from the left or right edge. Renders as a portal with a dismissible backdrop. |
| `Toast` | Notification message with icon, text, and close button. Supports Info, Success, Warning, and Error kinds. |
| `Portal` | Renders its child into the overlay layer, outside the normal layout tree. |

### Menus and selection

| Widget | Description |
|--------|-------------|
| `Menu` | Vertical list of `MenuItem` entries with optional icons. Rendered inside a scrollable, bordered container. |
| `MenuButton` | Button that toggles a `Menu` popover. |
| `MenuItem` | Single entry in a `Menu`: label, optional icon, and `on_select` action. |
| `Select` | Dropdown selector. Displays the selected label (or placeholder) and opens a `Menu` flyout on click. |
| `DropDown` | Simplified dropdown trigger button. |
| `Combobox` | Searchable dropdown. Combines a `TextInput` with a filterable item list inside a `Popover`. |
| `SegmentedControl` | Horizontal row of toggle buttons. Only one option is active at a time. |

### Navigation

| Widget | Description |
|--------|-------------|
| `Tabs` / `TabItem` | Tab bar with an active indicator and content area that swaps based on `active_index`. |
| `Accordion` / `AccordionItem` | Collapsible sections. Each item has a toggle header and expandable content body. |

### Display

| Widget | Description |
|--------|-------------|
| `Badge` | Small colored label, typically used for counts or status. |
| `Tag` | Pill-shaped label with an optional close button. |
| `Card` | Elevated surface container with rounded corners and a box shadow. |
| `Avatar` | Circular user avatar. Displays an image when `src` is provided, or initials derived from `name`. |
| `EmptyState` | Centered placeholder with icon, title, description, and an optional action button. |
| `Icon` | Re-exported from `fission-core`. Renders an SVG icon from a string. |

### Loading indicators

| Widget | Description |
|--------|-------------|
| `ProgressBar` | Determinate progress bar. `value` ranges from 0.0 to 1.0. |
| `Spinner` | Three-dot animated loading indicator. Each dot pulses with a staggered opacity animation. |
| `Skeleton` | Placeholder shimmer rectangle. Animates opacity between 0.4 and 0.8 in an 800ms loop. |

### Transitions

| Widget | Description |
|--------|-------------|
| `Hero` | Shared-element transition tag. Wraps a child with a `hero_tag` semantic so the framework can animate matched elements across navigation changes. |

### Utilities

| Widget | Description |
|--------|-------------|
| `FormControl` | Wraps a form field with a label, error message, and helper text. Adds a required-field asterisk when `required` is true. |
| `absolute_fill()` | Free function that wraps a child in an `AbsoluteFill` layout node. |

## Usage

```rust
use fission_widgets::{VStack, HStack, Badge, Card, Modal, ModalAction, Tabs, TabItem};

// Stack children vertically with 8px spacing
let layout = VStack {
    spacing: Some(8.0),
    children: vec![
        Badge { text: "New".into(), ..Default::default() }.into(),
        Card { child: content }.into(),
    ],
}.into();

// Open a modal dialog
let dialog = Modal {
    id: WidgetId::explicit("confirm-dialog"),
    title: "Confirm".into(),
    content: Text::new("Are you sure?").into(),
    is_open: true,
    on_dismiss: Some(dismiss_action),
    actions: vec![
        ModalAction { label: "Cancel".into(), on_press: Some(cancel_action), is_primary: false },
        ModalAction { label: "OK".into(), on_press: Some(ok_action), is_primary: true },
    ],
    width: None,
};
```

## Theming

All widgets read colors, spacing, radii, and elevation values from the `Theme` stored in `view.env.theme`. The theme is provided by the `fission-theme` crate and can be switched between light and dark modes at runtime.
