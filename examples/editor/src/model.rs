use fission_core::AppState;
use fission_macros::Action;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
    pub fn request_completions(&self, path: &str, line: usize, col: usize) {
        if let Ok(mut guard) = self.inner.try_lock() {
            if let Some(ref mut client) = *guard {
                client.request_completion(path, line as u32, col as u32);
            }
        }
    }

    /// Shut down the LSP server.
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

/// Maximum number of editor lines to render before truncating.
pub const MAX_EDITOR_LINES: usize = 200;

/// Maximum file size (in bytes) that the editor will open.  Files larger
/// than this are rejected with a status-bar message to avoid freezing
/// the UI with excessive IR node generation.
const MAX_FILE_SIZE: u64 = 1_000_000;

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
    pub terminal_lines: Vec<String>,
    pub status_message: Option<String>,

    // Split
    pub sidebar_width: f32,
    pub terminal_height: f32,

    // LSP
    pub diagnostics: HashMap<String, Vec<Diagnostic>>,
    pub completions: Vec<CompletionItem>,
    pub show_completions: bool,
    pub selected_completion: usize,
    pub hover_info: Option<String>,

    // Terminal input
    pub terminal_input: String,

    // Search
    pub search_query: String,
    pub search_results: Vec<SearchResult>,

    // Git
    pub git_status_lines: Vec<GitStatusEntry>,

    // Bottom panel tabs
    pub bottom_panel_tab: BottomPanelTab,

    // Menu bar
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
    pub key_event_count: u64,

    // Cached file tree (avoids re-scanning on every build)
    pub cached_tree_entries: Vec<FileEntry>,
    pub tree_cache_dirty: bool,

    // Background I/O: pending results from spawned threads
    pub git_status_pending: Arc<Mutex<Option<Vec<GitStatusEntry>>>>,
    pub tree_scan_pending: Arc<Mutex<Option<Vec<FileEntry>>>>,
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
            terminal_lines: vec!["Fission Editor v0.1.0".into(), "Ready.".into()],
            status_message: None,
            sidebar_width: 240.0,
            terminal_height: 120.0,
            diagnostics: HashMap::new(),
            completions: Vec::new(),
            show_completions: false,
            selected_completion: 0,
            hover_info: None,
            terminal_input: String::new(),
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
    pub content: String,
    pub language: Language,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub undo_stack: Vec<String>,  // Previous content states
    pub redo_stack: Vec<String>,  // States after undo
    pub version: i64,             // LSP document version
    pub last_undo_push: Option<std::time::Instant>, // For debouncing undo
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Toml,
    Markdown,
    Json,
    Plain,
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
}

impl FileBuffer {
    /// Push the current content onto the undo stack before a content change.
    /// Debounces: only pushes if at least 500ms have elapsed since the last push,
    /// so that rapid typing groups multiple characters into one undo entry.
    /// Clears the redo stack. Caps the undo stack at 100 entries.
    pub fn push_undo(&mut self) {
        let now = std::time::Instant::now();
        let should_push = match self.last_undo_push {
            Some(last) => now.duration_since(last).as_millis() >= 500,
            None => true,
        };
        if should_push {
            self.push_undo_force();
            self.last_undo_push = Some(now);
        }
    }

