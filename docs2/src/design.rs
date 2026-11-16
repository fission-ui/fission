include!(concat!(env!("OUT_DIR"), "/docs_design_system.rs"));

/// Semantic HTML headings inherit their size from the heading element rather
/// than a nested Text run. Keep that site's CSS scale sourced from the DSP too.
pub(crate) fn heading_css() -> String {
    use fission::prelude::{DesignMode, DesignSystem};

    let theme = DocsDesignSystem::theme(DesignMode::Light);
    let typography = &theme.tokens.typography;
    format!(
        ":root{{--docs-display-size:{}px;--docs-title-size:{}px;--docs-heading-size:{}px}}",
        typography.display_sm_size, typography.heading1_size, typography.heading2_size,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn semantic_heading_css_uses_the_generated_design_system() {
        use fission::prelude::{DesignMode, DesignSystem};

        let theme = super::DocsDesignSystem::theme(DesignMode::Light);
        let css = super::heading_css();
        assert!(css.contains(&format!(
            "--docs-display-size:{}px",
            theme.tokens.typography.display_sm_size
        )));
        assert!(css.contains(&format!(
            "--docs-title-size:{}px",
            theme.tokens.typography.heading1_size
        )));
    }
}
