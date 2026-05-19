//! # Fission
//!
//! A cross-platform, GPU-accelerated UI framework for Rust.
//!
//! This crate re-exports all Fission sub-crates so applications only need
//! a single dependency:
//!
//! ```toml
//! [dependencies]
//! fission = { path = "..." }
//! ```
//!
//! Then use via:
//! ```rust,ignore
//! use fission::prelude::*;           // Common widget + action types
//! use fission::core::*;              // Low-level runtime/action APIs
//! use fission::widgets::*;           // Authoring widgets (Modal, Popover, etc.)
//! use fission::ir::*;                // Intermediate representation
//! use fission::theme::*;             // Theming
//! use fission::icons::material::*;   // Material icons
//! use fission::shell::DesktopApp;    // Desktop shell
//! use fission::text_engine::*;       // Rope-backed text buffer
//! ```

extern crate self as fission;

// ── Sub-crate re-exports ─────────────────────────────────────────────────

/// Core runtime, widgets, actions, reducers, effects.
pub mod core {
    pub use fission_core::*;
}

/// Intermediate representation (IR) — the node graph between widgets and layout.
pub mod ir {
    pub use fission_ir::*;
}

/// Layout engine — constraint-based layout with Box, Flex, Grid, Scroll, etc.
pub mod layout {
    pub use fission_layout::*;
}

/// Theming — design tokens, component themes, dark/light mode.
pub mod theme {
    pub use fission_theme::*;
}

/// Internationalisation — locale registry, string lookups.
pub mod i18n {
    pub use fission_i18n::*;
}

/// Authoring widgets — Modal, Popover, Tooltip, Menu, Combobox, SplitView, etc.
pub mod widgets {
    pub use fission_widgets::*;
}

/// Derive and attribute macros — `#[fission_action]`, `#[fission_reducer]`, and friends.
pub mod macros {
    pub use fission_core::{reduce, reduce_with, with_reducer};
    pub use fission_macros::*;
}

/// Material Design icons.
pub mod icons {
    pub use fission_icons::*;
}

/// Platform shells — desktop, mobile, and web wrappers over the shared runtime.
pub mod shell {
    #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
    pub use fission_shell_desktop::*;
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub use fission_shell_mobile::*;
    #[cfg(target_arch = "wasm32")]
    pub use fission_shell_web::*;
}

/// Rendering primitives — DisplayList, DisplayOp, TextStyle, Color.
pub mod render {
    pub use fission_render::*;
}

/// Diagnostics system — structured logging, performance tracing.
pub mod diagnostics {
    pub use fission_diagnostics::*;
}

/// Serialization traits and derives used by Fission action macros.
pub use serde;

// text_engine re-export will be added when fission-text-engine is restored

/// Test driver — LiveTestClient, TestCommand, TestResponse.
pub mod test_driver {
    pub use fission_test_driver::*;
}

// ── Flat re-exports for convenience ──────────────────────────────────────

// Core widget types (Button, Text, Container, Row, Column, etc.)
pub use fission_core::ui::*;

// Core action/state types
pub use fission_core::{
    Action, ActionEnvelope, ActionId, ActionScopeId, AnimationPropertyId, AnimationRequest,
    AnimationStartValue, AppState, BuildCtx, EasingFunction, FlexDirection, Handler, NodeBuilder,
    Op, PortalLayer, ReducerContext, Selector, View, Widget, WidgetNodeId,
};

// Core event types
pub use fission_core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
pub use fission_core::{reduce, reduce_with, with_reducer};

// Core env types
pub use fission_core::env::Env;

// IR op types (Color, LayoutOp, PaintOp, etc.)
pub use fission_ir::op;
pub use fission_ir::NodeId;

// Layout types
pub use fission_layout::{LayoutPoint, LayoutRect, LayoutSize, LayoutUnit};

// Authoring widgets (HStack, VStack, Spacer, Icon, etc.)
pub use fission_widgets::{HStack, Icon, Spacer, VStack};

// Platform shells
#[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
pub use fission_shell_desktop::DesktopApp;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub use fission_shell_mobile::MobileApp;
#[cfg(target_arch = "wasm32")]
pub use fission_shell_web::WebApp;

// Macros
pub use fission_macros::{fission_action, fission_reducer, Action as ActionDerive};

// ── Prelude ──────────────────────────────────────────────────────────────

/// Prelude for UI authoring — import this for the most common types.
pub mod prelude {
    // Widgets
    pub use fission_core::ui::*;
    pub use fission_widgets::*;

    // Actions
    pub use fission_core::env::Env;
    pub use fission_core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
    pub use fission_core::op::{Color, Fill, PaintOp};
    pub use fission_core::{reduce, reduce_with, with_reducer};
    pub use fission_core::{
        Action, ActionEnvelope, ActionId, ActionScopeId, AnimationPropertyId, AnimationRequest,
        AnimationStartValue, AppState, BuildCtx, Effects, FlexDirection, Handler, NodeBuilder, Op,
        PortalLayer, ReducerContext, Selector, View, Widget, WidgetNodeId, WindowEnv, WindowTitle,
    };

    // Layout
    pub use fission_layout::{LayoutPoint, LayoutRect, LayoutSize};

    // Design systems and generated themes.
    pub use fission_theme::*;

    // IR
    pub use fission_ir::op as ir_op;
    pub use fission_ir::NodeId;

    // Icons
    pub use fission_icons::material;

    // Macros
    pub use fission_macros::{fission_action, fission_reducer, Action};

    // Shell
    #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
    pub use fission_shell_desktop::DesktopApp;
    #[cfg(target_os = "android")]
    pub use fission_shell_mobile::AndroidApp;
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub use fission_shell_mobile::MobileApp;
    #[cfg(target_arch = "wasm32")]
    pub use fission_shell_web::WebApp;

    // Serde (commonly needed for actions)
    pub use serde::{Deserialize, Serialize};
}
