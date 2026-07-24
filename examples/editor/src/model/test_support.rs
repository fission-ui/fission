pub(super) fn temp_file(name: &str, content: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, content).expect("write temp file");
    path.to_string_lossy().to_string()
}

pub(super) fn cleanup(path: &str) {
    std::fs::remove_file(path).ok();
}
