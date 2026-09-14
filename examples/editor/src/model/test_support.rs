pub(super) fn temp_file(name: &str, content: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, content).expect("write temp file");
    path.to_string_lossy().to_string()
}

pub(super) fn cleanup(path: &str) {
    std::fs::remove_file(path).ok();
}

/// Runs every queued file operation synchronously, including any an outcome
/// queues in turn, as `EditorApp` would through `FS_JOB`.
pub(super) fn run_pending_fs(state: &mut super::EditorState) {
    while let Some(request) = state.pending_fs.first().cloned() {
        match super::run_fs_job(request) {
            Ok(outcome) => state.apply_fs_outcome(outcome),
            Err(failure) => state.apply_fs_failure(failure),
        }
    }
}
