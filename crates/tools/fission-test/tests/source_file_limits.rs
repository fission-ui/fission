//! Holds every Rust source file in the repository below 2,000 lines.
//!
//! Files that already exceed the limit are listed in `OVERSIZED` with their
//! length when the limit was introduced. They may shrink but never grow, and an
//! entry must be removed once its file is split below the limit, so the list
//! only gets shorter.

use std::fs;
use std::path::{Path, PathBuf};

const LIMIT: usize = 2_000;

/// Files over the limit when it was introduced, with their line counts then.
const OVERSIZED: &[(&str, usize)] = &[
    ("crates/authoring/fission-charts/src/chart.rs", 4877),
    ("crates/core/fission-core/src/runtime.rs", 3155),
    (
        "crates/core/fission-core/tests/input_controller_tests.rs",
        3573,
    ),
    ("crates/core/fission-ir/src/op.rs", 2449),
    ("crates/core/fission-layout/src/lib.rs", 5307),
    ("crates/core/fission-theme/src/lib.rs", 3932),
    ("crates/rendering/fission-render-vello/src/lib.rs", 4044),
    ("crates/shell/fission-shell-server/src/render.rs", 4149),
    ("crates/shell/fission-shell-site/src/build.rs", 2098),
    ("crates/shell/fission-shell-site/src/html.rs", 6930),
    (
        "crates/shell/fission-shell-winit/src/android_capabilities.rs",
        2357,
    ),
    ("crates/shell/fission-shell-winit/src/lib.rs", 11714),
    ("crates/shell/fission-shell-winit/src/pipeline.rs", 4479),
    (
        "crates/shell/fission-shell-winit/src/video_backend.rs",
        2915,
    ),
    ("crates/tools/fission-command-core/src/lib.rs", 6415),
    (
        "crates/tools/fission-command-package/src/lib_tests.rs",
        2038,
    ),
    ("crates/tools/fission-command-package/src/package.rs", 2378),
    ("crates/tools/fission-command-release/src/lib.rs", 2234),
    (
        "crates/tools/fission-command-release/src/publish_workflow.rs",
        2001,
    ),
    (
        "crates/tools/fission-command-release/src/store_ops.rs",
        2085,
    ),
    ("crates/tools/fission-command-run/src/lib.rs", 2570),
    (
        "crates/tools/fission-design-system-codegen/src/lib.rs",
        3626,
    ),
    (
        "publications/popl2027/experiments/trace-capture/src/main.rs",
        4454,
    ),
];

const SKIPPED_DIRECTORIES: &[&str] = &["target", "third_party", "node_modules", "dist", "build"];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("fission-test lives at crates/tools/fission-test")
        .to_path_buf()
}

fn rust_files(directory: &Path, root: &Path, files: &mut Vec<(String, usize)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !name.starts_with('.') && !SKIPPED_DIRECTORIES.contains(&name.as_ref()) {
                rust_files(&path, root, files);
            }
        } else if name.ends_with(".rs") {
            let bytes = fs::read(&path).expect("read Rust source file");
            let lines = bytes.iter().filter(|byte| **byte == b'\n').count();
            let relative = path
                .strip_prefix(root)
                .expect("source file is inside the repository")
                .to_string_lossy()
                .replace('\\', "/");
            files.push((relative, lines));
        }
    }
}

#[test]
fn rust_source_files_stay_below_the_line_limit() {
    let root = repository_root();
    let mut files = Vec::new();
    rust_files(&root, &root, &mut files);
    assert!(
        !files.is_empty(),
        "found no Rust files under {}",
        root.display()
    );

    let mut failures = Vec::new();
    for (path, lines) in &files {
        match OVERSIZED.iter().find(|(allowed, _)| allowed == path) {
            Some((_, baseline)) if lines > baseline => failures.push(format!(
                "{path} grew to {lines} lines; it was {baseline} when the {LIMIT}-line limit was introduced and may only shrink"
            )),
            Some(_) => {}
            None if *lines >= LIMIT => failures.push(format!(
                "{path} has {lines} lines; split it into focused modules below {LIMIT}"
            )),
            None => {}
        }
    }
    for (allowed, _) in OVERSIZED {
        match files.iter().find(|(path, _)| path == allowed) {
            None => failures.push(format!(
                "{allowed} is listed in OVERSIZED but no longer exists; remove the entry"
            )),
            Some((_, lines)) if *lines < LIMIT => failures.push(format!(
                "{allowed} is now {lines} lines, below the limit; remove it from OVERSIZED"
            )),
            Some(_) => {}
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
