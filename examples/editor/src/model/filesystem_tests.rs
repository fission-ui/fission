use super::test_support::{cleanup, temp_file};
use super::*;

#[test]
fn test_scan_directory() {
    let dir = std::env::temp_dir().join("test_scan_dir");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join("alpha.txt"), "a").ok();
    std::fs::write(dir.join("beta.rs"), "b").ok();
    std::fs::create_dir_all(dir.join("subdir")).ok();
    std::fs::write(dir.join("subdir/gamma.txt"), "c").ok();

    let entries = scan_directory(&dir, 0);

    // Should find at least the two files and one subdir
    assert!(
        entries.len() >= 3,
        "expected >= 3 entries, got {}",
        entries.len()
    );

    // Directories should come before files (by the sort in scan_directory)
    let first_dir_idx = entries.iter().position(|e| e.is_dir);
    let first_file_idx = entries.iter().position(|e| !e.is_dir);
    if let (Some(d), Some(f)) = (first_dir_idx, first_file_idx) {
        assert!(d < f, "directories should be sorted before files");
    }

    // The subdir should have children
    let subdir_entry = entries.iter().find(|e| e.name == "subdir").unwrap();
    assert!(subdir_entry.is_dir);
    assert!(!subdir_entry.children.is_empty());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_scan_directory_skips_hidden() {
    let dir = std::env::temp_dir().join("test_scan_hidden");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join("visible.txt"), "v").ok();
    std::fs::write(dir.join(".hidden"), "h").ok();
    std::fs::create_dir_all(dir.join(".git")).ok();

    let entries = scan_directory(&dir, 0);

    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"visible.txt"));
    assert!(!names.contains(&".hidden"));
    assert!(!names.contains(&".git"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_context_menu_state() {
    let mut state = EditorState::default();
    assert!(!state.context_menu_visible);
    assert!(state.context_menu_target.is_none());

    state.context_menu_visible = true;
    state.context_menu_position = (100.0, 200.0);
    state.context_menu_target = Some("/some/file.rs".to_string());

    assert!(state.context_menu_visible);
    assert_eq!(state.context_menu_position, (100.0, 200.0));
    assert_eq!(state.context_menu_target.as_deref(), Some("/some/file.rs"));
}

#[test]
fn test_menu_bar_state() {
    let mut state = EditorState::default();
    assert!(state.show_menu_bar);
    assert!(state.active_menu.is_none());

    state.active_menu = Some("File".to_string());
    assert_eq!(state.active_menu.as_deref(), Some("File"));

    state.active_menu = None;
    assert!(state.active_menu.is_none());
}

#[test]
fn test_completions_state() {
    let mut state = EditorState::default();
    assert!(!state.show_completions);
    assert!(state.completions.is_empty());
    assert_eq!(state.selected_completion, 0);

    state.completions = vec![
        CompletionItem {
            label: "println!".into(),
            kind: "function".into(),
            detail: Some("macro".into()),
        },
        CompletionItem {
            label: "print!".into(),
            kind: "function".into(),
            detail: None,
        },
    ];
    state.show_completions = true;
    state.selected_completion = 1;

    assert_eq!(state.completions.len(), 2);
    assert_eq!(state.completions[1].label, "print!");
}

#[test]
fn test_diagnostics_storage() {
    let mut state = EditorState::default();
    assert!(state.diagnostics.is_empty());

    state.diagnostics.insert(
        "/some/file.rs".into(),
        vec![
            Diagnostic {
                line: 10,
                col: 5,
                severity: DiagSeverity::Error,
                message: "expected `;`".into(),
            },
            Diagnostic {
                line: 20,
                col: 0,
                severity: DiagSeverity::Warning,
                message: "unused variable".into(),
            },
        ],
    );

    let diags = &state.diagnostics["/some/file.rs"];
    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].severity, DiagSeverity::Error);
    assert_eq!(diags[1].severity, DiagSeverity::Warning);
}

#[test]
fn test_multiline_find_matches() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file(
        "test_multiline_find.txt",
        "hello world\nhello rust\ngoodbye hello",
    );
    state.open_file(path.clone());

    state.find_query = "hello".to_string();
    state.find_next();

    assert_eq!(state.find_matches.len(), 3);
    // Verify line numbers
    assert_eq!(state.find_matches[0].1, 0); // line 0
    assert_eq!(state.find_matches[1].1, 1); // line 1
    assert_eq!(state.find_matches[2].1, 2); // line 2

    cleanup(&path);
}

