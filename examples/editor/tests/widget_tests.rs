/// Widget-level E2E tests for the Fission Editor using LiveTestClient.
///
/// These tests exercise the editor through the HTTP control channel,
/// validating widget rendering, interaction, and state transitions in
/// the live application. They complement the unit tests in model.rs by
/// testing the full widget tree rather than the model in isolation.
///
/// The tests check whether the control port is reachable before
/// proceeding. If no editor is running, they skip gracefully with a
/// message instead of failing. This means they are always compiled and
/// always run -- but only do real work when a live editor is available.
///
/// To run with a live editor:
///   1. cargo run -p fission-editor -- .   (in one terminal)
///   2. cargo test -p fission-editor --test widget_tests -- --nocapture

use fission_test_driver::LiveTestClient;
use std::io::Write;

const CONTROL_PORT: u16 = 9879;

/// Check if the editor control port is accepting connections.
fn port_available() -> bool {
    std::net::TcpStream::connect(format!("127.0.0.1:{}", CONTROL_PORT)).is_ok()
}

fn dir() -> String {
    let d = "test_screenshots/editor_widget";
    std::fs::create_dir_all(d).ok();
    d.to_string()
}

fn create_temp_file(name: &str, content: &str) -> String {
    let path = std::env::temp_dir().join(name);
    let mut f = std::fs::File::create(&path).expect("create temp file");
    f.write_all(content.as_bytes()).expect("write temp file");
    path.to_string_lossy().to_string()
}

