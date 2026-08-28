use crate::{CodeHighlightingOptions, SitePageElement};
use fission_core::Env;
use fission_i18n::{I18nRegistry, TranslationBundle};
use fission_theme::{DesignMode, DesignSystem, PackagedFont, Theme};

/// Browser and social metadata for one rendered document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentMetadata {
    /// Browser/tab and social-card title.
    pub title: String,
    /// Optional search/social meta description.
    pub description: Option<String>,
}

impl DocumentMetadata {
    /// Creates document metadata with a required title and optional description.
    pub fn new(title: impl Into<String>, description: impl Into<Option<String>>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
        }
    }
}

/// Document-level presentation shared by Static site and SSR shells.
///
/// This owns browser-document concerns that have identical meaning whether a
/// page is generated at build time or rendered for a request. Target-specific
/// routing, content discovery, caching, and request behavior remain on their
/// respective shells.
#[derive(Clone)]
pub struct DocumentShellConfig {
    theme: Theme,
    env: Env,
    light_theme: Option<Theme>,
    dark_theme: Option<Theme>,
    default_theme_mode: Option<DesignMode>,
    theme_switching: bool,
    user_css: Vec<String>,
    page_elements: Vec<SitePageElement>,
    font_faces: &'static [PackagedFont],
    favicon_href: Option<String>,
    code_highlighting: Option<CodeHighlightingOptions>,
}

impl Default for DocumentShellConfig {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            env: Env::default(),
            light_theme: None,
            dark_theme: None,
            default_theme_mode: None,
            theme_switching: false,
            user_css: Vec::new(),
            page_elements: Vec::new(),
            font_faces: &[],
            favicon_href: None,
            code_highlighting: None,
        }
    }
}

impl DocumentShellConfig {
    /// Creates document configuration using Fission's default environment and
    /// theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the active theme used for build-time/request-time rendering.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.env.theme = theme.clone();
        self.theme = theme;
        self
    }

    /// Installs a generated design system theme and its packaged font faces.
    pub fn with_design_system<D: DesignSystem>(mut self, mode: DesignMode) -> Self {
        self.theme = D::theme(mode);
        self.env.theme = self.theme.clone();
        self.font_faces = D::font_faces();
        self
    }

    /// Replaces the packaged font faces embedded in generated CSS.
    pub fn with_fonts(mut self, fonts: &'static [PackagedFont]) -> Self {
        self.font_faces = fonts;
        self
    }

    /// Replaces the base environment while retaining the separately selected
    /// document theme as the single theme authority.
    pub fn with_env(mut self, mut env: Env) -> Self {
        env.theme = self.theme.clone();
        self.env = env;
        self
    }

    /// Replaces the complete translation registry in the base environment.
    pub fn with_i18n(mut self, i18n: I18nRegistry) -> Self {
        self.env.i18n = i18n;
        self
    }

    /// Adds or replaces one locale's translation bundle.
    pub fn with_translation_bundle(mut self, bundle: TranslationBundle) -> Self {
        self.env.i18n.add_bundle(bundle);
        self
    }

    /// Enables browser theme switching using complete light and dark themes
    /// and chooses which one is rendered initially.
    pub fn with_light_dark_themes(
        mut self,
        light: Theme,
        dark: Theme,
        default_mode: DesignMode,
    ) -> Self {
        self.theme = match default_mode {
            DesignMode::Light => light.clone(),
            DesignMode::Dark => dark.clone(),
        };
        self.env.theme = self.theme.clone();
        self.light_theme = Some(light);
        self.dark_theme = Some(dark);
        self.default_theme_mode = Some(default_mode);
        self.theme_switching = true;
        self
    }

    /// Appends a trusted CSS block after the framework stylesheet.
    pub fn with_user_css(mut self, css: impl Into<String>) -> Self {
        self.user_css.push(css.into());
        self
    }

    /// Adds trusted host markup to matching document positions/routes.
    pub fn with_page_element(mut self, element: SitePageElement) -> Self {
        self.page_elements.push(element);
        self
    }

    /// Sets the favicon URL emitted into document heads.
    pub fn with_favicon(mut self, href: impl Into<String>) -> Self {
        self.favicon_href = Some(href.into());
        self
    }

    /// Configures conditional code-highlighting assets.
    pub fn with_code_highlighting(mut self, options: CodeHighlightingOptions) -> Self {
        self.code_highlighting = Some(options);
        self
    }

    /// Returns the active render theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Returns the base environment cloned for route rendering.
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// Returns the browser-switchable light theme when configured.
    pub fn light_theme(&self) -> Option<&Theme> {
        self.light_theme.as_ref()
    }

    /// Returns the browser-switchable dark theme when configured.
    pub fn dark_theme(&self) -> Option<&Theme> {
        self.dark_theme.as_ref()
    }

    /// Returns the initially selected mode when theme switching is configured.
    pub fn default_theme_mode(&self) -> Option<DesignMode> {
        self.default_theme_mode
    }

    /// Reports whether both light and dark theme variable sets are emitted.
    pub fn theme_switching(&self) -> bool {
        self.theme_switching
    }

    /// Returns app CSS blocks in cascade order.
    pub fn user_css(&self) -> &[String] {
        &self.user_css
    }

    /// Returns trusted document insertions in registration order.
    pub fn page_elements(&self) -> &[SitePageElement] {
        &self.page_elements
    }

    /// Returns font faces embedded into generated site CSS.
    pub fn font_faces(&self) -> &'static [PackagedFont] {
        self.font_faces
    }

    /// Returns the configured favicon URL.
    pub fn favicon_href(&self) -> Option<&str> {
        self.favicon_href.as_deref()
    }

    /// Returns explicit syntax-highlighting configuration when supplied.
    pub fn code_highlighting(&self) -> Option<&CodeHighlightingOptions> {
        self.code_highlighting.as_ref()
    }
}