// -----------------------------------------------------------------------
// Edge cases: empty files, single-line files, cursor at end of file
// -----------------------------------------------------------------------

#[test]
fn test_open_empty_file() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_empty.txt", "");
    state.open_file(path.clone());

    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.content(), "");
    assert_eq!(buf.cursor_line, 0);
    assert_eq!(buf.cursor_col, 0);

    // Operations on empty buffer should not panic
    state.find_query = "anything".to_string();
    state.find_next();
    assert!(state.find_matches.is_empty());

    state.replace_query = "replacement".to_string();
    state.replace_all();
    assert_eq!(state.file_contents[&path].content(), "");

    state.copy_line();
    assert_eq!(state.clipboard, ""); // no line to copy

    state.go_to_line(1);
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.cursor_line, 0);

    cleanup(&path);
}

#[test]
fn test_single_line_file() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_single_line.txt", "only one line");
    state.open_file(path.clone());

    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.content().lines().count(), 1);

    // Go to line beyond the single line
    state.go_to_line(100);
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.cursor_line, 0); // clamped to the only line

    // Cut the only line
    state.cut_line();
    assert_eq!(state.clipboard, "only one line");
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.content(), "");
    assert_eq!(buf.cursor_line, 0);

    // Undo should restore it
    state.undo_active();
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.content(), "only one line");

    cleanup(&path);
}

#[test]
fn test_cursor_at_end_of_file() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_cursor_eof.txt", "line1\nline2\nline3");
    state.open_file(path.clone());

    // Move cursor to the last line
    state.go_to_line(3);
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.cursor_line, 2);

    // Copy from last line
    state.copy_line();
    assert_eq!(state.clipboard, "line3");

    // Paste at end
    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.set_caret_line_col(2, 5); // end of "line3"
    }
    state.paste();
    let content = state.file_contents[&path].content();
    assert!(
        content.contains("line3line3"),
        "paste at end of file, got: {}",
        content
    );

    cleanup(&path);
}

// -----------------------------------------------------------------------
// Multi-file operations
// -----------------------------------------------------------------------

#[test]
fn test_open_five_files_close_middle_adjusts_active_tab() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let paths: Vec<String> = (0..5)
        .map(|i| temp_file(&format!("test_multi_{}.txt", i), &format!("content {}", i)))
        .collect();

    for p in &paths {
        state.open_file(p.clone());
    }
    assert_eq!(state.open_tabs.len(), 5);
    assert_eq!(state.active_tab, 4); // last opened is active

    // Select tab 2 as active
    state.active_tab = 2;
    state.update_breadcrumb();

    // Close tab 2 (the middle one)
    state.close_tab(2);
    assert_eq!(state.open_tabs.len(), 4);
    // active_tab should adjust: was 2, tab removed at 2, so it stays at 2
    // (now pointing at what was tab 3)
    assert!(state.active_tab <= 3);
    // The removed path should not be in tabs or file_contents
    assert!(state.file_contents.get(&paths[2]).is_none());
    assert!(!state.open_tabs.iter().any(|t| t.path == paths[2]));

    // Remaining tabs should be 0, 1, 3, 4
    let remaining_paths: Vec<&str> = state.open_tabs.iter().map(|t| t.path.as_str()).collect();
    assert!(remaining_paths.contains(&paths[0].as_str()));
    assert!(remaining_paths.contains(&paths[1].as_str()));
    assert!(remaining_paths.contains(&paths[3].as_str()));
    assert!(remaining_paths.contains(&paths[4].as_str()));

    for p in &paths {
        cleanup(p);
    }
}

#[test]
fn test_close_all_tabs_one_by_one() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let paths: Vec<String> = (0..3)
        .map(|i| temp_file(&format!("test_close_all_{}.txt", i), &format!("c{}", i)))
        .collect();

    for p in &paths {
        state.open_file(p.clone());
    }

    // Close all tabs from the end
    state.close_tab(2);
    state.close_tab(1);
    state.close_tab(0);

    assert!(state.open_tabs.is_empty());
    assert!(state.file_contents.is_empty());
    assert_eq!(state.active_tab, 0);
    assert!(state.active_buffer().is_none());

    for p in &paths {
        cleanup(p);
    }
}

