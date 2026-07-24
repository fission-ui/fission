use super::test_support::{cleanup, temp_file};
use super::*;

#[test]
fn test_undo_redo() {
    let mut state = EditorState::default();
    state.root_path = PathBuf::from("/tmp");
    // Create a temp file
    let path = "/tmp/test_undo.txt".to_string();
    std::fs::write(&path, "hello").ok();
    state.open_file(path.clone());

    // Modify content
    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.replace_document("hello world");
    }

    // Undo
    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.undo();
        assert_eq!(buf.content(), "hello");
    }

    // Redo
    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.redo();
        assert_eq!(buf.content(), "hello world");
    }

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_undo_clears_redo_on_new_change() {
    let mut buf = FileBuffer {
        buffer: fission::text_engine::TextBuffer::from_str("a"),
        language: Language::Plain,
        wrap_mode: WrapMode::NoWrap,
        document_mode: DocumentMode::Normal,
        backing: DocumentBacking::InMemory,
        cursor_line: 0,
        cursor_col: 0,
        anchor_line: 0,
        anchor_col: 0,
        edit_history: fission::text_engine::EditHistory::new(),
        line_index: fission::text_engine::LineIndex::build_from_str("a"),
        preedit: None,
    };

    // Change to "b"
    buf.replace_document("b");

    // Undo back to "a"
    buf.undo();
    assert_eq!(buf.content(), "a");
    // edit_history.redo_depth() should be 1
    assert_eq!(buf.edit_history.redo_depth(), 1);

    // New change to "c" should clear redo
    buf.replace_document("c");
    assert_eq!(buf.edit_history.redo_depth(), 0);
}

#[test]
fn test_undo_stack_cap() {
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
        edit_history: fission::text_engine::EditHistory::with_max(100),
        line_index: fission::text_engine::LineIndex::build_from_str("start"),
        preedit: None,
    };

    for i in 0..110 {
        buf.replace_document(&format!("version_{}", i));
    }

    assert!(buf.edit_history.undo_depth() <= 100);
}

#[test]
fn test_sync_content_clears_history() {
    let mut buf = FileBuffer {
        buffer: fission::text_engine::TextBuffer::from_str("before"),
        language: Language::Plain,
        wrap_mode: WrapMode::NoWrap,
        document_mode: DocumentMode::Normal,
        backing: DocumentBacking::InMemory,
        cursor_line: 0,
        cursor_col: 0,
        anchor_line: 0,
        anchor_col: 0,
        edit_history: fission::text_engine::EditHistory::new(),
        line_index: fission::text_engine::LineIndex::build_from_str("before"),
        preedit: None,
    };

    buf.replace_document("during");
    assert_eq!(buf.edit_history.undo_depth(), 1);

    buf.sync_content("after");
    assert_eq!(buf.content(), "after");
    assert_eq!(buf.edit_history.undo_depth(), 0);
    assert_eq!(buf.edit_history.redo_depth(), 0);
}