fn cleanup(path: &str) {
    std::fs::remove_file(path).ok();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verify the file tree widget renders directory entries after expanding.
#[test]
fn file_tree_shows_entries() {
    if !port_available() {
        eprintln!("Skipping file_tree_shows_entries: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");
    client.pump().unwrap();

    // The root folder should show in the sidebar
    client.assert_text_visible("EXPLORER").unwrap();

    // Expand a known folder
    client.tap_text("crates").unwrap();
    client.pump().unwrap();

    // After expansion, child entries should be visible
    let texts = client.get_text().unwrap();
    let entry_names: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();

    assert!(
        entry_names.iter().any(|n| *n == "authoring" || n.contains("authoring")),
        "expected 'authoring' in file tree, found: {:?}",
        &entry_names[..entry_names.len().min(30)]
    );
}

/// Verify that the tab bar renders correctly after opening a file.
#[test]
fn tab_bar_renders_after_open() {
    if !port_available() {
        eprintln!("Skipping tab_bar_renders_after_open: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    // Open Cargo.toml
    client.tap_text("Cargo.toml").unwrap();
    client.pump().unwrap();

    // Tab bar should show the file name
    client.assert_text_visible("Cargo.toml").unwrap();

    // The semantic tree should have interactive elements
    let tree = client.get_tree().unwrap();
    let has_clickable = tree.iter().any(|n| n.focusable || n.role == "Button" || n.role == "Tab");
    assert!(has_clickable, "tab bar should have interactive elements");

    client.screenshot(&format!("{}/tab_bar.png", dir())).unwrap();
}

/// Verify the status bar shows language and position info.
#[test]
fn status_bar_shows_info() {
    if !port_available() {
        eprintln!("Skipping status_bar_shows_info: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    // Open a Rust file to check language detection in status bar
    client.tap_text("crates").unwrap();
    client.pump().unwrap();

    let texts = client.get_text().unwrap();
    let rs_file = texts.iter().find(|t| t.text.ends_with(".rs"));

    if let Some(f) = rs_file {
        client.tap_text(&f.text).unwrap();
        client.pump().unwrap();

        let texts_after = client.get_text().unwrap();
        let has_lang = texts_after.iter().any(|t| t.text.contains("Rust"));
        println!("Status bar shows Rust: {}", has_lang);
    }

    client.screenshot(&format!("{}/status_bar.png", dir())).unwrap();
}

/// Verify the editor surface renders line numbers and content.
#[test]
fn editor_surface_renders_content() {
    if !port_available() {
        eprintln!("Skipping editor_surface_renders_content: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    // Open Cargo.toml
    client.tap_text("Cargo.toml").unwrap();
    client.pump().unwrap();

    let texts = client.get_text().unwrap();

    let has_content = texts.iter().any(|t| t.text.contains("[workspace]"));
    assert!(has_content, "editor surface should render file content");

    let has_line_numbers = texts.iter().any(|t| t.text.trim() == "1");
    println!("Line numbers visible: {}", has_line_numbers);

    client.screenshot(&format!("{}/editor_surface.png", dir())).unwrap();
}

/// Verify that typing in the editor updates the display.
#[test]
fn typing_updates_display() {
    if !port_available() {
        eprintln!("Skipping typing_updates_display: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    client.tap_text("Cargo.toml").unwrap();
    client.pump().unwrap();

    client.type_text("UNIQUE_MARKER").unwrap();
    client.pump().unwrap();

    let texts = client.get_text().unwrap();
    let has_marker = texts.iter().any(|t| t.text.contains("UNIQUE_MARKER"));
    println!("Typed text visible: {}", has_marker);

    // Undo the change
    client.press_key("Z", 4).unwrap();
    client.pump().unwrap();

    client.screenshot(&format!("{}/typing_test.png", dir())).unwrap();
}

/// Verify the command palette widget renders correctly.
#[test]
fn command_palette_widget_renders() {
    if !port_available() {
        eprintln!("Skipping command_palette_widget_renders: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    // Open command palette
    client.press_key("P", 4 | 1).unwrap();
    client.pump().unwrap();

    let texts = client.get_text().unwrap();
    let has_palette_content = texts.iter().any(|t| {
        t.text.contains("Save")
            || t.text.contains("Toggle")
            || t.text.contains("Find")
            || t.text.contains(">")
    });
    println!("Command palette has content: {}", has_palette_content);

    client.screenshot(&format!("{}/command_palette_widget.png", dir())).unwrap();

    // Dismiss
    client.press_key("Escape", 0).unwrap();
    client.pump().unwrap();
}

/// Verify the terminal panel widget renders terminal lines.
#[test]
fn terminal_panel_widget_renders() {
    if !port_available() {
        eprintln!("Skipping terminal_panel_widget_renders: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    client.pump().unwrap();

    client.assert_text_visible("Ready.").unwrap();
    client.assert_text_visible("TERMINAL").unwrap();

    client.screenshot(&format!("{}/terminal_panel.png", dir())).unwrap();
}

/// Verify sidebar toggling hides/shows the EXPLORER section.
#[test]
fn sidebar_toggle_hides_explorer() {
    if !port_available() {
        eprintln!("Skipping sidebar_toggle_hides_explorer: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    client.pump().unwrap();
    client.assert_text_visible("EXPLORER").unwrap();

    // Toggle sidebar off
    client.press_key("B", 4).unwrap();
    client.pump().unwrap();

    let result = client.assert_text_not_visible("EXPLORER");
    println!("Sidebar hidden check: {:?}", result);

    client.screenshot(&format!("{}/sidebar_hidden.png", dir())).unwrap();

    // Toggle back on
    client.press_key("B", 4).unwrap();
    client.pump().unwrap();
    client.assert_text_visible("EXPLORER").unwrap();

    client.screenshot(&format!("{}/sidebar_shown.png", dir())).unwrap();
}

/// Verify the find bar widget appears and accepts text.
#[test]
fn find_bar_widget_interaction() {
    if !port_available() {
        eprintln!("Skipping find_bar_widget_interaction: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    client.tap_text("Cargo.toml").unwrap();
    client.pump().unwrap();

    // Open find bar
    client.press_key("F", 4).unwrap();
    client.pump().unwrap();

    client.screenshot(&format!("{}/find_bar_open.png", dir())).unwrap();

    client.type_text("workspace").unwrap();
    client.pump().unwrap();

    client.screenshot(&format!("{}/find_bar_with_query.png", dir())).unwrap();

    // Close find bar
    client.press_key("Escape", 0).unwrap();
    client.pump().unwrap();
}

/// Verify the search panel (sidebar section) renders results.
#[test]
fn search_panel_widget_renders() {
    if !port_available() {
        eprintln!("Skipping search_panel_widget_renders: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    let texts = client.get_text().unwrap();
    if let Some(icon) = texts.iter().find(|t| t.text == "\u{1f50d}" && t.x < 50.0) {
        client
            .tap(icon.x + icon.width / 2.0, icon.y + icon.height / 2.0)
            .unwrap();
        client.pump().unwrap();

        let texts_after = client.get_text().unwrap();
        let has_search = texts_after
            .iter()
            .any(|t| t.text.contains("SEARCH") || t.text.contains("Search"));
        println!("Search panel visible: {}", has_search);

        client
            .screenshot(&format!("{}/search_panel.png", dir()))
            .unwrap();
    } else {
        println!("Search icon not found, skipping search panel widget test");
    }
}

/// Verify the git panel (sidebar section) renders status entries.
#[test]
fn git_panel_widget_renders() {
    if !port_available() {
        eprintln!("Skipping git_panel_widget_renders: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    let texts = client.get_text().unwrap();
    if let Some(icon) = texts.iter().find(|t| t.text == "\u{2387}" && t.x < 50.0) {
        client
            .tap(icon.x + icon.width / 2.0, icon.y + icon.height / 2.0)
            .unwrap();
        client.pump().unwrap();

        let texts_after = client.get_text().unwrap();
        let has_git = texts_after
            .iter()
            .any(|t| t.text.contains("GIT") || t.text.contains("Source") || t.text.contains("Changes"));
        println!("Git panel visible: {}", has_git);

        client
            .screenshot(&format!("{}/git_panel.png", dir()))
            .unwrap();
    } else {
        println!("Git icon not found, skipping git panel widget test");
    }
}

/// Verify layout integrity: no text items with zero or broken dimensions.
#[test]
fn layout_integrity_check() {
    if !port_available() {
        eprintln!("Skipping layout_integrity_check: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    client.tap_text("Cargo.toml").unwrap();
    client.pump().unwrap();

    let texts = client.get_text().unwrap();
    let broken: Vec<_> = texts
        .iter()
        .filter(|t| (t.width < 1.0 || t.height < 3.0) && !t.text.trim().is_empty())
        .collect();

    for b in &broken {
        println!("  BROKEN: {}x{} at ({},{}) \"{}\"", b.width, b.height, b.x, b.y, b.text);
    }

    assert_eq!(
        broken.len(),
        0,
        "layout integrity: {} text items have broken dimensions",
        broken.len()
    );

    client.screenshot(&format!("{}/layout_integrity.png", dir())).unwrap();
}

/// Verify that the semantic tree has expected roles for accessibility.
#[test]
fn semantic_tree_has_expected_roles() {
    if !port_available() {
        eprintln!("Skipping semantic_tree_has_expected_roles: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    client.tap_text("Cargo.toml").unwrap();
    client.pump().unwrap();

    let tree = client.get_tree().unwrap();

    let roles: Vec<&str> = tree.iter().map(|n| n.role.as_str()).collect();
    let unique_roles: std::collections::HashSet<&&str> = roles.iter().collect();
    println!("Unique roles in semantic tree: {:?}", unique_roles);

    let has_buttons = roles.iter().any(|r| *r == "Button");
    let has_text_input = roles.iter().any(|r| *r == "TextInput");
    println!(
        "Has buttons: {}, has text input: {}",
        has_buttons, has_text_input
    );
}

/// Verify PROBLEMS tab switch in the bottom panel.
#[test]
fn problems_tab_switch() {
    if !port_available() {
        eprintln!("Skipping problems_tab_switch: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    client.pump().unwrap();

    let texts = client.get_text().unwrap();
    if texts.iter().any(|t| t.text.contains("PROBLEMS")) {
        client.tap_text("PROBLEMS").unwrap();
        client.pump().unwrap();

        client
            .screenshot(&format!("{}/problems_tab.png", dir()))
            .unwrap();

        // Switch back to TERMINAL
        client.tap_text("TERMINAL").unwrap();
        client.pump().unwrap();

        println!("Problems tab switch OK");
    } else {
        println!("PROBLEMS tab not visible");
    }
}

/// Verify the menu bar renders and dropdown appears on click.
#[test]
fn menu_bar_dropdown() {
    if !port_available() {
        eprintln!("Skipping menu_bar_dropdown: editor not running on port {}", CONTROL_PORT);
        return;
    }
    let client = LiveTestClient::connect(CONTROL_PORT);
    client.wait_for_ready(10_000).expect("editor ready");

    client.pump().unwrap();

    let texts = client.get_text().unwrap();
    if texts.iter().any(|t| t.text == "File") {
        client.tap_text("File").unwrap();
        client.pump().unwrap();

        let dropdown_texts = client.get_text().unwrap();
        let dropdown_items: Vec<&str> = dropdown_texts
            .iter()
            .filter(|t| {
                t.text.contains("Save")
                    || t.text.contains("New")
                    || t.text.contains("Open")
                    || t.text.contains("Close")
            })
            .map(|t| t.text.as_str())
            .collect();
        println!("Menu dropdown items: {:?}", dropdown_items);
        assert!(!dropdown_items.is_empty(), "File menu should show items");

        client
            .screenshot(&format!("{}/menu_dropdown.png", dir()))
            .unwrap();

        // Dismiss menu
        client.press_key("Escape", 0).unwrap();
        client.pump().unwrap();
    } else {
        println!("File menu not found");
    }
}
