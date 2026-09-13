//! Production-quality syntax highlighting using tree-sitter.
//!
//! Parses the entire document into a concrete syntax tree, walks it to extract
//! node types (keywords, strings, comments, etc.), and tags each span with a
//! [`SyntaxKind`]. Kinds are turned into colours when the editor renders, from
//! the active theme, so cached highlights stay correct across theme changes.
//! Results are cached by content hash so re-builds that do not change the text
//! skip parsing entirely.
//!
//! Currently supports Rust via `tree-sitter-rust`.  TOML uses a lightweight
//! hand-rolled tokenizer (good enough for config files).  Other languages fall
//! back to plain unstyled text.

use crate::model::Language;

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use tree_sitter::Parser;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub text: String,
    pub kind: SyntaxKind,
}

/// What a span of source is, for choosing its colour from the theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxKind {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
    Type,
    Macro,
    Attribute,
    Lifetime,
}

// ---------------------------------------------------------------------------
// Cached parsers (one per language)
// ---------------------------------------------------------------------------

lazy_static::lazy_static! {
    static ref RUST_PARSER: Mutex<Parser> = {
        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        parser.set_language(&lang).expect("failed to load Rust grammar");
        Mutex::new(parser)
    };

    /// Cache of highlighted results keyed by (language-tag, content-hash).
    /// Avoids re-parsing when the content has not changed between rebuilds.
    static ref HIGHLIGHT_CACHE: Mutex<HashMap<(u8, u64), Vec<Vec<StyledSpan>>>> =
        Mutex::new(HashMap::new());

    /// In debug builds, if the last non-cached highlight took longer than
    /// `SLOW_THRESHOLD` we set this flag and skip tree-sitter on subsequent
    /// calls, falling back to plain text.  This prevents multi-second stalls
    /// when the unoptimised parser runs on large files.
    static ref HIGHLIGHT_TOO_SLOW: Mutex<bool> = Mutex::new(false);
}

/// If a single non-cached highlight pass takes longer than this, future calls
/// for the same session fall back to plain text.
const SLOW_THRESHOLD: std::time::Duration = std::time::Duration::from_millis(50);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Tokenize a single line.  Provided for backward-compatibility with call
/// sites that highlight line-by-line.  Internally delegates to
/// [`highlight_document`] on a single-line document.
#[cfg_attr(not(test), allow(dead_code))]
pub fn highlight_line(line: &str, language: Language) -> Vec<StyledSpan> {
    match language {
        Language::Rust => {
            let doc = highlight_document(line, language);
            doc.into_iter().next().unwrap_or_else(|| {
                vec![StyledSpan {
                    text: line.to_string(),
                    kind: SyntaxKind::Plain,
                }]
            })
        }
        Language::Toml => highlight_toml_line(line),
        _ => vec![StyledSpan {
            text: line.to_string(),
            kind: SyntaxKind::Plain,
        }],
    }
}

/// Highlight an entire document, returning one `Vec<StyledSpan>` per line.
///
/// Results are cached by content hash — calling this repeatedly with the same
/// content is essentially free after the first parse.
pub fn highlight_document(content: &str, language: Language) -> Vec<Vec<StyledSpan>> {
    let lang_tag = language_tag(language);
    let hash = content_hash(content);

    // Fast path: check cache
    {
        let cache = HIGHLIGHT_CACHE.lock().unwrap();
        if let Some(cached) = cache.get(&(lang_tag, hash)) {
            return cached.clone();
        }
    }

    // In debug builds, if a previous highlight was too slow, skip tree-sitter
    // entirely and fall back to plain text to keep the UI responsive.
    if cfg!(debug_assertions) {
        if let Ok(guard) = HIGHLIGHT_TOO_SLOW.lock() {
            if *guard && matches!(language, Language::Rust) {
                let result = plain_document(content);
                let mut cache = HIGHLIGHT_CACHE.lock().unwrap();
                if cache.len() > 50 {
                    cache.clear();
                }
                cache.insert((lang_tag, hash), result.clone());
                return result;
            }
        }
    }

    // Slow path: compute highlights
    let start = std::time::Instant::now();
    let result = match language {
        Language::Rust => highlight_rust_document(content),
        Language::Toml => highlight_toml_document(content),
        _ => plain_document(content),
    };
    let elapsed = start.elapsed();

    // If parsing took too long in a debug build, remember it so future
    // calls skip tree-sitter.
    if cfg!(debug_assertions) && elapsed > SLOW_THRESHOLD {
        if let Ok(mut guard) = HIGHLIGHT_TOO_SLOW.lock() {
            *guard = true;
        }
    }

    // Store in cache (limit size to avoid unbounded memory growth)
    {
        let mut cache = HIGHLIGHT_CACHE.lock().unwrap();
        if cache.len() > 50 {
            cache.clear();
        }
        cache.insert((lang_tag, hash), result.clone());
    }

    result
}