#[test]
fn test_find_replace() {
    let mut state = EditorState::default();
    state.root_path = PathBuf::from("/tmp");
    let path = "/tmp/test_find.txt".to_string();
    std::fs::write(&path, "foo bar foo baz foo").ok();
    state.open_file(path.clone());
    state.find_query = "foo".to_string();
    state.find_next();
    assert_eq!(state.find_matches.len(), 3);

    state.replace_query = "qux".to_string();
    state.replace_all();
    let content = state.file_contents[&path].content();
    assert!(!content.contains("foo"));
    assert!(content.contains("qux"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_open_file_creates_tab() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_open_tab.rs", "fn main() {}");
    state.open_file(path.clone());

    assert_eq!(state.open_tabs.len(), 1);
    assert_eq!(state.open_tabs[0].title, "test_open_tab.rs");
    assert_eq!(state.open_tabs[0].path, path);
    assert!(!state.open_tabs[0].is_dirty);
    assert_eq!(state.active_tab, 0);

    // Verify content was loaded
    let buf = state.file_contents.get(&path).expect("buffer exists");
    assert_eq!(buf.content(), "fn main() {}");
    assert_eq!(buf.language, Language::Rust);

    cleanup(&path);
}

#[test]
fn test_open_file_deduplicates() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_dedup.txt", "hello");
    state.open_file(path.clone());
    state.open_file(path.clone());

    // Should only have one tab, not two
    assert_eq!(state.open_tabs.len(), 1);

    cleanup(&path);
}

#[test]
fn test_save_clears_dirty() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_save_dirty.txt", "original");
    state.open_file(path.clone());

    // Modify content, mark dirty
    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.replace_document("modified");
    }
    state.open_tabs[0].is_dirty = true;
    assert!(state.open_tabs[0].is_dirty);

    // Save
    state.save_active_file();
    assert!(!state.open_tabs[0].is_dirty);
    assert!(state.status_message.as_ref().unwrap().contains("Saved"));

    // Verify file on disk has new content
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert_eq!(on_disk, "modified");

    cleanup(&path);
}

#[test]
fn test_close_tab_removes() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path1 = temp_file("test_close1.txt", "one");
    let path2 = temp_file("test_close2.txt", "two");
    state.open_file(path1.clone());
    state.open_file(path2.clone());

    assert_eq!(state.open_tabs.len(), 2);
    assert_eq!(state.active_tab, 1); // second tab is active

    // Close first tab
    state.close_tab(0);
    assert_eq!(state.open_tabs.len(), 1);
    assert_eq!(state.open_tabs[0].path, path2);
    // Buffer for path1 should be removed
    assert!(state.file_contents.get(&path1).is_none());

    cleanup(&path1);
    cleanup(&path2);
}

#[test]
fn test_close_tab_adjusts_active_index() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let p1 = temp_file("test_close_adj1.txt", "a");
    let p2 = temp_file("test_close_adj2.txt", "b");
    let p3 = temp_file("test_close_adj3.txt", "c");
    state.open_file(p1.clone());
    state.open_file(p2.clone());
    state.open_file(p3.clone());
    assert_eq!(state.active_tab, 2);

    // Close the last tab; active_tab should adjust
    state.close_tab(2);
    assert_eq!(state.active_tab, 1);

    cleanup(&p1);
    cleanup(&p2);
    cleanup(&p3);
}

#[test]
fn test_find_matches_correct() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_find_match.txt", "apple banana apple cherry apple");
    state.open_file(path.clone());

    state.find_query = "apple".to_string();
    state.find_next();

    // "apple" appears 3 times on one line
    assert_eq!(state.find_matches.len(), 3);

    // Verify positions
    assert_eq!(state.find_matches[0].2, 0); // col 0
    assert_eq!(state.find_matches[1].2, 13); // col 13 ("apple banana apple...")
    assert_eq!(state.find_matches[2].2, 26); // col 26

    cleanup(&path);
}

#[test]
fn test_find_next_wraps_around() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_find_wrap.txt", "aa bb aa");
    state.open_file(path.clone());

    state.find_query = "aa".to_string();
    state.find_next();
    assert_eq!(state.find_matches.len(), 2);
    // find_next called once sets index to 1 (second match, since
    // rebuild sets it then advances)
    let idx1 = state.find_match_index;

    state.find_next();
    let idx2 = state.find_match_index;

    // After two advances it should have wrapped
    assert_ne!(idx1, idx2);

    // One more should wrap back
    state.find_next();
    // Should be back to where idx1 was or wrapped
    assert!(state.find_match_index < state.find_matches.len());

    cleanup(&path);
}