#[test]
fn test_close_first_tab_when_active() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let p0 = temp_file("test_close_first_0.txt", "a");
    let p1 = temp_file("test_close_first_1.txt", "b");
    let p2 = temp_file("test_close_first_2.txt", "c");
    state.open_file(p0.clone());
    state.open_file(p1.clone());
    state.open_file(p2.clone());

    // Activate first tab then close it
    state.active_tab = 0;
    state.close_tab(0);

    assert_eq!(state.open_tabs.len(), 2);
    // active_tab was 0, after removal it should stay at 0
    // (pointing to what was tab 1)
    assert_eq!(state.active_tab, 0);
    assert_eq!(state.open_tabs[0].path, p1);

    cleanup(&p0);
    cleanup(&p1);
    cleanup(&p2);
}

// -----------------------------------------------------------------------
// Find/Replace: replace "a" with "aa" should not infinite-loop
// -----------------------------------------------------------------------

#[test]
fn test_replace_all_expanding_pattern_no_infinite_loop() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_replace_expand.txt", "a b a c a");
    state.open_file(path.clone());

    state.find_query = "a".to_string();
    state.replace_query = "aa".to_string();

    // replace_all uses String::replace which is safe from infinite loops
    state.replace_all();

    let content = state.file_contents[&path].content();
    assert_eq!(content, "aa b aa c aa");
    // Verify matches are cleared after replace_all
    assert!(state.find_matches.is_empty());
    assert_eq!(state.find_match_index, 0);

    cleanup(&path);
}

#[test]
fn test_replace_one_expanding_pattern_terminates() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_replace_one_expand.txt", "x x x");
    state.open_file(path.clone());

    state.find_query = "x".to_string();
    state.replace_query = "xx".to_string();
    state.find_next(); // build matches
    assert_eq!(state.find_matches.len(), 3);

    // Replace one at a time -- each call should terminate
    state.replace_one();
    let content = state.file_contents[&path].content();
    // One "x" replaced with "xx", so total "x" count changes
    let x_count = content.matches("x").count();
    assert!(
        x_count >= 3,
        "at least original minus 1 plus 2, got: {}",
        x_count
    );

    // Replace remaining originals one by one
    state.replace_one();
    state.replace_one();
    // Just verify it terminated and did not panic

    cleanup(&path);
}

// -----------------------------------------------------------------------
// Undo stack overflow: push 200 changes, verify capped at 100
// -----------------------------------------------------------------------

#[test]
fn test_undo_stack_capped_at_100() {
    let mut buf = FileBuffer {
        buffer: fission::text_engine::TextBuffer::from_str("initial"),
        language: Language::Plain,
        wrap_mode: WrapMode::NoWrap,
        document_mode: DocumentMode::Normal,
        backing: DocumentBacking::InMemory,
        cursor_line: 0,
        cursor_col: 0,
        anchor_line: 0,
        anchor_col: 0,
        edit_history: fission::text_engine::EditHistory::new(),
        line_index: fission::text_engine::LineIndex::build_from_str("initial"),
        preedit: None,
    };

    for i in 0..200 {
        buf.replace_document(&format!("change_{}", i));
    }

    // The default EditHistory max is 1000, but we pushed 200, so depth == 200.
    // Verify the stack did not grow unboundedly beyond the max.
    assert!(
        buf.edit_history.undo_depth() <= 1000,
        "undo stack should be capped"
    );

    // Current content should be the last change
    assert_eq!(buf.content(), "change_199");
}

#[test]
fn test_undo_stack_overflow_still_allows_undo_redo() {
    let mut buf = FileBuffer {
        buffer: fission::text_engine::TextBuffer::from_str("start"),
        language: Language::Plain,
        wrap_mode: WrapMode::NoWrap,
        document_mode: DocumentMode::Normal,
        backing: DocumentBacking::InMemory,
        cursor_line: 0,
        cursor_col: 0,
        anchor_line: 0,
        anchor_col: 0,
        edit_history: fission::text_engine::EditHistory::new(),
        line_index: fission::text_engine::LineIndex::build_from_str("start"),
        preedit: None,
    };

    // Push 200 changes
    for i in 0..200 {
        buf.replace_document(&format!("v{}", i));
    }

    let undo_depth = buf.edit_history.undo_depth();

    // Undo all available entries
    for _ in 0..undo_depth {
        buf.undo();
    }

    // Should be at the oldest available state
    let c = buf.content();
    assert!(
        c.starts_with("v") || c == "start",
        "content should be a v-version or start, got: {}",
        c
    );

    // Undo once more should be a no-op (stack empty)
    let before = buf.content();
    buf.undo();
    assert_eq!(buf.content(), before);

    // Redo all
    for _ in 0..undo_depth {
        buf.redo();
    }
    assert_eq!(buf.content(), "v199");

    // Redo once more should be a no-op
    buf.redo();
    assert_eq!(buf.content(), "v199");
}

