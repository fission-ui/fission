use crate::{CodeHighlightingOptions, SitePageElement};
use fission_core::Env;
use fission_i18n::{I18nRegistry, TranslationBundle};
use fission_theme::{DesignMode, DesignSystem, PackagedFont, Theme};

/// Browser and social metadata for one rendered document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentMetadata {
    pub title: String,
    pub description: Option<String>,
}

impl DocumentMetadata {
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.env.theme = theme.clone();
        self.theme = theme;
        self
    }

    pub fn with_design_system<D: DesignSystem>(mut self, mode: DesignMode) -> Self {
        self.theme = D::theme(mode);
        self.env.theme = self.theme.clone();
        self.font_faces = D::font_faces();
        self
    }

    pub fn with_fonts(mut self, fonts: &'static [PackagedFont]) -> Self {
        self.font_faces = fonts;
        self
    }

    pub fn with_env(mut self, mut env: Env) -> Self {
        env.theme = self.theme.clone();
        self.env = env;
        self
    }

    pub fn with_i18n(mut self, i18n: I18nRegistry) -> Self {
        self.env.i18n = i18n;
        self
    }

    pub fn with_translation_bundle(mut self, bundle: TranslationBundle) -> Self {
        self.env.i18n.add_bundle(bundle);
        self
    }

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

    pub fn with_user_css(mut self, css: impl Into<String>) -> Self {
        self.user_css.push(css.into());
        self
    }

    pub fn with_page_element(mut self, element: SitePageElement) -> Self {
        self.page_elements.push(element);
        self
    }

    pub fn with_favicon(mut self, href: impl Into<String>) -> Self {
        self.favicon_href = Some(href.into());
        self
    }

    pub fn with_code_highlighting(mut self, options: CodeHighlightingOptions) -> Self {
        self.code_highlighting = Some(options);
        self
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn env(&self) -> &Env {
        &self.env
    }

    pub fn light_theme(&self) -> Option<&Theme> {
        self.light_theme.as_ref()
    }

    pub fn dark_theme(&self) -> Option<&Theme> {
        self.dark_theme.as_ref()
    }

    pub fn default_theme_mode(&self) -> Option<DesignMode> {
        self.default_theme_mode
    }

    pub fn theme_switching(&self) -> bool {
        self.theme_switching
    }

    pub fn user_css(&self) -> &[String] {
        &self.user_css
    }

    pub fn page_elements(&self) -> &[SitePageElement] {
        &self.page_elements
    }

    pub fn font_faces(&self) -> &'static [PackagedFont] {
        self.font_faces
    }

    pub fn favicon_href(&self) -> Option<&str> {
        self.favicon_href.as_deref()
    }

    pub fn code_highlighting(&self) -> Option<&CodeHighlightingOptions> {
        self.code_highlighting.as_ref()
    }
}