#[test]
fn test_find_previous() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_find_prev.txt", "xx yy xx yy xx");
    state.open_file(path.clone());

    state.find_query = "xx".to_string();
    state.find_next(); // build matches + advance
    let initial = state.find_match_index;
    state.find_previous();
    // Should wrap to last match
    let after_prev = state.find_match_index;
    assert_ne!(initial, after_prev);

    cleanup(&path);
}

#[test]
fn test_replace_one() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_replace_one.txt", "cat dog cat");
    state.open_file(path.clone());

    state.find_query = "cat".to_string();
    state.replace_query = "bird".to_string();
    state.find_next(); // build matches

    state.replace_one();
    let content = state.file_contents[&path].content();
    // One "cat" should be replaced with "bird"
    let cat_count = content.matches("cat").count();
    let bird_count = content.matches("bird").count();
    assert_eq!(cat_count, 1);
    assert_eq!(bird_count, 1);
    assert!(state.open_tabs[0].is_dirty);

    cleanup(&path);
}

#[test]
fn test_replace_all_works() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_replace_all.txt", "foo bar foo baz foo");
    state.open_file(path.clone());

    state.find_query = "foo".to_string();
    state.replace_query = "ZZZ".to_string();
    state.replace_all();

    let content = state.file_contents[&path].content();
    assert_eq!(content, "ZZZ bar ZZZ baz ZZZ");
    assert!(state.open_tabs[0].is_dirty);
    assert!(state
        .status_message
        .as_ref()
        .unwrap()
        .contains("Replaced all"));

    cleanup(&path);
}

#[test]
fn test_replace_all_empty_query_noop() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_replace_noop.txt", "unchanged");
    state.open_file(path.clone());

    state.find_query = "".to_string();
    state.replace_query = "something".to_string();
    state.replace_all();

    let content = state.file_contents[&path].content();
    assert_eq!(content, "unchanged");

    cleanup(&path);
}

#[test]
fn test_undo_redo_model() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_undo_redo_model.txt", "version_0");
    state.open_file(path.clone());

    // Make several changes
    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.replace_document("version_1");
        buf.replace_document("version_2");
    }

    // Undo through the state helper
    state.undo_active();
    assert_eq!(state.file_contents[&path].content(), "version_1");

    state.undo_active();
    assert_eq!(state.file_contents[&path].content(), "version_0");

    // Redo
    state.redo_active();
    assert_eq!(state.file_contents[&path].content(), "version_1");

    state.redo_active();
    assert_eq!(state.file_contents[&path].content(), "version_2");

    // Redo when nothing to redo should be a no-op
    state.redo_active();
    assert_eq!(state.file_contents[&path].content(), "version_2");

    cleanup(&path);
}

