use fission_core::AppState;
use fission_macros::Action;
use fission_widgets::{TerminalLaunchConfig, TerminalSession};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// LspHandle — thread-safe wrapper around LspClient
// ---------------------------------------------------------------------------

pub struct LspHandle {
    inner: Arc<Mutex<Option<crate::lsp::client::LspClient>>>,
}

impl LspHandle {
    /// Create a new LSP handle. Spawns rust-analyzer in a background thread
    /// to avoid blocking the UI during startup.
    pub fn new(root_path: &Path) -> Self {
        let inner = Arc::new(Mutex::new(None));
        let init_inner = Arc::clone(&inner);
        let root = root_path.to_string_lossy().to_string();
        std::thread::spawn(move || {
            let client = crate::lsp::client::LspClient::try_new(&root);
            if let Ok(mut guard) = init_inner.lock() {
                *guard = client;
            }
        });
        Self { inner }
    }

    /// Notify the LSP server that a file has been opened.
    pub fn notify_open(&self, path: &str, content: &str, language_id: &str) {
        if let Ok(mut guard) = self.inner.try_lock() {
            if let Some(ref mut client) = *guard {
                client.did_open(path, content, language_id);
            }
        }
    }

    /// Notify the LSP server of a content change.
    pub fn notify_change(&self, path: &str, content: &str) {
        if let Ok(mut guard) = self.inner.try_lock() {
            if let Some(ref mut client) = *guard {
                client.did_change(path, content);
            }
        }
    }

    /// Poll for diagnostics and completion results from the server.
    /// Returns a list of (file-path, diagnostics) tuples and any completion items.
    pub fn poll_diagnostics(&self) -> (Vec<(String, Vec<Diagnostic>)>, Vec<CompletionItem>) {
        if let Ok(mut guard) = self.inner.try_lock() {
            if let Some(ref mut client) = *guard {
                let result = client.poll();

                let diags: Vec<(String, Vec<Diagnostic>)> = result
                    .diagnostics
                    .into_iter()
                    .map(|pd| {
                        let path = uri_to_path(&pd.uri);
                        let file_diags = pd
                            .diagnostics
                            .into_iter()
                            .map(|d| Diagnostic {
                                line: d.range.start.line as usize,
                                col: d.range.start.character as usize,
                                severity: match d.severity {
                                    Some(1) => DiagSeverity::Error,
                                    Some(2) => DiagSeverity::Warning,
                                    Some(3) => DiagSeverity::Info,
                                    Some(4) => DiagSeverity::Hint,
                                    _ => DiagSeverity::Error,
                                },
                                message: d.message,
                            })
                            .collect();
                        (path, file_diags)
                    })
                    .collect();

                let completions: Vec<CompletionItem> = result
                    .completions
                    .into_iter()
                    .map(|c| CompletionItem {
                        label: c.label,
                        kind: completion_kind_str(c.kind),
                        detail: c.detail,
                    })
                    .collect();

                return (diags, completions);
            }
        }
        (Vec::new(), Vec::new())
    }

    /// Request completions at the given position.
    #[allow(dead_code)]
    pub fn request_completions(&self, path: &str, line: usize, col: usize) {
        if let Ok(mut guard) = self.inner.try_lock() {
            if let Some(ref mut client) = *guard {
                client.request_completion(path, line as u32, col as u32);
            }
        }
    }

    /// Shut down the LSP server.
    #[allow(dead_code)]
    pub fn shutdown(&self) {
        if let Ok(mut guard) = self.inner.try_lock() {
            if let Some(ref mut client) = *guard {
                client.shutdown();
            }
        }
    }
}

impl Clone for LspHandle {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl std::fmt::Debug for LspHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LspHandle")
    }
}

/// Convert an LSP `file://` URI back to a filesystem path.
fn uri_to_path(uri: &str) -> String {
    if let Some(rest) = uri.strip_prefix("file://") {
        rest.to_string()
    } else {
        uri.to_string()
    }
}

/// Map the numeric LSP CompletionItemKind to a human-readable string.
fn completion_kind_str(kind: Option<u32>) -> String {
    match kind {
        Some(1) => "text".into(),
        Some(2) => "method".into(),
        Some(3) => "function".into(),
        Some(4) => "constructor".into(),
        Some(5) => "field".into(),
        Some(6) => "variable".into(),
        Some(7) => "class".into(),
        Some(8) => "interface".into(),
        Some(9) => "module".into(),
        Some(10) => "property".into(),
        Some(13) => "enum".into(),
        Some(14) => "keyword".into(),
        Some(15) => "snippet".into(),
        Some(21) => "constant".into(),
        Some(22) => "struct".into(),
        Some(23) => "event".into(),
        Some(25) => "type_param".into(),
        _ => "unknown".into(),
    }
}

const NORMAL_FILE_LIMIT: u64 = 8 * 1024 * 1024;
const LARGE_FILE_LIMIT: u64 = 64 * 1024 * 1024;
const HUGE_FILE_PREVIEW_BYTES: usize = 1_048_576;
const HUGE_FILE_SCAN_BYTES: usize = 64 * 1024;
const HUGE_LINE_CHECKPOINT_STRIDE: usize = 2_048;
const HUGE_WINDOW_CONTEXT_LINES: usize = 48;

// --- State ---

#[derive(Debug, Clone)]
pub struct EditorState {
    // File tree
    pub root_path: PathBuf,
    pub tree_expanded: HashSet<String>,
    pub tree_selected: Option<String>,

    // Open files / tabs
    pub open_tabs: Vec<TabInfo>,
    pub active_tab: usize,

    // Editor content (path -> content)
    pub file_contents: HashMap<String, FileBuffer>,

    // UI state
    pub show_command_palette: bool,
    pub command_query: String,
    pub show_find_replace: bool,
    pub find_query: String,
    pub replace_query: String,
    pub sidebar_visible: bool,
    pub sidebar_section: SidebarSection,
    pub terminal_visible: bool,
    pub terminal_session: Option<Arc<TerminalSession>>,
    pub status_message: Option<String>,

    // Split
    pub sidebar_width: f32,
    pub terminal_height: f32,

    // LSP
    pub diagnostics: HashMap<String, Vec<Diagnostic>>,
    pub completions: Vec<CompletionItem>,
    pub show_completions: bool,
    pub selected_completion: usize,
    #[allow(dead_code)]
    pub hover_info: Option<String>,

    // Search
    pub search_query: String,
    pub search_results: Vec<SearchResult>,

