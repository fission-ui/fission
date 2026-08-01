use fission::core::{GlobalState, JobRef, JobSpec};
use fission::prelude::fission_action;
#[cfg(not(target_arch = "wasm32"))]
use fission::widgets::{TerminalLaunchConfig, TerminalSession};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
pub struct TerminalSession;

#[cfg(target_arch = "wasm32")]
impl TerminalSession {
    pub(crate) fn take_dirty(&self) -> bool {
        false
    }

    pub(crate) fn title(&self) -> String {
        "Terminal unavailable in a browser".into()
    }
}

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
#[allow(dead_code)]
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

    // Clipboard (in-app)
    pub clipboard: String,

    // File watcher
    pub file_mtimes: HashMap<String, std::time::SystemTime>,
    #[allow(dead_code)]
    pub key_event_count: u64,
    pub redraw_epoch: u64,

    // Cached file tree (avoids re-scanning on every build)
    pub cached_tree_entries: Vec<FileEntry>,
    pub tree_scan_generation: u64,
    pub tree_scan_loaded_generation: u64,

    // Async resource generations
    pub git_status_generation: u64,
    pub git_status_loaded_generation: u64,

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
            clipboard: String::new(),
            file_mtimes: HashMap::new(),
            key_event_count: 0,
            redraw_epoch: 0,
            cached_tree_entries: Vec::new(),
            tree_scan_generation: 0,
            tree_scan_loaded_generation: 0,
            git_status_generation: 0,
            git_status_loaded_generation: 0,
            untitled_counter: 0,
            renaming_path: None,
            rename_input: String::new(),
        }
    }
}