#[test]
fn test_large_file_rejected() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = std::env::temp_dir().join("test_large_file.txt");
    let path_str = path.to_string_lossy().to_string();

    // Create a sparse file larger than the Huge threshold.
    let file = std::fs::File::create(&path).expect("create large file");
    file.set_len(LARGE_FILE_LIMIT + 4096)
        .expect("resize large file");

    state.open_file(path_str.clone());

    assert_eq!(state.open_tabs.len(), 1);
    assert!(state.file_contents.contains_key(&path_str));
    let buf = state.file_contents.get(&path_str).expect("huge buffer");
    assert_eq!(buf.document_mode, DocumentMode::Huge);

    let msg = state.status_message.as_ref().expect("status message set");
    assert!(
        msg.contains("Opened huge file in windowed mode"),
        "expected huge-file window status, got: {}",
        msg
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_create_file() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = std::env::temp_dir().join("test_create_new.txt");
    let path_str = path.to_string_lossy().to_string();

    // Clean up in case a previous run left it
    std::fs::remove_file(&path).ok();

    state.create_file(path_str.clone());

    // File should exist on disk
    assert!(path.exists(), "file should be created on disk");

    // Should be opened in a tab
    assert_eq!(state.open_tabs.len(), 1);
    assert_eq!(state.open_tabs[0].path, path_str);

    // Content should be empty
    let buf = state.file_contents.get(&path_str).expect("buffer exists");
    assert_eq!(buf.content(), "");

    // Opening the new file currently replaces the initial create status.
    assert!(state.status_message.as_ref().unwrap().contains("Opened"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_delete_file() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_delete_target.txt", "to be deleted");
    state.open_file(path.clone());
    assert_eq!(state.open_tabs.len(), 1);

    state.delete_file(path.clone());

    // File should not exist on disk
    assert!(!std::path::Path::new(&path).exists());
    // Tab should be closed
    assert!(state.open_tabs.is_empty());
    // Buffer should be removed
    assert!(state.file_contents.get(&path).is_none());
    // Status message
    assert!(state.status_message.as_ref().unwrap().contains("Deleted"));
}

#[test]
fn test_rename_file() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_rename_src.txt", "rename me");
    state.open_file(path.clone());

    let new_name = "test_rename_dst.txt";
    state.rename_file(path.clone(), new_name.to_string());

    // Old file should not exist
    assert!(!std::path::Path::new(&path).exists());

    // New file should exist
    let new_path = std::env::temp_dir().join(new_name);
    assert!(new_path.exists());

    // Tab should reflect new path and title
    assert_eq!(state.open_tabs[0].title, new_name);
    assert_eq!(
        state.open_tabs[0].path,
        new_path.to_string_lossy().to_string()
    );

    // Buffer should be under new path
    let buf = state
        .file_contents
        .get(&new_path.to_string_lossy().to_string())
        .expect("buffer under new path");
    assert_eq!(buf.content(), "rename me");

    // Old path buffer gone
    assert!(state.file_contents.get(&path).is_none());

    // Status message
    assert!(state.status_message.as_ref().unwrap().contains("Renamed"));

    std::fs::remove_file(&new_path).ok();
}

#[test]
fn test_breadcrumb_updates() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let subdir = std::env::temp_dir().join("test_breadcrumb_dir");
    std::fs::create_dir_all(&subdir).ok();
    let file_path = subdir.join("deep.txt");
    std::fs::write(&file_path, "hello").ok();
    let path_str = file_path.to_string_lossy().to_string();

    state.open_file(path_str.clone());

    // Breadcrumb should contain the dir name and the file name
    assert!(
        state.breadcrumb_path.len() >= 2,
        "breadcrumb should have at least 2 segments, got: {:?}",
        state.breadcrumb_path
    );
    assert!(state
        .breadcrumb_path
        .contains(&"test_breadcrumb_dir".to_string()));
    assert!(state.breadcrumb_path.contains(&"deep.txt".to_string()));

    std::fs::remove_file(&file_path).ok();
    std::fs::remove_dir(&subdir).ok();
}

#[test]
fn test_breadcrumb_updates_on_tab_switch() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let p1 = temp_file("breadcrumb_a.txt", "a");
    let p2 = temp_file("breadcrumb_b.txt", "b");

    state.open_file(p1.clone());
    assert!(state.breadcrumb_path.last() == Some(&"breadcrumb_a.txt".to_string()));

    state.open_file(p2.clone());
    assert!(state.breadcrumb_path.last() == Some(&"breadcrumb_b.txt".to_string()));

    // Switch back to first
    state.active_tab = 0;
    state.update_breadcrumb();
    assert!(state.breadcrumb_path.last() == Some(&"breadcrumb_a.txt".to_string()));

    cleanup(&p1);
    cleanup(&p2);
}

#[test]
fn test_classify_document_mode_for_size() {
    assert_eq!(classify_document_mode_for_size(1_024), DocumentMode::Normal);
    assert_eq!(
        classify_document_mode_for_size(NORMAL_FILE_LIMIT + 1),
        DocumentMode::Large
    );
    assert_eq!(
        classify_document_mode_for_size(LARGE_FILE_LIMIT + 1),
        DocumentMode::Huge
    );
}