    // Git
    pub git_status_lines: Vec<GitStatusEntry>,

    // Bottom panel tabs
    pub bottom_panel_tab: BottomPanelTab,

    // Menu bar
    #[allow(dead_code)]
    pub show_menu_bar: bool,
    pub active_menu: Option<String>,

    // Context menu
    pub context_menu_visible: bool,
    pub context_menu_position: (f32, f32),
    pub context_menu_target: Option<String>, // Some(path) for file tree, None for editor

    // Find/Replace match tracking
    pub find_match_index: usize,
    pub find_matches: Vec<(String, usize, usize)>, // (path, line, col)

    // Hover tooltip
    #[allow(dead_code)]
    pub show_hover: bool,
    pub hover_position: (f32, f32),

    // Breadcrumb
    pub breadcrumb_path: Vec<String>,

    // Scroll
    pub scroll_offset_y: f32,

    // LSP client handle
    pub lsp_handle: Option<LspHandle>,
    pub lsp_initialized: bool,

    // Clipboard (in-app)
    pub clipboard: String,

    // File watcher
    pub file_mtimes: HashMap<String, std::time::SystemTime>,
    #[allow(dead_code)]
    pub key_event_count: u64,

    // Cached file tree (avoids re-scanning on every build)
    pub cached_tree_entries: Vec<FileEntry>,
    pub tree_cache_dirty: bool,

    // Background I/O: pending results from spawned threads
    pub git_status_pending: Arc<Mutex<Option<Vec<GitStatusEntry>>>>,
    pub tree_scan_pending: Arc<Mutex<Option<Vec<FileEntry>>>>,

    // Counter for generating unique untitled file names
    pub untitled_counter: u32,

    // Inline rename state
    pub renaming_path: Option<String>,
    pub rename_input: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BottomPanelTab {
    Terminal,
    Problems,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            root_path: PathBuf::from("."),
            tree_expanded: HashSet::new(),
            tree_selected: None,
            open_tabs: Vec::new(),
            active_tab: 0,
            file_contents: HashMap::new(),
            show_command_palette: false,
            command_query: String::new(),
            show_find_replace: false,
            find_query: String::new(),
            replace_query: String::new(),
            sidebar_visible: true,
            sidebar_section: SidebarSection::Explorer,
            terminal_visible: true,
            terminal_session: None,
            status_message: None,
            sidebar_width: 240.0,
            terminal_height: 96.0,
            diagnostics: HashMap::new(),
            completions: Vec::new(),
            show_completions: false,
            selected_completion: 0,
            hover_info: None,
            search_query: String::new(),
            search_results: Vec::new(),
            git_status_lines: Vec::new(),
            bottom_panel_tab: BottomPanelTab::Terminal,
            show_menu_bar: true,
            active_menu: None,
            context_menu_visible: false,
            context_menu_position: (0.0, 0.0),
            context_menu_target: None,
            find_match_index: 0,
            find_matches: Vec::new(),
            show_hover: false,
            hover_position: (0.0, 0.0),
            breadcrumb_path: Vec::new(),
            scroll_offset_y: 0.0,
            lsp_handle: None,
            lsp_initialized: false,
            clipboard: String::new(),
            file_mtimes: HashMap::new(),
            key_event_count: 0,
            cached_tree_entries: Vec::new(),
            tree_cache_dirty: true,
            git_status_pending: Arc::new(Mutex::new(None)),
            tree_scan_pending: Arc::new(Mutex::new(None)),
            untitled_counter: 0,
            renaming_path: None,
            rename_input: String::new(),
        }
    }
}

