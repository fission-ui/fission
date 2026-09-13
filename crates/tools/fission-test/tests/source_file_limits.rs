//! Holds every Rust source file in the repository below 2,000 lines.
//!
//! Files that already exceed the limit are listed in `OVERSIZED` with their
//! length when the limit was introduced. They may shrink but never grow, and an
//! entry must be removed once its file is split below the limit, so the list
//! only gets shorter.
//!
//! Files are listed through git, so only sources a commit would include count:
//! ignored local checkouts and build output never do.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    ("crates/core/fission-theme/src/lib.rs", 3934),
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
        3627,
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

/// Every Rust file a commit would include: tracked files still on disk, plus new
/// files git does not ignore. Paths are relative to `root`, with `/` separators.
fn rust_files(root: &Path) -> Vec<(String, usize)> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.rs",
        ])
        .current_dir(root)
        .output()
        .expect("run git ls-files");
    assert!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut files: Vec<(String, usize)> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).replace('\\', "/"))
        .filter(|path| {
            !path
                .split('/')
                .any(|part| part.starts_with('.') || SKIPPED_DIRECTORIES.contains(&part))
        })
        .filter_map(|path| {
            let bytes = fs::read(root.join(&path)).ok()?;
            let lines = bytes.iter().filter(|byte| **byte == b'\n').count();
            Some((path, lines))
        })
        .collect();
    files.sort();
    files.dedup();
    files
}

#[test]
fn rust_source_files_stay_below_the_line_limit() {
    let root = repository_root();
    let files = rust_files(&root);
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
