use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
}

fn rust_sources_under(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "target" || name == ".git" || name == "third_party" {
            continue;
        }
        if path.is_dir() {
            rust_sources_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn signature_returns_node(signature: &str) -> bool {
    let compact = signature.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.contains("fn build")
        && compact.contains("->")
        && !compact.contains("IntoWidget")
        && (compact.contains("-> Node")
            || compact.contains("-> fission_core::ui::Node")
            || compact.contains("-> fission::Node")
            || compact.contains("-> crate::ui::Node"))
}

fn find_matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn widget_build_methods_do_not_return_node() {
    let root = workspace_root();
    let mut sources = Vec::new();
    for dir in ["crates", "examples", "documentation/src"] {
        rust_sources_under(&root.join(dir), &mut sources);
    }

    let mut offenders = Vec::new();
    for source in sources {
        let text = fs::read_to_string(&source).expect("read source");
        let mut search_from = 0usize;
        while let Some(relative) = text[search_from..].find("impl") {
            let impl_start = search_from + relative;
            let Some(open) = text[impl_start..].find('{').map(|idx| impl_start + idx) else {
                break;
            };
            let signature = &text[impl_start..open];
            if !signature.contains("Widget<") || !signature.contains(" for ") {
                search_from = open + 1;
                continue;
            }
            let Some(close) = find_matching_brace(&text, open) else {
                break;
            };
            let body = &text[open + 1..close];
            let mut body_from = 0usize;
            while let Some(fn_relative) = body[body_from..].find("fn build(") {
                let fn_start = body_from + fn_relative;
                let signature_end = body[fn_start..]
                    .find('{')
                    .or_else(|| body[fn_start..].find(';'))
                    .map(|idx| fn_start + idx)
                    .unwrap_or(body.len());
                let build_signature = &body[fn_start..signature_end];
                if signature_returns_node(build_signature) {
                    let absolute = open + 1 + fn_start;
                    let line = text[..absolute].lines().count() + 1;
                    offenders.push(format!(
                        "{}:{}",
                        source.strip_prefix(&root).unwrap_or(&source).display(),
                        line
                    ));
                }
                body_from = signature_end + 1;
            }
            search_from = close + 1;
        }
    }

    assert!(
        offenders.is_empty(),
        "Widget::build must return impl IntoWidget<S>, not Node:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn node_is_not_a_widget_compatibility_type() {
    let view = fs::read_to_string(workspace_root().join("crates/core/fission-core/src/view.rs"))
        .expect("read view.rs");
    assert!(
        !view.contains("Widget<S> for Node") && !view.contains("IntoWidget<S> for Node"),
        "Node must not implement Widget or IntoWidget; build authors must return widgets"
    );
}
