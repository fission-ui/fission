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
    /// Clears the redo stack. Caps the undo stack at 100 entries.
    pub fn push_undo(&mut self) {
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
        // Search in open buffers first
        for (path, buf) in &self.file_contents {
            for (line_idx, line) in buf.content.lines().enumerate() {
                if let Some(col) = line.find(&query) {
                    results.push(SearchResult {
                        path: path.clone(),
                        line: line_idx + 1,
                        col,
                        context: line.trim().to_string(),
                    });
                }
            }
        }
        // Search files on disk
        search_files_recursive(&self.root_path, &query, &mut results, 0);
        self.search_results = results;
    }

    pub fn refresh_git_status(&mut self) {
        match std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.root_path)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                self.git_status_lines = stdout.lines().filter_map(|line| {
                    if line.len() >= 3 {
                        Some(GitStatusEntry {
                            status: line[..2].trim().to_string(),
                            path: line[3..].to_string(),
                        })
                    } else {
                        None
                    }
                }).collect();
            }
            Err(_) => {
                self.git_status_lines.clear();
            }
        }
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
                    buf.push_undo();
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
                buf.push_undo();
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
            buf.push_undo();
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
        };

        // Change to "b"
        buf.push_undo();
        buf.content = "b".to_string();

        // Undo back to "a"
        buf.undo();
        assert_eq!(buf.content, "a");
        assert_eq!(buf.redo_stack.len(), 1);

        // New change to "c" should clear redo
        buf.push_undo();
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
        };

        for i in 0..110 {
            buf.push_undo();
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
}