impl GlobalState for EditorState {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabInfo {
    pub path: String,
    pub title: String,
    pub is_dirty: bool,
}

#[derive(Debug, Clone)]
pub struct FileBuffer {
    pub buffer: fission::text_engine::TextBuffer,
    pub language: Language,
    #[allow(dead_code)]
    pub wrap_mode: WrapMode,
    pub document_mode: DocumentMode,
    pub backing: DocumentBacking,
    pub cursor_line: usize,
    pub cursor_col: usize,
    /// Selection anchor line (same as cursor when no selection).
    pub anchor_line: usize,
    /// Selection anchor column (same as cursor when no selection).
    pub anchor_col: usize,
    pub edit_history: fission::text_engine::EditHistory,
    pub line_index: fission::text_engine::LineIndex,
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

#[allow(dead_code)]
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
        self.line_index = fission::text_engine::LineIndex::build(self.buffer.text());
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
            .line_col_to_byte(fission::text_engine::LineCol {
                line: self.cursor_line,
                col: self.cursor_col,
            })
            .unwrap_or(max_offset)
            .min(max_offset);
        let anchor = self
            .line_index
            .line_col_to_byte(fission::text_engine::LineCol {
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
            .unwrap_or(fission::text_engine::LineCol { line: 0, col: 0 });
        let anchor_lc = self
            .line_index
            .byte_to_line_col(anchor)
            .unwrap_or(fission::text_engine::LineCol { line: 0, col: 0 });
        self.cursor_line = caret_lc.line;
        self.cursor_col = caret_lc.col;
        self.anchor_line = anchor_lc.line;
        self.anchor_col = anchor_lc.col;
    }

    pub fn set_caret_line_col(&mut self, line: usize, col: usize) {
        let offset = self
            .line_index
            .line_col_to_byte(fission::text_engine::LineCol { line, col })
            .unwrap_or_else(|| {
                self.line_index
                    .line_end_byte(line)
                    .unwrap_or(self.buffer.len_bytes())
            })
            .min(self.buffer.len_bytes());
        self.set_selection_offsets(offset, offset);
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    pub fn apply_transaction(&mut self, txn: &fission::text_engine::EditTransaction) {
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
        self.buffer = fission::text_engine::TextBuffer::from_str(new_text);
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

#[fission_action]
pub struct OpenFile(pub String);

#[fission_action]
pub struct CloseTab(pub usize);

#[fission_action]
pub struct SelectTab(pub usize);

#[fission_action]
pub struct ToggleTreeNode(pub String);

#[fission_action]
#[allow(dead_code)]
pub struct ApplyEditorEdit {
    pub range_start: usize,
    pub range_end: usize,
    pub new_text: String,
    pub caret: usize,
    pub anchor: usize,
}

#[fission_action]
#[serde(transparent)]
pub struct UpdateEditorDocument(pub String);

#[fission_action]
#[allow(dead_code)]
pub struct SetEditorPreedit {
    pub text: String,
}

#[fission_action]
pub struct ToggleCommandPalette;

#[fission_action]
#[serde(transparent)]
pub struct UpdateCommandQuery(pub String);

#[fission_action]
pub struct ToggleSidebar;

#[fission_action]
pub struct ToggleTerminal;

#[fission_action]
pub struct SetSidebarSection(pub SidebarSection);

#[fission_action]
pub struct SaveFile;

#[fission_action]
pub struct SaveAllFiles;

#[fission_action]
pub struct DismissMenu;

#[fission_action]
pub struct ShowMenuStatus(pub String);

#[fission_action]
pub struct SetBottomPanelTab(pub BottomPanelTab);

#[fission_action]
#[allow(dead_code)]
pub struct ShowContextStatus(pub String);

#[fission_action]
pub struct RenameContextTarget;

#[fission_action]
pub struct DeleteContextTarget;

#[fission_action]
#[serde(transparent)]
pub struct UpdateSearchQuery(pub String);

#[fission_action]
pub struct ExecuteSearch;

#[fission_action]
pub struct SelectCompletion(pub usize);

#[fission_action]
pub struct DismissCompletions;

#[fission_action]
pub struct RefreshGitStatus;

#[allow(dead_code)]
#[fission_action]
pub struct NavigateDiagnostic {
    pub path: String,
    pub line: usize,
}

#[fission_action(no_eq)]
pub struct ShowContextMenu {
    pub x: f32,
    pub y: f32,
    pub target: Option<String>,
}

#[fission_action]
pub struct DismissContextMenu;

#[fission_action]
#[serde(transparent)]
pub struct CreateFile(pub String);

#[fission_action]
#[serde(transparent)]
pub struct CreateFolder(pub String);

#[fission_action]
pub struct RefreshTree;

#[fission_action]
pub struct ToggleFindReplace;

#[fission_action]
#[serde(transparent)]
pub struct UpdateFindQuery(pub String);

#[fission_action]
#[serde(transparent)]
pub struct UpdateReplaceQuery(pub String);

#[fission_action]
pub struct FindNext;

#[fission_action]
pub struct FindPrevious;

#[fission_action]
pub struct ReplaceOne;

#[fission_action]
pub struct ReplaceAll;

#[allow(dead_code)]
#[fission_action]
#[serde(transparent)]
pub struct ShowHover(pub String);

#[allow(dead_code)]
#[fission_action]
pub struct DismissHover;

#[allow(dead_code)]
#[fission_action]
#[serde(transparent)]
pub struct DeleteFile(pub String);

#[allow(dead_code)]
#[fission_action]
pub struct RenameFile {
    pub old: String,
    pub new_name: String,
}

#[allow(dead_code)]
#[fission_action]
pub struct StartRename(pub String);

#[fission_action]
#[allow(dead_code)]
pub struct ConfirmRename;

#[fission_action]
pub struct CancelRename;

#[fission_action]
#[serde(transparent)]
pub struct UpdateRenameInput(pub String);

#[fission_action]
pub struct SetActiveMenu(pub Option<String>);

#[allow(dead_code)]
#[fission_action]
pub struct GoToLine(pub usize);

#[fission_action]
pub struct GoToDefinition;

#[fission_action]
pub struct Undo;

#[fission_action]
pub struct Redo;

#[fission_action]
pub struct CopySelection;

#[fission_action]
pub struct CutSelection;

#[fission_action]
pub struct PasteClipboard;

#[fission_action]
pub struct UpdateCursorPosition {
    pub caret: usize,
    pub anchor: usize,
}

/// Action dispatched by the editor render node to update the model's scroll
/// position so that scroll-follows-cursor works.
#[fission_action(no_eq)]
#[allow(dead_code)]
pub struct UpdateScrollY(pub f32);

#[fission_action]
#[allow(dead_code)]
pub struct ShiftActiveFileWindow {
    pub forward: bool,
}

#[fission_action]
pub struct EditorStarted {
    pub root_path: PathBuf,
}

#[fission_action]
pub struct TreeScanCompleted;

#[fission_action]
pub struct TreeScanFailed;

#[fission_action]
pub struct GitStatusLoaded;

#[fission_action]
pub struct GitStatusFailed;

#[fission_action]
pub struct PollTerminal;

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PollTerminalTick;

#[fission_action]
pub struct PollLsp;

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PollLspTick;

// --- Additional types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub line: usize,
    pub col: usize,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitStatusEntry {
    pub status: String,
    pub path: String,
}

#[derive(Debug)]
pub struct TreeScanJob;

impl JobSpec for TreeScanJob {
    type Request = TreeScanRequest;
    type Ok = TreeScanResult;
    type Err = String;
    const NAME: &'static str = "examples::editor::tree-scan";
}

pub const TREE_SCAN_JOB: JobRef<TreeScanJob> = JobRef::new(TreeScanJob::NAME);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeScanRequest {
    pub root_path: PathBuf,
    pub generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeScanResult {
    pub generation: u64,
    pub entries: Vec<FileEntry>,
}

#[derive(Debug)]
pub struct GitStatusJob;

impl JobSpec for GitStatusJob {
    type Request = GitStatusRequest;
    type Ok = GitStatusResult;
    type Err = String;
    const NAME: &'static str = "examples::editor::git-status";
}

pub const GIT_STATUS_JOB: JobRef<GitStatusJob> = JobRef::new(GitStatusJob::NAME);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitStatusRequest {
    pub root_path: PathBuf,
    pub generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitStatusResult {
    pub generation: u64,
    pub entries: Vec<GitStatusEntry>,
}

#[path = "model/editor_state.rs"]
mod editor_state;

// --- File tree scanning ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

pub fn run_tree_scan(request: TreeScanRequest) -> Result<TreeScanResult, String> {
    Ok(TreeScanResult {
        generation: request.generation,
        entries: scan_directory(&request.root_path, 0),
    })
}

pub fn collect_git_status(root: &Path) -> Result<Vec<GitStatusEntry>, String> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .map_err(|err| err.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
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
        .collect())
}

pub fn run_git_status(request: GitStatusRequest) -> Result<GitStatusResult, String> {
    Ok(GitStatusResult {
        generation: request.generation,
        entries: collect_git_status(&request.root_path)?,
    })
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
mod core_tests;
#[cfg(test)]
mod filesystem_tests;
#[cfg(test)]
mod test_support;