/// Invalidate the highlight cache. Highlights hold kinds, not colours, so a
/// theme change does not require this.
#[allow(dead_code)]
pub fn invalidate_cache() {
    HIGHLIGHT_CACHE.lock().unwrap().clear();
}

// ---------------------------------------------------------------------------
// Rust highlighting via tree-sitter
// ---------------------------------------------------------------------------

fn highlight_rust_document(content: &str) -> Vec<Vec<StyledSpan>> {
    let tree = {
        let mut parser = RUST_PARSER.lock().unwrap();
        parser.parse(content, None)
    };

    let tree = match tree {
        Some(t) => t,
        None => return plain_document(content),
    };

    let lines: Vec<&str> = content.lines().collect();
    // Handle trailing newline: if content ends with '\n' there is an implicit
    // empty final line that `lines()` drops.
    let _line_count = if content.ends_with('\n') {
        lines.len() + 1
    } else {
        lines.len().max(1)
    };

    // Start with every line as a single plain span
    let mut result: Vec<Vec<StyledSpan>> = lines
        .iter()
        .map(|l| {
            vec![StyledSpan {
                text: l.to_string(),
                kind: SyntaxKind::Plain,
            }]
        })
        .collect();

    // If content ended with newline, push an empty final line
    if content.ends_with('\n') {
        result.push(vec![StyledSpan {
            text: String::new(),
            kind: SyntaxKind::Plain,
        }]);
    }

    // Collect colored ranges from the syntax tree
    let mut colored_ranges: Vec<(usize, usize, usize, usize, SyntaxKind)> = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        let parent_kind = node.parent().map(|parent| parent.kind());
        if let Some(kind) = node_kind(node.kind(), parent_kind) {
            let start = node.start_position();
            let end = node.end_position();
            colored_ranges.push((start.row, start.column, end.row, end.column, kind));
            if is_leaf_colored(node.kind()) {
                continue;
            }
        }

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                stack.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // Sort by start position so we process them in order
    colored_ranges.sort_by_key(|&(sr, sc, _, _, _)| (sr, sc));

    // Apply colored ranges to lines, splitting spans as needed
    for &(start_row, start_col, end_row, end_col, kind) in &colored_ranges {
        apply_kind_to_range(
            &mut result,
            &lines,
            start_row,
            start_col,
            end_row,
            end_col,
            kind,
        );
    }

    result
}

/// Returns true for syntax tree item kinds whose entire text extent should be painted
/// with a single colour (no need to inspect children).
fn is_leaf_colored(kind: &str) -> bool {
    matches!(
        kind,
        "line_comment"
            | "block_comment"
            | "string_literal"
            | "raw_string_literal"
            | "string_content"
            | "char_literal"
            | "integer_literal"
            | "float_literal"
            | "boolean_literal"
            | "attribute_item"
            | "inner_attribute_item"
            | "lifetime"
            | "label"
    )
}

