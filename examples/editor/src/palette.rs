//! Colours for the editor chrome, derived from the active theme.
//!
//! Every colour comes from the design system: chrome from the colour tokens and
//! code-like accents from the `code_syntax` recipe. The editor therefore follows
//! light and dark modes and whichever design system is active, instead of fixing
//! one dark palette.

#[cfg(not(target_arch = "wasm32"))]
use crate::syntax::SyntaxKind;
use fission::core::op::Color;
use fission::theme::{recipes, Theme};

/// Fully transparent, for surfaces that exist only to take their child's shape.
const TRANSPARENT: Color = Color {
    r: 0,
    g: 0,
    b: 0,
    a: 0,
};

/// Visually invisible but not fully transparent, so a backdrop still receives
/// pointer events for dismissing flyouts.
const INTERACTION_BACKDROP: Color = Color {
    r: 0,
    g: 0,
    b: 0,
    a: 1,
};

/// The editor's colours, resolved from a theme.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EditorPalette {
    pub(crate) menu_bar_bg: Color,
    pub(crate) surface_bg: Color,
    pub(crate) border_color: Color,
    pub(crate) dim_text: Color,
    pub(crate) sidebar_heading: Color,
    pub(crate) bright_text: Color,
    pub(crate) flyout_bg: Color,
    pub(crate) flyout_border: Color,
    pub(crate) find_bar_bg: Color,
    pub(crate) terminal_bg: Color,
    pub(crate) activity_bar_bg: Color,
    pub(crate) transparent: Color,
    pub(crate) interaction_backdrop: Color,
    pub(crate) modal_backdrop: Color,
    pub(crate) welcome_bg: Color,
    pub(crate) welcome_title: Color,
    pub(crate) welcome_heading: Color,
    pub(crate) editor_selection: Color,
    pub(crate) completion_selected_bg: Color,
    pub(crate) completion_function: Color,
    pub(crate) completion_variable: Color,
    pub(crate) completion_keyword: Color,
    pub(crate) completion_type: Color,
    pub(crate) completion_module: Color,
    pub(crate) completion_fallback: Color,
    pub(crate) panel_text: Color,
    pub(crate) error_text: Color,
    pub(crate) warning_text: Color,
    pub(crate) info_text: Color,
    pub(crate) status_text: Color,
    pub(crate) status_error: Color,
    pub(crate) status_warning: Color,
    pub(crate) input_bg: Color,
    pub(crate) input_border: Color,
    pub(crate) git_added: Color,
    pub(crate) git_modified: Color,
    pub(crate) git_deleted: Color,
    pub(crate) tab_active_bg: Color,
    pub(crate) tab_inactive_bg: Color,
    pub(crate) tab_inactive_text: Color,
    pub(crate) tab_accent: Color,
    pub(crate) tooltip_bg: Color,
    pub(crate) tooltip_border: Color,
    pub(crate) minimap_bg: Color,
    pub(crate) minimap_viewport: Color,
    pub(crate) minimap_empty: Color,
    pub(crate) minimap_comment: Color,
    pub(crate) minimap_string: Color,
    pub(crate) minimap_code: Color,
    pub(crate) file_folder: Color,
    pub(crate) file_rust: Color,
    pub(crate) file_config: Color,
    pub(crate) file_web_style: Color,
    pub(crate) file_data: Color,
    pub(crate) file_script: Color,
    pub(crate) file_neutral: Color,
    pub(crate) file_muted: Color,
    pub(crate) file_html: Color,
    pub(crate) file_markup: Color,
    pub(crate) file_python: Color,
    pub(crate) file_yaml: Color,
    pub(crate) file_ruby: Color,
    pub(crate) file_go: Color,
    /// Highlighted source colours, looked up through [`EditorPalette::syntax`].
    #[cfg(not(target_arch = "wasm32"))]
    syntax_colors: SyntaxColors,
}

/// Colours for each [`SyntaxKind`], resolved from the `code_syntax` recipe.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
struct SyntaxColors {
    plain: Color,
    keyword: Color,
    string: Color,
    comment: Color,
    number: Color,
    type_name: Color,
    macro_name: Color,
    attribute: Color,
    lifetime: Color,
}

