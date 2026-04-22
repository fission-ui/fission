//! Production-quality syntax highlighting using tree-sitter.
//!
//! Parses the entire document into a concrete syntax tree, walks it to extract
//! node types (keywords, strings, comments, etc.), and maps them to VS-Code-
//! Dark+-inspired colors.  Results are cached by content hash so re-builds that
//! do not change the text skip parsing entirely.
//!
//! Currently supports Rust via `tree-sitter-rust`.  TOML uses a lightweight
//! hand-rolled tokenizer (good enough for config files).  Other languages fall
//! back to plain unstyled text.

use crate::model::Language;
use fission_core::op::Color;

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use tree_sitter::{Node as TsNode, Parser, Tree};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub color: Color,
}

// ---------------------------------------------------------------------------
// Colour palette (VS Code Dark+ inspired)
// ---------------------------------------------------------------------------

const KEYWORD: Color = Color { r: 86, g: 156, b: 214, a: 255 };     // blue
const STRING_LIT: Color = Color { r: 206, g: 145, b: 120, a: 255 };  // brown/orange
const COMMENT: Color = Color { r: 106, g: 153, b: 85, a: 255 };      // green
const NUMBER: Color = Color { r: 181, g: 206, b: 168, a: 255 };      // light green
const TYPE_COLOR: Color = Color { r: 78, g: 201, b: 176, a: 255 };   // teal
const MACRO_COLOR: Color = Color { r: 220, g: 220, b: 170, a: 255 }; // yellow
const PUNCT: Color = Color { r: 212, g: 212, b: 212, a: 255 };       // white/gray
const IDENT: Color = Color { r: 156, g: 220, b: 254, a: 255 };       // light blue
const DEFAULT: Color = Color { r: 212, g: 212, b: 212, a: 255 };
const ATTRIBUTE_COLOR: Color = Color { r: 156, g: 220, b: 254, a: 255 }; // light blue
const LIFETIME_COLOR: Color = Color { r: 86, g: 156, b: 214, a: 255 };   // blue

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
pub fn highlight_line(line: &str, language: Language) -> Vec<StyledSpan> {
    match language {
        Language::Rust => {
            let doc = highlight_document(line, language);
            doc.into_iter().next().unwrap_or_else(|| vec![StyledSpan {
                text: line.to_string(),
                color: DEFAULT,
            }])
        }
        Language::Toml => highlight_toml_line(line),
        _ => vec![StyledSpan {
            text: line.to_string(),
            color: DEFAULT,
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

/// Invalidate the highlight cache.  Useful if theme colours change.
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
    let line_count = if content.ends_with('\n') {
        lines.len() + 1
    } else {
        lines.len().max(1)
    };

    // Start with every line as a single DEFAULT span
    let mut result: Vec<Vec<StyledSpan>> = lines
        .iter()
        .map(|l| {
            vec![StyledSpan {
                text: l.to_string(),
                color: DEFAULT,
            }]
        })
        .collect();

    // If content ended with newline, push an empty final line
    if content.ends_with('\n') {
        result.push(vec![StyledSpan {
            text: String::new(),
            color: DEFAULT,
        }]);
    }

    // Collect colored ranges from the syntax tree
    let mut colored_ranges: Vec<(usize, usize, usize, usize, Color)> = Vec::new();
    collect_colored_ranges(tree.root_node(), content, &mut colored_ranges);

    // Sort by start position so we process them in order
    colored_ranges.sort_by_key(|&(sr, sc, _, _, _)| (sr, sc));

    // Apply colored ranges to lines, splitting spans as needed
    for &(start_row, start_col, end_row, end_col, color) in &colored_ranges {
        apply_color_to_range(&mut result, &lines, start_row, start_col, end_row, end_col, color);
    }

    result
}

/// Recursively walk the tree-sitter tree and collect (start_row, start_col,
/// end_row, end_col, color) tuples for every node that should be colored.
fn collect_colored_ranges(
    node: TsNode,
    source: &str,
    out: &mut Vec<(usize, usize, usize, usize, Color)>,
) {
    let color = node_color(node, source);

    if let Some(c) = color {
        let start = node.start_position();
        let end = node.end_position();
        out.push((start.row, start.column, end.row, end.column, c));
        // Don't recurse into children of colored leaf-like nodes (the whole
        // range already has the right colour).  We still recurse for container
        // nodes like `macro_invocation` where children may need different
        // colours.
        if is_leaf_colored(node.kind()) {
            return;
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_colored_ranges(cursor.node(), source, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Returns true for node kinds whose entire text extent should be painted
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

/// Map a tree-sitter node kind to a colour.  Returns `None` if the node
/// should inherit the default colour or be handled by its children.
fn node_color(node: TsNode, source: &str) -> Option<Color> {
    match node.kind() {
        // Comments
        "line_comment" | "block_comment" => Some(COMMENT),

        // String / char literals
        "string_literal" | "raw_string_literal" | "string_content" | "char_literal" => {
            Some(STRING_LIT)
        }

        // Numeric literals
        "integer_literal" | "float_literal" => Some(NUMBER),

        // Boolean
        "boolean_literal" | "true" | "false" => Some(KEYWORD),

        // Rust keywords (leaf nodes whose text is the keyword itself)
        "fn" | "let" | "mut" | "pub" | "use" | "mod" | "struct" | "enum" | "impl" | "trait"
        | "for" | "while" | "loop" | "if" | "else" | "match" | "return" | "break"
        | "continue" | "const" | "static" | "type" | "where" | "as" | "in" | "ref" | "self"
        | "Self" | "super" | "crate" | "async" | "await" | "dyn" | "move" | "unsafe"
        | "extern" | "yield" => Some(KEYWORD),

        // Identifier-like nodes that tree-sitter may emit as keywords
        "mutable_specifier" => Some(KEYWORD), // `mut`

        // Type identifiers
        "type_identifier" | "primitive_type" => Some(TYPE_COLOR),

        // Macro invocations
        "macro_invocation" => {
            // Colour only the macro name (first child) — we still recurse
            // into arguments.
            None
        }

        // The `!` in a macro call and the macro name
        "!" => {
            // Check if parent is macro_invocation
            if let Some(parent) = node.parent() {
                if parent.kind() == "macro_invocation" {
                    return Some(MACRO_COLOR);
                }
            }
            None
        }

        // Attributes
        "attribute_item" | "inner_attribute_item" => Some(ATTRIBUTE_COLOR),

        // Lifetime labels
        "lifetime" | "label" => Some(LIFETIME_COLOR),

        _ => {
            // Handle identifier nodes that are macro names
            if node.kind() == "identifier" {
                if let Some(parent) = node.parent() {
                    if parent.kind() == "macro_invocation" {
                        return Some(MACRO_COLOR);
                    }
                }
            }
            None
        }
    }
}

/// Apply a colour to a (start_row, start_col) .. (end_row, end_col) range,
/// splitting existing spans as necessary.
fn apply_color_to_range(
    result: &mut Vec<Vec<StyledSpan>>,
    lines: &[&str],
    start_row: usize,
    start_col: usize,
    end_row: usize,
    end_col: usize,
    color: Color,
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
                        color: span.color,
                    });
                }

                // 2. The coloured region (intersection)
                let overlap_start = col_start.max(span_start) - span_start;
                let overlap_end = col_end.min(span_end) - span_start;
                if overlap_start < overlap_end && overlap_end <= span.text.len() {
                    new_spans.push(StyledSpan {
                        text: span.text[overlap_start..overlap_end].to_string(),
                        color,
                    });
                }

                // 3. After the coloured region
                if span_end > col_end {
                    let after_byte = col_end - span_start;
                    new_spans.push(StyledSpan {
                        text: span.text[after_byte..].to_string(),
                        color: span.color,
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
            color: COMMENT,
        }];
    }

    if trimmed.starts_with('[') {
        return vec![StyledSpan {
            text: line.to_string(),
            color: KEYWORD,
        }];
    }

    // key = value
    if let Some(eq_pos) = line.find('=') {
        let key = &line[..eq_pos];
        let rest = &line[eq_pos..];
        return vec![
            StyledSpan {
                text: key.to_string(),
                color: TYPE_COLOR,
            },
            StyledSpan {
                text: rest.to_string(),
                color: STRING_LIT,
            },
        ];
    }

    vec![StyledSpan {
        text: line.to_string(),
        color: DEFAULT,
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
                color: DEFAULT,
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
            spans.iter().any(|s| s.text == "fn" && s.color == KEYWORD),
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
            .filter(|s| s.color == COMMENT)
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
            spans.iter().any(|s| s.text.contains("hello") && s.color == STRING_LIT),
            "expected string literal span, got: {:?}",
            spans
        );
    }

    #[test]
    fn toml_section_highlighted() {
        let spans = highlight_line("[package]", Language::Toml);
        assert_eq!(spans[0].color, KEYWORD);
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
            doc[0].iter().any(|s| s.text == "fn" && s.color == KEYWORD),
            "first line: {:?}",
            doc[0]
        );

        // Second line should contain a number
        assert!(
            doc[1].iter().any(|s| s.text == "42" && s.color == NUMBER),
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
        assert_eq!(doc[0][0].color, KEYWORD); // [package]
        assert_eq!(doc[2][0].color, COMMENT); // # comment
    }
}