/// Map a tree-sitter item kind to a syntax kind.  Returns `None` if the item
/// should stay plain or be handled by its children.
fn node_kind(kind: &str, parent_kind: Option<&str>) -> Option<SyntaxKind> {
    match kind {
        // Comments
        "line_comment" | "block_comment" => Some(SyntaxKind::Comment),

        // String / char literals
        "string_literal" | "raw_string_literal" | "string_content" | "char_literal" => {
            Some(SyntaxKind::String)
        }

        // Numeric literals
        "integer_literal" | "float_literal" => Some(SyntaxKind::Number),

        // Boolean
        "boolean_literal" | "true" | "false" => Some(SyntaxKind::Keyword),

        // Rust keywords (leaf nodes whose text is the keyword itself)
        "fn" | "let" | "mut" | "pub" | "use" | "mod" | "struct" | "enum" | "impl" | "trait"
        | "for" | "while" | "loop" | "if" | "else" | "match" | "return" | "break" | "continue"
        | "const" | "static" | "type" | "where" | "as" | "in" | "ref" | "self" | "Self"
        | "super" | "crate" | "async" | "await" | "dyn" | "move" | "unsafe" | "extern"
        | "yield" => Some(SyntaxKind::Keyword),

        // Identifier-like nodes that tree-sitter may emit as keywords
        "mutable_specifier" => Some(SyntaxKind::Keyword), // `mut`

        // Type identifiers
        "type_identifier" | "primitive_type" => Some(SyntaxKind::Type),

        // Macro invocations
        "macro_invocation" => {
            // Colour only the macro name (first child) — we still recurse
            // into arguments.
            None
        }

        // The `!` in a macro call and the macro name
        "!" => {
            if parent_kind == Some("macro_invocation") {
                return Some(SyntaxKind::Macro);
            }
            None
        }

        // Attributes
        "attribute_item" | "inner_attribute_item" => Some(SyntaxKind::Attribute),

        // Lifetime labels
        "lifetime" | "label" => Some(SyntaxKind::Lifetime),

        _ => {
            // Handle identifier nodes that are macro names
            if kind == "identifier" && parent_kind == Some("macro_invocation") {
                return Some(SyntaxKind::Macro);
            }
            None
        }
    }
}

/// Tag a (start_row, start_col) .. (end_row, end_col) range with `kind`,
/// splitting existing spans as necessary.
fn apply_kind_to_range(
    result: &mut Vec<Vec<StyledSpan>>,
    lines: &[&str],
    start_row: usize,
    start_col: usize,
    end_row: usize,
    end_col: usize,
    kind: SyntaxKind,
) {
    for row in start_row..=end_row {
        if row >= result.len() || row >= lines.len() {
            break;
        }
        let line = lines[row];
        let col_start = if row == start_row { start_col } else { 0 };
        let col_end = if row == end_row {
            end_col.min(line.len())
        } else {
            line.len()
        };

        if col_start >= col_end {
            continue;
        }

        // Rebuild the spans for this line, splitting any span that overlaps
        // with [col_start..col_end].
        let old_spans = std::mem::take(&mut result[row]);
        let mut new_spans: Vec<StyledSpan> = Vec::with_capacity(old_spans.len() + 2);
        let mut pos: usize = 0;

        for span in old_spans {
            let span_start = pos;
            let span_end = pos + span.text.len();

            if span_end <= col_start || span_start >= col_end {
                // No overlap — keep as-is
                new_spans.push(span);
            } else {
                // There is overlap — split into up to 3 pieces
                // 1. Before the coloured region
                if span_start < col_start {
                    let before_byte = col_start - span_start;
                    new_spans.push(StyledSpan {
                        text: span.text[..before_byte].to_string(),
                        kind: span.kind,
                    });
                }

                // 2. The coloured region (intersection)
                let overlap_start = col_start.max(span_start) - span_start;
                let overlap_end = col_end.min(span_end) - span_start;
                if overlap_start < overlap_end && overlap_end <= span.text.len() {
                    new_spans.push(StyledSpan {
                        text: span.text[overlap_start..overlap_end].to_string(),
                        kind,
                    });
                }

                // 3. After the coloured region
                if span_end > col_end {
                    let after_byte = col_end - span_start;
                    new_spans.push(StyledSpan {
                        text: span.text[after_byte..].to_string(),
                        kind: span.kind,
                    });
                }
            }

            pos = span_end;
        }

        result[row] = new_spans;
    }
}

// ---------------------------------------------------------------------------
// TOML highlighting (hand-rolled, kept from previous implementation)
// ---------------------------------------------------------------------------

fn highlight_toml_document(content: &str) -> Vec<Vec<StyledSpan>> {
    content.lines().map(|l| highlight_toml_line(l)).collect()
}