impl EditorPalette {
    /// Resolves every editor colour from `theme`.
    pub(crate) fn from_theme(theme: &Theme) -> Self {
        let c = &theme.tokens.colors;
        let code = theme.recipe(recipes::CodeSyntax);
        let syntax = |part: recipes::CodeSyntaxPart, fallback: Color| {
            code.part(part).text_color.unwrap_or(fallback)
        };
        Self {
            menu_bar_bg: c.surface_raised,
            surface_bg: c.surface,
            border_color: c.border,
            dim_text: c.text_secondary,
            sidebar_heading: c.text_secondary,
            bright_text: c.text_primary,
            flyout_bg: c.surface,
            flyout_border: c.border_strong,
            find_bar_bg: c.surface,
            terminal_bg: c.surface_sunken,
            activity_bar_bg: c.surface_raised,
            transparent: TRANSPARENT,
            interaction_backdrop: INTERACTION_BACKDROP,
            modal_backdrop: c.on_background.with_alpha(80),
            welcome_bg: c.background,
            welcome_title: c.text_muted,
            welcome_heading: c.text_secondary,
            editor_selection: c.primary.with_alpha(96),
            completion_selected_bg: c.primary_subtle,
            completion_function: syntax(recipes::CodeSyntaxPart::Function, c.warning),
            completion_variable: syntax(recipes::CodeSyntaxPart::Variable, c.info),
            completion_keyword: syntax(recipes::CodeSyntaxPart::Keyword, c.primary),
            completion_type: syntax(recipes::CodeSyntaxPart::Type, c.secondary),
            completion_module: syntax(recipes::CodeSyntaxPart::Constant, c.info),
            completion_fallback: c.text_secondary,
            panel_text: c.text_primary,
            error_text: c.error,
            warning_text: c.warning,
            info_text: c.info,
            status_text: c.text_primary,
            status_error: c.error,
            status_warning: c.warning,
            input_bg: c.surface_sunken,
            input_border: c.border_strong,
            git_added: c.success,
            git_modified: c.warning,
            git_deleted: c.error,
            tab_active_bg: c.background,
            tab_inactive_bg: c.surface_raised,
            tab_inactive_text: c.text_secondary,
            tab_accent: c.primary,
            tooltip_bg: c.surface_raised,
            tooltip_border: c.border_strong,
            minimap_bg: c.surface_sunken,
            minimap_viewport: c.text_primary.with_alpha(25),
            minimap_empty: c.background,
            minimap_comment: syntax(recipes::CodeSyntaxPart::Comment, c.text_muted),
            minimap_string: syntax(recipes::CodeSyntaxPart::String, c.success),
            minimap_code: c.text_muted,
            file_folder: c.warning,
            file_rust: syntax(recipes::CodeSyntaxPart::Keyword, c.primary),
            file_config: syntax(recipes::CodeSyntaxPart::Number, c.warning),
            file_web_style: syntax(recipes::CodeSyntaxPart::Function, c.info),
            file_data: syntax(recipes::CodeSyntaxPart::Constant, c.info),
            file_script: syntax(recipes::CodeSyntaxPart::Number, c.warning),
            file_neutral: c.text_secondary,
            file_muted: c.text_muted,
            file_html: syntax(recipes::CodeSyntaxPart::Deleted, c.error),
            file_markup: syntax(recipes::CodeSyntaxPart::Type, c.secondary),
            file_python: syntax(recipes::CodeSyntaxPart::String, c.success),
            file_yaml: syntax(recipes::CodeSyntaxPart::Deleted, c.error),
            file_ruby: c.error,
            file_go: c.info,
            #[cfg(not(target_arch = "wasm32"))]
            syntax_colors: SyntaxColors {
                plain: syntax(recipes::CodeSyntaxPart::Variable, c.text_primary),
                keyword: syntax(recipes::CodeSyntaxPart::Keyword, c.primary),
                string: syntax(recipes::CodeSyntaxPart::String, c.success),
                comment: syntax(recipes::CodeSyntaxPart::Comment, c.text_muted),
                number: syntax(recipes::CodeSyntaxPart::Number, c.warning),
                type_name: syntax(recipes::CodeSyntaxPart::Type, c.secondary),
                macro_name: syntax(recipes::CodeSyntaxPart::Function, c.info),
                attribute: syntax(recipes::CodeSyntaxPart::Constant, c.info),
                lifetime: syntax(recipes::CodeSyntaxPart::Punctuation, c.text_secondary),
            },
        }
    }

    /// The colour for a highlighted span of `kind`.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn syntax(&self, kind: SyntaxKind) -> Color {
        let colors = &self.syntax_colors;
        match kind {
            SyntaxKind::Plain => colors.plain,
            SyntaxKind::Keyword => colors.keyword,
            SyntaxKind::String => colors.string,
            SyntaxKind::Comment => colors.comment,
            SyntaxKind::Number => colors.number,
            SyntaxKind::Type => colors.type_name,
            SyntaxKind::Macro => colors.macro_name,
            SyntaxKind::Attribute => colors.attribute,
            SyntaxKind::Lifetime => colors.lifetime,
        }
    }
}