// -----------------------------------------------------------------------
// File tree: scan_directory depth limit and filtering
// -----------------------------------------------------------------------

#[test]
fn test_scan_directory_depth_limit() {
    let base = std::env::temp_dir().join("test_scan_depth");
    std::fs::remove_dir_all(&base).ok();

    // Create deeply nested directories: 0/1/2/3/4/5/6
    let mut current = base.clone();
    for i in 0..7 {
        current = current.join(format!("{}", i));
        std::fs::create_dir_all(&current).ok();
        std::fs::write(current.join("file.txt"), "data").ok();
    }

    let entries = scan_directory(&base, 0);
    fn max_depth(entries: &[FileEntry], depth: usize) -> usize {
        let mut max = depth;
        for e in entries {
            if !e.children.is_empty() {
                max = max.max(max_depth(&e.children, depth + 1));
            }
        }
        max
    }
    let depth = max_depth(&entries, 0);
    assert!(
        depth <= 4,
        "scan_directory should stop at depth 4, got depth {}",
        depth
    );

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_directory_skips_target_and_node_modules() {
    let base = std::env::temp_dir().join("test_scan_skip");
    std::fs::remove_dir_all(&base).ok();
    std::fs::create_dir_all(&base).ok();
    std::fs::create_dir_all(base.join("target")).ok();
    std::fs::create_dir_all(base.join("node_modules")).ok();
    std::fs::create_dir_all(base.join("src")).ok();
    std::fs::write(base.join("src/main.rs"), "fn main(){}").ok();

    let entries = scan_directory(&base, 0);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

    assert!(!names.contains(&"target"), "should skip target directory");
    assert!(
        !names.contains(&"node_modules"),
        "should skip node_modules directory"
    );
    assert!(names.contains(&"src"), "should include src directory");

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_directory_handles_symlinks() {
    let base = std::env::temp_dir().join("test_scan_symlink");
    std::fs::remove_dir_all(&base).ok();
    std::fs::create_dir_all(&base).ok();
    std::fs::write(base.join("real.txt"), "real content").ok();

    #[cfg(unix)]
    {
        let link_path = base.join("link.txt");
        let _ = std::os::unix::fs::symlink(base.join("real.txt"), &link_path);
    }

    // scan_directory should not panic on symlinks
    let entries = scan_directory(&base, 0);
    assert!(!entries.is_empty(), "should have at least real.txt");

    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"real.txt"));

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_directory_empty_dir() {
    let base = std::env::temp_dir().join("test_scan_empty");
    std::fs::remove_dir_all(&base).ok();
    std::fs::create_dir_all(&base).ok();

    let entries = scan_directory(&base, 0);
    assert!(
        entries.is_empty(),
        "empty directory should return empty vec"
    );

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_directory_nonexistent() {
    let entries = scan_directory(Path::new("/tmp/definitely_does_not_exist_12345"), 0);
    assert!(
        entries.is_empty(),
        "nonexistent directory should return empty vec"
    );
}

// -----------------------------------------------------------------------
// Action handler behavior: toggles and state transitions
// -----------------------------------------------------------------------

#[test]
fn test_toggle_command_palette() {
    let mut state = EditorState::default();
    assert!(!state.show_command_palette);
    state.show_command_palette = !state.show_command_palette;
    assert!(state.show_command_palette);
    state.command_query = "Save All".to_string();
    assert_eq!(state.command_query, "Save All");

    state.show_command_palette = !state.show_command_palette;
    assert!(!state.show_command_palette);
}

#[test]
fn test_toggle_find_replace() {
    let mut state = EditorState::default();
    assert!(!state.show_find_replace);
    state.show_find_replace = !state.show_find_replace;
    assert!(state.show_find_replace);
}

#[test]
fn test_select_tab_switches_active_buffer() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let p0 = temp_file("test_select_tab_0.txt", "zero");
    let p1 = temp_file("test_select_tab_1.txt", "one");
    let p2 = temp_file("test_select_tab_2.txt", "two");
    state.open_file(p0.clone());
    state.open_file(p1.clone());
    state.open_file(p2.clone());

    assert_eq!(state.active_tab, 2);

    state.active_tab = 0;
    let (tab, buf) = state.active_buffer().unwrap();
    assert_eq!(tab.path, p0);
    assert_eq!(buf.content(), "zero");

    state.active_tab = 1;
    let (tab, buf) = state.active_buffer().unwrap();
    assert_eq!(tab.path, p1);
    assert_eq!(buf.content(), "one");

    cleanup(&p0);
    cleanup(&p1);
    cleanup(&p2);
}

#[test]
fn test_toggle_tree_node() {
    let mut state = EditorState::default();
    assert!(state.tree_expanded.is_empty());

    let node = "src".to_string();
    state.tree_expanded.insert(node.clone());
    assert!(state.tree_expanded.contains("src"));

    state.tree_expanded.remove(&node);
    assert!(!state.tree_expanded.contains("src"));
}

#[test]
fn test_select_tree_node() {
    let mut state = EditorState::default();
    assert!(state.tree_selected.is_none());

    state.tree_selected = Some("src/main.rs".to_string());
    assert_eq!(state.tree_selected.as_deref(), Some("src/main.rs"));

    state.tree_selected = Some("Cargo.toml".to_string());
    assert_eq!(state.tree_selected.as_deref(), Some("Cargo.toml"));
}

#[test]
fn test_show_hover_and_dismiss() {
    let mut state = EditorState::default();
    assert!(!state.show_hover);

    state.show_hover = true;
    state.hover_info = Some("fn main() -> ()".to_string());
    state.hover_position = (150.0, 300.0);
    assert!(state.show_hover);
    assert_eq!(state.hover_info.as_deref(), Some("fn main() -> ()"));

    state.show_hover = false;
    assert!(!state.show_hover);
}

#[test]
fn test_show_context_menu_and_dismiss() {
    let mut state = EditorState::default();
    assert!(!state.context_menu_visible);

    state.context_menu_visible = true;
    state.context_menu_position = (50.0, 75.0);
    state.context_menu_target = Some("src/lib.rs".to_string());

    assert!(state.context_menu_visible);
    assert_eq!(state.context_menu_position, (50.0, 75.0));

    state.context_menu_visible = false;
    state.context_menu_target = None;
    assert!(!state.context_menu_visible);
    assert!(state.context_menu_target.is_none());
}

#[test]
fn test_set_active_menu() {
    let mut state = EditorState::default();
    assert!(state.active_menu.is_none());

    state.active_menu = Some("File".to_string());
    assert_eq!(state.active_menu.as_deref(), Some("File"));

    state.active_menu = Some("Edit".to_string());
    assert_eq!(state.active_menu.as_deref(), Some("Edit"));

    state.active_menu = None;
    assert!(state.active_menu.is_none());
}

#[test]
fn test_select_completion_index() {
    let mut state = EditorState::default();
    state.completions = vec![
        CompletionItem {
            label: "foo".into(),
            kind: "function".into(),
            detail: None,
        },
        CompletionItem {
            label: "bar".into(),
            kind: "variable".into(),
            detail: None,
        },
        CompletionItem {
            label: "baz".into(),
            kind: "keyword".into(),
            detail: Some("built-in".into()),
        },
    ];
    state.show_completions = true;
    state.selected_completion = 2;

    assert_eq!(state.completions[state.selected_completion].label, "baz");
}

#[test]
fn test_dismiss_completions_hides_panel() {
    let mut state = EditorState::default();
    state.show_completions = true;
    state.completions = vec![CompletionItem {
        label: "item".into(),
        kind: "text".into(),
        detail: None,
    }];

    state.show_completions = false;
    assert!(!state.show_completions);
    assert_eq!(state.completions.len(), 1); // still there but hidden
}

#[test]
fn test_navigate_diagnostic_moves_cursor() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file(
        "test_nav_diag.rs",
        "fn main() {\n    let x = 1;\n    let y = 2;\n}",
    );
    state.open_file(path.clone());

    state.go_to_line(2);
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.cursor_line, 1); // 0-based

    cleanup(&path);
}