fn highlight_toml_line(line: &str) -> Vec<StyledSpan> {
    let trimmed = line.trim_start();

    if trimmed.starts_with('#') {
        return vec![StyledSpan {
            text: line.to_string(),
            kind: SyntaxKind::Comment,
        }];
    }

    if trimmed.starts_with('[') {
        return vec![StyledSpan {
            text: line.to_string(),
            kind: SyntaxKind::Keyword,
        }];
    }

    // key = value
    if let Some(eq_pos) = line.find('=') {
        let key = &line[..eq_pos];
        let rest = &line[eq_pos..];
        return vec![
            StyledSpan {
                text: key.to_string(),
                kind: SyntaxKind::Type,
            },
            StyledSpan {
                text: rest.to_string(),
                kind: SyntaxKind::String,
            },
        ];
    }

    vec![StyledSpan {
        text: line.to_string(),
        kind: SyntaxKind::Plain,
    }]
}

// ---------------------------------------------------------------------------
// Fallback for unsupported languages
// ---------------------------------------------------------------------------

fn plain_document(content: &str) -> Vec<Vec<StyledSpan>> {
    content
        .lines()
        .map(|l| {
            vec![StyledSpan {
                text: l.to_string(),
                kind: SyntaxKind::Plain,
            }]
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn language_tag(lang: Language) -> u8 {
    match lang {
        Language::Rust => 0,
        Language::Toml => 1,
        Language::Markdown => 2,
        Language::Json => 3,
        Language::Plain => 255,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_keyword_highlighted() {
        let spans = highlight_line("fn main() {", Language::Rust);
        assert!(
            spans
                .iter()
                .any(|s| s.text == "fn" && s.kind == SyntaxKind::Keyword),
            "expected 'fn' keyword span, got: {:?}",
            spans
        );
    }

    #[test]
    fn rust_comment_highlighted() {
        let spans = highlight_line("// this is a comment", Language::Rust);
        // The whole line should be a comment
        let comment_text: String = spans
            .iter()
            .filter(|s| s.kind == SyntaxKind::Comment)
            .map(|s| s.text.as_str())
            .collect();
        assert!(
            comment_text.contains("// this is a comment"),
            "expected comment span, got: {:?}",
            spans
        );
    }

    #[test]
    fn rust_string_highlighted() {
        let spans = highlight_line("let x = \"hello\";", Language::Rust);
        assert!(
            spans
                .iter()
                .any(|s| s.text.contains("hello") && s.kind == SyntaxKind::String),
            "expected string literal span, got: {:?}",
            spans
        );
    }

    #[test]
    fn toml_section_highlighted() {
        let spans = highlight_line("[package]", Language::Toml);
        assert_eq!(spans[0].kind, SyntaxKind::Keyword);
    }

    #[test]
    fn plain_text_no_crash() {
        let spans = highlight_line("just some text", Language::Plain);
        assert!(!spans.is_empty());
    }

    #[test]
    fn document_level_rust_highlight() {
        let src = "fn main() {\n    let x = 42;\n}\n";
        let doc = highlight_document(src, Language::Rust);
        assert_eq!(doc.len(), 4); // 3 lines + trailing empty from '\n'

        // First line should contain an "fn" keyword span
        assert!(
            doc[0]
                .iter()
                .any(|s| s.text == "fn" && s.kind == SyntaxKind::Keyword),
            "first line: {:?}",
            doc[0]
        );

        // Second line should contain a number
        assert!(
            doc[1]
                .iter()
                .any(|s| s.text == "42" && s.kind == SyntaxKind::Number),
            "second line: {:?}",
            doc[1]
        );
    }

    #[test]
    fn caching_returns_same_result() {
        let src = "let x = 1;";
        let a = highlight_document(src, Language::Rust);
        let b = highlight_document(src, Language::Rust);
        assert_eq!(a.len(), b.len());
        for (a_line, b_line) in a.iter().zip(b.iter()) {
            assert_eq!(a_line.len(), b_line.len());
        }
    }

    #[test]
    fn toml_document_highlight() {
        let src = "[package]\nname = \"foo\"\n# comment\n";
        let doc = highlight_document(src, Language::Toml);
        assert!(doc.len() >= 3);
        assert_eq!(doc[0][0].kind, SyntaxKind::Keyword); // [package]
        assert_eq!(doc[2][0].kind, SyntaxKind::Comment); // # comment
    }
}
