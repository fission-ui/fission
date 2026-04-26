# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-23

### Added

- **Core framework** — Widget-based UI architecture with build/layout/paint pipeline
- **GPU rendering** — Vello + wgpu backend for hardware-accelerated 2D rendering
- **Widget library** — Buttons, text inputs, modals, popovers, menus, tooltips, tabs, accordions, drawers, select, combobox, split view, and more
- **State management** — Deterministic action/reducer architecture with bound-continuation effects system
- **Layout engine** — Constraint-based layout with Box, Flex, Grid, Scroll, ZStack, Positioned, AbsoluteFill
- **Text engine** — Rope-backed text buffer with line index, undo/redo transactions, UTF-8/UTF-16 coordinate mapping
- **Syntax highlighting** — Tree-sitter integration for Rust with cached incremental parsing
- **LSP support** — rust-analyzer integration with diagnostics, completions, and frame-based polling
- **Custom render objects** — Framework escape hatch for complex widgets (editors, charts, 3D) with custom hit-test and event handling
- **Desktop shell** — macOS/Linux/Windows via winit + Vello with GPU screenshot capture
- **Charts** — fission-charts crate with 22 chart types (line, bar, pie, scatter, heatmap, treemap, etc.)
- **3D** — fission-3d crate with basic 3D scene primitives
- **Icon system** — Material Design icons via fission-icons
- **Theming** — Dark/light theme support with design tokens
- **Diagnostics** — Category-based diagnostic system with configurable sinks and sampling
- **Testing** — Headless TestDriver, LiveTestClient with winit event injection, GPU screenshot verification
- **Editor example** — VS Code-style code editor dogfooding the framework: file tree, tabs, terminal, search, git, find/replace, menu bar, command palette, minimap, LSP diagnostics
- **Effects system** — Background thread executor for FileRead, HttpGet with bound continuations