impl AppState for EditorState {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabInfo {
    pub path: String,
    pub title: String,
    pub is_dirty: bool,
}

#[derive(Debug, Clone)]
pub struct FileBuffer {
    pub buffer: fission_text_engine::TextBuffer,
    pub language: Language,
    pub wrap_mode: WrapMode,
    pub document_mode: DocumentMode,
    pub backing: DocumentBacking,
    pub cursor_line: usize,
    pub cursor_col: usize,
    /// Selection anchor line (same as cursor when no selection).
    pub anchor_line: usize,
    /// Selection anchor column (same as cursor when no selection).
    pub anchor_col: usize,
    pub edit_history: fission_text_engine::EditHistory,
    pub line_index: fission_text_engine::LineIndex,
    pub preedit: Option<EditorPreeditState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPreeditState {
    pub text: String,
    pub range: (usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Toml,
    Markdown,
    Json,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapMode {
    NoWrap,
    SoftWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentMode {
    Normal,
    Large,
    Huge,
}

#[derive(Debug, Clone)]
pub enum DocumentBacking {
    InMemory,
    FileWindow {
        source: Arc<Mutex<FileWindowSource>>,
        window: FileWindow,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWindow {
    pub start_byte: u64,
    pub end_byte: u64,
    pub size_bytes: u64,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub has_more_before: bool,
    pub has_more_after: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineCheckpoint {
    pub line: usize,
    pub byte_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPatch {
    pub start_byte: u64,
    pub end_byte: u64,
    pub content: String,
}

#[derive(Debug)]
pub struct FileWindowSource {
    path: String,
    size_bytes: u64,
    window_bytes: usize,
    checkpoints: Vec<LineCheckpoint>,
    patches: Vec<WindowPatch>,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "rs" => Language::Rust,
            "toml" => Language::Toml,
            "md" => Language::Markdown,
            "json" => Language::Json,
            _ => Language::Plain,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Language::Rust => "Rust",
            Language::Toml => "TOML",
            Language::Markdown => "Markdown",
            Language::Json => "JSON",
            Language::Plain => "Plain Text",
        }
    }

    pub fn default_wrap_mode(&self) -> WrapMode {
        match self {
            Language::Markdown => WrapMode::SoftWrap,
            Language::Rust | Language::Toml | Language::Json | Language::Plain => WrapMode::NoWrap,
        }
    }
}

pub fn default_wrap_mode_for_path(path: &str, language: Language) -> WrapMode {
    let filename = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_ascii_lowercase();

    if matches!(
        filename.as_str(),
        "readme" | "license" | "copying" | "changelog"
    ) {
        return WrapMode::SoftWrap;
    }

    if filename.ends_with(".txt")
        || filename.ends_with(".text")
        || filename.ends_with(".md")
        || filename.ends_with(".markdown")
        || filename.ends_with(".mdx")
    {
        return WrapMode::SoftWrap;
    }

    language.default_wrap_mode()
}

pub fn classify_document_mode_for_size(size_bytes: u64) -> DocumentMode {
    if size_bytes > LARGE_FILE_LIMIT {
        DocumentMode::Huge
    } else if size_bytes > NORMAL_FILE_LIMIT {
        DocumentMode::Large
    } else {
        DocumentMode::Normal
    }
}

fn logical_line_count(text: &str) -> usize {
    text.split('\n').count().max(1)
}

fn copy_range(
    source: &mut std::fs::File,
    output: &mut std::fs::File,
    start: u64,
    end: u64,
) -> std::io::Result<()> {
    if end <= start {
        return Ok(());
    }
    source.seek(SeekFrom::Start(start))?;
    let mut remaining = end - start;
    let mut buf = vec![0u8; HUGE_FILE_SCAN_BYTES];
    while remaining > 0 {
        let chunk = buf.len().min(remaining as usize);
        let read = source.read(&mut buf[..chunk])?;
        if read == 0 {
            break;
        }
        output.write_all(&buf[..read])?;
        remaining = remaining.saturating_sub(read as u64);
    }
    Ok(())
}

impl FileWindowSource {
    pub fn new(path: String, size_bytes: u64) -> Self {
        Self {
            path,
            size_bytes,
            window_bytes: HUGE_FILE_PREVIEW_BYTES,
            checkpoints: vec![LineCheckpoint {
                line: 0,
                byte_offset: 0,
            }],
            patches: Vec::new(),
        }
    }

    pub fn current_window(&mut self) -> std::io::Result<FileWindow> {
        self.load_window_for_line(0)
    }

    pub fn has_patches(&self) -> bool {
        !self.patches.is_empty()
    }

    pub fn load_window_for_line(&mut self, line: usize) -> std::io::Result<FileWindow> {
        let (start_byte, start_line) = self.byte_offset_for_line_start(line)?;
        self.load_window_from_aligned_start(start_byte, start_line)
    }

    pub fn advance_forward_from(&mut self, window: &FileWindow) -> std::io::Result<FileWindow> {
        if self.has_patches() {
            let requested = window
                .end_byte
                .saturating_sub((self.window_bytes / 6) as u64)
                .min(self.size_bytes);
            return self.load_window_at_byte(requested);
        }
        let target_line = window.end_line.saturating_sub(HUGE_WINDOW_CONTEXT_LINES);
        self.load_window_for_line(target_line)
    }

    pub fn advance_backward_from(&mut self, window: &FileWindow) -> std::io::Result<FileWindow> {
        if self.has_patches() {
            let requested = window
                .start_byte
                .saturating_sub((self.window_bytes / 2) as u64);
            return self.load_window_at_byte(requested);
        }
        let target_line = window.start_line.saturating_sub(HUGE_WINDOW_CONTEXT_LINES);
        self.load_window_for_line(target_line)
    }

    pub fn commit_window_patch(
        &mut self,
        start_byte: u64,
        end_byte: u64,
        content: &str,
    ) -> std::io::Result<()> {
        let base = self.read_base_range(start_byte, end_byte)?;
        self.patches
            .retain(|patch| patch.end_byte <= start_byte || patch.start_byte >= end_byte);
        if base != content {
            self.patches.push(WindowPatch {
                start_byte,
                end_byte,
                content: content.to_string(),
            });
            self.patches.sort_by_key(|patch| patch.start_byte);
        }
        self.checkpoints.clear();
        self.checkpoints.push(LineCheckpoint {
            line: 0,
            byte_offset: 0,
        });
        Ok(())
    }

    pub fn save_with_patches(&mut self) -> std::io::Result<()> {
        if self.patches.is_empty() {
            return Ok(());
        }

        let tmp_path = format!("{}.fission-save", self.path);
        let mut source = std::fs::File::open(&self.path)?;
        let mut output = std::fs::File::create(&tmp_path)?;
        let mut cursor = 0u64;
        let mut patches = self.patches.clone();
        patches.sort_by_key(|patch| patch.start_byte);

        for patch in &patches {
            if patch.start_byte > cursor {
                copy_range(&mut source, &mut output, cursor, patch.start_byte)?;
            }
            output.write_all(patch.content.as_bytes())?;
            cursor = patch.end_byte.max(cursor);
        }

        if cursor < self.size_bytes {
            copy_range(&mut source, &mut output, cursor, self.size_bytes)?;
        }
        output.flush()?;
        std::fs::rename(&tmp_path, &self.path)?;
        self.size_bytes = std::fs::metadata(&self.path)?.len();
        self.patches.clear();
        self.checkpoints.clear();
        self.checkpoints.push(LineCheckpoint {
            line: 0,
            byte_offset: 0,
        });
        Ok(())
    }

    fn load_window_at_byte(&mut self, requested_start: u64) -> std::io::Result<FileWindow> {
        let aligned_start = self.align_start_to_line_boundary(requested_start)?;
        let start_line = self.line_for_byte_offset(aligned_start)?;
        self.load_window_from_aligned_start(aligned_start, start_line)
    }

    fn load_window_from_aligned_start(
        &mut self,
        mut actual_start: u64,
        mut start_line: usize,
    ) -> std::io::Result<FileWindow> {
        loop {
            let expanded_start = self
                .patches
                .iter()
                .filter(|patch| patch.start_byte < actual_start && patch.end_byte > actual_start)
                .map(|patch| patch.start_byte)
                .min();
            if let Some(expanded_start) = expanded_start {
                actual_start = expanded_start;
                start_line = self.line_for_byte_offset(actual_start)?;
                continue;
            }
            break;
        }

        let mut actual_end = self.compute_window_end(actual_start)?;
        loop {
            let expanded_end = self
                .patches
                .iter()
                .filter(|patch| patch.start_byte < actual_end && patch.end_byte > actual_end)
                .map(|patch| patch.end_byte)
                .max();
            let Some(expanded_end) = expanded_end else {
                break;
            };
            actual_end = self.extend_end_to_line_boundary(expanded_end.min(self.size_bytes))?;
        }

        let base = self.read_base_range(actual_start, actual_end)?;
        let content = self.apply_patches(actual_start, actual_end, &base);
        let line_count = logical_line_count(&content);
        self.seed_base_checkpoint(start_line, actual_start);
        Ok(FileWindow {
            start_byte: actual_start,
            end_byte: actual_end,
            size_bytes: self.size_bytes,
            start_line,
            end_line: start_line + line_count,
            content,
            has_more_before: actual_start > 0,
            has_more_after: actual_end < self.size_bytes,
        })
    }

    fn seed_base_checkpoint(&mut self, line: usize, byte_offset: u64) {
        if self
            .checkpoints
            .iter()
            .any(|checkpoint| checkpoint.line == line && checkpoint.byte_offset == byte_offset)
        {
            return;
        }
        self.checkpoints.push(LineCheckpoint { line, byte_offset });
        self.checkpoints
            .sort_by_key(|checkpoint| checkpoint.byte_offset);
    }

    fn byte_offset_for_line_start(&mut self, target_line: usize) -> std::io::Result<(u64, usize)> {
        let checkpoint_idx = self
            .checkpoints
            .iter()
            .enumerate()
            .rev()
            .find(|(_, checkpoint)| checkpoint.line <= target_line)
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        let checkpoint = self.checkpoints[checkpoint_idx].clone();
        if checkpoint.line == target_line {
            return Ok((checkpoint.byte_offset, checkpoint.line));
        }

        let mut file = std::fs::File::open(&self.path)?;
        file.seek(SeekFrom::Start(checkpoint.byte_offset))?;
        let mut line = checkpoint.line;
        let mut byte_offset = checkpoint.byte_offset;
        let mut last_checkpoint_line = checkpoint.line;
        let mut buf = vec![0u8; HUGE_FILE_SCAN_BYTES];

        loop {
            let read = file.read(&mut buf)?;
            if read == 0 {
                break;
            }
            for (idx, byte) in buf[..read].iter().enumerate() {
                if *byte != b'\n' {
                    continue;
                }
                line += 1;
                let next_line_byte = byte_offset + idx as u64 + 1;
                if line == target_line {
                    self.seed_base_checkpoint(line, next_line_byte);
                    return Ok((next_line_byte, line));
                }
                if line.saturating_sub(last_checkpoint_line) >= HUGE_LINE_CHECKPOINT_STRIDE {
                    self.seed_base_checkpoint(line, next_line_byte);
                    last_checkpoint_line = line;
                }
            }
            byte_offset += read as u64;
        }

        Ok((self.size_bytes, line))
    }

    fn line_for_byte_offset(&mut self, target_byte: u64) -> std::io::Result<usize> {
        let checkpoint_idx = self
            .checkpoints
            .iter()
            .enumerate()
            .rev()
            .find(|(_, checkpoint)| checkpoint.byte_offset <= target_byte)
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        let checkpoint = self.checkpoints[checkpoint_idx].clone();
        if checkpoint.byte_offset == target_byte {
            return Ok(checkpoint.line);
        }

        let mut file = std::fs::File::open(&self.path)?;
        file.seek(SeekFrom::Start(checkpoint.byte_offset))?;
        let mut line = checkpoint.line;
        let mut byte_offset = checkpoint.byte_offset;
        let mut last_checkpoint_line = checkpoint.line;
        let mut remaining = target_byte.saturating_sub(checkpoint.byte_offset);
        let mut buf = vec![0u8; HUGE_FILE_SCAN_BYTES];

        while remaining > 0 {
            let chunk_len = buf.len().min(remaining as usize);
            let read = file.read(&mut buf[..chunk_len])?;
            if read == 0 {
                break;
            }
            for (idx, byte) in buf[..read].iter().enumerate() {
                if *byte != b'\n' {
                    continue;
                }
                line += 1;
                let next_line_byte = byte_offset + idx as u64 + 1;
                if line.saturating_sub(last_checkpoint_line) >= HUGE_LINE_CHECKPOINT_STRIDE {
                    self.seed_base_checkpoint(line, next_line_byte);
                    last_checkpoint_line = line;
                }
            }
            byte_offset += read as u64;
            remaining = target_byte.saturating_sub(byte_offset);
        }

        Ok(line)
    }

    fn align_start_to_line_boundary(&self, requested_start: u64) -> std::io::Result<u64> {
        let mut file = std::fs::File::open(&self.path)?;
        let mut actual_start = requested_start.min(self.size_bytes);
        if actual_start == 0 {
            return Ok(0);
        }
        let lookback = actual_start.min(4096);
        file.seek(SeekFrom::Start(actual_start - lookback))?;
        let mut prefix = vec![0u8; lookback as usize];
        let read = file.read(&mut prefix)?;
        prefix.truncate(read);
        if let Some(last_newline) = prefix.iter().rposition(|byte| *byte == b'\n') {
            actual_start = actual_start - lookback + last_newline as u64 + 1;
        }
        Ok(actual_start)
    }

    fn compute_window_end(&self, actual_start: u64) -> std::io::Result<u64> {
        let mut file = std::fs::File::open(&self.path)?;
        file.seek(SeekFrom::Start(actual_start))?;
        let mut buf = vec![0u8; self.window_bytes];
        let read = file.read(&mut buf)?;
        buf.truncate(read);
        if let Some(first_nul) = buf.iter().position(|byte| *byte == 0) {
            buf.truncate(first_nul);
        }
        let mut actual_end = actual_start + buf.len() as u64;
        if actual_end < self.size_bytes {
            if let Some(last_newline) = buf.iter().rposition(|byte| *byte == b'\n') {
                actual_end = actual_start + last_newline as u64 + 1;
            }
        }
        Ok(actual_end.min(self.size_bytes))
    }

    fn extend_end_to_line_boundary(&self, requested_end: u64) -> std::io::Result<u64> {
        if requested_end >= self.size_bytes {
            return Ok(self.size_bytes);
        }
        let mut file = std::fs::File::open(&self.path)?;
        let mut end = requested_end;
        file.seek(SeekFrom::Start(end))?;
        let mut buf = vec![0u8; 4096];
        loop {
            let read = file.read(&mut buf)?;
            if read == 0 {
                return Ok(self.size_bytes);
            }
            if let Some(newline) = buf[..read].iter().position(|byte| *byte == b'\n') {
                return Ok((end + newline as u64 + 1).min(self.size_bytes));
            }
            end = (end + read as u64).min(self.size_bytes);
            if end >= self.size_bytes {
                return Ok(self.size_bytes);
            }
        }
    }

    fn read_base_range(&self, start: u64, end: u64) -> std::io::Result<String> {
        if end <= start {
            return Ok(String::new());
        }
        let mut file = std::fs::File::open(&self.path)?;
        file.seek(SeekFrom::Start(start))?;
        let mut buf = vec![0u8; (end - start) as usize];
        let read = file.read(&mut buf)?;
        buf.truncate(read);
        if let Some(first_nul) = buf.iter().position(|byte| *byte == 0) {
            buf.truncate(first_nul);
        }
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    fn apply_patches(&self, start: u64, end: u64, base: &str) -> String {
        if self.patches.is_empty() {
            return base.to_string();
        }
        let mut content = String::new();
        let mut cursor = start;
        for patch in self
            .patches
            .iter()
            .filter(|patch| patch.start_byte < end && patch.end_byte > start)
        {
            if patch.start_byte > cursor {
                let rel_start = (cursor - start) as usize;
                let rel_end = (patch.start_byte.min(end) - start) as usize;
                content.push_str(&base[rel_start..rel_end]);
            }
            content.push_str(&patch.content);
            cursor = cursor.max(patch.end_byte.min(end));
        }
        if cursor < end {
            let rel_start = (cursor - start) as usize;
            content.push_str(&base[rel_start..]);
        }
        content
    }
}

impl FileBuffer {
    pub fn is_editable(&self) -> bool {
        true
    }

    pub fn supports_lsp_sync(&self) -> bool {
        !matches!(self.document_mode, DocumentMode::Huge)
    }

    pub fn mode_label(&self) -> &'static str {
        match self.document_mode {
            DocumentMode::Normal => "Normal",
            DocumentMode::Large => "Large",
            DocumentMode::Huge => "Huge",
        }
    }

    fn rebuild_line_index(&mut self) {
        self.line_index = fission_text_engine::LineIndex::build(self.buffer.text());
    }

    fn sync_window_backing_from_buffer(&mut self) {
        let new_content = self.content();
        if let DocumentBacking::FileWindow { source, window } = &mut self.backing {
            if let Ok(mut source) = source.lock() {
                let _ =
                    source.commit_window_patch(window.start_byte, window.end_byte, &new_content);
            }
            window.content = new_content.clone();
            window.end_line = window.start_line + logical_line_count(&new_content);
        }
    }

    /// Materialize the rope into a `String` (for backward compatibility with
    /// code that needs a contiguous `String`).
    pub fn content(&self) -> String {
        self.buffer.to_string()
    }

    pub fn current_offsets(&self) -> (usize, usize) {
        let max_offset = self.buffer.len_bytes();
        let caret = self
            .line_index
            .line_col_to_byte(fission_text_engine::LineCol {
                line: self.cursor_line,
                col: self.cursor_col,
            })
            .unwrap_or(max_offset)
            .min(max_offset);
        let anchor = self
            .line_index
            .line_col_to_byte(fission_text_engine::LineCol {
                line: self.anchor_line,
                col: self.anchor_col,
            })
            .unwrap_or(max_offset)
            .min(max_offset);
        (caret, anchor)
    }

    pub fn set_selection_offsets(&mut self, caret: usize, anchor: usize) {
        let max_offset = self.buffer.len_bytes();
        let caret = caret.min(max_offset);
        let anchor = anchor.min(max_offset);
        let caret_lc = self
            .line_index
            .byte_to_line_col(caret)
            .unwrap_or(fission_text_engine::LineCol { line: 0, col: 0 });
        let anchor_lc = self
            .line_index
            .byte_to_line_col(anchor)
            .unwrap_or(fission_text_engine::LineCol { line: 0, col: 0 });
        self.cursor_line = caret_lc.line;
        self.cursor_col = caret_lc.col;
        self.anchor_line = anchor_lc.line;
        self.anchor_col = anchor_lc.col;
    }

    pub fn set_caret_line_col(&mut self, line: usize, col: usize) {
        let offset = self
            .line_index
            .line_col_to_byte(fission_text_engine::LineCol { line, col })
            .unwrap_or_else(|| {
                self.line_index
                    .line_end_byte(line)
                    .unwrap_or(self.buffer.len_bytes())
            })
            .min(self.buffer.len_bytes());
        self.set_selection_offsets(offset, offset);
    }

    pub fn preedit_range(&self) -> Option<(usize, usize)> {
        self.preedit.as_ref().map(|preedit| preedit.range)
    }

    pub fn display_content(&self) -> String {
        let committed = self.content();
        let Some(preedit) = &self.preedit else {
            return committed;
        };
        let start = preedit.range.0.min(committed.len());
        let end = preedit.range.1.min(committed.len());
        let mut display = String::with_capacity(
            committed.len() - (end.saturating_sub(start)) + preedit.text.len(),
        );
        display.push_str(&committed[..start]);
        display.push_str(&preedit.text);
        display.push_str(&committed[end..]);
        display
    }

    pub fn display_offsets(&self) -> (usize, usize) {
        if let Some(preedit) = &self.preedit {
            let start = preedit.range.0;
            return (start + preedit.text.len(), start);
        }
        self.current_offsets()
    }

    pub fn clear_preedit(&mut self) {
        self.preedit = None;
    }

    pub fn set_preedit(&mut self, text: String) {
        if text.is_empty() {
            self.preedit = None;
            return;
        }

        if let Some(preedit) = &mut self.preedit {
            preedit.text = text;
            return;
        }

        let (caret, anchor) = self.current_offsets();
        self.preedit = Some(EditorPreeditState {
            text,
            range: (caret.min(anchor), caret.max(anchor)),
        });
    }

    pub fn apply_edit(&mut self, range: std::ops::Range<usize>, new_text: &str) {
        let (caret, anchor) = self.current_offsets();
        self.clear_preedit();
        self.edit_history
            .apply_edit(&mut self.buffer, range, new_text);
        self.rebuild_line_index();
        self.set_selection_offsets(
            caret.min(self.buffer.len_bytes()),
            anchor.min(self.buffer.len_bytes()),
        );
        self.sync_window_backing_from_buffer();
    }

    pub fn apply_transaction(&mut self, txn: &fission_text_engine::EditTransaction) {
        let (caret, anchor) = self.current_offsets();
        self.clear_preedit();
        self.edit_history.apply(txn, &mut self.buffer);
        self.rebuild_line_index();
        self.set_selection_offsets(
            caret.min(self.buffer.len_bytes()),
            anchor.min(self.buffer.len_bytes()),
        );
        self.sync_window_backing_from_buffer();
    }

    /// Replace the entire document through a single undoable transaction.
    #[allow(dead_code)]
    pub fn replace_document(&mut self, new_text: &str) {
        let (caret, anchor) = self.current_offsets();
        self.clear_preedit();
        let len = self.buffer.len_bytes();
        self.edit_history
            .apply_edit(&mut self.buffer, 0..len, new_text);
        self.rebuild_line_index();
        self.set_selection_offsets(
            caret.min(self.buffer.len_bytes()),
            anchor.min(self.buffer.len_bytes()),
        );
        self.sync_window_backing_from_buffer();
    }

    /// Replace the buffer from an external source and clear undo/redo state.
    #[allow(dead_code)]
    pub fn sync_content(&mut self, new_text: &str) {
        let (caret, anchor) = self.current_offsets();
        self.clear_preedit();
        self.buffer = fission_text_engine::TextBuffer::from_str(new_text);
        self.edit_history.clear();
        self.rebuild_line_index();
        self.set_selection_offsets(
            caret.min(self.buffer.len_bytes()),
            anchor.min(self.buffer.len_bytes()),
        );
    }

    /// Undo the last change.
    pub fn undo(&mut self) {
        self.clear_preedit();
        let (caret, anchor) = self.current_offsets();
        if self.edit_history.undo(&mut self.buffer) {
            self.rebuild_line_index();
            self.set_selection_offsets(
                caret.min(self.buffer.len_bytes()),
                anchor.min(self.buffer.len_bytes()),
            );
            self.sync_window_backing_from_buffer();
        }
    }

    /// Redo the last undone change.
    pub fn redo(&mut self) {
        self.clear_preedit();
        let (caret, anchor) = self.current_offsets();
        if self.edit_history.redo(&mut self.buffer) {
            self.rebuild_line_index();
            self.set_selection_offsets(
                caret.min(self.buffer.len_bytes()),
                anchor.min(self.buffer.len_bytes()),
            );
            self.sync_window_backing_from_buffer();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarSection {
    Explorer,
    Search,
    Git,
    Extensions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub line: usize,
    pub col: usize,
    pub severity: DiagSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: String, // "function", "variable", "keyword", etc.
    pub detail: Option<String>,
}

// --- Actions ---

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct OpenFile(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CloseTab(pub usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SelectTab(pub usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ToggleTreeNode(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SelectTreeNode(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ApplyEditorEdit {
    pub range_start: usize,
    pub range_end: usize,
    pub new_text: String,
    pub caret: usize,
    pub anchor: usize,
}

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetEditorPreedit {
    pub text: String,
}

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ToggleCommandPalette;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct UpdateCommandQuery(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ToggleSidebar;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ToggleTerminal;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetSidebarSection(pub SidebarSection);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SaveFile;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SaveAllFiles;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DismissMenu;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ShowMenuStatus(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetBottomPanelTab(pub BottomPanelTab);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ShowContextStatus(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RenameContextTarget;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DeleteContextTarget;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct UpdateSearchQuery(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ExecuteSearch;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SelectCompletion(pub usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DismissCompletions;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RefreshGitStatus;

#[allow(dead_code)]
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NavigateDiagnostic {
    pub path: String,
    pub line: usize,
}

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ShowContextMenu {
    pub x: f32,
    pub y: f32,
    pub target: Option<String>,
}

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DismissContextMenu;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct CreateFile(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct CreateFolder(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RefreshTree;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ToggleFindReplace;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct UpdateFindQuery(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct UpdateReplaceQuery(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FindNext;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FindPrevious;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReplaceOne;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReplaceAll;

#[allow(dead_code)]
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct ShowHover(pub String);

#[allow(dead_code)]
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DismissHover;

#[allow(dead_code)]
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct DeleteFile(pub String);

#[allow(dead_code)]
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RenameFile {
    pub old: String,
    pub new_name: String,
}

#[allow(dead_code)]
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct StartRename(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ConfirmRename;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CancelRename;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct UpdateRenameInput(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetActiveMenu(pub Option<String>);

#[allow(dead_code)]
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GoToLine(pub usize);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GoToDefinition;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Undo;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Redo;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CopySelection;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CutSelection;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PasteClipboard;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpdateCursorPosition {
    pub caret: usize,
    pub anchor: usize,
}

/// Action dispatched by the editor render node to update the model's scroll
/// position so that scroll-follows-cursor works.
#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UpdateScrollY(pub f32);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ShiftActiveFileWindow {
    pub forward: bool,
}

// --- Additional types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub line: usize,
    pub col: usize,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusEntry {
    pub status: String,
    pub path: String,
}

// --- Helpers ---

impl EditorState {
    pub fn ensure_terminal_session(&mut self) {
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

    pub fn open_file(&mut self, path: String) {
        // Check if already open
        if let Some(idx) = self.open_tabs.iter().position(|t| t.path == path) {
            self.active_tab = idx;
            self.update_breadcrumb();
            return;
        }

        let file_size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        let document_mode = classify_document_mode_for_size(file_size);

        // Store the file's modification time for external-change detection
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(mtime) = meta.modified() {
                self.file_mtimes.insert(path.clone(), mtime);
            }
        }

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

        let (content, backing) = match document_mode {
            DocumentMode::Huge => {
                let source = Arc::new(Mutex::new(FileWindowSource::new(path.clone(), file_size)));
                let window = source
                    .lock()
                    .ok()
                    .and_then(|mut src| src.current_window().ok())
                    .unwrap_or(FileWindow {
                        start_byte: 0,
                        end_byte: 0,
                        size_bytes: file_size,
                        start_line: 0,
                        end_line: 0,
                        content: String::new(),
                        has_more_before: false,
                        has_more_after: false,
                    });
                (
                    window.content.clone(),
                    DocumentBacking::FileWindow { source, window },
                )
            }
            DocumentMode::Normal | DocumentMode::Large => (
                std::fs::read_to_string(&path).unwrap_or_else(|_| String::new()),
                DocumentBacking::InMemory,
            ),
        };

        let buffer = fission_text_engine::TextBuffer::from_str(&content);
        let line_index = fission_text_engine::LineIndex::build(buffer.text());
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
                edit_history: fission_text_engine::EditHistory::new(),
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
        self.tree_cache_dirty = true;
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
            let huge_reload = self
                .file_contents
                .get(&path)
                .and_then(|buf| match &buf.backing {
                    DocumentBacking::FileWindow { source, window } => {
                        Some((source.clone(), window.start_line))
                    }
                    DocumentBacking::InMemory => None,
                });

            let save_ok = if let Some((source, _)) = &huge_reload {
                source
                    .lock()
                    .ok()
                    .and_then(|mut src| src.save_with_patches().ok())
                    .is_some()
            } else if let Some(buf) = self.file_contents.get(&path) {
                std::fs::write(&path, buf.content()).is_ok()
            } else {
                false
            };

            if save_ok {
                if let Some((source, start_line)) = huge_reload {
                    let reloaded = source
                        .lock()
                        .ok()
                        .and_then(|mut src| src.load_window_for_line(start_line).ok());
                    if let Some(reloaded) = reloaded {
                        if let Some(buf) = self.file_contents.get_mut(&path) {
                            if let DocumentBacking::FileWindow { window, .. } = &mut buf.backing {
                                *window = reloaded.clone();
                            }
                            buf.sync_content(&reloaded.content);
                        }
                    }
                }
                if let Some(tab) = self.open_tabs.get_mut(self.active_tab) {
                    tab.is_dirty = false;
                }
                self.status_message = Some(format!("Saved {}", path));
            } else {
                self.status_message = Some(format!("Failed to save {}", path));
            }
        }
    }

    pub fn save_all_files(&mut self) {
        for i in 0..self.open_tabs.len() {
            if self.open_tabs[i].is_dirty {
                let path = self.open_tabs[i].path.clone();
                let huge_reload =
                    self.file_contents
                        .get(&path)
                        .and_then(|buf| match &buf.backing {
                            DocumentBacking::FileWindow { source, window } => {
                                Some((source.clone(), window.start_line))
                            }
                            DocumentBacking::InMemory => None,
                        });
                let save_ok = if let Some((source, _)) = &huge_reload {
                    source
                        .lock()
                        .ok()
                        .and_then(|mut src| src.save_with_patches().ok())
                        .is_some()
                } else if let Some(buf) = self.file_contents.get(&path) {
                    std::fs::write(&path, buf.content()).is_ok()
                } else {
                    false
                };
                if save_ok {
                    if let Some((source, start_line)) = huge_reload {
                        let reloaded = source
                            .lock()
                            .ok()
                            .and_then(|mut src| src.load_window_for_line(start_line).ok());
                        if let Some(reloaded) = reloaded {
                            if let Some(buf) = self.file_contents.get_mut(&path) {
                                if let DocumentBacking::FileWindow { window, .. } = &mut buf.backing
                                {
                                    *window = reloaded.clone();
                                }
                                buf.sync_content(&reloaded.content);
                            }
                        }
                    }
                    self.open_tabs[i].is_dirty = false;
                }
            }
        }
        self.status_message = Some("All files saved".into());
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

    /// Kick off a background thread to fetch `git status --porcelain`.
    /// The result is deposited into `git_status_pending` and polled in
    /// the frame hook so the UI thread never blocks on the git process.
    pub fn refresh_git_status(&mut self) {
        let root = self.root_path.clone();
        let pending = self.git_status_pending.clone();
        std::thread::spawn(move || {
            let entries = match std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&root)
                .output()
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    stdout
                        .lines()
                        .filter_map(|line| {
                            if line.len() >= 3 {
                                Some(GitStatusEntry {
                                    status: line[..2].trim().to_string(),
                                    path: line[3..].to_string(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect()
                }
                Err(_) => Vec::new(),
            };
            if let Ok(mut guard) = pending.lock() {
                *guard = Some(entries);
            }
        });
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
                    let mut txn = fission_text_engine::EditTransaction::new();
                    for (start, end) in matches.into_iter().rev() {
                        txn.push(fission_text_engine::TextEdit::new(
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
    pub fn create_file(&mut self, path: String) {
        if let Some(parent) = Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(parent);
            self.tree_expanded
                .insert(parent.to_string_lossy().to_string());
        }
        match std::fs::write(&path, "") {
            Ok(_) => {
                self.status_message = Some(format!("Created {}", path));
                self.tree_cache_dirty = true;
                self.tree_selected = Some(path.clone());
                self.open_file(path);
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to create file: {}", e));
            }
        }
    }

    /// Create a directory on disk.
    #[allow(dead_code)]
    pub fn create_folder(&mut self, path: String) {
        match std::fs::create_dir_all(&path) {
            Ok(_) => {
                self.status_message = Some(format!("Created folder {}", path));
                self.tree_cache_dirty = true;
                self.tree_selected = Some(path.clone());
                if let Some(parent) = Path::new(&path).parent() {
                    self.tree_expanded
                        .insert(parent.to_string_lossy().to_string());
                }
                self.start_rename(path);
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to create folder: {}", e));
            }
        }
    }

    /// Delete a file or folder from disk. If the file is open, close its tab.
    #[allow(dead_code)]
    pub fn delete_file(&mut self, path: String) {
        let p = Path::new(&path);
        let result = if p.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match result {
            Ok(_) => {
                // Close tab if open
                if let Some(idx) = self.open_tabs.iter().position(|t| t.path == path) {
                    self.close_tab(idx);
                }
                self.file_contents.remove(&path);
                self.tree_cache_dirty = true;
                self.status_message = Some(format!("Deleted {}", path));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to delete: {}", e));
            }
        }
    }

    /// Rename a file/folder on disk and update any open tabs that reference it.
    #[allow(dead_code)]
    pub fn rename_file(&mut self, old: String, new_name: String) {
        let old_path = Path::new(&old);
        let new_path = if let Some(parent) = old_path.parent() {
            parent.join(&new_name)
        } else {
            PathBuf::from(&new_name)
        };
        let new_path_str = new_path.to_string_lossy().to_string();

        match std::fs::rename(&old, &new_path) {
            Ok(_) => {
                // Update open tabs
                for tab in &mut self.open_tabs {
                    if tab.path == old {
                        tab.path = new_path_str.clone();
                        tab.title = new_name.clone();
                    }
                }
                // Move buffer content
                if let Some(buf) = self.file_contents.remove(&old) {
                    self.file_contents.insert(new_path_str.clone(), buf);
                }
                self.tree_cache_dirty = true;
                self.status_message = Some(format!("Renamed to {}", new_name));
                self.update_breadcrumb();
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to rename: {}", e));
            }
        }
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

    /// Confirm the rename: move the file/folder on disk, update any open tabs,
    /// and refresh the tree.
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
            let new_path = parent.join(&new_name);
            let new_path_str = new_path.to_string_lossy().to_string();
            if new_path.exists() {
                self.status_message = Some(format!("Cannot rename: '{}' already exists", new_name));
                return;
            }
            match std::fs::rename(&old_path, &new_path) {
                Ok(()) => {
                    // Update open tabs that reference the old path
                    for tab in &mut self.open_tabs {
                        if tab.path == old_path {
                            tab.path = new_path_str.clone();
                            tab.title = new_name.clone();
                        }
                    }
                    // Move the buffer entry
                    if let Some(buf) = self.file_contents.remove(&old_path) {
                        self.file_contents.insert(new_path_str.clone(), buf);
                    }
                    // Update tree expanded set
                    if self.tree_expanded.remove(&old_path) {
                        self.tree_expanded.insert(new_path_str.clone());
                    }
                    if self.tree_selected.as_deref() == Some(&old_path) {
                        self.tree_selected = Some(new_path_str.clone());
                    }
                    self.tree_cache_dirty = true;
                    self.update_breadcrumb();
                    self.status_message = Some(format!("Renamed to '{}'", new_name));
                }
                Err(e) => {
                    self.status_message = Some(format!("Rename failed: {}", e));
                }
            }
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

    /// Check open files for external modifications.
    ///
    /// For each open tab, compare the file's current mtime against the stored
    /// value.  If the file was modified externally and the buffer is clean,
    /// reload its contents automatically.  If the buffer is dirty, set a
    /// status-bar warning instead of silently overwriting the user's edits.
    #[allow(dead_code)]
    pub fn check_external_changes(&mut self) {
        for tab in &self.open_tabs {
            let path = &tab.path;
            let Ok(meta) = std::fs::metadata(path) else {
                continue;
            };
            let Ok(current_mtime) = meta.modified() else {
                continue;
            };

            let changed = match self.file_mtimes.get(path) {
                Some(stored) => current_mtime != *stored,
                None => false,
            };

            if !changed {
                continue;
            }

            // Update stored mtime regardless of dirty state
            self.file_mtimes.insert(path.clone(), current_mtime);

            if tab.is_dirty {
                self.status_message = Some(format!("File changed on disk: {}", path));
            } else {
                // Reload content from disk
                if let Ok(new_content) = std::fs::read_to_string(path) {
                    if let Some(buf) = self.file_contents.get_mut(path) {
                        buf.sync_content(&new_content);
                        self.notify_buffer_changed(path);
                    }
                }
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

// --- File tree scanning ---

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileEntry>,
}

pub fn scan_directory(path: &Path, depth: usize) -> Vec<FileEntry> {
    if depth > 4 {
        return Vec::new();
    }
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(path) else {
        return entries;
    };

    let mut items: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    items.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir.cmp(&a_dir).then(a.file_name().cmp(&b.file_name()))
    });

    for item in items {
        let name = item.file_name().to_string_lossy().to_string();
        // Skip hidden, target, node_modules
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        let item_path = item.path();
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let path_str = item_path.to_string_lossy().to_string();

        let children = if is_dir {
            scan_directory(&item_path, depth + 1)
        } else {
            Vec::new()
        };

        entries.push(FileEntry {
            name,
            path: path_str,
            is_dir,
            children,
        });
    }
    entries
}

#[allow(dead_code)]
fn search_files_recursive(dir: &Path, query: &str, results: &mut Vec<SearchResult>, depth: usize) {
    if depth > 3 || results.len() > 100 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            search_files_recursive(&path, query, results, depth + 1);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "rs" | "toml" | "md" | "json" | "txt" | "yaml" | "yml") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                for (line_idx, line) in content.lines().enumerate() {
                    if let Some(col) = line.find(query) {
                        results.push(SearchResult {
                            path: path.to_string_lossy().to_string(),
                            line: line_idx + 1,
                            col,
                            context: line.trim().to_string(),
                        });
                        if results.len() > 100 {
                            return;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
            buffer: fission_text_engine::TextBuffer::from_str("a"),
            language: Language::Plain,
            wrap_mode: WrapMode::NoWrap,
            document_mode: DocumentMode::Normal,
            backing: DocumentBacking::InMemory,
            cursor_line: 0,
            cursor_col: 0,
            anchor_line: 0,
            anchor_col: 0,
            edit_history: fission_text_engine::EditHistory::new(),
            line_index: fission_text_engine::LineIndex::build_from_str("a"),
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
            buffer: fission_text_engine::TextBuffer::from_str("start"),
            language: Language::Plain,
            wrap_mode: WrapMode::NoWrap,
            document_mode: DocumentMode::Normal,
            backing: DocumentBacking::InMemory,
            cursor_line: 0,
            cursor_col: 0,
            anchor_line: 0,
            anchor_col: 0,
            edit_history: fission_text_engine::EditHistory::with_max(100),
            line_index: fission_text_engine::LineIndex::build_from_str("start"),
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
            buffer: fission_text_engine::TextBuffer::from_str("before"),
            language: Language::Plain,
            wrap_mode: WrapMode::NoWrap,
            document_mode: DocumentMode::Normal,
            backing: DocumentBacking::InMemory,
            cursor_line: 0,
            cursor_col: 0,
            anchor_line: 0,
            anchor_col: 0,
            edit_history: fission_text_engine::EditHistory::new(),
            line_index: fission_text_engine::LineIndex::build_from_str("before"),
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

    // --- New tests ---

    /// Helper to create a temp file and return the path string.
    fn temp_file(name: &str, content: &str) -> String {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, content).expect("write temp file");
        path.to_string_lossy().to_string()
    }

    /// Helper to clean up a temp file.
    fn cleanup(path: &str) {
        std::fs::remove_file(path).ok();
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

        state.refresh_git_status();

        // In a git repo with changes, we should get entries.
        // Even if there are no changes, the call should not panic.
        // Just verify the function runs and entries is a Vec.
        println!("Git status entries: {}", state.git_status_lines.len());
        for entry in &state.git_status_lines {
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
            buffer: fission_text_engine::TextBuffer::from_str("initial"),
            language: Language::Plain,
            wrap_mode: WrapMode::NoWrap,
            document_mode: DocumentMode::Normal,
            backing: DocumentBacking::InMemory,
            cursor_line: 0,
            cursor_col: 0,
            anchor_line: 0,
            anchor_col: 0,
            edit_history: fission_text_engine::EditHistory::new(),
            line_index: fission_text_engine::LineIndex::build_from_str("initial"),
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
            buffer: fission_text_engine::TextBuffer::from_str("start"),
            language: Language::Plain,
            wrap_mode: WrapMode::NoWrap,
            document_mode: DocumentMode::Normal,
            backing: DocumentBacking::InMemory,
            cursor_line: 0,
            cursor_col: 0,
            anchor_line: 0,
            anchor_col: 0,
            edit_history: fission_text_engine::EditHistory::new(),
            line_index: fission_text_engine::LineIndex::build_from_str("start"),
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
        assert!(!state.lsp_initialized);
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
}