#[test]
fn test_open_file_uses_huge_window_mode() {
    let mut state = EditorState::default();
    let dir = std::env::temp_dir().join("fission_editor_huge_preview");
    std::fs::create_dir_all(&dir).ok();
    let file = dir.join("huge.tsv");
    let mut payload = String::from("col1\\tcol2\\nvalue\\tvalue\\n");
    while payload.len() < 8192 {
        payload.push_str("abcdefghij\\tklmnopqrst\\n");
    }
    std::fs::write(&file, payload).ok();
    let sparse = std::fs::OpenOptions::new().write(true).open(&file).unwrap();
    sparse.set_len(LARGE_FILE_LIMIT + 4096).unwrap();

    state.open_file(file.to_string_lossy().to_string());

    let buf = state
        .active_buffer()
        .map(|(_, buf)| buf)
        .expect("buffer should open");
    assert_eq!(buf.document_mode, DocumentMode::Huge);
    assert!(buf.is_editable());
    assert!(matches!(buf.backing, DocumentBacking::FileWindow { .. }));
    assert!(state
        .status_message
        .as_deref()
        .unwrap_or_default()
        .contains("windowed mode"));
}

#[test]
fn test_shift_active_file_window_moves_between_windows() {
    let mut state = EditorState::default();
    let dir = std::env::temp_dir().join("fission_editor_huge_window_shift");
    std::fs::create_dir_all(&dir).ok();
    let file = dir.join("huge.log");
    let mut payload = String::new();
    payload.push_str("WINDOW-000\n");
    for idx in 1..80_000 {
        payload.push_str(&format!("WINDOW-{idx:05} :: lorem ipsum dolor sit amet\n"));
    }
    std::fs::write(&file, &payload).unwrap();
    let sparse = std::fs::OpenOptions::new().write(true).open(&file).unwrap();
    sparse.set_len(LARGE_FILE_LIMIT + 4096).unwrap();

    state.open_file(file.to_string_lossy().to_string());
    let initial_window = state
        .active_buffer()
        .and_then(|(_, buf)| match &buf.backing {
            DocumentBacking::FileWindow { window, .. } => Some(window.clone()),
            DocumentBacking::InMemory => None,
        })
        .expect("initial huge window metadata");
    let initial = state
        .active_buffer()
        .map(|(_, buf)| buf.content())
        .expect("initial huge window content");
    assert!(initial.contains("WINDOW-000"));

    state.shift_active_file_window(true);
    let moved_window = state
        .active_buffer()
        .and_then(|(_, buf)| match &buf.backing {
            DocumentBacking::FileWindow { window, .. } => Some(window.clone()),
            DocumentBacking::InMemory => None,
        })
        .expect("shifted huge window metadata");
    let moved = state
        .active_buffer()
        .map(|(_, buf)| buf.content())
        .expect("shifted huge window content");
    assert_ne!(
        initial, moved,
        "forward shift should load a different file window"
    );
    assert!(
        state
            .status_message
            .as_deref()
            .unwrap_or_default()
            .contains("Huge file window"),
        "status should describe the active huge-file byte window"
    );

    state.shift_active_file_window(false);
    let restored_window = state
        .active_buffer()
        .and_then(|(_, buf)| match &buf.backing {
            DocumentBacking::FileWindow { window, .. } => Some(window.clone()),
            DocumentBacking::InMemory => None,
        })
        .expect("restored huge window metadata");
    assert!(
        restored_window.start_line <= moved_window.start_line,
        "backward shift should move the window earlier in the file"
    );
    assert!(
        restored_window.start_byte <= moved_window.start_byte,
        "backward shift should move the byte window earlier in the file"
    );
    assert!(
        restored_window.start_line <= initial_window.end_line,
        "backward shift should land near the previous viewport window"
    );
}