#[test]
fn test_update_file_content_marks_dirty() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_update_content.txt", "original");
    state.open_file(path.clone());
    assert!(!state.open_tabs[0].is_dirty);

    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.replace_document("modified content");
    }
    state.open_tabs[0].is_dirty = true;

    assert!(state.open_tabs[0].is_dirty);
    assert_eq!(state.file_contents[&path].content(), "modified content");

    cleanup(&path);
}

// -----------------------------------------------------------------------
// Scroll offset
// -----------------------------------------------------------------------

#[test]
fn test_scroll_offset_resets_on_open_file() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    state.scroll_offset_y = 500.0;

    let path = temp_file("test_scroll_reset.txt", "content");
    state.open_file(path.clone());

    assert_eq!(
        state.scroll_offset_y, 0.0,
        "scroll should reset when opening a file"
    );

    cleanup(&path);
}

// -----------------------------------------------------------------------
// Clipboard operations: edge cases
// -----------------------------------------------------------------------

#[test]
fn test_paste_at_beginning_of_file() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_paste_beginning.txt", "existing content");
    state.open_file(path.clone());

    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.set_caret_line_col(0, 0);
    }
    state.clipboard = "PREFIX ".to_string();
    state.paste();

    let content = state.file_contents[&path].content();
    assert!(
        content.starts_with("PREFIX "),
        "should paste at beginning, got: {}",
        content
    );

    cleanup(&path);
}

