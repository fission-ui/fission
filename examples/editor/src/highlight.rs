//! Syntax-highlighted text runs for the editor surface.
//!
//! `TextInput` draws `styled_runs` instead of its single-colour text, provided
//! the runs join back into exactly the input's value. Runs are built from the
//! highlighter's spans and coloured through [`EditorPalette`], so highlighting
//! follows the active theme and design system.

use crate::model::Language;
use crate::palette::EditorPalette;
use fission::op::TextRun;

/// Documents longer than this are shown as plain text; highlighting them on
/// every rebuild costs more than it helps.
pub(crate) const HIGHLIGHT_LINE_LIMIT: usize = 1000;

/// Highlighted runs for `content`, or `None` to show it as plain text.
///
/// Returns `None` for documents over [`HIGHLIGHT_LINE_LIMIT`] lines, and
/// whenever the runs would not reproduce `content` exactly (for example CRLF
/// line endings, which the highlighter normalises), since `TextInput` requires
/// the runs to match its value.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn highlighted_runs(
    content: &str,
    language: Language,
    palette: &EditorPalette,
    font_size: f32,
    line_height: f32,
) -> Option<Vec<TextRun>> {
    use crate::syntax::{self, SyntaxKind};
    use fission::op::{FontStyle, TextStyle, TextTypography};

    if content.lines().count() > HIGHLIGHT_LINE_LIMIT {
        return None;
    }
    let style = |kind: SyntaxKind| TextStyle {
        font_size,
        color: palette.syntax(kind),
        underline: false,
        font_family: None,
        locale: None,
        font_weight: 400,
        font_style: FontStyle::default(),
        line_height: Some(line_height),
        letter_spacing: 0.0,
        background_color: None,
        typography: TextTypography::default(),
    };

    let mut runs = Vec::new();
    for (index, line) in syntax::highlight_document(content, language)
        .into_iter()
        .enumerate()
    {
        if index > 0 {
            runs.push(TextRun {
                text: "\n".into(),
                style: style(SyntaxKind::Plain),
            });
        }
        runs.extend(
            line.into_iter()
                .filter(|span| !span.text.is_empty())
                .map(|span| TextRun {
                    style: style(span.kind),
                    text: span.text,
                }),
        );
    }

    let joined: String = runs.iter().map(|run| run.text.as_str()).collect();
    (joined == content).then_some(runs)
}

/// The syntax highlighter is not built for the web, so the editor shows plain
/// text there.
#[cfg(target_arch = "wasm32")]
pub(crate) fn highlighted_runs(
    _content: &str,
    _language: Language,
    _palette: &EditorPalette,
    _font_size: f32,
    _line_height: f32,
) -> Option<Vec<TextRun>> {
    None
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn runs(content: &str) -> Option<Vec<TextRun>> {
        let palette = EditorPalette::from_theme(&fission::theme::Theme::default());
        highlighted_runs(content, Language::Rust, &palette, 13.0, 20.0)
    }

    #[test]
    fn runs_rejoin_into_the_document() {
        let content = "fn main() {\n    let x = 42;\n}\n";
        let runs = runs(content).expect("rust source is highlighted");
        let joined: String = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(joined, content);
    }

    #[test]
    fn keywords_and_plain_text_get_different_colours() {
        let runs = runs("fn main() {}").expect("rust source is highlighted");
        let keyword = runs.iter().find(|run| run.text == "fn").expect("fn run");
        let name = runs
            .iter()
            .find(|run| run.text.contains("main"))
            .expect("main run");
        assert_ne!(keyword.style.color, name.style.color);
    }

    #[test]
    fn crlf_documents_fall_back_to_plain_text() {
        assert!(runs("fn main() {\r\n}\r\n").is_none());
    }

    #[test]
    fn long_documents_fall_back_to_plain_text() {
        let content = "let x = 1;\n".repeat(HIGHLIGHT_LINE_LIMIT + 1);
        assert!(runs(&content).is_none());
    }
}