#[test]
fn test_huge_window_edits_save_via_overlay_journal() {
    let mut state = EditorState::default();
    let dir = std::env::temp_dir().join("fission_editor_huge_window_save");
    std::fs::create_dir_all(&dir).ok();
    let file = dir.join("huge.txt");
    let mut payload = String::new();
    payload.push_str("HEADER\n");
    for idx in 0..6000 {
        payload.push_str(&format!("ROW-{idx:04}\n"));
    }
    std::fs::write(&file, &payload).unwrap();
    let sparse = std::fs::OpenOptions::new().write(true).open(&file).unwrap();
    sparse.set_len(LARGE_FILE_LIMIT + 4096).unwrap();

    state.open_file(file.to_string_lossy().to_string());
    {
        let (_, buf) = state.active_buffer_mut().expect("active huge buffer");
        let original = buf.content();
        let replace_end = original.find('\n').unwrap_or(original.len());
        buf.apply_edit(0..replace_end, "PATCHED-HEADER");
        assert!(buf.content().starts_with("PATCHED-HEADER"));
    }
    state.mark_active_tab_dirty();
    state.save_active_file();

    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(
        saved.starts_with("PATCHED-HEADER"),
        "overlay-journal save should rewrite the underlying huge file stream"
    );
    assert!(
        state
            .status_message
            .as_deref()
            .unwrap_or_default()
            .contains("Saved"),
        "saving the huge file should report success"
    );
}

#[test]
fn test_search_finds_results() {
    let mut state = EditorState::default();
    // Use a temp directory with a known file
    let dir = std::env::temp_dir().join("test_search_dir");
    std::fs::create_dir_all(&dir).ok();
    let file = dir.join("searchable.txt");
    std::fs::write(&file, "hello world\nfoo bar\nhello again").ok();

    state.root_path = dir.clone();
    // Also open the file so it is in file_contents
    state.open_file(file.to_string_lossy().to_string());

    state.search_query = "hello".to_string();
    state.run_search();

    // Should find at least 2 matches (lines 1 and 3)
    assert!(
        state.search_results.len() >= 2,
        "expected >= 2 search results, got {}",
        state.search_results.len()
    );

    // Results should reference the correct file
    for r in &state.search_results {
        assert!(r.path.contains("searchable.txt"));
        assert!(r.context.contains("hello"));
    }

    std::fs::remove_file(&file).ok();
    std::fs::remove_dir(&dir).ok();
}

#[test]
fn test_search_empty_query_clears() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    state.search_results = vec![SearchResult {
        path: "fake".into(),
        line: 1,
        col: 0,
        context: "old result".into(),
    }];

    state.search_query = "".to_string();
    state.run_search();
    assert!(state.search_results.is_empty());
}

#[test]
fn test_git_status_parses() {
    let mut state = EditorState::default();
    // Use the repo root so git status works
    state.root_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let entries = collect_git_status(&state.root_path).expect("git status entries");

    // In a git repo with changes, we should get entries.
    // Even if there are no changes, the call should not panic.
    // Just verify the function runs and entries is a Vec.
    println!("Git status entries: {}", entries.len());
    for entry in &entries {
        assert!(!entry.path.is_empty(), "git entry path should not be empty");
        // Status should be one of the standard git status codes
        assert!(
            entry.status.len() <= 2,
            "status should be 1-2 chars, got: '{}'",
            entry.status
        );
    }
}

#[test]
fn test_paste_at_cursor() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_paste.txt", "line one\nline two\nline three");
    state.open_file(path.clone());

    // Set cursor to line 1, col 5 ("line |two")
    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.set_caret_line_col(1, 5);
    }

    state.clipboard = "INSERTED".to_string();
    state.paste();

    let content = state.file_contents[&path].content();
    assert!(
        content.contains("line INSERTEDtwo"),
        "paste should insert at cursor position, got: {}",
        content
    );
    assert!(state.open_tabs[0].is_dirty);

    cleanup(&path);
}