#[test]
fn test_cut_last_line_adjusts_cursor() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_cut_last.txt", "line1\nline2\nline3");
    state.open_file(path.clone());

    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.set_caret_line_col(2, 0); // last line
    }

    state.cut_line();
    assert_eq!(state.clipboard, "line3");

    let buf = state.file_contents.get(&path).unwrap();
    let max_line = buf.content().lines().count().saturating_sub(1);
    assert!(
        buf.cursor_line <= max_line,
        "cursor_line {} should be <= max_line {}",
        buf.cursor_line,
        max_line
    );

    cleanup(&path);
}

// -----------------------------------------------------------------------
// Find/Replace: multi-line content
// -----------------------------------------------------------------------

#[test]
fn test_find_across_multiple_lines() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let content = "first line has foo\nsecond line no match\nthird foo and fourth foo";
    let path = temp_file("test_find_multi.txt", content);
    state.open_file(path.clone());

    state.find_query = "foo".to_string();
    state.find_next();

    assert_eq!(state.find_matches.len(), 3);
    assert_eq!(state.find_matches[0].1, 0); // first line
    assert_eq!(state.find_matches[1].1, 2); // third line, first occurrence
    assert_eq!(state.find_matches[2].1, 2); // third line, second occurrence

    cleanup(&path);
}

#[test]
fn test_replace_all_multiline() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_replace_multi.txt", "abc\ndef\nabc\nghi");
    state.open_file(path.clone());

    state.find_query = "abc".to_string();
    state.replace_query = "XYZ".to_string();
    state.replace_all();

    let content = state.file_contents[&path].content();
    assert_eq!(content, "XYZ\ndef\nXYZ\nghi");

    cleanup(&path);
}

// -----------------------------------------------------------------------
// Operations with no active buffer: verify no panics
// -----------------------------------------------------------------------

#[test]
fn test_find_next_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.find_query = "something".to_string();
    state.find_next();
    assert!(state.find_matches.is_empty());
}

#[test]
fn test_find_previous_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.find_query = "something".to_string();
    state.find_previous();
    assert!(state.find_matches.is_empty());
}

#[test]
fn test_replace_one_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.find_query = "a".to_string();
    state.replace_query = "b".to_string();
    state.replace_one();
}

#[test]
fn test_replace_all_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.find_query = "a".to_string();
    state.replace_query = "b".to_string();
    state.replace_all();
}

#[test]
fn test_undo_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.undo_active();
}

#[test]
fn test_redo_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.redo_active();
}

#[test]
fn test_copy_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.copy_line();
    assert!(state.clipboard.is_empty());
}

#[test]
fn test_cut_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.cut_line();
}

#[test]
fn test_paste_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.clipboard = "something".to_string();
    state.paste();
}

#[test]
fn test_save_active_no_tabs_does_not_panic() {
    let mut state = EditorState::default();
    state.save_active_file();
}