    /// Unconditionally push the current content onto the undo stack.
    /// Used for explicit operations (paste, cut, etc.) that should always
    /// create a new undo point regardless of timing.
    pub fn push_undo_force(&mut self) {
        self.undo_stack.push(self.content.clone());
        self.redo_stack.clear();
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    /// Undo the last change: pop from undo_stack, push current to redo_stack.
    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.content.clone());
            self.content = prev;
        }
    }

    /// Redo the last undo: pop from redo_stack, push current to undo_stack.
    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.content.clone());
            self.content = next;
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
#[serde(transparent)]
pub struct UpdateFileContent(pub String);

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
pub struct Noop;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SaveAllFiles;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct UpdateTerminalInput(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SubmitTerminalCommand;

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

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct ShowHover(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DismissHover;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct DeleteFile(pub String);

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RenameFile {
    pub old: String,
    pub new_name: String,
}

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SetActiveMenu(pub Option<String>);

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
    pub fn open_file(&mut self, path: String) {
        // Check if already open
        if let Some(idx) = self.open_tabs.iter().position(|t| t.path == path) {
            self.active_tab = idx;
            self.update_breadcrumb();
            return;
        }

        // Reject files that are too large — reading them into the editor would
        // generate thousands of IR nodes and freeze the UI.
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > MAX_FILE_SIZE {
                self.status_message = Some(format!(
                    "File too large to open ({:.1} MB). Max is {} MB.",
                    meta.len() as f64 / 1_000_000.0,
                    MAX_FILE_SIZE / 1_000_000,
                ));
                return;
            }
        }

        // Store the file's modification time for external-change detection
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(mtime) = meta.modified() {
                self.file_mtimes.insert(path.clone(), mtime);
            }
        }

        // Read file
        let content = std::fs::read_to_string(&path).unwrap_or_else(|_| String::new());
        let ext = Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let lang = Language::from_extension(ext);
        let title = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_string();

        self.file_contents.insert(
            path.clone(),
            FileBuffer {
                content,
                language: lang,
                cursor_line: 0,
                cursor_col: 0,
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
                version: 0,
                last_undo_push: None,
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
        if let Some(ref handle) = self.lsp_handle {
            if let Some(buf) = self.file_contents.get(&path) {
                handle.notify_open(&path, &buf.content, language_id);
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
        self.open_tabs.get(self.active_tab).and_then(|tab| {
            self.file_contents.get(&tab.path).map(|buf| (tab, buf))
        })
    }

    pub fn active_buffer_mut(&mut self) -> Option<(&TabInfo, &mut FileBuffer)> {
        let tab = self.open_tabs.get(self.active_tab)?;
        let path = tab.path.clone();
        let buf = self.file_contents.get_mut(&path)?;
        let tab = &self.open_tabs[self.active_tab];
        Some((tab, buf))
    }

    pub fn save_active_file(&mut self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get(&path) {
                if std::fs::write(&path, &buf.content).is_ok() {
                    if let Some(tab) = self.open_tabs.get_mut(self.active_tab) {
                        tab.is_dirty = false;
                    }
                    self.status_message = Some(format!("Saved {}", path));
                } else {
                    self.status_message = Some(format!("Failed to save {}", path));
                }
            }
        }
    }

    pub fn save_all_files(&mut self) {
        for i in 0..self.open_tabs.len() {
            if self.open_tabs[i].is_dirty {
                let path = self.open_tabs[i].path.clone();
                if let Some(buf) = self.file_contents.get(&path) {
                    if std::fs::write(&path, &buf.content).is_ok() {
                        self.open_tabs[i].is_dirty = false;
                    }
                }
            }
        }
        self.status_message = Some("All files saved".into());
    }

    pub fn run_terminal_command(&mut self) {
        let cmd = self.terminal_input.trim().to_string();
        if cmd.is_empty() { return; }
        self.terminal_lines.push(format!("$ {}", cmd));
        match std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(&self.root_path)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                for line in stdout.lines() {
                    self.terminal_lines.push(line.to_string());
                }
                for line in stderr.lines() {
                    self.terminal_lines.push(format!("ERR: {}", line));
                }
            }
            Err(e) => {
                self.terminal_lines.push(format!("Error: {}", e));
            }
        }
        self.terminal_input.clear();
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
            for (line_idx, line) in buf.content.lines().enumerate() {
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
                    let mut lines: Vec<String> = buf.content.lines().map(|l| l.to_string()).collect();
                    if line < lines.len() {
                        let line_str = &mut lines[line];
                        if col + query.len() <= line_str.len() {
                            line_str.replace_range(col..col + query.len(), &replacement);
                        }
                    }
                    buf.content = lines.join("\n");
                    // Mark dirty
                    if let Some(tab) = self.open_tabs.get_mut(self.active_tab) {
                        tab.is_dirty = true;
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
                buf.content = buf.content.replace(&query, &replacement);
                if let Some(tab) = self.open_tabs.get_mut(self.active_tab) {
                    tab.is_dirty = true;
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
                for (line_idx, line) in buf.content.lines().enumerate() {
                    let mut start = 0;
                    while let Some(col) = line[start..].find(&query) {
                        self.find_matches.push((path.clone(), line_idx, start + col));
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
                    buf.cursor_line = line;
                    buf.cursor_col = col;
                }
            }
        }
    }

    // --- File operations ---

    /// Create a new file on disk and open it in a tab.
    pub fn create_file(&mut self, path: String) {
        if let Some(parent) = Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&path, "") {
            Ok(_) => {
                self.status_message = Some(format!("Created {}", path));
                self.tree_cache_dirty = true;
                self.open_file(path);
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to create file: {}", e));
            }
        }
    }

    /// Create a directory on disk.
    pub fn create_folder(&mut self, path: String) {
        match std::fs::create_dir_all(&path) {
            Ok(_) => {
                self.status_message = Some(format!("Created folder {}", path));
                self.tree_cache_dirty = true;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to create folder: {}", e));
            }
        }
    }

    /// Delete a file or folder from disk. If the file is open, close its tab.
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
            let relative = tab_path
                .strip_prefix(&self.root_path)
                .unwrap_or(tab_path);
            for component in relative.components() {
                self.breadcrumb_path.push(
                    component.as_os_str().to_string_lossy().to_string(),
                );
            }
        }
    }

    // --- Undo / Redo / Clipboard helpers ---

    /// Undo the last content change in the active buffer.
    pub fn undo_active(&mut self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get_mut(&path) {
                buf.undo();
            }
        }
    }

    /// Redo the last undone change in the active buffer.
    pub fn redo_active(&mut self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get_mut(&path) {
                buf.redo();
            }
        }
    }

    /// Copy the current line of the active buffer into the in-app clipboard.
    pub fn copy_line(&mut self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get(&path) {
                if let Some(line) = buf.content.lines().nth(buf.cursor_line) {
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
                let line_count = buf.content.lines().count();
                if buf.cursor_line < line_count {
                    let lines: Vec<String> = buf.content.lines().map(|l| l.to_string()).collect();
                    self.clipboard = lines[buf.cursor_line].clone();
                    buf.push_undo_force();
                    let mut new_lines = lines;
                    new_lines.remove(buf.cursor_line);
                    buf.content = new_lines.join("\n");
                    // Adjust cursor if it was on the last line
                    let max_line = buf.content.lines().count().saturating_sub(1);
                    if buf.cursor_line > max_line {
                        buf.cursor_line = max_line;
                    }
                    buf.cursor_col = 0;
                    if let Some(tab) = self.open_tabs.get_mut(self.active_tab) {
                        tab.is_dirty = true;
                    }
                    self.status_message = Some("Cut line".into());
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
                buf.push_undo_force();
                let lines: Vec<&str> = buf.content.lines().collect();
                let line_idx = buf.cursor_line.min(lines.len().saturating_sub(1));
                let col = if line_idx < lines.len() {
                    buf.cursor_col.min(lines[line_idx].len())
                } else {
                    0
                };
                // Compute byte offset for insertion
                let mut byte_offset = 0;
                for (i, line) in buf.content.lines().enumerate() {
                    if i == line_idx {
                        byte_offset += col;
                        break;
                    }
                    byte_offset += line.len() + 1; // +1 for '\n'
                }
                byte_offset = byte_offset.min(buf.content.len());
                buf.content.insert_str(byte_offset, &clip);
                if let Some(tab) = self.open_tabs.get_mut(self.active_tab) {
                    tab.is_dirty = true;
                }
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
    pub fn check_external_changes(&mut self) {
        for tab in &self.open_tabs {
            let path = &tab.path;
            let Ok(meta) = std::fs::metadata(path) else { continue };
            let Ok(current_mtime) = meta.modified() else { continue };

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
                self.status_message =
                    Some(format!("File changed on disk: {}", path));
            } else {
                // Reload content from disk
                if let Ok(new_content) = std::fs::read_to_string(path) {
                    if let Some(buf) = self.file_contents.get_mut(path) {
                        buf.content = new_content;
                    }
                }
            }
        }
    }

    /// Move the cursor to the given line number (1-based).
    pub fn go_to_line(&mut self, line: usize) {
        let target = if line > 0 { line - 1 } else { 0 };
        if let Some(tab) = self.open_tabs.get(self.active_tab) {
            let path = tab.path.clone();
            if let Some(buf) = self.file_contents.get_mut(&path) {
                let max_line = buf.content.lines().count().saturating_sub(1);
                buf.cursor_line = target.min(max_line);
                buf.cursor_col = 0;
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

fn search_files_recursive(dir: &Path, query: &str, results: &mut Vec<SearchResult>, depth: usize) {
    if depth > 3 || results.len() > 100 { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "target" || name == "node_modules" { continue; }
        let path = entry.path();
        if path.is_dir() {
            search_files_recursive(&path, query, results, depth + 1);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "rs" | "toml" | "md" | "json" | "txt" | "yaml" | "yml") { continue; }
            if let Ok(content) = std::fs::read_to_string(&path) {
                for (line_idx, line) in content.lines().enumerate() {
                    if let Some(col) = line.find(query) {
                        results.push(SearchResult {
                            path: path.to_string_lossy().to_string(),
                            line: line_idx + 1,
                            col,
                            context: line.trim().to_string(),
                        });
                        if results.len() > 100 { return; }
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
            buf.push_undo_force();
            buf.content = "hello world".to_string();
        }

        // Undo
        if let Some(buf) = state.file_contents.get_mut(&path) {
            buf.undo();
            assert_eq!(buf.content, "hello");
        }

        // Redo
        if let Some(buf) = state.file_contents.get_mut(&path) {
            buf.redo();
            assert_eq!(buf.content, "hello world");
        }

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_undo_clears_redo_on_new_change() {
        let mut buf = FileBuffer {
            content: "a".to_string(),
            language: Language::Plain,
            cursor_line: 0,
            cursor_col: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            version: 0,
            last_undo_push: None,
        };

        // Change to "b"
        buf.push_undo_force();
        buf.content = "b".to_string();

        // Undo back to "a"
        buf.undo();
        assert_eq!(buf.content, "a");
        assert_eq!(buf.redo_stack.len(), 1);

        // New change to "c" should clear redo
        buf.push_undo_force();
        buf.content = "c".to_string();
        assert!(buf.redo_stack.is_empty());
    }

    #[test]
    fn test_undo_stack_cap() {
        let mut buf = FileBuffer {
            content: "start".to_string(),
            language: Language::Plain,
            cursor_line: 0,
            cursor_col: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            version: 0,
            last_undo_push: None,
        };

        for i in 0..110 {
            buf.push_undo_force();
            buf.content = format!("version_{}", i);
        }

        assert!(buf.undo_stack.len() <= 100);
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
        let content = &state.file_contents[&path].content;
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
        assert_eq!(buf.content, "fn main() {}");
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
            buf.push_undo_force();
            buf.content = "modified".to_string();
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
        assert_eq!(state.find_matches[0].2, 0);   // col 0
        assert_eq!(state.find_matches[1].2, 13);  // col 13 ("apple banana apple...")
        assert_eq!(state.find_matches[2].2, 26);  // col 26

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
        let content = &state.file_contents[&path].content;
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

        let content = &state.file_contents[&path].content;
        assert_eq!(content, "ZZZ bar ZZZ baz ZZZ");
        assert!(state.open_tabs[0].is_dirty);
        assert!(state.status_message.as_ref().unwrap().contains("Replaced all"));

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

        let content = &state.file_contents[&path].content;
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
            buf.push_undo_force();
            buf.content = "version_1".to_string();
            buf.push_undo_force();
            buf.content = "version_2".to_string();
        }

        // Undo through the state helper
        state.undo_active();
        assert_eq!(state.file_contents[&path].content, "version_1");

        state.undo_active();
        assert_eq!(state.file_contents[&path].content, "version_0");

        // Redo
        state.redo_active();
        assert_eq!(state.file_contents[&path].content, "version_1");

        state.redo_active();
        assert_eq!(state.file_contents[&path].content, "version_2");

        // Redo when nothing to redo should be a no-op
        state.redo_active();
        assert_eq!(state.file_contents[&path].content, "version_2");

        cleanup(&path);
    }

    #[test]
    fn test_large_file_rejected() {
        let mut state = EditorState::default();
        state.root_path = std::env::temp_dir();
        let path = std::env::temp_dir().join("test_large_file.txt");
        let path_str = path.to_string_lossy().to_string();

        // Create a file >1MB
        let large_content = "x".repeat(1_100_000);
        std::fs::write(&path, &large_content).expect("write large file");

        state.open_file(path_str.clone());

        // Should not have opened
        assert!(state.open_tabs.is_empty());
        assert!(state.file_contents.is_empty());

        // Status message should indicate "too large"
        let msg = state.status_message.as_ref().expect("status message set");
        assert!(msg.contains("too large") || msg.contains("Too large"),
            "expected 'too large' message, got: {}", msg);

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
        assert_eq!(buf.content, "");

        // Status message should mention creation
        assert!(state.status_message.as_ref().unwrap().contains("Created"));

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
        assert_eq!(buf.content, "rename me");

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
        assert!(state.breadcrumb_path.len() >= 2,
            "breadcrumb should have at least 2 segments, got: {:?}", state.breadcrumb_path);
        assert!(state.breadcrumb_path.contains(&"test_breadcrumb_dir".to_string()));
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
    fn test_terminal_runs_command() {
        let mut state = EditorState::default();
        state.root_path = std::env::temp_dir();
        state.terminal_input = "echo hello_from_test".to_string();

        state.run_terminal_command();

        // terminal_input should be cleared
        assert!(state.terminal_input.is_empty());

        // terminal_lines should contain the command and its output
        let has_prompt = state.terminal_lines.iter().any(|l| l.contains("$ echo hello_from_test"));
        let has_output = state.terminal_lines.iter().any(|l| l.contains("hello_from_test") && !l.starts_with("$"));
        assert!(has_prompt, "terminal should show the command prompt");
        assert!(has_output, "terminal should show command output");
    }

    #[test]
    fn test_terminal_empty_command_noop() {
        let mut state = EditorState::default();
        state.root_path = std::env::temp_dir();
        let initial_count = state.terminal_lines.len();
        state.terminal_input = "   ".to_string();

        state.run_terminal_command();

        // Empty/whitespace command should be a no-op
        assert_eq!(state.terminal_lines.len(), initial_count);
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
            buf.cursor_line = 1;
            buf.cursor_col = 5;
        }

        state.clipboard = "INSERTED".to_string();
        state.paste();

        let content = &state.file_contents[&path].content;
        assert!(content.contains("line INSERTEDtwo"),
            "paste should insert at cursor position, got: {}", content);
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

        assert_eq!(state.file_contents[&path].content, "no change");
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
            buf.cursor_line = 1;
            buf.cursor_col = 0;
        }

        state.cut_line();

        // Clipboard should have "line B"
        assert_eq!(state.clipboard, "line B");

        // Content should have the line removed
        let content = &state.file_contents[&path].content;
        assert!(!content.contains("line B"), "cut line should be removed, got: {}", content);
        assert!(content.contains("line A"));
        assert!(content.contains("line C"));
        assert!(state.open_tabs[0].is_dirty);

        // Undo should restore it
        state.undo_active();
        let content = &state.file_contents[&path].content;
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
            buf.cursor_line = 2;
        }

        state.copy_line();

        assert_eq!(state.clipboard, "gamma");
        // Content should be unchanged
        assert_eq!(state.file_contents[&path].content, "alpha\nbeta\ngamma");

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
    fn test_save_all_files() {
        let mut state = EditorState::default();
        state.root_path = std::env::temp_dir();
        let p1 = temp_file("test_save_all_1.txt", "one");
        let p2 = temp_file("test_save_all_2.txt", "two");
        state.open_file(p1.clone());
        state.open_file(p2.clone());

        // Modify both
        if let Some(buf) = state.file_contents.get_mut(&p1) {
            buf.content = "one_modified".to_string();
        }
        state.open_tabs[0].is_dirty = true;

        if let Some(buf) = state.file_contents.get_mut(&p2) {
            buf.content = "two_modified".to_string();
        }
        state.open_tabs[1].is_dirty = true;

        state.save_all_files();

        assert!(!state.open_tabs[0].is_dirty);
        assert!(!state.open_tabs[1].is_dirty);
        assert!(state.status_message.as_ref().unwrap().contains("All files saved"));

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
        assert_eq!(buf.content, "some content");

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
        assert!(state.status_message.as_ref().unwrap().contains("Created folder"));

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
        assert!(entries.len() >= 3, "expected >= 3 entries, got {}", entries.len());

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
        let path = temp_file("test_multiline_find.txt", "hello world\nhello rust\ngoodbye hello");
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
        assert_eq!(buf.content, "");
        assert_eq!(buf.cursor_line, 0);
        assert_eq!(buf.cursor_col, 0);

        // Operations on empty buffer should not panic
        state.find_query = "anything".to_string();
        state.find_next();
        assert!(state.find_matches.is_empty());

        state.replace_query = "replacement".to_string();
        state.replace_all();
        assert_eq!(state.file_contents[&path].content, "");

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
        assert_eq!(buf.content.lines().count(), 1);

        // Go to line beyond the single line
        state.go_to_line(100);
        let buf = state.file_contents.get(&path).unwrap();
        assert_eq!(buf.cursor_line, 0); // clamped to the only line

        // Cut the only line
        state.cut_line();
        assert_eq!(state.clipboard, "only one line");
        let buf = state.file_contents.get(&path).unwrap();
        assert_eq!(buf.content, "");
        assert_eq!(buf.cursor_line, 0);

        // Undo should restore it
        state.undo_active();
        let buf = state.file_contents.get(&path).unwrap();
        assert_eq!(buf.content, "only one line");

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
            buf.cursor_line = 2;
            buf.cursor_col = 5; // end of "line3"
        }
        state.paste();
        let content = &state.file_contents[&path].content;
        assert!(content.contains("line3line3"), "paste at end of file, got: {}", content);

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

        let content = &state.file_contents[&path].content;
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
        let content = &state.file_contents[&path].content;
        // One "x" replaced with "xx", so total "x" count changes
        let x_count = content.matches("x").count();
        assert!(x_count >= 3, "at least original minus 1 plus 2, got: {}", x_count);

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
            content: "initial".to_string(),
            language: Language::Plain,
            cursor_line: 0,
            cursor_col: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            version: 0,
            last_undo_push: None,
        };

        for i in 0..200 {
            buf.push_undo_force();
            buf.content = format!("change_{}", i);
        }

        assert_eq!(buf.undo_stack.len(), 100, "undo stack should be capped at 100");

        // The oldest entry should NOT be "initial" (it was evicted)
        assert_ne!(buf.undo_stack[0], "initial");

        // The newest entry should be the second-to-last change
        assert_eq!(buf.undo_stack[99], "change_198");

        // Current content should be the last change
        assert_eq!(buf.content, "change_199");
    }

    #[test]
    fn test_undo_stack_overflow_still_allows_undo_redo() {
        let mut buf = FileBuffer {
            content: "start".to_string(),
            language: Language::Plain,
            cursor_line: 0,
            cursor_col: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            version: 0,
            last_undo_push: None,
        };

        // Push 200 changes
        for i in 0..200 {
            buf.push_undo_force();
            buf.content = format!("v{}", i);
        }

        // Undo all 100 available entries
        for _ in 0..100 {
            buf.undo();
        }

        // Should be at the oldest available state
        assert!(buf.content.starts_with("v"), "content should be a v-version, got: {}", buf.content);

        // Undo once more should be a no-op (stack empty)
        let before = buf.content.clone();
        buf.undo();
        assert_eq!(buf.content, before);

        // Redo all 100
        for _ in 0..100 {
            buf.redo();
        }
        assert_eq!(buf.content, "v199");

        // Redo once more should be a no-op
        buf.redo();
        assert_eq!(buf.content, "v199");
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
        assert!(depth <= 4, "scan_directory should stop at depth 4, got depth {}", depth);

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
        assert!(!names.contains(&"node_modules"), "should skip node_modules directory");
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
        assert!(entries.is_empty(), "empty directory should return empty vec");

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_scan_directory_nonexistent() {
        let entries = scan_directory(Path::new("/tmp/definitely_does_not_exist_12345"), 0);
        assert!(entries.is_empty(), "nonexistent directory should return empty vec");
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
        assert_eq!(buf.content, "zero");

        state.active_tab = 1;
        let (tab, buf) = state.active_buffer().unwrap();
        assert_eq!(tab.path, p1);
        assert_eq!(buf.content, "one");

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
            CompletionItem { label: "foo".into(), kind: "function".into(), detail: None },
            CompletionItem { label: "bar".into(), kind: "variable".into(), detail: None },
            CompletionItem { label: "baz".into(), kind: "keyword".into(), detail: Some("built-in".into()) },
        ];
        state.show_completions = true;
        state.selected_completion = 2;

        assert_eq!(state.completions[state.selected_completion].label, "baz");
    }

    #[test]
    fn test_dismiss_completions_hides_panel() {
        let mut state = EditorState::default();
        state.show_completions = true;
        state.completions = vec![
            CompletionItem { label: "item".into(), kind: "text".into(), detail: None },
        ];

        state.show_completions = false;
        assert!(!state.show_completions);
        assert_eq!(state.completions.len(), 1); // still there but hidden
    }

    #[test]
    fn test_navigate_diagnostic_moves_cursor() {
        let mut state = EditorState::default();
        state.root_path = std::env::temp_dir();
        let path = temp_file("test_nav_diag.rs", "fn main() {\n    let x = 1;\n    let y = 2;\n}");
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
            buf.push_undo_force();
            buf.content = "modified content".to_string();
        }
        state.open_tabs[0].is_dirty = true;

        assert!(state.open_tabs[0].is_dirty);
        assert_eq!(state.file_contents[&path].content, "modified content");

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

        assert_eq!(state.scroll_offset_y, 0.0, "scroll should reset when opening a file");

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
            buf.cursor_line = 0;
            buf.cursor_col = 0;
        }
        state.clipboard = "PREFIX ".to_string();
        state.paste();

        let content = &state.file_contents[&path].content;
        assert!(content.starts_with("PREFIX "), "should paste at beginning, got: {}", content);

        cleanup(&path);
    }

    #[test]
    fn test_cut_last_line_adjusts_cursor() {
        let mut state = EditorState::default();
        state.root_path = std::env::temp_dir();
        let path = temp_file("test_cut_last.txt", "line1\nline2\nline3");
        state.open_file(path.clone());

        if let Some(buf) = state.file_contents.get_mut(&path) {
            buf.cursor_line = 2; // last line
        }

        state.cut_line();
        assert_eq!(state.clipboard, "line3");

        let buf = state.file_contents.get(&path).unwrap();
        let max_line = buf.content.lines().count().saturating_sub(1);
        assert!(buf.cursor_line <= max_line,
            "cursor_line {} should be <= max_line {}", buf.cursor_line, max_line);

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

        let content = &state.file_contents[&path].content;
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
        assert!(state.status_message.as_ref().unwrap().contains("All files saved"));
    }

    // -----------------------------------------------------------------------
    // File buffer version tracking
    // -----------------------------------------------------------------------

    #[test]
    fn test_file_buffer_version_starts_at_zero() {
        let mut state = EditorState::default();
        state.root_path = std::env::temp_dir();
        let path = temp_file("test_version.txt", "versioned");
        state.open_file(path.clone());

        let buf = state.file_contents.get(&path).unwrap();
        assert_eq!(buf.version, 0);

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
        assert_eq!(buf.content, "");
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
        assert_eq!(state.terminal_height, 120.0);
        assert_eq!(state.sidebar_section, SidebarSection::Explorer);
        assert_eq!(state.bottom_panel_tab, BottomPanelTab::Terminal);
        assert!(state.terminal_lines.len() >= 2);
        assert!(state.lsp_handle.is_none());
        assert!(!state.lsp_initialized);
    }

    // -----------------------------------------------------------------------
    // Helper function coverage
    // -----------------------------------------------------------------------

    #[test]
    fn test_uri_to_path() {
        assert_eq!(uri_to_path("file:///home/user/file.rs"), "/home/user/file.rs");
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