#[test]
fn test_paste_empty_clipboard_noop() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_paste_noop.txt", "no change");
    state.open_file(path.clone());

    state.clipboard = "".to_string();
    state.paste();

    assert_eq!(state.file_contents[&path].content(), "no change");
    assert!(!state.open_tabs[0].is_dirty);

    cleanup(&path);
}

#[test]
fn test_cut_line() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_cut.txt", "line A\nline B\nline C");
    state.open_file(path.clone());

    // Set cursor to line 1 ("line B")
    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.set_caret_line_col(1, 0);
    }

    state.cut_line();

    // Clipboard should have "line B"
    assert_eq!(state.clipboard, "line B");

    // Content should have the line removed
    let content = state.file_contents[&path].content();
    assert!(
        !content.contains("line B"),
        "cut line should be removed, got: {}",
        content
    );
    assert!(content.contains("line A"));
    assert!(content.contains("line C"));
    assert!(state.open_tabs[0].is_dirty);

    // Undo should restore it
    state.undo_active();
    let content = state.file_contents[&path].content();
    assert!(content.contains("line B"), "undo should restore cut line");

    cleanup(&path);
}

#[test]
fn test_copy_line() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_copy.txt", "alpha\nbeta\ngamma");
    state.open_file(path.clone());

    if let Some(buf) = state.file_contents.get_mut(&path) {
        buf.set_caret_line_col(2, 0);
    }

    state.copy_line();

    assert_eq!(state.clipboard, "gamma");
    // Content should be unchanged
    assert_eq!(state.file_contents[&path].content(), "alpha\nbeta\ngamma");

    cleanup(&path);
}

#[test]
fn test_go_to_line() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_goto.txt", "line 1\nline 2\nline 3\nline 4\nline 5");
    state.open_file(path.clone());

    // Go to line 3 (1-based)
    state.go_to_line(3);
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.cursor_line, 2); // 0-based
    assert_eq!(buf.cursor_col, 0);

    // Go to line 0 (edge case) -- should go to line 0
    state.go_to_line(0);
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.cursor_line, 0);

    // Go to line beyond end -- should clamp
    state.go_to_line(999);
    let buf = state.file_contents.get(&path).unwrap();
    assert_eq!(buf.cursor_line, 4); // last line (0-based)

    cleanup(&path);
}

#[test]
fn test_go_to_line_no_tabs_noop() {
    let mut state = EditorState::default();
    // No tabs open -- should not panic
    state.go_to_line(5);
}

#[test]
fn test_language_detection() {
    assert_eq!(Language::from_extension("rs"), Language::Rust);
    assert_eq!(Language::from_extension("toml"), Language::Toml);
    assert_eq!(Language::from_extension("md"), Language::Markdown);
    assert_eq!(Language::from_extension("json"), Language::Json);
    assert_eq!(Language::from_extension("txt"), Language::Plain);
    assert_eq!(Language::from_extension("xyz"), Language::Plain);
}

#[test]
fn test_language_display_name() {
    assert_eq!(Language::Rust.display_name(), "Rust");
    assert_eq!(Language::Toml.display_name(), "TOML");
    assert_eq!(Language::Markdown.display_name(), "Markdown");
    assert_eq!(Language::Json.display_name(), "JSON");
    assert_eq!(Language::Plain.display_name(), "Plain Text");
}

#[test]
fn test_markdown_defaults_to_soft_wrap() {
    assert_eq!(
        default_wrap_mode_for_path("README.md", Language::Markdown),
        WrapMode::SoftWrap
    );
}

#[test]
fn test_readme_without_extension_defaults_to_soft_wrap() {
    assert_eq!(
        default_wrap_mode_for_path("README", Language::Plain),
        WrapMode::SoftWrap
    );
}

#[test]
fn test_rust_defaults_to_no_wrap() {
    assert_eq!(
        default_wrap_mode_for_path("main.rs", Language::Rust),
        WrapMode::NoWrap
    );
}

