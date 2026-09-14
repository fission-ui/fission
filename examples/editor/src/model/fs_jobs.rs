//! File operations run as a background job.
//!
//! Opening, saving, creating, renaming and deleting touch the disk, so reducers never do
//! it themselves. They queue an [`FsRequest`]; `EditorApp` runs each one through
//! [`FS_JOB`], and the result is applied with
//! [`EditorState::apply_fs_outcome`](crate::model::EditorState::apply_fs_outcome)
//! or [`EditorState::apply_fs_failure`](crate::model::EditorState::apply_fs_failure).

use super::{classify_document_mode_for_size, DocumentMode, FileWindow, FileWindowSource};
use fission::core::{JobRef, JobSpec};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct FsJob;

impl JobSpec for FsJob {
    type Request = FsRequest;
    type Ok = FsOutcome;
    type Err = FsFailure;
    const NAME: &'static str = "examples::editor::fs";
}

pub const FS_JOB: JobRef<FsJob> = JobRef::new(FsJob::NAME);

/// A queued file operation and the id its result is matched back by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsRequest {
    pub id: u64,
    pub op: FsOp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsOp {
    /// Read a file to open it in a tab.
    Open { path: String },
    /// Write a buffer's content. `all` marks a save started by "save all".
    Save {
        path: String,
        content: String,
        all: bool,
    },
    /// Create an empty file at `base`, or at `base-N` if that path is taken on
    /// disk or by an open tab listed in `taken`.
    CreateFile { base: String, taken: Vec<String> },
    /// Create a folder at `base`, or at `base-N` if that path is taken.
    CreateFolder { base: String },
    /// Rename `from` to `to`, refusing to overwrite an existing path.
    Rename { from: String, to: String },
    /// Delete a file, or a folder and everything in it.
    Delete { path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsOutcome {
    pub id: u64,
    pub result: FsResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsResult {
    /// A file was read. `window` is set for huge files shown a window at a
    /// time; `content` for files loaded whole.
    Opened {
        path: String,
        size: u64,
        content: Option<String>,
        window: Option<FileWindow>,
    },
    Saved {
        path: String,
        all: bool,
    },
    CreatedFile {
        path: String,
    },
    CreatedFolder {
        path: String,
    },
    Renamed {
        from: String,
        to: String,
    },
    Deleted {
        path: String,
    },
}

/// Why a file operation failed, with the id of the request that failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsFailure {
    pub id: u64,
    pub message: String,
}

/// Performs a queued file operation. Runs off the UI thread in the app, and
/// synchronously in tests.
pub fn run_fs_job(request: FsRequest) -> Result<FsOutcome, FsFailure> {
    let id = request.id;
    let fail = |message: String| FsFailure { id, message };
    let result = match request.op {
        FsOp::Open { path } => {
            let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
            if matches!(classify_document_mode_for_size(size), DocumentMode::Huge) {
                let window = FileWindowSource::new(path.clone(), size)
                    .current_window()
                    .unwrap_or(FileWindow {
                        start_byte: 0,
                        end_byte: 0,
                        size_bytes: size,
                        start_line: 0,
                        end_line: 0,
                        content: String::new(),
                        has_more_before: false,
                        has_more_after: false,
                    });
                FsResult::Opened {
                    path,
                    size,
                    content: None,
                    window: Some(window),
                }
            } else {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                FsResult::Opened {
                    path,
                    size,
                    content: Some(content),
                    window: None,
                }
            }
        }
        FsOp::Save { path, content, all } => {
            std::fs::write(&path, content).map_err(|_| fail(format!("Failed to save {}", path)))?;
            FsResult::Saved { path, all }
        }
        FsOp::CreateFile { base, taken } => {
            let path = unique_path(&base, |candidate| {
                taken.iter().any(|open| open == candidate)
            });
            if let Some(parent) = Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, "")
                .map_err(|error| fail(format!("Failed to create file: {}", error)))?;
            FsResult::CreatedFile { path }
        }
        FsOp::CreateFolder { base } => {
            let path = unique_path(&base, |_| false);
            std::fs::create_dir_all(&path)
                .map_err(|error| fail(format!("Failed to create folder: {}", error)))?;
            FsResult::CreatedFolder { path }
        }
        FsOp::Rename { from, to } => {
            let name = Path::new(&to)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&to)
                .to_string();
            if Path::new(&to).exists() {
                return Err(fail(format!("Cannot rename: '{}' already exists", name)));
            }
            std::fs::rename(&from, PathBuf::from(&to))
                .map_err(|error| fail(format!("Rename failed: {}", error)))?;
            FsResult::Renamed { from, to }
        }
        FsOp::Delete { path } => {
            let target = Path::new(&path);
            let removed = if target.is_dir() {
                std::fs::remove_dir_all(target)
            } else {
                std::fs::remove_file(target)
            };
            removed.map_err(|error| fail(format!("Delete failed: {}", error)))?;
            FsResult::Deleted { path }
        }
    };
    Ok(FsOutcome { id, result })
}

/// `base`, or `base-N` for the first N whose path is free on disk and not
/// `reserved`.
fn unique_path(base: &str, reserved: impl Fn(&str) -> bool) -> String {
    let mut path = base.to_string();
    let mut counter = 0u32;
    while Path::new(&path).exists() || reserved(&path) {
        counter += 1;
        path = format!("{}-{}", base, counter);
    }
    path
}