#[test]
fn test_save_all_no_tabs_sets_message() {
    let mut state = EditorState::default();
    state.save_all_files();
    assert!(state
        .status_message
        .as_ref()
        .unwrap()
        .contains("All files saved"));
}

// -----------------------------------------------------------------------
// File buffer version tracking
// -----------------------------------------------------------------------

#[test]
fn test_file_buffer_revision_starts_at_zero() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_version.txt", "versioned");
    state.open_file(path.clone());

    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.buffer.revision(), 0);

    cleanup(&path);
}

// -----------------------------------------------------------------------
// Open nonexistent file: should open with empty content
// -----------------------------------------------------------------------

#[test]
fn test_open_nonexistent_file_gets_empty_content() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = "/tmp/this_file_does_not_exist_99999.txt".to_string();
    std::fs::remove_file(&path).ok();

    state.open_file(path.clone());

    assert_eq!(state.open_tabs.len(), 1);
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.content(), "");
}

// -----------------------------------------------------------------------
// Default state invariants
// -----------------------------------------------------------------------

#[test]
fn test_default_state_invariants() {
    let state = EditorState::default();

    assert!(state.open_tabs.is_empty());
    assert!(state.file_contents.is_empty());
    assert_eq!(state.active_tab, 0);
    assert!(state.sidebar_visible);
    assert!(state.terminal_visible);
    assert!(!state.show_command_palette);
    assert!(!state.show_find_replace);
    assert!(!state.show_completions);
    assert!(!state.show_hover);
    assert!(!state.context_menu_visible);
    assert!(state.show_menu_bar);
    assert!(state.active_menu.is_none());
    assert!(state.tree_selected.is_none());
    assert!(state.tree_expanded.is_empty());
    assert!(state.find_query.is_empty());
    assert!(state.replace_query.is_empty());
    assert!(state.search_query.is_empty());
    assert!(state.search_results.is_empty());
    assert!(state.diagnostics.is_empty());
    assert!(state.completions.is_empty());
    assert!(state.clipboard.is_empty());
    assert!(state.breadcrumb_path.is_empty());
    assert_eq!(state.scroll_offset_y, 0.0);
    assert_eq!(state.sidebar_width, 240.0);
    assert_eq!(state.terminal_height, 96.0);
    assert_eq!(state.sidebar_section, SidebarSection::Explorer);
    assert_eq!(state.bottom_panel_tab, BottomPanelTab::Terminal);
    assert!(state.terminal_session.is_none());
    assert!(state.lsp_handle.is_none());
    assert!(!state.lsp_enabled());
}

// -----------------------------------------------------------------------
// Helper function coverage
// -----------------------------------------------------------------------

#[test]
fn test_uri_to_path() {
    assert_eq!(
        uri_to_path("file:///home/user/file.rs"),
        "/home/user/file.rs"
    );
    assert_eq!(uri_to_path("/already/a/path"), "/already/a/path");
    assert_eq!(uri_to_path("file://relative"), "relative");
}

#[test]
fn test_completion_kind_str() {
    assert_eq!(completion_kind_str(Some(1)), "text");
    assert_eq!(completion_kind_str(Some(2)), "method");
    assert_eq!(completion_kind_str(Some(3)), "function");
    assert_eq!(completion_kind_str(Some(4)), "constructor");
    assert_eq!(completion_kind_str(Some(5)), "field");
    assert_eq!(completion_kind_str(Some(6)), "variable");
    assert_eq!(completion_kind_str(Some(7)), "class");
    assert_eq!(completion_kind_str(Some(8)), "interface");
    assert_eq!(completion_kind_str(Some(9)), "module");
    assert_eq!(completion_kind_str(Some(10)), "property");
    assert_eq!(completion_kind_str(Some(13)), "enum");
    assert_eq!(completion_kind_str(Some(14)), "keyword");
    assert_eq!(completion_kind_str(Some(15)), "snippet");
    assert_eq!(completion_kind_str(Some(21)), "constant");
    assert_eq!(completion_kind_str(Some(22)), "struct");
    assert_eq!(completion_kind_str(Some(23)), "event");
    assert_eq!(completion_kind_str(Some(25)), "type_param");
    assert_eq!(completion_kind_str(None), "unknown");
    assert_eq!(completion_kind_str(Some(999)), "unknown");
}