#[test]
fn test_save_all_files() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let p1 = temp_file("test_save_all_1.txt", "one");
    let p2 = temp_file("test_save_all_2.txt", "two");
    state.open_file(p1.clone());
    state.open_file(p2.clone());

    // Modify both
    if let Some(buf) = state.file_contents.get_mut(&p1) {
        buf.replace_document("one_modified");
    }
    state.open_tabs[0].is_dirty = true;

    if let Some(buf) = state.file_contents.get_mut(&p2) {
        buf.replace_document("two_modified");
    }
    state.open_tabs[1].is_dirty = true;

    state.save_all_files();

    assert!(!state.open_tabs[0].is_dirty);
    assert!(!state.open_tabs[1].is_dirty);
    assert!(state
        .status_message
        .as_ref()
        .unwrap()
        .contains("All files saved"));

    // Verify on disk
    assert_eq!(std::fs::read_to_string(&p1).unwrap(), "one_modified");
    assert_eq!(std::fs::read_to_string(&p2).unwrap(), "two_modified");

    cleanup(&p1);
    cleanup(&p2);
}

#[test]
fn test_toggle_state_flags() {
    let mut state = EditorState::default();

    // Sidebar
    assert!(state.sidebar_visible);
    state.sidebar_visible = !state.sidebar_visible;
    assert!(!state.sidebar_visible);
    state.sidebar_visible = !state.sidebar_visible;
    assert!(state.sidebar_visible);

    // Terminal
    assert!(state.terminal_visible);
    state.terminal_visible = !state.terminal_visible;
    assert!(!state.terminal_visible);

    // Command palette
    assert!(!state.show_command_palette);
    state.show_command_palette = true;
    assert!(state.show_command_palette);

    // Find/Replace
    assert!(!state.show_find_replace);
    state.show_find_replace = true;
    assert!(state.show_find_replace);
}

#[test]
fn test_sidebar_section_switch() {
    let mut state = EditorState::default();
    assert_eq!(state.sidebar_section, SidebarSection::Explorer);

    state.sidebar_section = SidebarSection::Search;
    assert_eq!(state.sidebar_section, SidebarSection::Search);

    state.sidebar_section = SidebarSection::Git;
    assert_eq!(state.sidebar_section, SidebarSection::Git);

    state.sidebar_section = SidebarSection::Extensions;
    assert_eq!(state.sidebar_section, SidebarSection::Extensions);
}

#[test]
fn test_bottom_panel_tab_switch() {
    let mut state = EditorState::default();
    assert_eq!(state.bottom_panel_tab, BottomPanelTab::Terminal);

    state.bottom_panel_tab = BottomPanelTab::Problems;
    assert_eq!(state.bottom_panel_tab, BottomPanelTab::Problems);
}

#[test]
fn test_active_buffer_returns_correct_pair() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let path = temp_file("test_active_buf.txt", "some content");
    state.open_file(path.clone());

    let (tab, buf) = state.active_buffer().expect("active buffer");
    assert_eq!(tab.path, path);
    assert_eq!(buf.content(), "some content");

    cleanup(&path);
}

#[test]
fn test_active_buffer_none_when_no_tabs() {
    let state = EditorState::default();
    assert!(state.active_buffer().is_none());
}

#[test]
fn test_create_folder() {
    let mut state = EditorState::default();
    state.root_path = std::env::temp_dir();
    let folder_path = std::env::temp_dir().join("test_create_folder_dir");
    let folder_str = folder_path.to_string_lossy().to_string();

    // Clean up first
    std::fs::remove_dir_all(&folder_path).ok();

    state.create_folder(folder_str.clone());

    assert!(folder_path.exists());
    assert!(folder_path.is_dir());
    assert!(state
        .status_message
        .as_ref()
        .unwrap()
        .contains("Created folder"));

    std::fs::remove_dir_all(&folder_path).ok();
}
