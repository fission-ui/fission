//! Colours for the editor chrome, derived from the active theme.
//!
//! Every colour comes from the design system: chrome from the colour tokens and
//! code-like accents from the `code_syntax` recipe. The editor therefore follows
//! light and dark modes and whichever design system is active, instead of fixing
//! one dark palette.

use fission::core::op::Color;
use fission::theme::{recipe_names, Theme};

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
}

impl EditorPalette {
    /// Resolves every editor colour from `theme`.
    pub(crate) fn from_theme(theme: &Theme) -> Self {
        let c = &theme.tokens.colors;
        let code = theme.recipe(recipe_names::CODE_SYNTAX);
        let syntax = |part: &str, fallback: Color| code.part(part).text_color.unwrap_or(fallback);
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
            completion_function: syntax("function", c.warning),
            completion_variable: syntax("variable", c.info),
            completion_keyword: syntax("keyword", c.primary),
            completion_type: syntax("type", c.secondary),
            completion_module: syntax("constant", c.info),
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
            minimap_comment: syntax("comment", c.text_muted),
            minimap_string: syntax("string", c.success),
            minimap_code: c.text_muted,
            file_folder: c.warning,
            file_rust: syntax("keyword", c.primary),
            file_config: syntax("number", c.warning),
            file_web_style: syntax("function", c.info),
            file_data: syntax("constant", c.info),
            file_script: syntax("number", c.warning),
            file_neutral: c.text_secondary,
            file_muted: c.text_muted,
            file_html: syntax("deleted", c.error),
            file_markup: syntax("type", c.secondary),
            file_python: syntax("string", c.success),
            file_yaml: syntax("deleted", c.error),
            file_ruby: c.error,
            file_go: c.info,
        }
    }
}
