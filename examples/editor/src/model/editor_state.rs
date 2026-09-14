use super::*;

impl EditorState {
    pub fn request_tree_refresh(&mut self) {
        self.tree_scan_generation = self.tree_scan_generation.wrapping_add(1);
    }

    pub fn tree_scan_pending(&self) -> bool {
        self.tree_scan_generation != self.tree_scan_loaded_generation
    }

    pub fn lsp_enabled(&self) -> bool {
        self.lsp_handle.is_some()
    }

    pub fn ensure_terminal_session(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.terminal_session.is_some() {
                return;
            }
            self.terminal_session = TerminalSession::spawn(TerminalLaunchConfig {
                cwd: Some(self.root_path.clone()),
                program: std::env::var("SHELL").ok(),
                ..Default::default()
            })
            .ok();
        }
    }

    /// Opens `path` in a tab. An already open file is focused straight away;
    /// otherwise the read is queued and the tab appears when it completes.
    pub fn open_file(&mut self, path: String) {
        if let Some(idx) = self.open_tabs.iter().position(|t| t.path == path) {
            self.active_tab = idx;
            self.update_breadcrumb();
            return;
        }
        let already_queued = self
            .pending_fs
            .iter()
            .any(|request| matches!(&request.op, FsOp::Open { path: queued } if *queued == path));
        if !already_queued {
            self.queue_fs(FsOp::Open { path });
        }
    }

    /// Queues a file operation for `EditorApp` to run through `FS_JOB`.
    pub fn queue_fs(&mut self, op: FsOp) -> u64 {
        let id = self.next_fs_id;
        self.next_fs_id += 1;
        self.pending_fs.push(FsRequest { id, op });
        id
    }

    /// Applies a completed file operation.
    pub fn apply_fs_outcome(&mut self, outcome: FsOutcome) {
        self.pending_fs.retain(|request| request.id != outcome.id);
        match outcome.result {
            FsResult::Opened {
                path,
                size,
                content,
                window,
            } => self.apply_opened(path, size, content, window),
            FsResult::Saved { path, all } => {
                if let Some(tab) = self.open_tabs.iter_mut().find(|tab| tab.path == path) {
                    tab.is_dirty = false;
                }
                self.status_message = Some(if all {
                    "All files saved".to_string()
                } else {
                    format!("Saved {}", path)
                });
            }
            FsResult::CreatedFile { path } => {
                if let Some(parent) = Path::new(&path).parent() {
                    self.tree_expanded
                        .insert(parent.to_string_lossy().to_string());
                }
                self.status_message = Some(format!("Created {}", path));
                self.request_tree_refresh();
                self.tree_selected = Some(path.clone());
                self.open_file(path);
            }
            FsResult::CreatedFolder { path } => {
                self.status_message = Some(format!("Created folder {}", path));
                self.request_tree_refresh();
                self.tree_selected = Some(path.clone());
                if let Some(parent) = Path::new(&path).parent() {
                    self.tree_expanded
                        .insert(parent.to_string_lossy().to_string());
                }
                self.start_rename(path);
            }
            FsResult::Renamed { from, to } => {
                let new_name = Path::new(&to)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&to)
                    .to_string();
                for tab in &mut self.open_tabs {
                    if tab.path == from {
                        tab.path = to.clone();
                        tab.title = new_name.clone();
                    }
                }
                if let Some(buf) = self.file_contents.remove(&from) {
                    self.file_contents.insert(to.clone(), buf);
                }
                if self.tree_expanded.remove(&from) {
                    self.tree_expanded.insert(to.clone());
                }
                if self.tree_selected.as_deref() == Some(&from) {
                    self.tree_selected = Some(to.clone());
                }
                self.request_tree_refresh();
                self.update_breadcrumb();
                self.status_message = Some(format!("Renamed to '{}'", new_name));
            }
            FsResult::Deleted { path } => {
                let removed = |candidate: &str| Path::new(candidate).starts_with(&path);
                self.open_tabs.retain(|tab| !removed(&tab.path));
                self.active_tab = self.active_tab.min(self.open_tabs.len().saturating_sub(1));
                self.file_contents.retain(|open, _| !removed(open));
                self.tree_expanded.retain(|expanded| !removed(expanded));
                if self.tree_selected.as_deref().is_some_and(removed) {
                    self.tree_selected = None;
                }
                let name = Path::new(&path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&path)
                    .to_string();
                self.request_tree_refresh();
                self.update_breadcrumb();
                self.status_message = Some(format!("Deleted '{}'", name));
            }
        }
    }

    /// Records a failed file operation.
    pub fn apply_fs_failure(&mut self, failure: FsFailure) {
        self.pending_fs.retain(|request| request.id != failure.id);
        self.status_message = Some(failure.message);
    }

    /// Opens a file the filesystem job has read, in a new tab.
    fn apply_opened(
        &mut self,
        path: String,
        size: u64,
        loaded: Option<String>,
        window: Option<FileWindow>,
    ) {
        let file_size = size;
        let document_mode = classify_document_mode_for_size(file_size);

        let ext = Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let lang = Language::from_extension(ext);
        let wrap_mode = if matches!(document_mode, DocumentMode::Huge) {
            WrapMode::NoWrap
        } else {
            default_wrap_mode_for_path(&path, lang)
        };
        let title = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_string();

        let (content, backing) = match (window, loaded) {
            (Some(window), _) => {
                // The job already read the first window, so the source starts from it.
                let source = Arc::new(Mutex::new(FileWindowSource::new(path.clone(), file_size)));
                (
                    window.content.clone(),
                    DocumentBacking::FileWindow { source, window },
                )
            }
            (None, loaded) => (loaded.unwrap_or_default(), DocumentBacking::InMemory),
        };

        let buffer = fission::text_engine::TextBuffer::from_str(&content);
        let line_index = fission::text_engine::LineIndex::build(buffer.text());
        self.file_contents.insert(
            path.clone(),
            FileBuffer {
                buffer,
                language: lang,
                wrap_mode,
                document_mode,
                backing,
                cursor_line: 0,
                cursor_col: 0,
                anchor_line: 0,
                anchor_col: 0,
                edit_history: fission::text_engine::EditHistory::new(),
                line_index,
                preedit: None,
            },
        );

        // Notify LSP that the file was opened
        let language_id = match lang {
            Language::Rust => "rust",
            Language::Toml => "toml",
            Language::Markdown => "markdown",
            Language::Json => "json",
            Language::Plain => "plaintext",
        };
        if matches!(document_mode, DocumentMode::Normal | DocumentMode::Large) {
            if let Some(ref handle) = self.lsp_handle {
                if let Some(buf) = self.file_contents.get(&path) {
                    let content_str = buf.content();
                    handle.notify_open(&path, &content_str, language_id);
                }
            }
        }

        self.open_tabs.push(TabInfo {
            path: path.clone(),
            title,
            is_dirty: false,
        });
        self.active_tab = self.open_tabs.len() - 1;
        self.scroll_offset_y = 0.0;
        self.request_tree_refresh();
        self.update_breadcrumb();
        self.status_message = match document_mode {
            DocumentMode::Normal => Some(format!("Opened {}", path)),
            DocumentMode::Large => Some(format!(
                "Opened large file ({:.1} MB)",
                file_size as f64 / 1_000_000.0
            )),
            DocumentMode::Huge => {
                self.file_contents
                    .get(&path)
                    .and_then(|buf| match &buf.backing {
                        DocumentBacking::FileWindow { window, .. } => Some(format!(
                        "Opened huge file in windowed mode ({:.1} MB, lines {}..{}, bytes {}..{})",
                        file_size as f64 / 1_000_000.0,
                        window.start_line,
                        window.end_line,
                        window.start_byte,
                        window.end_byte
                    )),
                        DocumentBacking::InMemory => None,
                    })
            }
        };
    }

    pub fn close_tab(&mut self, idx: usize) {
        if idx < self.open_tabs.len() {
            let tab = self.open_tabs.remove(idx);
            self.file_contents.remove(&tab.path);
            if self.active_tab >= self.open_tabs.len() && self.active_tab > 0 {
                self.active_tab -= 1;
            }
            self.update_breadcrumb();
        }
    }

    pub fn active_buffer(&self) -> Option<(&TabInfo, &FileBuffer)> {
        self.open_tabs
            .get(self.active_tab)
            .and_then(|tab| self.file_contents.get(&tab.path).map(|buf| (tab, buf)))
    }

    pub fn active_buffer_mut(&mut self) -> Option<(&TabInfo, &mut FileBuffer)> {
        let tab = self.open_tabs.get(self.active_tab)?;
        let path = tab.path.clone();
        let buf = self.file_contents.get_mut(&path)?;
        let tab = &self.open_tabs[self.active_tab];
        Some((tab, buf))
    }

    #[allow(dead_code)]
    pub fn shift_active_file_window(&mut self, forward: bool) {
        let Some(path) = self
            .open_tabs
            .get(self.active_tab)
            .map(|tab| tab.path.clone())
        else {
            return;
        };
        let Some(buf) = self.file_contents.get_mut(&path) else {
            return;
        };
        let (source, current_start, current_end, current_window) = match &buf.backing {
            DocumentBacking::FileWindow { source, window } => (
                source.clone(),
                window.start_byte,
                window.end_byte,
                window.clone(),
            ),
            DocumentBacking::InMemory => return,
        };
        let next_window = source.lock().ok().and_then(|mut src| {
            if forward {
                src.advance_forward_from(&current_window).ok()
            } else {
                src.advance_backward_from(&current_window).ok()
            }
        });
        let Some(next_window) = next_window else {
            return;
        };
        let moved = next_window.start_byte != current_start || next_window.end_byte != current_end;
        if let DocumentBacking::FileWindow { window, .. } = &mut buf.backing {
            *window = next_window.clone();
        }
        buf.sync_content(&next_window.content);
        if forward {
            buf.set_caret_line_col(0, 0);
        } else {
            let last_line = buf.content().lines().count().saturating_sub(1);
            buf.set_caret_line_col(last_line, 0);
        }
        if moved {
            self.scroll_offset_y = 0.0;
            self.status_message = Some(format!(
                "Huge file window lines {}..{} (bytes {}..{} of {})",
                next_window.start_line,
                next_window.end_line,
                next_window.start_byte,
                next_window.end_byte,
                next_window.size_bytes
            ));
        }
    }

    pub fn notify_buffer_changed(&self, path: &str) {
        if let Some(ref handle) = self.lsp_handle {
            if let Some(buf) = self.file_contents.get(path) {
                if !buf.supports_lsp_sync() {
                    return;
                }
                let content = buf.content();
                handle.notify_change(path, &content);
            }
        }
    }

    pub fn mark_active_tab_dirty(&mut self) {
        if let Some(tab) = self.open_tabs.get_mut(self.active_tab) {
            tab.is_dirty = true;
        }
    }

    pub fn save_active_file(&mut self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if self.save_windowed(&path) {
                if let Some(tab) = self.open_tabs.get_mut(self.active_tab) {
                    tab.is_dirty = false;
                }
                self.status_message = Some(format!("Saved {}", path));
            } else if let Some(buf) = self.file_contents.get(&path) {
                let content = buf.content();
                self.queue_fs(FsOp::Save {
                    path,
                    content,
                    all: false,
                });
            }
        }
    }

    pub fn save_all_files(&mut self) {
        let mut queued = false;
        for i in 0..self.open_tabs.len() {
            if !self.open_tabs[i].is_dirty {
                continue;
            }
            let path = self.open_tabs[i].path.clone();
            if self.save_windowed(&path) {
                self.open_tabs[i].is_dirty = false;
            } else if let Some(buf) = self.file_contents.get(&path) {
                let content = buf.content();
                self.queue_fs(FsOp::Save {
                    path,
                    content,
                    all: true,
                });
                queued = true;
            }
        }
        if !queued {
            self.status_message = Some("All files saved".into());
        }
    }

    /// Saves a huge file shown a window at a time, returning whether `path` is
    /// such a file.
    ///
    /// These saves stay here rather than in `FS_JOB`: the pending edits live in
    /// the shared `FileWindowSource`, which cannot be serialized into a job.
    fn save_windowed(&mut self, path: &str) -> bool {
        let Some((source, start_line)) =
            self.file_contents
                .get(path)
                .and_then(|buf| match &buf.backing {
                    DocumentBacking::FileWindow { source, window } => {
                        Some((source.clone(), window.start_line))
                    }
                    DocumentBacking::InMemory => None,
                })
        else {
            return false;
        };
        let saved = source
            .lock()
            .ok()
            .and_then(|mut src| src.save_with_patches().ok())
            .is_some();
        if !saved {
            self.status_message = Some(format!("Failed to save {}", path));
            return true;
        }
        let reloaded = source
            .lock()
            .ok()
            .and_then(|mut src| src.load_window_for_line(start_line).ok());
        if let (Some(reloaded), Some(buf)) = (reloaded, self.file_contents.get_mut(path)) {
            if let DocumentBacking::FileWindow { window, .. } = &mut buf.backing {
                *window = reloaded.clone();
            }
            buf.sync_content(&reloaded.content);
        }
        true
    }

    pub fn run_search(&mut self) {
        let query = self.search_query.clone();
        if query.is_empty() {
            self.search_results.clear();
            return;
        }
        let mut results = Vec::new();
        // Only search open buffers (instant, no I/O)
        // TODO: Add background search via effects system for full-project search
        for (path, buf) in &self.file_contents {
            let content_str = buf.content();
            for (line_idx, line) in content_str.lines().enumerate() {
                if let Some(col) = line.to_lowercase().find(&query.to_lowercase()) {
                    results.push(SearchResult {
                        path: path.clone(),
                        line: line_idx + 1,
                        col,
                        context: line.trim().to_string(),
                    });
                }
            }
        }
        self.search_results = results;
    }

    pub fn refresh_git_status(&mut self) {
        self.git_status_generation = self.git_status_generation.wrapping_add(1);
    }

    pub fn git_status_pending(&self) -> bool {
        self.git_status_generation != self.git_status_loaded_generation
    }

    // --- Find / Replace helpers ---

    /// Search forward in the active buffer for `find_query`, populating
    /// `find_matches` and advancing `find_match_index`.
    pub fn find_next(&mut self) {
        self.rebuild_find_matches();
        if self.find_matches.is_empty() {
            self.find_match_index = 0;
            return;
        }
        if self.find_match_index + 1 < self.find_matches.len() {
            self.find_match_index += 1;
        } else {
            self.find_match_index = 0; // wrap around
        }
        self.jump_to_current_match();
    }

    /// Search backward in the active buffer for `find_query`.
    pub fn find_previous(&mut self) {
        self.rebuild_find_matches();
        if self.find_matches.is_empty() {
            self.find_match_index = 0;
            return;
        }
        if self.find_match_index > 0 {
            self.find_match_index -= 1;
        } else {
            self.find_match_index = self.find_matches.len() - 1; // wrap around
        }
        self.jump_to_current_match();
    }

    /// Replace the current match with `replace_query` and advance to next.
    pub fn replace_one(&mut self) {
        if self.find_matches.is_empty() || self.find_query.is_empty() {
            return;
        }
        let query = self.find_query.clone();
        let replacement = self.replace_query.clone();

        if let Some((_path, line, col)) = self.find_matches.get(self.find_match_index).cloned() {
            if let Some(tab) = self.open_tabs.get(self.active_tab) {
                let path = tab.path.clone();
                if let Some(buf) = self.file_contents.get_mut(&path) {
                    if let Some(line_start) = buf.line_index.line_start_byte(line) {
                        let start = line_start.saturating_add(col).min(buf.buffer.len_bytes());
                        let end = start
                            .saturating_add(query.len())
                            .min(buf.buffer.len_bytes());
                        if start <= end {
                            buf.apply_edit(start..end, &replacement);
                            let caret = start + replacement.len();
                            buf.set_selection_offsets(caret, caret);
                            self.mark_active_tab_dirty();
                            self.notify_buffer_changed(&path);
                        }
                    }
                }
            }
        }
        // Rebuild matches and advance
        self.rebuild_find_matches();
        if !self.find_matches.is_empty() && self.find_match_index >= self.find_matches.len() {
            self.find_match_index = 0;
        }
    }

    /// Replace all matches in the active buffer with `replace_query`.
    pub fn replace_all(&mut self) {
        if self.find_query.is_empty() {
            return;
        }
        let query = self.find_query.clone();
        let replacement = self.replace_query.clone();

        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get_mut(&path) {
                let content = buf.content();
                let mut matches = Vec::new();
                let mut search_from = 0usize;
                while let Some(found) = content[search_from..].find(&query) {
                    let start = search_from + found;
                    let end = start + query.len();
                    matches.push((start, end));
                    search_from = end;
                }

                if !matches.is_empty() {
                    let mut txn = fission::text_engine::EditTransaction::new();
                    for (start, end) in matches.into_iter().rev() {
                        txn.push(fission::text_engine::TextEdit::new(
                            start..end,
                            replacement.clone(),
                            &content[start..end],
                        ));
                    }
                    buf.apply_transaction(&txn);
                    self.mark_active_tab_dirty();
                    self.notify_buffer_changed(&path);
                }
            }
        }
        self.find_matches.clear();
        self.find_match_index = 0;
        self.status_message = Some("Replaced all occurrences".into());
    }

    /// Rebuild the vector of find matches from the active buffer.
    fn rebuild_find_matches(&mut self) {
        self.find_matches.clear();
        if self.find_query.is_empty() {
            return;
        }
        let query = self.find_query.clone();
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get(&path) {
                let content_str = buf.content();
                for (line_idx, line) in content_str.lines().enumerate() {
                    let mut start = 0;
                    while let Some(col) = line[start..].find(&query) {
                        self.find_matches
                            .push((path.clone(), line_idx, start + col));
                        start += col + query.len();
                    }
                }
            }
        }
    }

    /// Move the cursor to the currently selected find match.
    fn jump_to_current_match(&mut self) {
        if let Some((_path, line, col)) = self.find_matches.get(self.find_match_index).cloned() {
            if let Some(tab) = self.open_tabs.get(self.active_tab) {
                let path = tab.path.clone();
                if let Some(buf) = self.file_contents.get_mut(&path) {
                    buf.set_caret_line_col(line, col);
                }
            }
        }
    }

    // --- File operations ---

    /// Create a new file on disk and open it in a tab.
    #[allow(dead_code)]
    /// Creates an empty file at `path`, or at the first free `path-N`, then
    /// opens it.
    pub fn create_file(&mut self, path: String) {
        let taken = self.open_tabs.iter().map(|tab| tab.path.clone()).collect();
        self.queue_fs(FsOp::CreateFile { base: path, taken });
    }

    /// Creates a folder at `path`, or at the first free `path-N`, then starts
    /// renaming it.
    pub fn create_folder(&mut self, path: String) {
        self.queue_fs(FsOp::CreateFolder { base: path });
    }

    /// Update the breadcrumb path segments from the active tab's path
    /// relative to `root_path`.
    pub fn update_breadcrumb(&mut self) {
        self.breadcrumb_path.clear();
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let tab_path = Path::new(&tab.path);
            let relative = tab_path.strip_prefix(&self.root_path).unwrap_or(tab_path);
            for component in relative.components() {
                self.breadcrumb_path
                    .push(component.as_os_str().to_string_lossy().to_string());
            }
        }
    }

    // --- Rename helpers ---

    /// Start an inline rename for the given path. Populates `rename_input`
    /// with the current file/folder name so the user can edit it.
    pub fn start_rename(&mut self, path: String) {
        let name = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        self.renaming_path = Some(path);
        self.rename_input = name;
    }

    /// Confirm the rename: queues the move on disk, after which open tabs and
    /// the tree follow the new path.
    pub fn confirm_rename(&mut self) {
        if let Some(old_path) = self.renaming_path.take() {
            let new_name = self.rename_input.trim().to_string();
            self.rename_input.clear();
            if new_name.is_empty() {
                self.status_message = Some("Rename cancelled: empty name".into());
                return;
            }
            let parent = Path::new(&old_path)
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf();
            let to = parent.join(&new_name).to_string_lossy().to_string();
            self.queue_fs(FsOp::Rename { from: old_path, to });
        }
    }

    /// Cancel an in-progress rename.
    pub fn cancel_rename(&mut self) {
        self.renaming_path = None;
        self.rename_input.clear();
    }

    // --- Undo / Redo / Clipboard helpers ---

    /// Undo the last content change in the active buffer.
    pub fn undo_active(&mut self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get_mut(&path) {
                buf.undo();
            }
            self.mark_active_tab_dirty();
            self.notify_buffer_changed(&path);
        }
    }

    /// Redo the last undone change in the active buffer.
    pub fn redo_active(&mut self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get_mut(&path) {
                buf.redo();
            }
            self.mark_active_tab_dirty();
            self.notify_buffer_changed(&path);
        }
    }

    /// Copy the current line of the active buffer into the in-app clipboard.
    pub fn copy_line(&mut self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get(&path) {
                let content_str = buf.content();
                if let Some(line) = content_str.lines().nth(buf.cursor_line) {
                    self.clipboard = line.to_string();
                    self.status_message = Some("Copied line".into());
                }
            }
        }
    }

    /// Cut the current line of the active buffer into the in-app clipboard.
    pub fn cut_line(&mut self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get_mut(&path) {
                let content = buf.content();
                let line_count = content.lines().count();
                if buf.cursor_line < line_count {
                    self.clipboard = content
                        .lines()
                        .nth(buf.cursor_line)
                        .unwrap_or("")
                        .to_string();
                    if let (Some(mut start), Some(end)) = (
                        buf.line_index.line_start_byte(buf.cursor_line),
                        buf.line_index.line_end_byte(buf.cursor_line),
                    ) {
                        if end == buf.buffer.len_bytes()
                            && start > 0
                            && content.as_bytes().get(start - 1) == Some(&b'\n')
                        {
                            start -= 1;
                        }
                        buf.apply_edit(start..end, "");
                        buf.set_selection_offsets(start, start);
                        self.mark_active_tab_dirty();
                        self.notify_buffer_changed(&path);
                        self.status_message = Some("Cut line".into());
                    }
                }
            }
        }
    }

    /// Paste the in-app clipboard content at the cursor position in the active buffer.
    pub fn paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        let clip = self.clipboard.clone();
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get_mut(&path) {
                let (caret, anchor) = buf.current_offsets();
                let start = caret.min(anchor);
                let end = caret.max(anchor);
                buf.apply_edit(start..end, &clip);
                let next = start + clip.len();
                buf.set_selection_offsets(next, next);
                self.mark_active_tab_dirty();
                self.notify_buffer_changed(&path);
                self.status_message = Some("Pasted".into());
            }
        }
    }

    /// Move the cursor to the given line number (1-based).
    #[allow(dead_code)]
    pub fn go_to_line(&mut self, line: usize) {
        let target = if line > 0 { line - 1 } else { 0 };
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get_mut(&path) {
                let content_str = buf.content();
                let max_line = content_str.lines().count().saturating_sub(1);
                buf.set_caret_line_col(target.min(max_line), 0);
            }
        }
    }
}
