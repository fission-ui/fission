use anyhow::{anyhow, bail, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use fission_core::{
    registry::{VideoRegistration, WebRegistration},
    MotionDeclaration, MotionDeclarationKind, MotionEasing, MotionExpr, MotionPredicate,
    MotionPropertyId, MotionStartValue, MotionTrack, MotionTransition, MotionValue,
};
use fission_ir::op::{
    decode_inline_widget_marker, AlignItems, BorderSides, BoxShadow, Color, CompositeScalar,
    CornerRadii, EmbedKind, Fill, FlexDirection, FlexWrap, FontStyle, GradientExtend,
    GridPlacement, GridTrack, ImageAlignment, ImageFit, ImageSource, JustifyContent, LayoutOp,
    Length, LineCap, LineJoin, Op, Overflow, PaintOp, RichTextAnnotation, Stroke, TextAlign,
    TextOverflow, TextRun,
};
use fission_ir::{
    semantics::ActionTrigger, CoreIR, CoreNode, PopupKind, Role, SemanticOrientation, Semantics,
    WidgetId,
};
use fission_theme::{DesignMode, PackagedFont, PackagedFontStyle, Theme};
use std::collections::{BTreeMap, HashSet};

use crate::WebFontDelivery;

mod path_css;

#[derive(Clone, Debug)]
/// Document, enhancement, and renderer inputs for semantic HTML lowering.
pub struct HtmlRenderOptions {
    /// BCP 47 language emitted on the root HTML element.
    pub lang: String,
    /// Browser document title.
    pub document_title: String,
    /// Optional meta description.
    pub description: Option<String>,
    /// Optional absolute canonical URL.
    pub canonical_url: Option<String>,
    /// Optional Open Graph site name.
    pub site_name: Option<String>,
    /// Optional favicon URL.
    pub favicon_href: Option<String>,
    /// URL of the generated/shared site stylesheet.
    pub stylesheet_href: String,
    /// CSS class placed on the rendered app root.
    pub root_class: String,
    /// Current public path used to resolve links and current-page state.
    pub current_route_path: String,
    /// CSS custom properties resolved from the active theme.
    pub css_variables: CssVariableMap,
    /// Initially selected light/dark mode when switching is enabled.
    pub default_theme_mode: Option<DesignMode>,
    /// Whether browser theme controls and persistence are enabled.
    pub theme_switching: bool,
    /// Conditional syntax-highlighting asset configuration.
    pub code_highlighting: CodeHighlightingOptions,
    /// Optional browser search-controller script URL.
    pub search_script_href: Option<String>,
    /// SSR endpoint receiving signed server-action form posts.
    pub server_action_post_path: Option<String>,
    /// Signed token indexed by semantic target and action ID.
    pub server_action_tokens: BTreeMap<(WidgetId, u128), String>,
    /// Whether browser-island action attributes are emitted.
    pub browser_action_bindings: bool,
    /// Trusted JSON-LD documents inserted into the page head.
    pub structured_data: Vec<String>,
    /// Trusted markup inserted immediately after opening `<head>`.
    pub head_start_html: Vec<String>,
    /// Trusted markup inserted immediately before closing `</head>`.
    pub head_end_html: Vec<String>,
    /// Trusted markup inserted immediately after opening `<body>`.
    pub body_start_html: Vec<String>,
    /// Trusted markup inserted immediately before closing `</body>`.
    pub body_end_html: Vec<String>,
    /// Build-time motion declarations lowered to CSS.
    pub motion_declarations: Vec<MotionDeclaration>,
    /// Video registrations keyed by lowered semantic node.
    pub video_registrations: BTreeMap<WidgetId, VideoRegistration>,
    /// WebView/iframe registrations keyed by lowered semantic node.
    pub web_registrations: BTreeMap<WidgetId, WebRegistration>,
    /// Font faces embedded by the selected design system.
    pub font_faces: &'static [PackagedFont],
    /// Whether generated node CSS is linked as a content-addressed stylesheet.
    ///
    /// SSR enables this so route-specific rules never accumulate in the shared
    /// stylesheet. Static-site builds reuse one registry and emit one complete
    /// stylesheet after rendering every route.
    pub separate_generated_stylesheet: bool,
}

impl Default for HtmlRenderOptions {
    fn default() -> Self {
        Self {
            lang: "en".to_string(),
            document_title: "Static site".to_string(),
            description: None,
            canonical_url: None,
            site_name: None,
            favicon_href: None,
            stylesheet_href: "/site.css".to_string(),
            root_class: "fission-site-root".to_string(),
            current_route_path: "/".to_string(),
            css_variables: CssVariableMap::default(),
            default_theme_mode: None,
            theme_switching: false,
            code_highlighting: CodeHighlightingOptions::default(),
            search_script_href: None,
            server_action_post_path: None,
            server_action_tokens: BTreeMap::new(),
            browser_action_bindings: false,
            structured_data: Vec::new(),
            head_start_html: Vec::new(),
            head_end_html: Vec::new(),
            body_start_html: Vec::new(),
            body_end_html: Vec::new(),
            motion_declarations: Vec::new(),
            video_registrations: BTreeMap::new(),
            web_registrations: BTreeMap::new(),
            font_faces: &[],
            separate_generated_stylesheet: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// External assets used to highlight code blocks in generated pages.
pub struct CodeHighlightingOptions {
    /// Whether highlighting assets and initialization are emitted.
    pub enabled: bool,
    /// Highlighting theme stylesheet URL.
    pub stylesheet_href: String,
    /// Highlighting JavaScript URL.
    pub script_src: String,
}

impl Default for CodeHighlightingOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            stylesheet_href:
                "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.11.1/styles/github-dark.min.css"
                    .to_string(),
            script_src:
                "https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.11.1/highlight.min.js"
                    .to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// HTML and CSS products of lowering one Core IR document.
pub struct RenderedHtml {
    /// Complete standalone HTML document.
    pub html: String,
    /// Rendered app-root fragment without surrounding document chrome.
    pub body_html: String,
    /// Deduplicated CSS generated while rendering.
    pub css: String,
    /// Content-addressed stylesheet URL when generated CSS is emitted separately.
    pub generated_stylesheet_href: Option<String>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StaticFormSpec {
    action: String,
    #[serde(default = "default_form_method")]
    method: String,
    #[serde(default)]
    fields: Vec<StaticFormField>,
    #[serde(default)]
    submit_label: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StaticFormField {
    name: String,
    #[serde(default = "default_form_field_kind")]
    kind: StaticFormFieldKind,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    placeholder: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    max_length: Option<usize>,
    #[serde(default)]
    rows: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum StaticFormFieldKind {
    #[default]
    Text,
    Email,
    Tel,
    Url,
    Hidden,
    Textarea,
    Checkbox,
}

fn default_form_method() -> String {
    "post".to_string()
}

fn default_form_field_kind() -> StaticFormFieldKind {
    StaticFormFieldKind::Text
}

/// Validates and lowers Core IR using a fresh style registry.
pub fn render_ir_to_html(ir: &CoreIR, options: &HtmlRenderOptions) -> Result<RenderedHtml> {
    let mut registry = StyleRegistry::default();
    render_ir_to_html_with_styles(ir, options, &mut registry)
}

/// Lowers Core IR while reusing `styles` to deduplicate CSS across routes.
pub fn render_ir_to_html_with_styles(
    ir: &CoreIR,
    options: &HtmlRenderOptions,
    styles: &mut StyleRegistry,
) -> Result<RenderedHtml> {
    validate_static_ir(
        ir,
        options.server_action_post_path.is_some() || options.browser_action_bindings,
    )?;
    let root = ir
        .root
        .ok_or_else(|| anyhow!("site render failed: Core IR has no root node"))?;
    let mut renderer = HtmlRenderer {
        ir,
        options,
        styles,
        has_code_blocks: false,
    };
    renderer.register_interaction_motion_styles();
    let body = renderer.render_node(root)?;
    let has_code_blocks = renderer.has_code_blocks;
    let direction_attr = match ir.layout_direction {
        fission_ir::LayoutDirection::LeftToRight => "",
        fission_ir::LayoutDirection::RightToLeft => " dir=\"rtl\"",
    };
    let body_html = format!(
        "<div class=\"{}\"{direction_attr}>{body}</div>",
        escape_attr(&options.root_class)
    );
    let css = renderer.styles.to_css();
    let generated_stylesheet_href = (options.separate_generated_stylesheet && !css.is_empty())
        .then(|| generated_stylesheet_href(&css));
    let html = render_document(
        &body_html,
        options,
        has_code_blocks,
        generated_stylesheet_href.as_deref(),
    );
    Ok(RenderedHtml {
        html,
        body_html,
        css,
        generated_stylesheet_href,
    })
}

/// Returns the relative, content-addressed URL used by generated font CSS.
pub fn packaged_font_asset_href(font: &PackagedFont) -> String {
    format!(
        "__fission/fonts/{:016x}.{}",
        stable_hash(font.data),
        font_extension(font.format)
    )
}

/// Returns an absolute request path for a packaged font asset.
pub fn packaged_font_asset_path(font: &PackagedFont) -> String {
    format!("/{}", packaged_font_asset_href(font))
}

/// Returns the MIME type served for a packaged font asset.
pub fn packaged_font_content_type(font: &PackagedFont) -> &'static str {
    match font_extension(font.format) {
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "otf" => "font/otf",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

/// Generates one `@font-face` rule when packaged-font delivery is enabled.
pub fn packaged_font_css(font: &PackagedFont, delivery: WebFontDelivery) -> Option<String> {
    let style = match font.style {
        PackagedFontStyle::Normal => "normal",
        PackagedFontStyle::Italic => "italic",
        PackagedFontStyle::Oblique => "oblique",
    };
    let axes = if font.axes.is_empty() {
        String::new()
    } else {
        let settings = font
            .axes
            .iter()
            .map(|axis| {
                let tag = String::from_utf8_lossy(&axis.tag);
                format!("'{}' {}", escape_attr(&tag), axis.value)
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("font-variation-settings:{settings};")
    };
    let source = match delivery {
        WebFontDelivery::None => return None,
        WebFontDelivery::External => packaged_font_asset_href(font),
        WebFontDelivery::Inline => format!(
            "data:{};base64,{}",
            packaged_font_content_type(font),
            BASE64_STANDARD.encode(font.data)
        ),
    };
    Some(format!(
        "@font-face{{font-family:'{}';font-style:{style};font-weight:{};font-display:swap;{axes}src:url('{}') format('{}')}}",
        escape_attr(font.family),
        font.weight,
        escape_attr(&source),
        escape_attr(font.format),
    ))
}

fn font_extension(format: &str) -> &'static str {
    match format.to_ascii_lowercase().as_str() {
        "woff2" => "woff2",
        "woff" => "woff",
        "opentype" | "otf" => "otf",
        "truetype" | "ttf" => "ttf",
        _ => "bin",
    }
}

/// Returns the content-addressed URL for generated route CSS.
pub fn generated_stylesheet_href(css: &str) -> String {
    format!("/__fission/styles/{:016x}.css", stable_hash(css.as_bytes()))
}

fn render_document(
    body_html: &str,
    options: &HtmlRenderOptions,
    has_code_blocks: bool,
    generated_stylesheet_href: Option<&str>,
) -> String {
    let head_start_html = raw_page_elements(&options.head_start_html, 4);
    let head_end_html = raw_page_elements(&options.head_end_html, 4);
    let body_start_html = raw_page_elements(&options.body_start_html, 4);
    let body_end_html = raw_page_elements(&options.body_end_html, 4);
    let mut metadata = String::new();
    if let Some(value) = options.description.as_ref() {
        metadata.push_str(&format!(
            "\n    <meta name=\"description\" content=\"{}\">",
            escape_attr(value)
        ));
    }
    if let Some(canonical) = options.canonical_url.as_ref() {
        metadata.push_str(&format!(
            "\n    <link rel=\"canonical\" href=\"{}\">",
            escape_attr(canonical)
        ));
    }
    metadata.push_str(&format!(
        "\n    <meta property=\"og:title\" content=\"{}\">",
        escape_attr(&options.document_title)
    ));
    if let Some(value) = options.description.as_ref() {
        metadata.push_str(&format!(
            "\n    <meta property=\"og:description\" content=\"{}\">",
            escape_attr(value)
        ));
    }
    metadata.push_str("\n    <meta property=\"og:type\" content=\"website\">");
    if let Some(canonical) = options.canonical_url.as_ref() {
        metadata.push_str(&format!(
            "\n    <meta property=\"og:url\" content=\"{}\">",
            escape_attr(canonical)
        ));
    }
    if let Some(site_name) = options.site_name.as_ref() {
        metadata.push_str(&format!(
            "\n    <meta property=\"og:site_name\" content=\"{}\">",
            escape_attr(site_name)
        ));
    }
    metadata.push_str(&format!(
        "\n    <meta property=\"og:locale\" content=\"{}\">",
        escape_attr(&options.lang.replace('-', "_"))
    ));
    metadata.push_str("\n    <meta name=\"robots\" content=\"index,follow\">");
    if let Some(site_name) = options.site_name.as_ref() {
        metadata.push_str(&format!(
            "\n    <meta name=\"application-name\" content=\"{}\">",
            escape_attr(site_name)
        ));
    }
    metadata.push_str("\n    <meta name=\"twitter:card\" content=\"summary_large_image\">");
    metadata.push_str(&format!(
        "\n    <meta name=\"twitter:title\" content=\"{}\">",
        escape_attr(&options.document_title)
    ));
    if let Some(value) = options.description.as_ref() {
        metadata.push_str(&format!(
            "\n    <meta name=\"twitter:description\" content=\"{}\">",
            escape_attr(value)
        ));
    }
    for json in &options.structured_data {
        metadata.push_str("\n    <script type=\"application/ld+json\">");
        metadata.push_str(&escape_script_data(json));
        metadata.push_str("</script>");
    }
    if let Some(favicon) = options.favicon_href.as_ref() {
        metadata.push_str(&favicon_link_tags(favicon));
    }
    let theme_attr = options
        .default_theme_mode
        .map(|mode| {
            let mode = match mode {
                DesignMode::Light => "light",
                DesignMode::Dark => "dark",
            };
            format!(" data-theme=\"{mode}\"")
        })
        .unwrap_or_default();
    let code_highlighting_assets = code_highlighting_assets(options, has_code_blocks);
    let search_script = search_script(options);
    let enhancement_script = site_enhancement_script(options);
    let generated_stylesheet = generated_stylesheet_href
        .map(|href| {
            format!(
                "\n    <link rel=\"stylesheet\" href=\"{}\">",
                escape_attr(href)
            )
        })
        .unwrap_or_default();
    format!(
        "<!doctype html>\n<html lang=\"{}\"{theme_attr}>\n  <head>{head_start_html}\n    <meta charset=\"utf-8\">\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">{metadata}\n    <title>{}</title>\n    <link rel=\"stylesheet\" href=\"{}\">{generated_stylesheet}{code_highlighting_assets}{search_script}{enhancement_script}{head_end_html}\n  </head>\n  <body>{body_start_html}\n    {body_html}{body_end_html}\n  </body>\n</html>\n",
        escape_attr(&options.lang),
        escape_text(&options.document_title),
        escape_attr(&options.stylesheet_href)
    )
}

fn raw_page_elements(elements: &[String], indent_spaces: usize) -> String {
    if elements.is_empty() {
        return String::new();
    }
    let indent = " ".repeat(indent_spaces);
    let mut out = String::new();
    for element in elements {
        out.push('\n');
        for line in element.trim().lines() {
            out.push_str(&indent);
            out.push_str(line);
            out.push('\n');
        }
        if out.ends_with('\n') {
            out.pop();
        }
    }
    out
}

fn favicon_link_tags(href: &str) -> String {
    let mime = favicon_mime_type(href);
    format!(
        "\n    <link rel=\"icon\" href=\"{}\" type=\"{}\">\n    <link rel=\"shortcut icon\" href=\"{}\" type=\"{}\">",
        escape_attr(href),
        mime,
        escape_attr(href),
        mime,
    )
}

fn favicon_mime_type(href: &str) -> &'static str {
    let path = href.split(['#', '?']).next().unwrap_or(href);
    match path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
    {
        Some(extension) if extension == "svg" => "image/svg+xml",
        Some(extension) if extension == "png" => "image/png",
        Some(extension) if extension == "jpg" || extension == "jpeg" => "image/jpeg",
        Some(extension) if extension == "webp" => "image/webp",
        Some(extension) if extension == "ico" => "image/x-icon",
        _ => "image/x-icon",
    }
}

fn code_highlighting_assets(options: &HtmlRenderOptions, has_code_blocks: bool) -> String {
    if !has_code_blocks || !options.code_highlighting.enabled {
        return String::new();
    }
    format!(
        "\n    <link rel=\"stylesheet\" href=\"{}\">\n    <script defer src=\"{}\"></script>\n    <script>document.addEventListener('DOMContentLoaded',function(){{if(window.hljs){{window.hljs.highlightAll();}}}});</script>",
        escape_attr(&options.code_highlighting.stylesheet_href),
        escape_attr(&options.code_highlighting.script_src),
    )
}

fn escape_script_data(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len() + 1);
    let mut index = 0;
    while index < bytes.len() {
        let is_script_end = index + 8 <= bytes.len()
            && bytes[index] == b'<'
            && bytes[index + 1] == b'/'
            && is_case_insensitive_eq(&bytes[index + 2..index + 8], b"script");
        if is_script_end {
            out.push_str("<\\/script");
            index += 8;
            continue;
        }
        let ch = value[index..]
            .chars()
            .next()
            .expect("non-empty string slice has first char");
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn is_case_insensitive_eq(value: &[u8], target: &[u8]) -> bool {
    value.len() == target.len()
        && value
            .iter()
            .zip(target.iter())
            .all(|(left, right)| left.to_ascii_lowercase() == right.to_ascii_lowercase())
}

fn search_script(options: &HtmlRenderOptions) -> String {
    options
        .search_script_href
        .as_ref()
        .map(|href| {
            format!(
                "\n    <script defer src=\"{}\"></script>",
                escape_attr(href)
            )
        })
        .unwrap_or_default()
}

fn site_enhancement_script(options: &HtmlRenderOptions) -> String {
    let src = site_enhancement_script_href(&options.stylesheet_href);
    let script = format!(
        "\n    <script defer src=\"{}\"></script>",
        escape_attr(&src)
    );
    if !options.theme_switching {
        return script;
    }
    format!(
        "\n    <script>(function(){{var d=document.documentElement;d.classList.add('fission-site-js');var k='fission-site-theme';try{{var s=localStorage.getItem(k);if(s){{d.dataset.theme=s;}}}}catch(_){{}}document.addEventListener('click',function(e){{var b=e.target.closest('[data-fission-theme-toggle]');if(!b)return;var n=d.dataset.theme==='dark'?'light':'dark';d.dataset.theme=n;try{{localStorage.setItem(k,n);}}catch(_){{}}}});}}());</script>{script}"
    )
}

fn site_enhancement_script_href(stylesheet_href: &str) -> String {
    stylesheet_href
        .strip_suffix("site.css")
        .map(|prefix| format!("{prefix}site-enhancement.js"))
        .unwrap_or_else(|| "site-enhancement.js".to_string())
}

#[derive(Clone, Debug, Default, PartialEq)]
/// Deterministic CSS custom-property lookup resolved from a Fission theme.
pub struct CssVariableMap {
    color_vars: Vec<(Color, &'static str)>,
    font_vars: Vec<(String, &'static str)>,
}

impl CssVariableMap {
    /// Builds color and font variable mappings from `theme`.
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            color_vars: theme_color_vars(theme)
                .into_iter()
                .map(|(name, color)| (color, name))
                .collect(),
            font_vars: theme_font_vars(theme)
                .into_iter()
                .map(|(name, family)| (family.to_string(), name))
                .collect(),
        }
    }

    fn color_var(&self, color: Color) -> Option<&'static str> {
        self.color_vars
            .iter()
            .find_map(|(candidate, name)| (*candidate == color).then_some(*name))
    }

    fn font_var(&self, family: &str) -> Option<&'static str> {
        self.font_vars
            .iter()
            .find_map(|(candidate, name)| (candidate == family).then_some(*name))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Deduplicates normalized declaration sets into stable generated classes.
pub struct StyleRegistry {
    style_to_class: BTreeMap<String, String>,
    class_to_style: BTreeMap<String, String>,
    raw_rules: BTreeMap<String, String>,
}

impl StyleRegistry {
    /// Returns a stable class for `style`, creating it if needed. Empty
    /// declaration sets return `None`.
    pub fn class_for(&mut self, style: Vec<String>) -> Option<String> {
        let style = normalize_style(style)?;
        if let Some(class_name) = self.style_to_class.get(&style) {
            return Some(class_name.clone());
        }
        let base = format!("fs_{:016x}", stable_hash(style.as_bytes()));
        let mut class_name = base.clone();
        let mut suffix = 2usize;
        while self
            .class_to_style
            .get(&class_name)
            .is_some_and(|existing| existing != &style)
        {
            class_name = format!("{base}_{suffix}");
            suffix += 1;
        }
        self.style_to_class
            .insert(style.clone(), class_name.clone());
        self.class_to_style.insert(class_name.clone(), style);
        Some(class_name)
    }

    /// Serializes generated classes and raw rules in deterministic order.
    pub fn to_css(&self) -> String {
        let mut out = String::new();
        for (class_name, style) in &self.class_to_style {
            out.push('.');
            out.push_str(class_name);
            out.push('{');
            out.push_str(style);
            out.push_str("}\n");
        }
        for rule in self.raw_rules.values() {
            out.push_str(rule);
            if !rule.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    }

    /// Registers a complete trusted CSS rule under a deterministic dedup key.
    pub fn raw_rule(&mut self, key: impl Into<String>, rule: impl Into<String>) {
        self.raw_rules.insert(key.into(), rule.into());
    }
}

/// Serializes `theme` as CSS custom properties beneath `selector`.
pub fn theme_variables_css(selector: &str, theme: &Theme) -> String {
    let mut out = String::new();
    out.push_str(selector);
    out.push_str("{\n");
    for (name, color) in theme_color_vars(theme) {
        out.push_str("  --fs-color-");
        out.push_str(name);
        out.push(':');
        out.push_str(&raw_color_css(color));
        out.push_str(";\n");
    }
    for (name, family) in theme_font_vars(theme) {
        out.push_str("  --fs-font-");
        out.push_str(name);
        out.push(':');
        out.push_str(family);
        out.push_str(";\n");
    }
    out.push_str("}\n");
    out
}

fn theme_color_vars(theme: &Theme) -> Vec<(&'static str, Color)> {
    let colors = &theme.tokens.colors;
    vec![
        ("primary", colors.primary),
        ("on-primary", colors.on_primary),
        ("primary-hover", colors.primary_hover),
        ("primary-subtle", colors.primary_subtle),
        ("secondary", colors.secondary),
        ("on-secondary", colors.on_secondary),
        ("surface", colors.surface),
        ("on-surface", colors.on_surface),
        ("surface-raised", colors.surface_raised),
        ("surface-sunken", colors.surface_sunken),
        ("background", colors.background),
        ("on-background", colors.on_background),
        ("error", colors.error),
        ("on-error", colors.on_error),
        ("success", colors.success),
        ("warning", colors.warning),
        ("info", colors.info),
        ("border", colors.border),
        ("border-strong", colors.border_strong),
        ("divider", colors.divider),
        ("text-primary", colors.text_primary),
        ("text-secondary", colors.text_secondary),
        ("text-muted", colors.text_muted),
        ("text-link", colors.text_link),
        ("heading", colors.heading),
        ("focus-ring", colors.focus_ring),
    ]
}

fn theme_font_vars(theme: &Theme) -> Vec<(&'static str, &str)> {
    let typography = &theme.tokens.typography;
    vec![
        ("sans", &typography.font_family_sans),
        ("serif", &typography.font_family_serif),
        ("mono", &typography.font_family_mono),
    ]
}

fn normalize_style(style: Vec<String>) -> Option<String> {
    let mut by_property = BTreeMap::new();
    let mut unkeyed = Vec::new();
    for entry in style {
        let entry = entry.trim().trim_end_matches(';').to_string();
        if entry.is_empty() {
            continue;
        }
        if let Some((property, _)) = entry.split_once(':') {
            // Preserve renderer precedence by letting later declarations for the
            // same CSS property win before sorting the canonical rule.
            by_property.insert(property.trim().to_string(), entry);
        } else {
            unkeyed.push(entry);
        }
    }
    let mut style = by_property.into_values().collect::<Vec<_>>();
    unkeyed.sort();
    unkeyed.dedup();
    style.extend(unkeyed);
    if style.is_empty() {
        return None;
    }
    Some(style.join(";"))
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn validate_static_ir(ir: &CoreIR, allow_server_actions: bool) -> Result<()> {
    for node in ir.nodes.values() {
        if fission_core::hit_test::is_interaction_inert(ir, node.id) {
            continue;
        }
        match &node.op {
            Op::Semantics(semantics) => {
                if semantics.hyperlink.is_none()
                    && !semantics.actions.entries.is_empty()
                    && !allow_server_actions
                {
                    bail!(
                        "static site renderer cannot lower interactive actions on node {}; use a web target or add explicit static enhancement support",
                        node.id
                    );
                }
            }
            _ => {}
        }
    }
    Ok(())
}

struct HtmlRenderer<'a> {
    ir: &'a CoreIR,
    options: &'a HtmlRenderOptions,
    styles: &'a mut StyleRegistry,
    has_code_blocks: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum InteractionPseudo {
    Hover,
    Focused,
    Pressed,
}

impl InteractionPseudo {
    fn selector(self) -> &'static str {
        match self {
            Self::Hover => ":hover",
            Self::Focused => ":focus-visible",
            Self::Pressed => ":active",
        }
    }

    fn matches(self, predicate: &MotionPredicate) -> bool {
        matches!(
            (self, predicate),
            (Self::Hover, MotionPredicate::Hovered(_))
                | (Self::Focused, MotionPredicate::Focused(_))
                | (Self::Pressed, MotionPredicate::Pressed(_))
        )
    }
}

impl HtmlRenderer<'_> {
    fn register_interaction_motion_styles(&mut self) {
        let declarations = self.options.motion_declarations.clone();
        let mut state_rules: BTreeMap<
            (WidgetId, WidgetId, InteractionPseudo, WidgetId),
            Vec<String>,
        > = BTreeMap::new();
        let mut transitions: BTreeMap<WidgetId, Vec<String>> = BTreeMap::new();

        for declaration in declarations {
            let MotionDeclarationKind::Tracks { tracks } = declaration.kind else {
                continue;
            };
            for track in tracks {
                let Some(interaction_id) = interaction_predicate_id(&track.to) else {
                    continue;
                };
                let paint_target = self
                    .first_styled_box_descendant(interaction_id)
                    .unwrap_or(declaration.id);
                let target = match track.property {
                    MotionPropertyId::Opacity | MotionPropertyId::Scale => declaration.id,
                    _ => paint_target,
                };
                let Some(property) = interaction_css_property(&track.property) else {
                    continue;
                };
                let transition = interaction_transition_css(property, &track.transition);
                let target_transitions = transitions.entry(target).or_default();
                if !target_transitions.contains(&transition) {
                    target_transitions.push(transition);
                }
                for pseudo in [
                    InteractionPseudo::Hover,
                    InteractionPseudo::Focused,
                    InteractionPseudo::Pressed,
                ] {
                    let selected = select_interaction_expr(&track.to, pseudo);
                    let Some(value) = self.interaction_css_value(&track.property, selected) else {
                        continue;
                    };
                    state_rules
                        .entry((declaration.id, interaction_id, pseudo, target))
                        .or_default()
                        .push(format!("{property}:{value}"));
                    if matches!(
                        track.property,
                        MotionPropertyId::BorderColor | MotionPropertyId::BorderWidth
                    ) {
                        let declarations = state_rules
                            .entry((declaration.id, interaction_id, pseudo, target))
                            .or_default();
                        if !declarations
                            .iter()
                            .any(|value| value == "border-style:solid")
                        {
                            declarations.push("border-style:solid".into());
                        }
                    }
                }
            }
        }

        for (target, declarations) in transitions {
            self.styles.raw_rule(
                format!("fission-interaction-transition-{target}"),
                format!(
                    "[data-fission-node=\"{target}\"]{{transition:{}}}",
                    declarations.join(",")
                ),
            );
        }
        for ((motion_id, interaction_id, pseudo, target), declarations) in state_rules {
            let selector = format!(
                "[data-fission-node=\"{motion_id}\"]:has([data-fission-node=\"{interaction_id}\"]{})",
                pseudo.selector()
            );
            let target_selector = if target == motion_id {
                selector
            } else {
                format!("{selector} [data-fission-node=\"{target}\"]")
            };
            let css = format!("{target_selector}{{{}}}", declarations.join(";"));
            self.styles.raw_rule(
                format!("fission-interaction-{motion_id}-{pseudo:?}-{target}"),
                css,
            );
        }
        self.styles.raw_rule(
            "fission-site-reduced-motion-transitions",
            "@media (prefers-reduced-motion:reduce){[data-fission-node]{transition:none!important;}}",
        );
    }

    fn first_styled_box_descendant(&self, root: WidgetId) -> Option<WidgetId> {
        let mut pending = self.ir.nodes.get(&root)?.children.clone();
        while let Some(id) = pending.pop() {
            let node = self.ir.nodes.get(&id)?;
            if matches!(node.op, Op::Layout(LayoutOp::StyledBox { .. })) {
                return Some(id);
            }
            pending.extend(node.children.iter().rev().copied());
        }
        None
    }

    fn interaction_css_value(
        &self,
        property: &MotionPropertyId,
        expression: &MotionExpr,
    ) -> Option<String> {
        match property {
            MotionPropertyId::BackgroundColor | MotionPropertyId::BorderColor => {
                motion_expr_color_value(expression).map(|color| self.color_css(color))
            }
            MotionPropertyId::BackgroundFill => match expression {
                MotionExpr::Value(MotionValue::Fill(fill)) => Some(self.fill_css(fill)),
                _ => None,
            },
            MotionPropertyId::BoxShadows => match expression {
                MotionExpr::Value(MotionValue::Shadows(shadows)) => Some(
                    shadows
                        .iter()
                        .map(|shadow| self.box_shadow_css(shadow))
                        .collect::<Vec<_>>()
                        .join(","),
                ),
                _ => None,
            },
            MotionPropertyId::Opacity | MotionPropertyId::Scale => {
                motion_expr_scalar_value(expression).map(|value| px(value).to_string())
            }
            MotionPropertyId::BorderWidth
            | MotionPropertyId::CornerRadius
            | MotionPropertyId::PaddingLeft
            | MotionPropertyId::PaddingRight
            | MotionPropertyId::PaddingTop
            | MotionPropertyId::PaddingBottom => {
                motion_expr_scalar_value(expression).map(|value| format!("{}px", px(value)))
            }
            _ => None,
        }
    }

    fn render_node(&mut self, node_id: WidgetId) -> Result<String> {
        let node = self
            .ir
            .nodes
            .get(&node_id)
            .ok_or_else(|| anyhow!("site render failed: missing IR node {node_id}"))?;
        match &node.op {
            Op::Structural(fission_ir::StructuralOp::PointerTransparent { .. }) => self
                .render_element(
                    "div",
                    node,
                    "fission-site-node fission-site-pointer-transparent",
                    vec!["pointer-events:none".to_string()],
                ),
            Op::Structural(fission_ir::StructuralOp::InteractionInert { .. }) => self
                .render_element_with_attrs(
                    "div",
                    node,
                    node.id,
                    "fission-site-node fission-site-interaction-inert",
                    vec!["pointer-events:none".to_string()],
                    " aria-hidden=\"true\" inert",
                ),
            Op::Structural(_) => self.render_element("div", node, "fission-site-node", Vec::new()),
            Op::Layout(layout) => self.render_layout(node, layout),
            Op::Paint(paint) => self.render_paint(node, paint),
            Op::Semantics(_) => self.render_semantics(node),
        }
    }

    fn render_children(
        &mut self,
        children: &[WidgetId],
        skip: &HashSet<WidgetId>,
    ) -> Result<String> {
        let mut out = String::new();
        for child in children {
            if skip.contains(child) {
                continue;
            }
            out.push_str(&self.render_node(*child)?);
        }
        Ok(out)
    }

    fn render_element(
        &mut self,
        tag: &str,
        node: &CoreNode,
        class_name: &str,
        style: Vec<String>,
    ) -> Result<String> {
        self.render_element_with_attrs(tag, node, node.id, class_name, style, "")
    }

    fn render_element_with_attrs(
        &mut self,
        tag: &str,
        node: &CoreNode,
        rendered_node_id: WidgetId,
        class_name: &str,
        mut style: Vec<String>,
        attrs: &str,
    ) -> Result<String> {
        let mut skip = HashSet::new();
        style.extend(self.coalesced_paint_style(node, &mut skip)?);
        let (composite_style, animated) = self.composite_style(node);
        style.extend(composite_style);
        let (motion_style, node_motion_animated) = self.node_motion_style(node.id);
        style.extend(motion_style);
        let children = self.render_children(&node.children, &skip)?;
        let class_name = if animated || node_motion_animated {
            format!("{class_name} fission-site-animated")
        } else {
            class_name.to_string()
        };
        let class_name = self.class_name(&class_name, style);
        Ok(format!(
            "<{tag} class=\"{}\"{attrs} data-fission-node=\"{rendered_node_id}\">{children}</{tag}>",
            escape_attr(&class_name),
        ))
    }

    fn stretches_auto_width_content_child(&self, node: &CoreNode) -> bool {
        if self.parent_uses_intrinsic_inline_sizing(node) {
            return false;
        }

        let mut content_children = node
            .children
            .iter()
            .filter_map(|child_id| self.ir.nodes.get(child_id))
            .filter(|child| !is_coalesced_paint_child(child));
        let Some(child) = content_children.next() else {
            return false;
        };

        content_children.next().is_none() && !site_node_has_explicit_width(child)
    }

    fn parent_uses_intrinsic_inline_sizing(&self, node: &CoreNode) -> bool {
        let Some(parent) = node
            .parent
            .and_then(|parent_id| self.ir.nodes.get(&parent_id))
        else {
            return false;
        };
        let Op::Semantics(semantics) = &parent.op else {
            return false;
        };

        matches!(semantics.role, Role::Button | Role::Link | Role::MenuItem)
            || semantics
                .actions
                .entries
                .iter()
                .any(|entry| entry.trigger == ActionTrigger::Default)
            || semantics.identifier.as_deref().is_some_and(|identifier| {
                identifier.starts_with("site-link:")
                    || identifier.starts_with("site-route:")
                    || identifier.starts_with("site-heading:")
                    || identifier.starts_with("site-client-action:")
                    || identifier.starts_with("markdown-link:")
                    || matches!(
                        identifier,
                        "site-theme-toggle" | "site-search-trigger" | "site-sidebar-toggle"
                    )
            })
    }

    fn class_name(&mut self, base: &str, style: Vec<String>) -> String {
        if let Some(generated) = self.styles.class_for(style) {
            format!("{base} {generated}")
        } else {
            base.to_string()
        }
    }

    fn composite_style(&mut self, node: &CoreNode) -> (Vec<String>, bool) {
        let mut style = Vec::new();
        let mut animations = Vec::new();

        if node.composite.clip_to_bounds {
            style.push("overflow:hidden".to_string());
        }

        if let Some(opacity) = node.composite.opacity.as_ref() {
            style.push(format!("opacity:{}", opacity.base));
            if let Some(request) = self.animation_request(opacity, MotionPropertyId::Opacity) {
                animations.push(self.animation_css(
                    CssAnimationProperty::Opacity,
                    opacity.base,
                    &request,
                    None,
                ));
            }
        }

        let translate_x = node
            .composite
            .translate_x
            .as_ref()
            .map(|v| v.base)
            .unwrap_or(0.0);
        let translate_y = node
            .composite
            .translate_y
            .as_ref()
            .map(|v| v.base)
            .unwrap_or(0.0);
        let scale = node.composite.scale.as_ref().map(|v| v.base).unwrap_or(1.0);
        let rotation = node
            .composite
            .rotation
            .as_ref()
            .map(|v| v.base)
            .unwrap_or(0.0);

        let translate_x_request = node
            .composite
            .translate_x
            .as_ref()
            .and_then(|scalar| self.animation_request(scalar, MotionPropertyId::TranslateX));
        let translate_y_request = node
            .composite
            .translate_y
            .as_ref()
            .and_then(|scalar| self.animation_request(scalar, MotionPropertyId::TranslateY));
        let scale_request = node
            .composite
            .scale
            .as_ref()
            .and_then(|scalar| self.animation_request(scalar, MotionPropertyId::Scale));
        let rotation_request = node
            .composite
            .rotation
            .as_ref()
            .and_then(|scalar| self.animation_request(scalar, MotionPropertyId::Rotation));
        let has_transform_animation = translate_x_request.is_some()
            || translate_y_request.is_some()
            || scale_request.is_some()
            || rotation_request.is_some();

        if has_transform_animation {
            style.push(format!(
                "translate:{}px {}px",
                px(translate_x),
                px(translate_y)
            ));
            style.push(format!("scale:{}", px(scale)));
            style.push(format!("rotate:{}deg", px(rotation)));
        } else if translate_x != 0.0
            || translate_y != 0.0
            || (scale - 1.0).abs() > f32::EPSILON
            || rotation != 0.0
        {
            style.push(format!(
                "transform:translate({}px,{}px) scale({}) rotate({}deg)",
                px(translate_x),
                px(translate_y),
                scale,
                rotation
            ));
        }

        if let Some(request) = translate_x_request {
            animations.push(self.animation_css(
                CssAnimationProperty::TranslateX {
                    other_axis: translate_y,
                },
                translate_x,
                &request,
                None,
            ));
        }
        if let Some(request) = translate_y_request {
            animations.push(self.animation_css(
                CssAnimationProperty::TranslateY {
                    other_axis: translate_x,
                },
                translate_y,
                &request,
                None,
            ));
        }
        if let Some(request) = scale_request {
            animations.push(self.animation_css(CssAnimationProperty::Scale, scale, &request, None));
        }
        if let Some(request) = rotation_request {
            animations.push(self.animation_css(
                CssAnimationProperty::Rotation,
                rotation,
                &request,
                None,
            ));
        }
        if let Some(offset) = node.composite.path_offset.as_ref() {
            self.path_offset_style(offset, &mut style, &mut animations);
        }

        if !animations.is_empty() {
            style.push(format!("animation:{}", animations.join(",")));
            self.register_reduced_motion_rule();
        }

        (style, !animations.is_empty())
    }

    fn node_motion_style(&mut self, target: WidgetId) -> (Vec<String>, bool) {
        let mut style = Vec::new();
        let mut animations = Vec::new();

        for (property, css_property) in [
            (MotionPropertyId::Width, CssAnimationProperty::Width),
            (MotionPropertyId::Height, CssAnimationProperty::Height),
            (
                MotionPropertyId::CornerRadius,
                CssAnimationProperty::CornerRadius,
            ),
        ] {
            let Some(track) = self.motion_track_for_target(target, property.clone()) else {
                continue;
            };
            if let Some(final_value) = motion_expr_length_css(&track.to) {
                style.push(format!("{}:{final_value}", css_property.property_name()));
            }
            if let (Some(from), Some(to)) = (
                animation_start_scalar(&track, 0.0),
                motion_expr_scalar_value(&track.to),
            ) {
                animations.push(self.animation_css(css_property, to, &track, Some(from)));
            }
        }

        for (property, css_property) in [
            (
                MotionPropertyId::BackgroundColor,
                CssColorAnimationProperty::BackgroundColor,
            ),
            (
                MotionPropertyId::BorderColor,
                CssColorAnimationProperty::BorderColor,
            ),
            (
                MotionPropertyId::TextColor,
                CssColorAnimationProperty::TextColor,
            ),
        ] {
            let Some(track) = self.motion_track_for_target(target, property.clone()) else {
                continue;
            };
            if let Some(final_color) = motion_expr_color_value(&track.to) {
                style.push(format!(
                    "{}:{}",
                    css_property.property_name(),
                    self.color_css(final_color)
                ));
                let from = animation_start_color(&track).unwrap_or(final_color);
                animations.push(self.color_animation_css(css_property, from, final_color, &track));
            }
        }

        if !animations.is_empty() {
            style.push(format!("animation:{}", animations.join(",")));
            self.register_reduced_motion_rule();
        }

        (style, !animations.is_empty())
    }

    fn animation_request(
        &self,
        scalar: &CompositeScalar,
        property: MotionPropertyId,
    ) -> Option<MotionTrack> {
        let target = scalar.motion_target?;
        self.motion_track_for_target(target, property)
    }

    fn motion_track_for_target(
        &self,
        target: WidgetId,
        property: MotionPropertyId,
    ) -> Option<MotionTrack> {
        self.options
            .motion_declarations
            .iter()
            .rev()
            .find_map(|declaration| {
                if declaration.id != target {
                    return None;
                }
                match &declaration.kind {
                    MotionDeclarationKind::Tracks { tracks } => tracks
                        .iter()
                        .rev()
                        .find(|track| track.property == property)
                        .cloned(),
                    MotionDeclarationKind::Presence {
                        enter,
                        exit,
                        visible,
                        ..
                    } => {
                        let tracks = if *visible { enter } else { exit };
                        tracks
                            .iter()
                            .rev()
                            .find(|track| track.property == property)
                            .cloned()
                    }
                    MotionDeclarationKind::RippleLayer(_) => None,
                }
            })
    }

    fn animation_css(
        &mut self,
        property: CssAnimationProperty,
        base: f32,
        request: &MotionTrack,
        override_from: Option<f32>,
    ) -> String {
        let from = override_from.unwrap_or_else(|| animation_start_value(request, base));
        let to = motion_expr_scalar(&request.to, base);
        let name = self.register_animation_keyframes(property, from, to, request);
        let (duration_ms, delay_ms, easing, repeat) = transition_css_parts(&request.transition);
        format!(
            "{} {}ms {} {}ms {} normal both",
            name,
            duration_ms,
            easing_css(&easing),
            delay_ms,
            if repeat { "infinite" } else { "1" }
        )
    }

    fn register_animation_keyframes(
        &mut self,
        property: CssAnimationProperty,
        from: f32,
        to: f32,
        request: &MotionTrack,
    ) -> String {
        let (duration_ms, delay_ms, easing, repeat) = transition_css_parts(&request.transition);
        let key = format!(
            "{property:?}:{from:?}:{to:?}:{:?}:{}:{}:{}",
            easing, duration_ms, delay_ms, repeat
        );
        let name = format!("fission_anim_{:016x}", stable_hash(key.as_bytes()));
        let rule = format!(
            "@keyframes {name}{{from{{{}}}to{{{}}}}}\n",
            property.css_declaration(from),
            property.css_declaration(to)
        );
        self.styles.raw_rule(name.clone(), rule);
        name
    }

    fn color_animation_css(
        &mut self,
        property: CssColorAnimationProperty,
        from: Color,
        to: Color,
        request: &MotionTrack,
    ) -> String {
        let name = self.register_color_animation_keyframes(property, from, to, request);
        let (duration_ms, delay_ms, easing, repeat) = transition_css_parts(&request.transition);
        format!(
            "{} {}ms {} {}ms {} normal both",
            name,
            duration_ms,
            easing_css(&easing),
            delay_ms,
            if repeat { "infinite" } else { "1" }
        )
    }

    fn register_color_animation_keyframes(
        &mut self,
        property: CssColorAnimationProperty,
        from: Color,
        to: Color,
        request: &MotionTrack,
    ) -> String {
        let (duration_ms, delay_ms, easing, repeat) = transition_css_parts(&request.transition);
        let key = format!(
            "{property:?}:{from:?}:{to:?}:{:?}:{}:{}:{}",
            easing, duration_ms, delay_ms, repeat
        );
        let name = format!("fission_anim_{:016x}", stable_hash(key.as_bytes()));
        let rule = format!(
            "@keyframes {name}{{from{{{}}}to{{{}}}}}\n",
            property.css_declaration(self, from),
            property.css_declaration(self, to)
        );
        self.styles.raw_rule(name.clone(), rule);
        name
    }

    fn render_layout(&mut self, node: &CoreNode, layout: &LayoutOp) -> Result<String> {
        match layout {
            LayoutOp::Box {
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                flex_grow,
                flex_shrink,
                aspect_ratio,
            } => {
                let mut style = vec!["display:block".to_string(), "position:relative".to_string()];
                push_box_constraints(
                    &mut style,
                    *width,
                    *height,
                    *min_width,
                    *max_width,
                    *min_height,
                    *max_height,
                );
                push_padding(&mut style, *padding);
                push_flex_item(&mut style, *flex_grow, *flex_shrink);
                if let Some(aspect_ratio) = aspect_ratio {
                    style.push(format!("aspect-ratio:{aspect_ratio}"));
                }
                self.render_element("div", node, "fission-site-node fission-site-box", style)
            }
            LayoutOp::StyledBox {
                style: box_style,
                flex_grow,
                flex_shrink,
            } => {
                let mut style = vec![
                    "display:block".to_string(),
                    "position:relative".to_string(),
                    "box-sizing:border-box".to_string(),
                ];
                push_length_property(&mut style, "width", box_style.width.as_ref());
                push_length_property(&mut style, "height", box_style.height.as_ref());
                push_length_property(&mut style, "min-width", box_style.min_width.as_ref());
                push_length_property(&mut style, "max-width", box_style.max_width.as_ref());
                push_length_property(&mut style, "min-height", box_style.min_height.as_ref());
                push_length_property(&mut style, "max-height", box_style.max_height.as_ref());
                if let Some(padding) = box_style.padding.as_ref() {
                    style.push(format!(
                        "padding:{} {} {} {}",
                        length_css(&padding[2]),
                        length_css(&padding[1]),
                        length_css(&padding[3]),
                        length_css(&padding[0])
                    ));
                }
                if let Some(margin) = box_style.margin.as_ref() {
                    style.push(format!(
                        "margin:{} {} {} {}",
                        length_css(&margin[2]),
                        length_css(&margin[1]),
                        length_css(&margin[3]),
                        length_css(&margin[0])
                    ));
                }
                if let Some(aspect_ratio) = box_style.aspect_ratio {
                    style.push(format!("aspect-ratio:{}", aspect_ratio.0));
                }
                if box_style.overflow == Overflow::Clip {
                    style.push("overflow:hidden".into());
                }
                match box_style.alignment {
                    fission_ir::op::BoxAlignment::Start => {}
                    alignment => {
                        style.push("display:flex".into());
                        let alignment = match alignment {
                            fission_ir::op::BoxAlignment::Start => "flex-start",
                            fission_ir::op::BoxAlignment::Center => "center",
                            fission_ir::op::BoxAlignment::End => "flex-end",
                            fission_ir::op::BoxAlignment::Stretch => "stretch",
                        };
                        style.push(format!("align-items:{alignment}"));
                        style.push(format!("justify-content:{alignment}"));
                    }
                }
                if let Some(position) = box_style.position.as_ref() {
                    style.push("position:absolute".into());
                    push_length_property(&mut style, "left", position.left.as_ref());
                    push_length_property(&mut style, "top", position.top.as_ref());
                    push_length_property(&mut style, "right", position.right.as_ref());
                    push_length_property(&mut style, "bottom", position.bottom.as_ref());
                }
                if let Some(grid) = box_style.grid {
                    push_grid_placement(&mut style, "grid-row-start", grid.row_start);
                    push_grid_placement(&mut style, "grid-row-end", grid.row_end);
                    push_grid_placement(&mut style, "grid-column-start", grid.col_start);
                    push_grid_placement(&mut style, "grid-column-end", grid.col_end);
                }
                push_flex_item(&mut style, *flex_grow, *flex_shrink);
                let stretches_auto_width_child = box_style.alignment
                    == fission_ir::op::BoxAlignment::Stretch
                    && self.stretches_auto_width_content_child(node);
                let class_name = if stretches_auto_width_child {
                    "fission-site-node fission-site-box fission-site-box-stretch-auto-width"
                } else {
                    "fission-site-node fission-site-box"
                };
                self.render_element("div", node, class_name, style)
            }
            LayoutOp::Flex {
                direction,
                wrap,
                flex_grow,
                flex_shrink,
                padding,
                gap,
                line_gap,
                align_items,
                justify_content,
            } => {
                let (layout_class, style) = flex_layout_style(
                    *direction,
                    *wrap,
                    *flex_grow,
                    *flex_shrink,
                    *padding,
                    *gap,
                    *line_gap,
                    *align_items,
                    *justify_content,
                );
                self.render_element(
                    "div",
                    node,
                    &format!("fission-site-node {layout_class}"),
                    style,
                )
            }
            LayoutOp::Grid {
                columns,
                rows,
                column_gap,
                row_gap,
                padding,
            } => {
                let style = grid_layout_style(columns, rows, *column_gap, *row_gap, *padding);
                self.render_element("div", node, "fission-site-node fission-site-grid", style)
            }
            LayoutOp::GridItem {
                row_start,
                row_end,
                col_start,
                col_end,
            } => {
                let mut style = Vec::new();
                push_grid_placement(&mut style, "grid-row-start", *row_start);
                push_grid_placement(&mut style, "grid-row-end", *row_end);
                push_grid_placement(&mut style, "grid-column-start", *col_start);
                push_grid_placement(&mut style, "grid-column-end", *col_end);
                self.render_element(
                    "div",
                    node,
                    "fission-site-node fission-site-grid-item",
                    style,
                )
            }
            LayoutOp::Responsive { query, cases } => {
                self.render_responsive(node, *query, cases, "div")
            }
            LayoutOp::Scroll {
                direction,
                show_scrollbar: _,
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                flex_grow,
                flex_shrink,
            } => {
                let mut style = vec![
                    "display:flex".to_string(),
                    format!("flex-direction:{}", flex_direction(*direction)),
                    "overflow:auto".to_string(),
                ];
                push_box_constraints(
                    &mut style,
                    *width,
                    *height,
                    *min_width,
                    *max_width,
                    *min_height,
                    *max_height,
                );
                push_padding(&mut style, *padding);
                push_flex_item(&mut style, *flex_grow, *flex_shrink);
                self.render_element("div", node, "fission-site-node fission-site-scroll", style)
            }
            LayoutOp::Embed {
                kind,
                widget_id,
                width,
                height,
            } => self.render_embed(node, kind, *widget_id, *width, *height),
            LayoutOp::AbsoluteFill => self.render_element(
                "div",
                node,
                "fission-site-node fission-site-absolute-fill",
                vec!["position:absolute".to_string(), "inset:0".to_string()],
            ),
            LayoutOp::Positioned {
                left,
                top,
                right,
                bottom,
                width,
                height,
            } => {
                let mut style = vec!["position:absolute".to_string()];
                push_optional_px(&mut style, "left", *left);
                push_optional_px(&mut style, "top", *top);
                push_optional_px(&mut style, "right", *right);
                push_optional_px(&mut style, "bottom", *bottom);
                push_optional_px(&mut style, "width", *width);
                push_optional_px(&mut style, "height", *height);
                self.render_element(
                    "div",
                    node,
                    "fission-site-node fission-site-positioned",
                    style,
                )
            }
            LayoutOp::PositionedLengths {
                left,
                top,
                right,
                bottom,
                width,
                height,
            } => {
                let mut style = vec!["position:absolute".to_string()];
                push_length_property(&mut style, "left", left.as_ref());
                push_length_property(&mut style, "top", top.as_ref());
                push_length_property(&mut style, "right", right.as_ref());
                push_length_property(&mut style, "bottom", bottom.as_ref());
                push_length_property(&mut style, "width", width.as_ref());
                push_length_property(&mut style, "height", height.as_ref());
                self.render_element(
                    "div",
                    node,
                    "fission-site-node fission-site-positioned",
                    style,
                )
            }
            LayoutOp::AnchoredPositioned {
                x,
                y,
                anchor_x,
                anchor_y,
            } => self.render_element(
                "div",
                node,
                "fission-site-node fission-site-anchored-positioned",
                vec![
                    "position:absolute".to_string(),
                    format!("left:{}px", px(*x)),
                    format!("top:{}px", px(*y)),
                    format!(
                        "transform:translate({}%,{}%)",
                        px(-*anchor_x * 100.0),
                        px(-*anchor_y * 100.0)
                    ),
                ],
            ),
            LayoutOp::ZStack => self.render_element(
                "div",
                node,
                "fission-site-node fission-site-zstack",
                vec!["display:grid".to_string(), "position:relative".to_string()],
            ),
            LayoutOp::Align => self.render_element(
                "div",
                node,
                "fission-site-node fission-site-align",
                vec![
                    "display:flex".to_string(),
                    "width:100%".to_string(),
                    "height:100%".to_string(),
                    "align-items:center".to_string(),
                    "justify-content:center".to_string(),
                ],
            ),
            LayoutOp::Aligned {
                horizontal,
                vertical,
            } => self.render_element(
                "div",
                node,
                "fission-site-node fission-site-align",
                vec![
                    "display:flex".to_string(),
                    "width:100%".to_string(),
                    "height:100%".to_string(),
                    format!("align-items:{}", box_alignment_css(*vertical)),
                    format!("justify-content:{}", box_alignment_css(*horizontal)),
                ],
            ),
            LayoutOp::Flyout {
                anchor,
                content,
                options,
            } => {
                let children = self.render_children(&node.children, &HashSet::new())?;
                let class_name = self.class_name(
                    "fission-site-node fission-site-flyout",
                    vec![
                        "position:absolute".to_string(),
                        "z-index:1000".to_string(),
                        "inset:auto".to_string(),
                    ],
                );
                let alignment_target = options
                    .alignment_target
                    .map(|target| target.to_string())
                    .unwrap_or_default();
                Ok(format!(
                    "<div class=\"{}\" data-fission-flyout-anchor=\"{}\" data-fission-flyout-content=\"{}\" data-fission-flyout-alignment=\"{:?}\" data-fission-flyout-placement=\"{:?}\" data-fission-flyout-width=\"{:?}\" data-fission-flyout-gap=\"{}\" data-fission-flyout-alignment-target=\"{}\" data-fission-node=\"{}\">{children}</div>",
                    escape_attr(&class_name),
                    anchor,
                    content,
                    options.alignment,
                    options.placement,
                    options.width,
                    options.gap,
                    escape_attr(&alignment_target),
                    node.id
                ))
            }
            LayoutOp::Spotlight { anchor, padding } => {
                let children = self.render_children(&node.children, &HashSet::new())?;
                let class_name = self.class_name(
                    "fission-site-node fission-site-spotlight",
                    vec![
                        "position:fixed".to_string(),
                        "inset:0".to_string(),
                        "z-index:1000".to_string(),
                        "pointer-events:none".to_string(),
                    ],
                );
                Ok(format!(
                    "<div class=\"{}\" data-fission-spotlight-anchor=\"{}\" data-fission-spotlight-padding=\"{}\" data-fission-node=\"{}\">{children}</div>",
                    escape_attr(&class_name),
                    anchor,
                    padding,
                    node.id
                ))
            }
            LayoutOp::Transform { transform } => self.render_element(
                "div",
                node,
                "fission-site-node fission-site-transform",
                vec![format!("transform:matrix3d({})", matrix3d(transform))],
            ),
            LayoutOp::InteractiveViewport { .. } => {
                bail!("interactive viewports are unavailable in Static site and SSR targets")
            }
            LayoutOp::Clip { path } => {
                let mut style = vec!["overflow:hidden".to_string()];
                if let Some(path) = path {
                    style.push(format!("clip-path:path('{}')", css_string(path)));
                }
                self.render_element("div", node, "fission-site-node fission-site-clip", style)
            }
        }
    }

    fn render_embed(
        &mut self,
        node: &CoreNode,
        kind: &EmbedKind,
        widget_id: WidgetId,
        width: Option<f32>,
        height: Option<f32>,
    ) -> Result<String> {
        let mut style = vec!["display:block".to_string()];
        if let Some(width) = width {
            style.push(format!("width:{}px", px(width)));
        } else {
            style.push("width:100%".to_string());
        }
        if let Some(height) = height {
            style.push(format!("height:{}px", px(height)));
        } else {
            style.push("height:100%".to_string());
        }
        let class_name = self.class_name("fission-site-node fission-site-embed", style);
        match kind {
            EmbedKind::Video => {
                if let Some(video) = self.options.video_registrations.get(&widget_id) {
                    let mut attrs = format!(
                        " class=\"{}\" src=\"{}\" controls playsinline data-fission-video=\"{}\" data-fission-node=\"{}\"",
                        escape_attr(&class_name),
                        escape_attr(&self.resolve_asset_src(&video.source)),
                        widget_id,
                        node.id
                    );
                    if video.autoplay {
                        attrs.push_str(" autoplay muted");
                    }
                    if video.loop_playback {
                        attrs.push_str(" loop");
                    }
                    Ok(format!("<video{attrs}></video>"))
                } else {
                    Ok(self.render_embed_fallback(
                        node,
                        &class_name,
                        "video",
                        "Video embed unavailable during static render",
                    ))
                }
            }
            EmbedKind::Web => {
                if let Some(web) = self.options.web_registrations.get(&widget_id) {
                    let title = self
                        .nearest_ancestor_semantic_label(node)
                        .unwrap_or("Embedded web content");
                    Ok(format!(
                        "<iframe class=\"{}\" src=\"{}\" title=\"{}\" loading=\"lazy\" referrerpolicy=\"strict-origin-when-cross-origin\" data-fission-web-view=\"{}\" data-fission-node=\"{}\"></iframe>",
                        escape_attr(&class_name),
                        escape_attr(&web.url),
                        escape_attr(title),
                        widget_id,
                        node.id
                    ))
                } else {
                    Ok(self.render_embed_fallback(
                        node,
                        &class_name,
                        "web",
                        "Web embed unavailable during static render",
                    ))
                }
            }
            EmbedKind::Custom(_) => Ok(self.render_embed_fallback(
                node,
                &class_name,
                "custom",
                "Custom embedded surface is not available in static HTML",
            )),
        }
    }

    fn nearest_ancestor_semantic_label<'a>(&'a self, node: &'a CoreNode) -> Option<&'a str> {
        let mut parent_id = node.parent;
        while let Some(id) = parent_id {
            let parent = self.ir.nodes.get(&id)?;
            if let Op::Semantics(semantics) = &parent.op {
                if let Some(label) = semantics.label.as_deref() {
                    return Some(label);
                }
            }
            parent_id = parent.parent;
        }
        None
    }

    fn has_ancestor_semantic_identifier(&self, node: &CoreNode, expected: &str) -> bool {
        let mut parent_id = node.parent;
        while let Some(id) = parent_id {
            let Some(parent) = self.ir.nodes.get(&id) else {
                return false;
            };
            if let Op::Semantics(semantics) = &parent.op {
                if semantics.identifier.as_deref() == Some(expected) {
                    return true;
                }
            }
            parent_id = parent.parent;
        }
        false
    }

    fn render_embed_fallback(
        &self,
        node: &CoreNode,
        class_name: &str,
        kind: &str,
        message: &str,
    ) -> String {
        format!(
            "<div class=\"{}\" data-fission-embed-kind=\"{}\" data-fission-node=\"{}\">{}</div>",
            escape_attr(class_name),
            escape_attr(kind),
            node.id,
            escape_text(message)
        )
    }

    fn render_paint(&mut self, node: &CoreNode, paint: &PaintOp) -> Result<String> {
        match paint {
            PaintOp::BackdropFilter {
                filter,
                corner_radius,
                corner_radii,
            } => {
                let functions = filter
                    .flatten()
                    .into_iter()
                    .filter_map(|leaf| match leaf {
                        fission_ir::op::BackdropFilter::Blur(sigma) => {
                            Some(format!("blur({}px)", px(*sigma)))
                        }
                        fission_ir::op::BackdropFilter::Saturate(amount) => {
                            Some(format!("saturate({amount})"))
                        }
                        fission_ir::op::BackdropFilter::Brightness(amount) => {
                            Some(format!("brightness({amount})"))
                        }
                        fission_ir::op::BackdropFilter::Chain(_) => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let mut style = vec![
                    format!("backdrop-filter:{functions}"),
                    format!("-webkit-backdrop-filter:{functions}"),
                ];
                match corner_radii {
                    Some(radii) if !radii.is_square() => {
                        style.push(format!(
                            "border-radius:{}px {}px {}px {}px",
                            px(radii.top_left),
                            px(radii.top_right),
                            px(radii.bottom_right),
                            px(radii.bottom_left)
                        ));
                        style.push("overflow:hidden".into());
                    }
                    Some(_) => {}
                    None if *corner_radius > 0.0 => {
                        style.push(format!("border-radius:{}px", px(*corner_radius)));
                        style.push("overflow:hidden".into());
                    }
                    None => {}
                }
                style.push("min-height:1px".into());
                self.render_element(
                    "div",
                    node,
                    "fission-site-node fission-site-backdrop-filter",
                    style,
                )
            }
            PaintOp::DrawRect {
                fill,
                stroke,
                corner_radius,
                shadow,
                corner_radii,
                border_sides,
            } => {
                let mut style = self.draw_rect_style(
                    fill.as_ref(),
                    stroke.as_ref(),
                    *corner_radius,
                    shadow.as_ref(),
                    *corner_radii,
                    border_sides.as_ref(),
                );
                style.push("min-height:1px".to_string());
                self.render_element("div", node, "fission-site-node fission-site-rect", style)
            }
            PaintOp::DrawText {
                text,
                size,
                color,
                underline,
                locale,
                wrap,
                paragraph_style,
                ..
            } => {
                let mut style = self.text_style(*size, *color, *underline, *wrap);
                if paragraph_needs_text_box(paragraph_style.as_ref()) {
                    style.push("display:block".to_string());
                    style.push("width:100%".to_string());
                }
                push_paragraph_style(&mut style, paragraph_style.as_ref());
                let class_name = self.class_name("fission-site-text", style);
                let language = language_attribute(locale.as_deref());
                let direction = direction_attribute(paragraph_style.as_ref());
                Ok(format!(
                    "<span class=\"{}\"{language}{direction} data-fission-node=\"{}\">{}</span>",
                    escape_attr(&class_name),
                    node.id,
                    escape_text(text)
                ))
            }
            PaintOp::DrawRichText {
                runs,
                wrap,
                paragraph_style,
                ..
            } => {
                let mut style = Vec::new();
                if paragraph_needs_text_box(paragraph_style.as_ref()) {
                    style.push("display:block".to_string());
                    style.push("width:100%".to_string());
                } else {
                    style.push("display:inline".to_string());
                }
                style.push(format!(
                    "white-space:{}",
                    if *wrap { "pre-wrap" } else { "pre" }
                ));
                push_paragraph_style(&mut style, paragraph_style.as_ref());
                let annotations = self
                    .rich_text_annotations(node.id)
                    .map(|annotations| annotations.to_vec())
                    .unwrap_or_default();
                let content = self.render_rich_text_runs(node, runs, &annotations)?;
                let class_name = self.class_name("fission-site-rich-text", style);
                let direction = direction_attribute(paragraph_style.as_ref());
                Ok(format!(
                    "<span class=\"{}\"{direction} data-fission-node=\"{}\">{content}</span>",
                    escape_attr(&class_name),
                    node.id
                ))
            }
            PaintOp::DrawImage {
                request,
                fit,
                alignment,
            } => {
                let class_name = self.class_name(
                    "fission-site-img",
                    vec![
                        "width:100%".to_string(),
                        "height:100%".to_string(),
                        format!("object-fit:{}", image_fit_css(*fit)),
                        format!("object-position:{}", image_alignment_css(*alignment)),
                    ],
                );
                Ok(format!(
                    "<img class=\"{}\" src=\"{}\" alt=\"{}\" data-fission-node=\"{}\">",
                    escape_attr(&class_name),
                    escape_attr(&self.resolve_image_src(&request.source)),
                    escape_attr(request.semantic_label.as_deref().unwrap_or("")),
                    node.id
                ))
            }
            PaintOp::DrawImageBatch { .. } => anyhow::bail!(
                "image batches require an interactive graphical renderer (node {})",
                node.id
            ),
            PaintOp::DrawPath {
                path,
                fill,
                stroke,
                view_box,
            } => Ok(self.render_svg_path(node, path, fill.as_ref(), stroke.as_ref(), *view_box)),
            PaintOp::DrawSvg {
                content,
                fill,
                stroke,
            } => {
                let base = if fill.is_some() || stroke.is_some() {
                    "fission-site-svg fission-site-svg-colored"
                } else {
                    "fission-site-svg"
                };
                let class_name =
                    self.class_name(base, self.svg_paint_style(fill.as_ref(), stroke.as_ref()));
                Ok(format!(
                    "<span class=\"{}\" data-fission-node=\"{}\">{}</span>",
                    escape_attr(&class_name),
                    node.id,
                    content
                ))
            }
        }
    }

    fn render_semantics(&mut self, node: &CoreNode) -> Result<String> {
        let Op::Semantics(semantics) = &node.op else {
            unreachable!();
        };
        if let Some(hyperlink) = semantics.hyperlink.as_ref() {
            let mut attrs = self.hyperlink_attrs(hyperlink);
            if let Some(popover) = semantics.popover_target.as_ref() {
                attrs.push_str(&format!(
                    " popovertarget=\"{}\" popovertargetaction=\"{}\"",
                    escape_attr(&popover.id),
                    popover.action.as_html_action(),
                ));
            }
            if let Some(label) = semantics.label.as_deref() {
                attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
            }
            let children = self.render_children(&node.children, &HashSet::new())?;
            return Ok(format!(
                "<a class=\"fission-site-node fission-site-link\"{attrs} data-fission-node=\"{}\">{children}</a>",
                node.id
            ));
        }
        if let Some(identifier) = semantics.identifier.as_deref() {
            if identifier.starts_with("site-disclosure:") {
                let children = self.render_semantic_payload_children(node)?;
                return Ok(format!(
                    "<details class=\"fission-site-node fission-site-semantics fission-site-disclosure\" data-fission-semantics=\"{}\" data-fission-node=\"{}\">{children}</details>",
                    escape_attr(identifier),
                    node.id,
                ));
            }
            if let Some(name) = identifier.strip_prefix("site-disclosure-summary:") {
                let disclosure = format!("site-disclosure:{name}");
                if self.has_ancestor_semantic_identifier(node, &disclosure) {
                    let children = self.render_semantic_payload_children(node)?;
                    return Ok(format!(
                        "<summary class=\"fission-site-node fission-site-semantics fission-site-disclosure-summary\" data-fission-semantics=\"{}\" data-fission-node=\"{}\">{children}</summary>",
                        escape_attr(identifier),
                        node.id,
                    ));
                }
            }
            if let Some(target) = identifier.strip_prefix("site-route:") {
                return self.render_semantic_link(
                    node,
                    target,
                    semantics.label.as_deref(),
                    "fission-site-route-link",
                );
            }
            if let Some(target) = identifier.strip_prefix("site-link:") {
                return self.render_semantic_link(
                    node,
                    target,
                    semantics.label.as_deref(),
                    "fission-site-general-link",
                );
            }
            if let Some(anchor) = identifier.strip_prefix("site-heading:") {
                return self.render_semantic_link(
                    node,
                    &format!("#{anchor}"),
                    semantics.label.as_deref(),
                    "fission-site-heading-link",
                );
            }
            if let Some(target) = identifier.strip_prefix("markdown-link:") {
                if self.subtree_has_rich_text_annotation(node, identifier) {
                    return self.render_children(&node.children, &HashSet::new());
                }
                return self.render_semantic_link(
                    node,
                    target,
                    semantics.label.as_deref(),
                    "fission-site-markdown-link",
                );
            }
            if identifier == "site-theme-toggle" {
                let children = self.render_children(&node.children, &HashSet::new())?;
                let label = semantics.label.as_deref().unwrap_or("Toggle color theme");
                return Ok(format!(
                    "<button class=\"fission-site-node fission-site-theme-toggle\" type=\"button\" aria-label=\"{}\" data-fission-theme-toggle data-fission-node=\"{}\">{children}</button>",
                    escape_attr(label),
                    node.id,
                ));
            }
            if identifier == "site-locale-switcher" {
                return Ok(format!(
                    "<label class=\"fission-site-locale-switcher\" aria-label=\"Language\"><select data-fission-locale-switcher data-fission-node=\"{}\"><option value=\"en\">English</option><option value=\"es\">Español</option></select></label>",
                    node.id
                ));
            }
            if identifier == "site-search-trigger" {
                let children = self.render_children(&node.children, &HashSet::new())?;
                return Ok(format!(
                    "<button class=\"fission-site-node fission-site-search-trigger\" type=\"button\" aria-label=\"Search this site\" data-fission-search-trigger data-fission-node=\"{}\">{children}</button>",
                    node.id
                ));
            }
            if identifier == "site-sidebar-toggle" {
                let children = self.render_children(&node.children, &HashSet::new())?;
                let label = semantics
                    .label
                    .as_deref()
                    .unwrap_or("Open documentation navigation");
                return Ok(format!(
                    "<button class=\"fission-site-node fission-site-sidebar-toggle\" type=\"button\" aria-label=\"{}\" aria-expanded=\"false\" data-fission-sidebar-toggle data-fission-node=\"{}\">{children}</button>",
                    escape_attr(label),
                    node.id
                ));
            }
            if let Some(action) = identifier.strip_prefix("site-client-action:") {
                let children = self.render_children(&node.children, &HashSet::new())?;
                let mut attrs = format!(
                    " data-fission-client-action=\"{}\" data-fission-semantics=\"{}\"",
                    escape_attr(action),
                    escape_attr(identifier)
                );
                if let Some(value) = semantics.label.as_deref() {
                    attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(value)));
                }
                if semantics.disabled {
                    attrs.push_str(" disabled");
                }
                return Ok(format!(
                    "<button class=\"fission-site-node fission-site-semantics fission-site-client-action\" type=\"button\"{attrs} data-fission-node=\"{}\">{children}</button>",
                    node.id
                ));
            }
            if identifier == "markdown-table" {
                return self.render_markdown_table(node);
            }
            if let Some(row_kind) = identifier.strip_prefix("markdown-table-row:") {
                return self.render_markdown_table_row(node, row_kind);
            }
            if let Some(cell_kind) = identifier.strip_prefix("markdown-table-cell:") {
                return self.render_markdown_table_cell(node, cell_kind);
            }
            if let Some(language) = identifier.strip_prefix("markdown-code-block:") {
                return self.render_markdown_code_block(node, language, semantics.value.as_deref());
            }
            if identifier == "site-form" || identifier.starts_with("site-form:") {
                return self.render_static_form(node, identifier, semantics);
            }
            if identifier.starts_with("site-") {
                return self.render_site_semantic_wrapper(
                    node,
                    identifier,
                    semantics.label.as_deref(),
                );
            }
        }
        if is_native_control_semantics(semantics) {
            return self.render_native_control_semantics(node, semantics);
        }
        if let Some(html) = self.render_server_action_semantics(node, semantics)? {
            return Ok(html);
        }
        if let Some(html) = self.render_browser_action_semantics(node, semantics)? {
            return Ok(html);
        }
        let tag = match semantics.role {
            Role::Button => "button",
            Role::Link => "a",
            Role::MenuItem => "button",
            Role::Option | Role::Tab => "button",
            Role::Image => "figure",
            Role::Video => "div",
            Role::List => "ul",
            Role::ListItem => "li",
            Role::Dialog | Role::TabPanel => "section",
            Role::Text
            | Role::Generic
            | Role::Menu
            | Role::ListBox
            | Role::ComboBox
            | Role::TabList
            | Role::Alert
            | Role::Group
            | Role::Separator
            | Role::Tree
            | Role::TreeItem
            | Role::Toolbar
            | Role::RadioGroup
            | Role::Table
            | Role::TableRow
            | Role::TableCell
            | Role::ColumnHeader
            | Role::ProgressBar
            | Role::Tooltip
            | Role::Status
            | Role::SpinButton => "div",
            Role::TextInput
            | Role::Checkbox
            | Role::Radio
            | Role::Switch
            | Role::Slider
            | Role::Input => {
                unreachable!("interactive controls are rendered before generic semantics")
            }
        };
        let tag = semantics
            .identifier
            .as_deref()
            .and_then(markdown_heading_tag)
            .unwrap_or(tag);
        let mut attrs = semantic_html_attrs(self.ir, node.id, semantics);
        if let Some(role) = semantic_html_role(semantics.role) {
            attrs.push_str(&format!(" role=\"{role}\""));
        }
        if tag == "button" {
            attrs.push_str(" type=\"button\" disabled");
        }
        if tag == "ul" {
            if let Some(html) = self.render_transparent_list_layout(node, &attrs)? {
                return Ok(html);
            }
        }
        if semantics.role == Role::ComboBox
            && semantics.text_field_name.is_some()
            && !semantics.form_select_options.is_empty()
        {
            let mut native_semantics = semantics.clone();
            native_semantics.expanded = None;
            native_semantics.has_popup = None;
            native_semantics.controls.clear();
            native_semantics.active_descendant = None;
            let mut attrs = self.native_control_attrs(node, &native_semantics);
            if native_semantics.required {
                attrs.push_str(" required");
            }
            let mut options = String::new();
            let selected_value = semantics.form_value.as_deref();
            let selected_value_is_present = selected_value.is_some_and(|selected| {
                semantics
                    .form_select_options
                    .iter()
                    .any(|option| option.value == selected)
            });
            if !selected_value_is_present {
                options.push_str(&format!(
                    "<option value=\"\" selected disabled>{}</option>",
                    escape_text(semantics.value.as_deref().unwrap_or_default()),
                ));
            }
            for option in &semantics.form_select_options {
                let selected = (selected_value == Some(option.value.as_str()))
                    .then_some(" selected")
                    .unwrap_or_default();
                let disabled = option.disabled.then_some(" disabled").unwrap_or_default();
                options.push_str(&format!(
                    "<option value=\"{}\"{selected}{disabled}>{}</option>",
                    escape_attr(&option.value),
                    escape_text(&option.label),
                ));
            }
            return Ok(format!(
                "<select class=\"fission-site-node fission-site-input\"{attrs}>{options}</select>"
            ));
        }
        let mut children = self.render_children(&node.children, &HashSet::new())?;
        if semantics.role == Role::ComboBox && !semantics.disabled {
            if let (Some(name), Some(value)) = (
                semantics.text_field_name.as_deref(),
                semantics.form_value.as_deref(),
            ) {
                let form = semantics
                    .text_form_id
                    .as_deref()
                    .map(|form_id| format!(" form=\"{}\"", escape_attr(form_id)))
                    .unwrap_or_default();
                children.push_str(&format!(
                    "<input type=\"hidden\" name=\"{}\" value=\"{}\"{form}>",
                    escape_attr(name),
                    escape_attr(value),
                ));
            }
        }
        Ok(format!(
            "<{tag} class=\"fission-site-node fission-site-semantics\"{attrs} data-fission-node=\"{}\">{children}</{tag}>",
            node.id
        ))
    }

    fn render_transparent_list_layout(
        &mut self,
        node: &CoreNode,
        attrs: &str,
    ) -> Result<Option<String>> {
        let [layout_id] = node.children.as_slice() else {
            return Ok(None);
        };
        let Some(layout_node) = self.ir.nodes.get(layout_id) else {
            return Ok(None);
        };
        let Op::Layout(layout) = &layout_node.op else {
            return Ok(None);
        };
        let Some((layout_class, style)) = transparent_list_layout_style(layout) else {
            return Ok(None);
        };

        // Flex and grid nodes only arrange the list payload here. Apply their
        // presentation to the semantic element so <li> nodes remain direct
        // children of <ul>; semantic descendants still render normally.
        let class_name = format!("fission-site-node fission-site-semantics {layout_class}");
        self.render_element_with_attrs("ul", layout_node, node.id, &class_name, style, attrs)
            .map(Some)
    }

    fn render_native_control_semantics(
        &mut self,
        node: &CoreNode,
        semantics: &Semantics,
    ) -> Result<String> {
        let mut attrs = self.native_control_attrs(node, semantics);
        if matches!(semantics.role, Role::Checkbox | Role::Radio | Role::Switch)
            && !semantics.disabled
        {
            self.push_browser_default_action_attrs(node, semantics, &mut attrs);
        }
        if self.options.browser_action_bindings
            && semantics.supports_text_editing()
            && !semantics.disabled
            && !semantics.read_only
            && semantics
                .actions
                .entries
                .iter()
                .any(|entry| entry.trigger == ActionTrigger::TextChanged)
        {
            attrs.push_str(&format!(
                " data-fission-browser-text-action=\"true\" data-fission-action-target=\"{}\"",
                node.id.as_u128()
            ));
        }
        if self.options.browser_action_bindings
            && semantics.role == Role::Slider
            && !semantics.disabled
            && semantics
                .actions
                .entries
                .iter()
                .any(|entry| entry.trigger == ActionTrigger::Change)
        {
            attrs.push_str(&format!(
                " data-fission-browser-range-action=\"true\" data-fission-action-target=\"{}\"",
                node.id.as_u128()
            ));
        }
        let label_text = semantics.label.as_deref().unwrap_or_default();
        let native_control_layout = if semantics.supports_text_editing() {
            self.native_text_control_layout_style(node)
        } else {
            String::new()
        };
        let external_validation = self.external_validation_message_id(semantics);
        let validation = if external_validation.is_some() {
            String::new()
        } else {
            semantics
                .validation_message
                .as_deref()
                .map(|message| {
                    format!(
                        "<span class=\"fission-site-control-validation\" id=\"{}\">{}</span>",
                        text_validation_message_id(node.id),
                        escape_text(message)
                    )
                })
                .unwrap_or_default()
        };
        if semantics.supports_text_editing() {
            if semantics.multiline {
                let value = semantics.value.as_deref().unwrap_or_default();
                return Ok(format!(
                    "<label class=\"fission-site-node fission-site-control\"{native_control_layout} data-fission-node=\"{}\"><span class=\"fission-site-control-label\">{}</span><textarea class=\"fission-site-input\"{attrs}>{}</textarea>{validation}</label>",
                    node.id,
                    escape_text(label_text),
                    escape_text(value)
                ));
            } else {
                attrs.push_str(&format!(
                    " type=\"{}\" value=\"{}\"",
                    html_text_input_type(semantics),
                    escape_attr(semantics.value.as_deref().unwrap_or_default())
                ));
                return Ok(format!(
                    "<label class=\"fission-site-node fission-site-control\"{native_control_layout} data-fission-node=\"{}\"><span class=\"fission-site-control-label\">{}</span><input class=\"fission-site-input\"{attrs}>{validation}</label>",
                    node.id,
                    escape_text(label_text)
                ));
            }
        }
        // Native successful controls must own their visible state. A named
        // checkbox or radio can change locally before its form is submitted;
        // retaining the separately painted indicator would leave that visual
        // frozen at the server-rendered value. As with range controls, render
        // only the native control and its semantic label in this case.
        let native_form_checkable = matches!(semantics.role, Role::Checkbox | Role::Radio)
            && semantics.text_field_name.is_some();
        let children = if semantics.role == Role::Slider || native_form_checkable {
            String::new()
        } else {
            self.render_children(&node.children, &HashSet::new())?
        };
        let retained_checkable =
            matches!(semantics.role, Role::Checkbox | Role::Radio | Role::Switch)
                && !node.children.is_empty()
                && !native_form_checkable;
        let control_class = if retained_checkable {
            "fission-site-node fission-site-control fission-site-retained-control"
        } else {
            "fission-site-node fission-site-control"
        };
        let visible_label = (!retained_checkable)
            .then(|| {
                format!(
                    "<span class=\"fission-site-control-label\">{}</span>",
                    escape_text(label_text)
                )
            })
            .unwrap_or_default();
        let retained_input_class = if retained_checkable {
            " fission-site-retained-control-input"
        } else {
            ""
        };
        match semantics.role {
            Role::Checkbox | Role::Switch => {
                if semantics.role == Role::Switch {
                    attrs.push_str(" role=\"switch\"");
                }
                if let Some(value) = semantics.form_value.as_deref() {
                    attrs.push_str(&format!(" value=\"{}\"", escape_attr(value)));
                }
                if semantics.checked.unwrap_or(false) {
                    attrs.push_str(" checked");
                }
                Ok(format!(
                    "<label class=\"{control_class}\" data-fission-node=\"{}\"><input class=\"fission-site-checkbox{retained_input_class}\" type=\"checkbox\"{attrs}>{visible_label}{children}</label>",
                    node.id,
                ))
            }
            Role::Radio => {
                if semantics.checked.or(semantics.selected).unwrap_or(false) {
                    attrs.push_str(" checked");
                }

                Ok(format!(
                    "<label class=\"{control_class}\" data-fission-node=\"{}\"><input class=\"fission-site-radio{retained_input_class}\" type=\"radio\"{attrs}>{visible_label}{children}</label>",
                    node.id,
                ))
            }
            Role::Slider => {
                if let Some(value) = semantics.min_value {
                    attrs.push_str(&format!(" min=\"{}\"", px(value)));
                }
                if let Some(value) = semantics.max_value {
                    attrs.push_str(&format!(" max=\"{}\"", px(value)));
                }
                if let Some(value) = semantics.current_value {
                    attrs.push_str(&format!(" value=\"{}\"", px(value)));
                }
                if let Some(step) = semantics.step_value {
                    attrs.push_str(&format!(" step=\"{}\"", px(step)));
                } else {
                    attrs.push_str(" step=\"any\"");
                }
                Ok(format!(
                    "<label class=\"fission-site-node fission-site-control\" data-fission-node=\"{}\"><span class=\"fission-site-control-label\">{}</span><input class=\"fission-site-range\" type=\"range\"{attrs}>{children}</label>",
                    node.id,
                    escape_text(label_text)
                ))
            }
            _ => unreachable!("native control role checked by caller"),
        }
    }

    fn native_control_attrs(&self, node: &CoreNode, semantics: &Semantics) -> String {
        let mut attrs = semantic_html_attrs(self.ir, node.id, semantics);
        attrs.push_str(&format!(" data-fission-node=\"{}\"", node.id));
        if let Some(role) = semantic_html_role(semantics.role) {
            attrs.push_str(&format!(" role=\"{role}\""));
        }
        if let Some(name) = semantics
            .text_field_name
            .as_deref()
            .or(semantics.identifier.as_deref())
        {
            attrs.push_str(&format!(" name=\"{}\"", escape_attr(name)));
        }
        if semantics.disabled {
            attrs.push_str(" disabled");
        }
        if semantics.read_only {
            attrs.push_str(" readonly");
        }
        if semantics.autofocus {
            attrs.push_str(" autofocus");
        }
        if let Some(form_id) = semantics.text_form_id.as_deref() {
            attrs.push_str(&format!(" form=\"{}\"", escape_attr(form_id)));
        }
        if let Some(max_length) = semantics.max_length {
            attrs.push_str(&format!(" maxlength=\"{max_length}\""));
        }
        if semantics.supports_text_editing() {
            if semantics.required {
                attrs.push_str(" required");
            }
            if let Some(min_length) = semantics.min_length {
                attrs.push_str(&format!(" minlength=\"{min_length}\""));
            }
            if let Some(pattern) = semantics.validation_pattern.as_deref() {
                attrs.push_str(&format!(" pattern=\"{}\"", escape_attr(pattern)));
            }
            match semantics.validation_state {
                fission_ir::semantics::TextFieldValidationState::Unvalidated => {}
                fission_ir::semantics::TextFieldValidationState::Valid => {
                    attrs.push_str(" aria-invalid=\"false\"");
                }
                fission_ir::semantics::TextFieldValidationState::Invalid => {}
            }
            if semantics.validation_message.is_some() {
                let validation_id = self
                    .external_validation_message_id(semantics)
                    .unwrap_or_else(|| text_validation_message_id(node.id));
                attrs.push_str(&format!(" aria-errormessage=\"{}\"", validation_id));
            }
            attrs.push_str(&format!(
                " inputmode=\"{}\" enterkeyhint=\"{}\" autocapitalize=\"{}\" autocorrect=\"{}\" spellcheck=\"{}\"",
                html_text_input_mode(semantics),
                html_enter_key_hint(semantics.text_input_action),
                html_autocapitalize(semantics.text_capitalization),
                if semantics.autocorrect { "on" } else { "off" },
                if semantics.spell_check { "true" } else { "false" },
            ));
            if semantics.autofill_group.is_some() || !semantics.autofill_hints.is_empty() {
                attrs.push_str(&format!(
                    " autocomplete=\"{}\"",
                    escape_attr(&html_autocomplete(semantics))
                ));
            }
            if let Some(group) = semantics.autofill_group.as_deref() {
                attrs.push_str(&format!(
                    " data-fission-autofill-group=\"{}\"",
                    escape_attr(group)
                ));
            }
            if semantics.multiline {
                let wrap = match semantics.text_wrap_mode {
                    fission_ir::semantics::TextWrapMode::Soft => "soft",
                    fission_ir::semantics::TextWrapMode::Hard => "hard",
                    fission_ir::semantics::TextWrapMode::NoWrap => "off",
                };
                attrs.push_str(&format!(" wrap=\"{wrap}\""));
            }
        }
        attrs
    }

    fn native_text_control_layout_style(&self, node: &CoreNode) -> String {
        let mut pending = node.children.clone();
        let mut visited = HashSet::new();
        while let Some(node_id) = pending.pop() {
            if !visited.insert(node_id) {
                continue;
            }
            let Some(descendant) = self.ir.nodes.get(&node_id) else {
                continue;
            };
            if let Op::Layout(LayoutOp::Box {
                width,
                min_width,
                max_width,
                ..
            }) = &descendant.op
            {
                let mut style = Vec::new();
                push_optional_px(&mut style, "width", *width);
                push_optional_px(&mut style, "min-width", *min_width);
                push_optional_px(&mut style, "max-width", *max_width);
                if !style.is_empty() {
                    return format!(" style=\"{}\"", escape_attr(&style.join(";")));
                }
            }
            pending.extend(descendant.children.iter().copied().rev());
        }
        String::new()
    }

    fn external_validation_message_id(&self, semantics: &Semantics) -> Option<String> {
        let message = semantics.validation_message.as_deref()?;
        semantics.described_by.iter().rev().find_map(|id| {
            let node = self.ir.nodes.get(id)?;
            let Op::Semantics(description) = &node.op else {
                return None;
            };
            (description.label.as_deref() == Some(message)).then(|| semantic_dom_id(self.ir, *id))
        })
    }

    fn render_server_action_semantics(
        &mut self,
        node: &CoreNode,
        semantics: &fission_ir::Semantics,
    ) -> Result<Option<String>> {
        let Some(action_path) = self.options.server_action_post_path.as_ref() else {
            return Ok(None);
        };
        let Some(action) = semantics
            .actions
            .entries
            .iter()
            .find(|entry| entry.trigger == ActionTrigger::Default)
        else {
            return Ok(None);
        };
        let Some(token) = self
            .options
            .server_action_tokens
            .get(&(node.id, action.action_id))
        else {
            return Ok(None);
        };
        let children = self.render_children(&node.children, &HashSet::new())?;
        let mut attrs = semantic_html_attrs(self.ir, node.id, semantics);
        attrs.push_str(&format!(" data-fission-node=\"{}\"", node.id));
        if let Some(role) = semantic_html_role(semantics.role) {
            attrs.push_str(&format!(" role=\"{role}\""));
        }
        let form_id = semantics
            .text_form_id
            .as_deref()
            .map(|form_id| format!(" id=\"{}\"", escape_attr(form_id)))
            .unwrap_or_default();
        Ok(Some(format!(
            "<form class=\"fission-site-node fission-server-action-form\" method=\"post\" action=\"{}\"{form_id} data-fission-node=\"{}\"><input type=\"hidden\" name=\"token\" value=\"{}\"><button class=\"fission-site-node fission-site-semantics fission-server-action\" type=\"submit\"{attrs}>{children}</button></form>",
            escape_attr(action_path),
            node.id,
            escape_attr(token),
        )))
    }

    fn render_browser_action_semantics(
        &mut self,
        node: &CoreNode,
        semantics: &fission_ir::Semantics,
    ) -> Result<Option<String>> {
        if !self.options.browser_action_bindings {
            return Ok(None);
        }
        let mut attrs = semantic_html_attrs(self.ir, node.id, semantics);
        attrs.push_str(&format!(" data-fission-node=\"{}\"", node.id));
        if let Some(role) = semantic_html_role(semantics.role) {
            attrs.push_str(&format!(" role=\"{role}\""));
        }
        if !self.push_browser_default_action_attrs(node, semantics, &mut attrs) {
            return Ok(None);
        }
        let children = self.render_children(&node.children, &HashSet::new())?;
        Ok(Some(format!(
            "<button class=\"fission-site-node fission-site-semantics fission-browser-action\" type=\"button\"{attrs}>{children}</button>"
        )))
    }

    fn push_browser_default_action_attrs(
        &self,
        node: &CoreNode,
        semantics: &Semantics,
        attrs: &mut String,
    ) -> bool {
        if !self.options.browser_action_bindings {
            return false;
        }
        let Some(action) = semantics
            .actions
            .entries
            .iter()
            .find(|entry| entry.trigger == ActionTrigger::Default)
        else {
            return false;
        };
        let Some(payload) = action.payload_data.as_ref() else {
            return false;
        };
        attrs.push_str(&format!(
            " data-fission-browser-action=\"true\" data-fission-action-id=\"{}\" data-fission-action-target=\"{}\" data-fission-action-payload=\"{}\"",
            action.action_id,
            node.id.as_u128(),
            hex_encode(payload)
        ));
        true
    }

    fn render_markdown_table(&mut self, node: &CoreNode) -> Result<String> {
        let mut header_rows = String::new();
        let mut body_rows = String::new();
        for child in self.semantic_payload_children(node) {
            let rendered = self.render_node(child)?;
            if self.semantic_identifier(child).is_some_and(|identifier| {
                identifier
                    .strip_prefix("markdown-table-row:")
                    .is_some_and(|kind| kind == "header")
            }) {
                header_rows.push_str(&rendered);
            } else {
                body_rows.push_str(&rendered);
            }
        }
        let header = (!header_rows.is_empty())
            .then(|| format!("<thead>{header_rows}</thead>"))
            .unwrap_or_default();
        Ok(format!(
            "<div class=\"fission-site-markdown-table-wrap\" data-fission-node=\"{}\"><table class=\"fission-site-markdown-table\">{header}<tbody>{body_rows}</tbody></table></div>",
            node.id
        ))
    }

    fn render_markdown_table_row(&mut self, node: &CoreNode, row_kind: &str) -> Result<String> {
        let children = self.render_semantic_payload_children(node)?;
        let class_name = if row_kind == "header" {
            "fission-site-markdown-table-row fission-site-markdown-table-head-row"
        } else {
            "fission-site-markdown-table-row"
        };
        Ok(format!(
            "<tr class=\"{class_name}\" data-fission-node=\"{}\">{children}</tr>",
            node.id
        ))
    }

    fn render_markdown_table_cell(&mut self, node: &CoreNode, cell_kind: &str) -> Result<String> {
        let mut parts = cell_kind.split(':');
        let kind = parts.next().unwrap_or("body");
        let align = parts.next().unwrap_or("none");
        let tag = if kind == "header" { "th" } else { "td" };
        let align_class = match align {
            "left" => " fission-site-markdown-align-left",
            "center" => " fission-site-markdown-align-center",
            "right" => " fission-site-markdown-align-right",
            _ => "",
        };
        let children = self.render_semantic_payload_children(node)?;
        Ok(format!(
            "<{tag} class=\"fission-site-markdown-table-cell{align_class}\" data-fission-node=\"{}\">{children}</{tag}>",
            node.id
        ))
    }

    fn render_markdown_code_block(
        &mut self,
        node: &CoreNode,
        language: &str,
        code: Option<&str>,
    ) -> Result<String> {
        self.has_code_blocks = true;
        let Some(code) = code else {
            return self.render_site_semantic_wrapper(node, "markdown-code-block", None);
        };
        let language = code_language_class(language);
        let class_attr = language
            .as_ref()
            .map(|language| format!(" class=\"language-{}\"", escape_attr(language)))
            .unwrap_or_default();
        let data_language = language
            .as_ref()
            .map(|language| format!(" data-fission-code-language=\"{}\"", escape_attr(language)))
            .unwrap_or_default();
        Ok(format!(
            "<pre class=\"fission-site-code-block\"{data_language} data-fission-node=\"{}\"><code{class_attr}>{}</code></pre>",
            node.id,
            escape_text(code)
        ))
    }

    fn render_static_form(
        &mut self,
        node: &CoreNode,
        identifier: &str,
        semantics: &fission_ir::Semantics,
    ) -> Result<String> {
        let Some(value) = semantics.value.as_deref() else {
            return self.render_site_semantic_wrapper(node, identifier, semantics.label.as_deref());
        };
        let spec: StaticFormSpec = serde_json::from_str(value)
            .map_err(|error| anyhow!("invalid static form spec on node {}: {error}", node.id))?;
        let method = spec.method.to_ascii_lowercase();
        let action = self.resolve_link_href(&spec.action);
        let mut attrs = format!(
            " method=\"{}\" action=\"{}\" data-fission-semantics=\"{}\"",
            escape_attr(&method),
            escape_attr(&action),
            escape_attr(identifier)
        );
        if let Some(label) = semantics.label.as_deref() {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        let mut fields = String::new();
        for field in &spec.fields {
            fields.push_str(&self.render_static_form_field(field));
        }
        let submit = spec
            .submit_label
            .as_deref()
            .unwrap_or_else(|| semantics.label.as_deref().unwrap_or("Submit"));
        Ok(format!(
            "<form class=\"fission-site-node fission-site-form {}\"{attrs} data-fission-node=\"{}\">{fields}<button class=\"fission-site-form-submit\" type=\"submit\">{}</button></form>",
            site_semantic_class(identifier),
            node.id,
            escape_text(submit),
        ))
    }

    fn render_static_form_field(&self, field: &StaticFormField) -> String {
        match field.kind {
            StaticFormFieldKind::Hidden => format!(
                "<input type=\"hidden\" name=\"{}\" value=\"{}\">",
                escape_attr(&field.name),
                escape_attr(field.value.as_deref().unwrap_or(""))
            ),
            StaticFormFieldKind::Textarea => {
                let mut attrs = static_form_input_attrs(field);
                let rows = field.rows.unwrap_or(5).max(2);
                attrs.push_str(&format!(" rows=\"{}\"", rows));
                let control = format!(
                    "<textarea class=\"fission-site-form-input fission-site-form-textarea\"{attrs}>{}</textarea>",
                    escape_text(field.value.as_deref().unwrap_or(""))
                );
                static_form_label(field, control)
            }
            StaticFormFieldKind::Checkbox => {
                let mut attrs = static_form_input_attrs(field);
                attrs.push_str(" type=\"checkbox\"");
                if field.value.as_deref() == Some("true") {
                    attrs.push_str(" checked");
                }
                let control =
                    format!("<input class=\"fission-site-form-checkbox\"{attrs} value=\"true\">");
                static_form_label(field, control)
            }
            StaticFormFieldKind::Text
            | StaticFormFieldKind::Email
            | StaticFormFieldKind::Tel
            | StaticFormFieldKind::Url => {
                let input_type = match field.kind {
                    StaticFormFieldKind::Email => "email",
                    StaticFormFieldKind::Tel => "tel",
                    StaticFormFieldKind::Url => "url",
                    _ => "text",
                };
                let mut attrs = static_form_input_attrs(field);
                attrs.push_str(&format!(" type=\"{}\"", input_type));
                let control = format!("<input class=\"fission-site-form-input\"{attrs}>");
                static_form_label(field, control)
            }
        }
    }

    fn render_site_semantic_wrapper(
        &mut self,
        node: &CoreNode,
        identifier: &str,
        label: Option<&str>,
    ) -> Result<String> {
        let class_name = site_semantic_class(identifier);
        let mut attrs = format!(" data-fission-semantics=\"{}\"", escape_attr(identifier));
        if let Some(label) = label {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        let (tag, anchor) = site_semantic_element(identifier);
        if let Some(anchor) = anchor {
            attrs.push_str(&format!(" id=\"{}\"", escape_attr(anchor)));
        }
        attrs.push_str(&site_semantic_data_attrs(identifier));
        // Heading content must remain phrasing content in HTML. Widget lowering commonly places a
        // transparent layout node between semantics and text; emitting that node would produce an
        // invalid `<h1><div>...</div></h1>` structure and hide the heading from some crawlers.
        let children = if is_site_heading_identifier(identifier) {
            self.render_semantic_payload_children(node)?
        } else {
            self.render_children(&node.children, &HashSet::new())?
        };
        Ok(format!(
            "<{tag} class=\"fission-site-node fission-site-semantics {class_name}\"{attrs} data-fission-node=\"{}\">{children}</{tag}>",
            node.id,
        ))
    }

    fn render_semantic_payload_children(&mut self, node: &CoreNode) -> Result<String> {
        if let [child_id] = node.children.as_slice() {
            if let Some(child) = self.ir.nodes.get(child_id) {
                if let Op::Layout(LayoutOp::Responsive { query, cases }) = &child.op {
                    return self.render_responsive(child, *query, cases, "span");
                }
            }
        }
        let children = self.semantic_payload_children(node);
        self.render_children(&children, &HashSet::new())
    }

    fn semantic_payload_children(&self, node: &CoreNode) -> Vec<WidgetId> {
        if node.children.len() == 1 {
            if let Some(child) = self.ir.nodes.get(&node.children[0]) {
                match child.op {
                    Op::Layout(LayoutOp::Responsive { .. }) => {}
                    Op::Layout(_) | Op::Structural(_) => return child.children.clone(),
                    _ => {}
                }
            }
        }
        node.children.clone()
    }

    fn render_responsive(
        &mut self,
        node: &CoreNode,
        query: fission_ir::op::ResponsiveQuery,
        cases: &[fission_ir::op::ResponsiveCondition],
        tag: &str,
    ) -> Result<String> {
        let root_class = format!("fission-responsive-{:x}", node.id.as_u128());
        let child_class = format!("{root_class}-branch");
        let fallback_index = cases.len();
        let children = node
            .children
            .iter()
            .enumerate()
            .map(|(index, child)| {
                self.render_node(*child).map(|html| {
                    format!(
                        "<{tag} class=\"{} {}-{}\">{html}</{tag}>",
                        escape_attr(&child_class),
                        escape_attr(&root_class),
                        index
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?
            .join("");
        let mut css = format!(
            ".{child_class}{{display:none}}.{root_class}-{fallback_index}{{display:block}}"
        );
        for (index, condition) in cases.iter().enumerate().rev() {
            let mut terms = Vec::new();
            if let Some(minimum) = condition.min_width {
                terms.push(format!("(min-width:{}px)", px(minimum)));
            }
            if let Some(maximum) = condition.max_width {
                terms.push(format!("(max-width:{}px)", px(maximum - 0.01)));
            }
            let expression = if terms.is_empty() {
                "(min-width:0px)".to_string()
            } else {
                terms.join(" and ")
            };
            let selector = (0..=fallback_index)
                .map(|branch| format!(".{root_class}-{branch}"))
                .collect::<Vec<_>>()
                .join(",");
            let body = format!("{selector}{{display:none}}.{root_class}-{index}{{display:block}}");
            match query {
                fission_ir::op::ResponsiveQuery::Viewport => {
                    css.push_str(&format!("@media {expression}{{{body}}}"));
                }
                fission_ir::op::ResponsiveQuery::Container => {
                    css.push_str(&format!("@container {expression}{{{body}}}"));
                }
            }
        }
        self.styles.raw_rule(root_class.clone(), css);
        let container_style = match query {
            fission_ir::op::ResponsiveQuery::Viewport => "",
            fission_ir::op::ResponsiveQuery::Container => " style=\"container-type:inline-size\"",
        };
        Ok(format!(
            "<{tag} class=\"fission-site-node fission-site-responsive {root_class}\"{container_style} data-fission-node=\"{}\">{children}</{tag}>",
            node.id
        ))
    }

    fn semantic_identifier(&self, node_id: WidgetId) -> Option<&str> {
        let node = self.ir.nodes.get(&node_id)?;
        let Op::Semantics(semantics) = &node.op else {
            return None;
        };
        semantics.identifier.as_deref()
    }

    fn rich_text_annotations(&self, node_id: WidgetId) -> Option<&[RichTextAnnotation]> {
        self.ir
            .custom_render_objects
            .get(&node_id)?
            .downcast_ref::<Vec<RichTextAnnotation>>()
            .map(Vec::as_slice)
    }

    fn subtree_has_rich_text_annotation(&self, node: &CoreNode, identifier: &str) -> bool {
        let mut pending = node.children.clone();
        while let Some(node_id) = pending.pop() {
            let Some(descendant) = self.ir.nodes.get(&node_id) else {
                continue;
            };
            if matches!(descendant.op, Op::Paint(PaintOp::DrawRichText { .. }))
                && self
                    .rich_text_annotations(node_id)
                    .is_some_and(|annotations| {
                        annotations.iter().any(|annotation| {
                            annotation.semantics_identifier.as_deref() == Some(identifier)
                        })
                    })
            {
                return true;
            }
            pending.extend(descendant.children.iter().copied());
        }
        false
    }

    fn render_semantic_link(
        &mut self,
        node: &CoreNode,
        target: &str,
        label: Option<&str>,
        link_class: &str,
    ) -> Result<String> {
        let mut attrs = self.link_destination_attrs(target);
        if let Some(label) = label {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        let children = self.render_children(&node.children, &HashSet::new())?;
        Ok(format!(
            "<a class=\"fission-site-node fission-site-link {link_class}\"{attrs} data-fission-node=\"{}\">{children}</a>",
            node.id
        ))
    }

    fn link_destination_attrs(&self, target: &str) -> String {
        let mut attrs = format!(" href=\"{}\"", escape_attr(&self.resolve_link_href(target)));
        if is_external_web_link(target) {
            attrs.push_str(" rel=\"noopener noreferrer\"");
        }
        attrs.push_str(&format!(
            " data-fission-current-route=\"{}\"",
            escape_attr(&self.options.current_route_path)
        ));
        if site_link_is_current_page(target, &self.options.current_route_path) {
            attrs.push_str(" aria-current=\"page\"");
        }
        attrs
    }

    fn hyperlink_attrs(&self, hyperlink: &fission_ir::Hyperlink) -> String {
        let mut attrs = format!(
            " href=\"{}\"",
            escape_attr(&self.resolve_link_href(&hyperlink.href))
        );
        if !matches!(hyperlink.target, fission_ir::LinkTarget::Current) {
            attrs.push_str(&format!(
                " target=\"{}\"",
                escape_attr(hyperlink.target.as_html_target())
            ));
        }
        if let Some(rel) = hyperlink.rel.as_deref() {
            attrs.push_str(&format!(" rel=\"{}\"", escape_attr(rel)));
        } else if matches!(hyperlink.target, fission_ir::LinkTarget::NewWindow) {
            attrs.push_str(" rel=\"noopener noreferrer\"");
        }
        if let Some(filename) = hyperlink.download.as_deref() {
            attrs.push_str(&format!(" download=\"{}\"", escape_attr(filename)));
        }
        attrs
    }

    fn resolve_link_href(&self, target: &str) -> String {
        if target.starts_with('#')
            || is_external_web_link(target)
            || target.starts_with("mailto:")
            || target.starts_with("tel:")
        {
            target.to_string()
        } else if target.starts_with('/') {
            relative_href_for_route(&self.options.current_route_path, target)
        } else {
            target.to_string()
        }
    }

    fn resolve_asset_src(&self, source: &str) -> String {
        if source.starts_with('/') {
            relative_href_for_route(&self.options.current_route_path, source)
        } else {
            source.to_string()
        }
    }

    fn resolve_image_src(&self, source: &ImageSource) -> String {
        match source {
            ImageSource::Asset { path } | ImageSource::File { path } => {
                self.resolve_asset_src(path)
            }
            ImageSource::Network { url, .. } => url.clone(),
            ImageSource::Memory { bytes, mime_type } => format!(
                "data:{};base64,{}",
                mime_type.as_deref().unwrap_or("application/octet-stream"),
                BASE64_STANDARD.encode(bytes)
            ),
            ImageSource::SvgText { content } => {
                format!(
                    "data:image/svg+xml;base64,{}",
                    BASE64_STANDARD.encode(content)
                )
            }
        }
    }

    fn coalesced_paint_style(
        &self,
        node: &CoreNode,
        skip: &mut HashSet<WidgetId>,
    ) -> Result<Vec<String>> {
        let mut style = Vec::new();
        let mut shadows = Vec::new();
        for child_id in &node.children {
            let Some(child) = self.ir.nodes.get(child_id) else {
                continue;
            };
            if !is_coalesced_paint_child(child) {
                continue;
            }
            let Op::Paint(PaintOp::DrawRect {
                fill,
                stroke,
                corner_radius,
                shadow,
                corner_radii,
                border_sides,
            }) = &child.op
            else {
                unreachable!("coalesced site paint children are rectangles");
            };
            if let Some(shadow) = shadow {
                shadows.push(self.box_shadow_css(shadow));
            }
            style.extend(self.draw_rect_style(
                fill.as_ref(),
                stroke.as_ref(),
                *corner_radius,
                None,
                *corner_radii,
                border_sides.as_ref(),
            ));
            skip.insert(*child_id);
        }
        if !shadows.is_empty() {
            style.push(format!("box-shadow:{}", shadows.join(",")));
        }
        Ok(style)
    }

    fn draw_rect_style(
        &self,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
        corner_radius: f32,
        shadow: Option<&BoxShadow>,
        corner_radii: Option<CornerRadii>,
        border_sides: Option<&BorderSides>,
    ) -> Vec<String> {
        let mut style = Vec::new();
        if let Some(fill) = fill {
            style.push(format!("background:{}", self.fill_css(fill)));
        }
        // Per-edge strokes take precedence, matching the IR's replacement rule.
        // CSS expresses each edge natively, so nothing is approximated here.
        match border_sides {
            Some(sides) if !sides.is_empty() => match sides.as_uniform() {
                Some(stroke) => style.push(format!(
                    "border:{}px {} {}",
                    px(stroke.width),
                    border_line_style(stroke),
                    self.stroke_css(stroke)
                )),
                None => {
                    for (property, side) in [
                        ("border-top", &sides.top),
                        ("border-right", &sides.right),
                        ("border-bottom", &sides.bottom),
                        ("border-left", &sides.left),
                    ] {
                        if let Some(stroke) = side {
                            style.push(format!(
                                "{property}:{}px {} {}",
                                px(stroke.width),
                                border_line_style(stroke),
                                self.stroke_css(stroke)
                            ));
                        }
                    }
                }
            },
            _ => {
                if let Some(stroke) = stroke {
                    style.push(format!(
                        "border:{}px {} {}",
                        px(stroke.width),
                        border_line_style(stroke),
                        self.stroke_css(stroke)
                    ));
                }
            }
        }
        match corner_radii {
            Some(radii) if !radii.is_uniform() => style.push(format!(
                "border-radius:{}px {}px {}px {}px",
                px(radii.top_left),
                px(radii.top_right),
                px(radii.bottom_right),
                px(radii.bottom_left)
            )),
            Some(radii) if radii.top_left > 0.0 => {
                style.push(format!("border-radius:{}px", px(radii.top_left)))
            }
            Some(_) => {}
            None => {
                if corner_radius > 0.0 {
                    style.push(format!("border-radius:{}px", px(corner_radius)));
                }
            }
        }
        if let Some(shadow) = shadow {
            style.push(format!("box-shadow:{}", self.box_shadow_css(shadow)));
        }
        style
    }

    fn box_shadow_css(&self, shadow: &BoxShadow) -> String {
        format!(
            "{}{}px {}px {}px {}px {}",
            if shadow.inset { "inset " } else { "" },
            px(shadow.offset.0),
            px(shadow.offset.1),
            px(shadow.blur_radius),
            px(shadow.spread_radius),
            self.color_css(shadow.color)
        )
    }

    fn text_style(&self, size: f32, color: Color, underline: bool, wrap: bool) -> Vec<String> {
        let mut style = vec![
            format!("font-size:{}px", px(size)),
            format!("color:{}", self.color_css(color)),
            format!("white-space:{}", if wrap { "pre-wrap" } else { "pre" }),
        ];
        if underline {
            style.push("text-decoration:underline".to_string());
        }
        style
    }

    fn render_text_run(&mut self, run: &TextRun) -> String {
        let typography = &run.style.typography;
        let mut style = vec![
            format!("font-size:{}px", px(run.style.font_size)),
            format!("color:{}", self.color_css(run.style.color)),
            format!("font-weight:{}", run.style.font_weight),
            format!("letter-spacing:{}px", px(run.style.letter_spacing)),
            format!("word-spacing:{}px", px(typography.word_spacing)),
        ];
        let decorations = &typography.decoration.lines;
        let mut decoration_lines = Vec::new();
        if run.style.underline || decorations.underline {
            decoration_lines.push("underline");
        }
        if decorations.overline {
            decoration_lines.push("overline");
        }
        if decorations.line_through {
            decoration_lines.push("line-through");
        }
        if !decoration_lines.is_empty() {
            style.push(format!(
                "text-decoration-line:{}",
                decoration_lines.join(" ")
            ));
            style.push(format!(
                "text-decoration-style:{}",
                match typography.decoration.style {
                    fission_ir::op::TextDecorationStyle::Solid => "solid",
                    fission_ir::op::TextDecorationStyle::Double => "double",
                    fission_ir::op::TextDecorationStyle::Dotted => "dotted",
                    fission_ir::op::TextDecorationStyle::Dashed => "dashed",
                    fission_ir::op::TextDecorationStyle::Wavy => "wavy",
                }
            ));
            if let Some(color) = typography.decoration.color {
                style.push(format!("text-decoration-color:{}", self.color_css(color)));
            }
            if let Some(thickness) = typography.decoration.thickness {
                style.push(format!("text-decoration-thickness:{}px", px(thickness)));
            }
        }
        if let Some(family) = &run.style.font_family {
            let mut families = vec![self.font_family_css(family)];
            families.extend(
                typography
                    .font_fallback
                    .iter()
                    .map(|family| self.font_family_css(family)),
            );
            style.push(format!("font-family:{}", families.join(",")));
        } else if !typography.font_fallback.is_empty() {
            style.push(format!(
                "font-family:{}",
                typography
                    .font_fallback
                    .iter()
                    .map(|family| self.font_family_css(family))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if let Some(line_height) = run.style.line_height {
            style.push(format!("line-height:{}px", px(line_height)));
        }
        if run.style.font_style == FontStyle::Italic {
            style.push("font-style:italic".to_string());
        }
        if let Some(background) = run.style.background_color {
            style.push(format!("background:{}", self.color_css(background)));
            style.push("border-radius:0.35em".to_string());
            style.push("padding:0.1em 0.3em".to_string());
        }
        if let Some(background) = &typography.background {
            style.push(format!("background:{}", self.fill_css(background)));
        }
        if let Some(foreground) = &typography.foreground {
            style.push(format!("background:{}", self.fill_css(foreground)));
            style.push("background-clip:text".to_string());
            style.push("-webkit-background-clip:text".to_string());
            style.push("color:transparent".to_string());
        }
        if !typography.shadows.is_empty() {
            style.push(format!(
                "text-shadow:{}",
                typography
                    .shadows
                    .iter()
                    .map(|shadow| format!(
                        "{}px {}px {}px {}",
                        px(shadow.offset.0),
                        px(shadow.offset.1),
                        px(shadow.blur_radius),
                        self.color_css(shadow.color)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !typography.font_features.is_empty() {
            style.push(format!(
                "font-feature-settings:{}",
                typography
                    .font_features
                    .iter()
                    .map(|feature| format!("'{}' {}", escape_attr(&feature.tag), feature.value))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !typography.font_variations.is_empty() {
            style.push(format!(
                "font-variation-settings:{}",
                typography
                    .font_variations
                    .iter()
                    .map(|variation| format!(
                        "'{}' {}",
                        escape_attr(&variation.axis),
                        px(variation.value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if typography.baseline == fission_ir::op::TextBaseline::Ideographic {
            style.push("dominant-baseline:ideographic".to_string());
        }
        if typography.leading_distribution == fission_ir::op::TextLeadingDistribution::Even {
            style.push("text-box-trim:trim-both".to_string());
        }
        style.push(format!(
            "line-break:{}",
            match typography.line_break {
                fission_ir::op::TextLineBreakPolicy::Auto => "auto",
                fission_ir::op::TextLineBreakPolicy::Normal => "normal",
                fission_ir::op::TextLineBreakPolicy::Strict => "strict",
                fission_ir::op::TextLineBreakPolicy::Loose => "loose",
                fission_ir::op::TextLineBreakPolicy::Anywhere => "anywhere",
            }
        ));
        style.push(format!(
            "hyphens:{}",
            match typography.hyphenation {
                fission_ir::op::TextHyphenation::None => "none",
                fission_ir::op::TextHyphenation::Auto => "auto",
            }
        ));
        let class_name = self.class_name("fission-site-text-run", style);
        let language = language_attribute(run.style.locale.as_deref());
        format!(
            "<span class=\"{}\"{language}>{}</span>",
            escape_attr(&class_name),
            escape_text(&run.text)
        )
    }

    fn render_rich_text_runs(
        &mut self,
        node: &CoreNode,
        runs: &[TextRun],
        annotations: &[RichTextAnnotation],
    ) -> Result<String> {
        let mut content = String::new();
        let mut rendered_inline_children = HashSet::new();
        let mut active_annotations: Vec<&RichTextAnnotation> = Vec::new();
        let mut run_start = 0usize;
        for run in runs {
            let run_end = run_start.saturating_add(run.text.len());
            if run.text.is_empty() {
                if let Some(marker) = decode_inline_widget_marker(run.style.font_family.as_deref())
                {
                    // A real inline widget may itself contain an interactive
                    // element. Keep it outside text links rather than nesting
                    // anchors or buttons inside an anchor.
                    for annotation in active_annotations.drain(..).rev() {
                        content.push_str(Self::rich_text_annotation_closing(annotation));
                    }
                    if let Ok(child_index) = usize::try_from(marker.id) {
                        if let Some(child_id) = node.children.get(child_index) {
                            if rendered_inline_children.insert(*child_id) {
                                content.push_str(&self.render_node(*child_id)?);
                            }
                        }
                    }
                    run_start = run_end;
                    continue;
                }
            }
            let mut boundaries = vec![run_start, run_end];
            for annotation in annotations {
                let start = annotation.range.start.max(run_start).min(run_end);
                let end = annotation.range.end.max(run_start).min(run_end);
                let relative_start = start.saturating_sub(run_start);
                let relative_end = end.saturating_sub(run_start);
                if start < end && run.text.is_char_boundary(relative_start) {
                    boundaries.push(start);
                }
                if start < end && run.text.is_char_boundary(relative_end) {
                    boundaries.push(end);
                }
            }
            boundaries.sort_unstable();
            boundaries.dedup();

            for bounds in boundaries.windows(2) {
                let segment_start = bounds[0];
                let segment_end = bounds[1];
                if segment_start == segment_end {
                    continue;
                }
                let relative_start = segment_start - run_start;
                let relative_end = segment_end - run_start;
                let mut segment = run.clone();
                segment.text = run.text[relative_start..relative_end].to_string();
                let rendered = self.render_text_run(&segment);
                let mut segment_annotations = annotations
                    .iter()
                    .filter(|annotation| {
                        annotation.range.start <= segment_start
                            && annotation.range.end >= segment_end
                    })
                    .collect::<Vec<_>>();
                segment_annotations.sort_by_key(|annotation| {
                    annotation.range.end.saturating_sub(annotation.range.start)
                });
                // A style change does not create a new link or another Tab
                // stop. Nested annotations select the innermost destination;
                // HTML anchors cannot nest, so resume the outer link afterward.
                let segment_link = segment_annotations
                    .iter()
                    .copied()
                    .find(|annotation| Self::rich_text_annotation_is_link(annotation));
                // Preserve parent semantics outside their child annotations,
                // including when the parent and child cover the same bytes.
                let next_annotations = segment_annotations
                    .iter()
                    .rev()
                    .copied()
                    .filter(|annotation| {
                        if Self::rich_text_annotation_is_link(annotation) {
                            segment_link.is_some_and(|link| std::ptr::eq(link, *annotation))
                        } else {
                            annotation.semantics_label.is_some()
                                || annotation.semantics_identifier.is_some()
                                || annotation.spell_out.unwrap_or(false)
                        }
                    })
                    .collect::<Vec<_>>();
                let common = active_annotations
                    .iter()
                    .zip(&next_annotations)
                    .take_while(|(active, next)| std::ptr::eq(**active, **next))
                    .count();
                for annotation in active_annotations.drain(common..).rev() {
                    content.push_str(Self::rich_text_annotation_closing(annotation));
                }
                for annotation in &next_annotations[common..] {
                    content.push_str(&self.rich_text_annotation_opening(annotation));
                }
                active_annotations = next_annotations;
                content.push_str(&rendered);
            }
            run_start = run_end;
        }
        for annotation in active_annotations.into_iter().rev() {
            content.push_str(Self::rich_text_annotation_closing(annotation));
        }
        for child_id in &node.children {
            // Inline hyperlink semantics exist for retained focus and native
            // accessibility. Their visible content was rendered from the
            // annotation range above; do not append a second empty anchor.
            let generated_link = self.ir.nodes.get(child_id).is_some_and(|child| {
                child.children.is_empty()
                    && matches!(&child.op, Op::Semantics(semantics)
                    if semantics.role == Role::Link
                        && semantics.hyperlink.is_some()
                        && annotations.iter().any(|annotation| {
                            annotation.hyperlink == semantics.hyperlink
                                && annotation.semantics_identifier == semantics.identifier
                        }))
            });
            if generated_link {
                continue;
            }
            if rendered_inline_children.insert(*child_id) {
                content.push_str(&self.render_node(*child_id)?);
            }
        }
        Ok(content)
    }

    fn rich_text_annotation_attrs(&self, annotation: &RichTextAnnotation) -> String {
        let mut attrs = String::new();
        if let Some(label) = annotation.semantics_label.as_deref() {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        if let Some(identifier) = annotation.semantics_identifier.as_deref() {
            attrs.push_str(&format!(
                " data-fission-semantics=\"{}\"",
                escape_attr(identifier)
            ));
        }
        attrs
    }

    fn rich_text_link_opening(&self, annotation: &RichTextAnnotation) -> String {
        let attrs = self.rich_text_annotation_attrs(annotation);
        if let Some(hyperlink) = annotation.hyperlink.as_ref() {
            let link_attrs = self.hyperlink_attrs(hyperlink);
            format!("<a class=\"fission-site-link\"{link_attrs}{attrs}>")
        } else {
            let target = annotation
                .semantics_identifier
                .as_deref()
                .and_then(|id| id.strip_prefix("markdown-link:"))
                .expect("link annotations have a destination");
            let link_attrs = self.link_destination_attrs(target);
            format!("<a class=\"fission-site-link fission-site-markdown-link\"{link_attrs}{attrs}>")
        }
    }

    fn rich_text_annotation_is_link(annotation: &RichTextAnnotation) -> bool {
        annotation.hyperlink.is_some()
            || annotation
                .semantics_identifier
                .as_deref()
                .is_some_and(|id| id.starts_with("markdown-link:"))
    }

    fn rich_text_annotation_closing(annotation: &RichTextAnnotation) -> &'static str {
        if Self::rich_text_annotation_is_link(annotation) {
            "</a>"
        } else {
            "</span>"
        }
    }

    fn rich_text_annotation_opening(&self, annotation: &RichTextAnnotation) -> String {
        if Self::rich_text_annotation_is_link(annotation) {
            return self.rich_text_link_opening(annotation);
        }
        let mut attrs = self.rich_text_annotation_attrs(annotation);
        if annotation.spell_out.unwrap_or(false) {
            attrs.push_str(" role=\"text\"");
        }
        format!("<span{attrs}>")
    }

    fn fill_css(&self, fill: &Fill) -> String {
        // CSS has no reflect mode, but reflect is exactly a repeat over a stop
        // list mirrored about its midpoint, so it is emulated rather than
        // approximated. The mirrored list spans two periods of the original.
        let (prefix, reflect) = match fill.extend() {
            GradientExtend::Pad => ("", false),
            GradientExtend::Repeat => ("repeating-", false),
            GradientExtend::Reflect => ("repeating-", true),
        };
        let resolved = |stops: &[(f32, Color)]| -> (Vec<(f32, Color)>, f32) {
            if reflect {
                let mut out: Vec<(f32, Color)> = stops.iter().map(|(o, c)| (o * 0.5, *c)).collect();
                out.extend(stops.iter().rev().map(|(o, c)| (1.0 - o * 0.5, *c)));
                (out, 2.0)
            } else {
                (stops.to_vec(), 1.0)
            }
        };
        let stop_list = |stops: &[(f32, Color)], position: &dyn Fn(f32) -> String| {
            stops
                .iter()
                .map(|(offset, color)| format!("{} {}", self.color_css(*color), position(*offset)))
                .collect::<Vec<_>>()
                .join(",")
        };

        match fill {
            Fill::Solid(color) => self.color_css(*color),
            Fill::LinearGradient {
                start, end, stops, ..
            } => {
                let line = CssLinearGradientLine::new(*start, *end);
                let (stops, period) = resolved(stops);
                let stops = stop_list(&stops, &|offset| {
                    format!("{}%", px(line.css_position(offset * period) * 100.0))
                });
                format!(
                    "{prefix}linear-gradient({}deg,{stops})",
                    px(line.angle_degrees)
                )
            }
            Fill::RadialGradient {
                center,
                radius,
                stops,
                ..
            } => {
                // The IR radius is a fraction of the larger side and CSS offers
                // no length that tracks it, so the radius is written as a
                // percentage of each side. That is exact for a square box and
                // stretches into an ellipse otherwise.
                let (stops, period) = resolved(stops);
                let stops = stop_list(&stops, &|offset| {
                    format!("{}%", px(offset * period * 100.0))
                });
                format!(
                    "{prefix}radial-gradient({r}% {r}% at {}% {}%,{stops})",
                    px(center.0 * 100.0),
                    px(center.1 * 100.0),
                    r = px(radius * 100.0),
                )
            }
            Fill::SweepGradient {
                center,
                start_angle,
                end_angle,
                stops,
                ..
            } => {
                let sweep = css_sweep(*start_angle, *end_angle);
                let (stops, period) = resolved(stops);
                let stops = stop_list(&stops, &|offset| {
                    format!("{}deg", px(offset * period * sweep.span_degrees))
                });
                format!(
                    "{prefix}conic-gradient(from {}deg at {}% {}%,{stops})",
                    px(sweep.from_degrees),
                    px(center.0 * 100.0),
                    px(center.1 * 100.0)
                )
            }
        }
    }

    fn stroke_css(&self, stroke: &Stroke) -> String {
        match &stroke.fill {
            Fill::Solid(color) => self.color_css(*color),
            fill => self.fill_css(fill),
        }
    }

    fn svg_paint_style(&self, fill: Option<&Fill>, stroke: Option<&Stroke>) -> Vec<String> {
        let mut style = Vec::new();
        if let Some(fill) = fill {
            style.push(format!("fill:{}", self.fill_css(fill)));
        } else {
            style.push("fill:currentColor".to_string());
        }
        if let Some(stroke) = stroke {
            style.push(format!("stroke:{}", self.stroke_css(stroke)));
            style.push(format!("stroke-width:{}", px(stroke.width)));
            if let Some(dash_array) = stroke.dash_array.as_ref() {
                let values = dash_array
                    .iter()
                    .map(|value| px(*value))
                    .collect::<Vec<_>>()
                    .join(" ");
                style.push(format!("stroke-dasharray:{values}"));
            }
            style.push(format!("stroke-linecap:{}", line_cap_css(stroke.line_cap)));
            style.push(format!(
                "stroke-linejoin:{}",
                line_join_css(stroke.line_join)
            ));
        }
        style
    }

    fn color_css(&self, color: Color) -> String {
        self.options
            .css_variables
            .color_var(color)
            .map(|name| format!("var(--fs-color-{name})"))
            .unwrap_or_else(|| raw_color_css(color))
    }

    fn font_family_css(&self, family: &str) -> String {
        self.options
            .css_variables
            .font_var(family)
            .map(|name| format!("var(--fs-font-{name})"))
            .unwrap_or_else(|| family.to_string())
    }
}

fn html_autocomplete(semantics: &Semantics) -> String {
    let mut tokens = Vec::new();
    if let Some(group) = semantics.autofill_group.as_deref() {
        let normalized = group
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' {
                    ch
                } else {
                    '-'
                }
            })
            .collect::<String>();
        if !normalized.is_empty() {
            tokens.push(format!("section-{normalized}"));
        }
    }
    tokens.extend(semantics.autofill_hints.iter().cloned());
    tokens.join(" ")
}

fn markdown_heading_tag(identifier: &str) -> Option<&'static str> {
    let level = identifier
        .strip_prefix("markdown-heading-")?
        .split_once(':')
        .map(|(level, _)| level)
        .unwrap_or_else(|| identifier.strip_prefix("markdown-heading-").unwrap_or(""));
    match level {
        "1" => Some("h1"),
        "2" => Some("h2"),
        "3" => Some("h3"),
        "4" => Some("h4"),
        "5" => Some("h5"),
        "6" => Some("h6"),
        _ => None,
    }
}

fn markdown_heading_anchor(identifier: &str) -> Option<&str> {
    identifier
        .strip_prefix("markdown-heading-")?
        .split_once(':')
        .map(|(_, anchor)| anchor)
        .filter(|anchor| !anchor.is_empty())
}

fn code_language_class(language: &str) -> Option<String> {
    let mut class = String::new();
    for ch in language.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            class.push(ch.to_ascii_lowercase());
        }
    }
    (!class.is_empty()).then_some(class)
}

fn relative_href_for_route(current_route_path: &str, target: &str) -> String {
    let suffix_start = target
        .find('#')
        .or_else(|| target.find('?'))
        .unwrap_or(target.len());
    let (path, suffix) = target.split_at(suffix_start);
    let depth = current_route_path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .count();
    let prefix = "../".repeat(depth);
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        if prefix.is_empty() {
            format!("./{suffix}")
        } else {
            format!("{prefix}{suffix}")
        }
    } else {
        format!("{prefix}{trimmed}{suffix}")
    }
}

fn site_semantic_class(identifier: &str) -> String {
    let base = identifier.split(':').next().unwrap_or(identifier);
    let suffix = base
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("fission-{suffix}")
}

fn site_semantic_element(identifier: &str) -> (&'static str, Option<&str>) {
    match identifier {
        "site-header" => return ("header", None),
        "site-main" => return ("main", None),
        "site-navigation" => return ("nav", None),
        "site-footer" => return ("footer", None),
        "site-aside" => return ("aside", None),
        "site-address" => return ("address", None),
        _ => {}
    }
    if let Some(anchor) = identifier
        .strip_prefix("site-section:")
        .filter(|anchor| !anchor.is_empty())
    {
        return ("section", Some(anchor));
    }
    if let Some(anchor) = identifier
        .strip_prefix("site-anchor:")
        .filter(|anchor| !anchor.is_empty())
    {
        return ("div", Some(anchor));
    }
    for (prefix, tag) in [
        ("site-heading-1:", "h1"),
        ("site-heading-2:", "h2"),
        ("site-heading-3:", "h3"),
        ("site-heading-4:", "h4"),
        ("site-heading-5:", "h5"),
        ("site-heading-6:", "h6"),
    ] {
        if let Some(anchor) = identifier
            .strip_prefix(prefix)
            .filter(|anchor| !anchor.is_empty())
        {
            return (tag, Some(anchor));
        }
    }
    ("div", None)
}

fn is_site_heading_identifier(identifier: &str) -> bool {
    [
        "site-heading-1:",
        "site-heading-2:",
        "site-heading-3:",
        "site-heading-4:",
        "site-heading-5:",
        "site-heading-6:",
    ]
    .iter()
    .any(|prefix| identifier.starts_with(prefix))
}

fn site_link_is_current_page(target: &str, current_route_path: &str) -> bool {
    if target.starts_with('#')
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        || is_external_web_link(target)
    {
        return false;
    }
    let target = target.split(['?', '#']).next().unwrap_or(target);
    if !target.starts_with('/') {
        return false;
    }
    normalize_route_for_comparison(target) == normalize_route_for_comparison(current_route_path)
}

fn is_external_web_link(target: &str) -> bool {
    if target.starts_with("//") {
        return true;
    }
    target.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

fn normalize_route_for_comparison(path: &str) -> &str {
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/"
    } else {
        trimmed
    }
}

fn site_node_has_explicit_width(node: &CoreNode) -> bool {
    match &node.op {
        Op::Layout(LayoutOp::Box {
            width, max_width, ..
        })
        | Op::Layout(LayoutOp::Scroll {
            width, max_width, ..
        }) => width.is_some() || max_width.is_some(),
        Op::Layout(LayoutOp::StyledBox { style, .. }) => {
            style.width.is_some() || style.max_width.is_some()
        }
        Op::Layout(LayoutOp::Embed { width, .. }) => width.is_some(),
        _ => false,
    }
}

fn is_coalesced_paint_child(node: &CoreNode) -> bool {
    matches!(node.op, Op::Paint(PaintOp::DrawRect { .. }))
}

fn static_form_label(field: &StaticFormField, control: String) -> String {
    let label = field.label.as_deref().unwrap_or(field.name.as_str());
    format!(
        "<label class=\"fission-site-form-field\"><span class=\"fission-site-form-label\">{}</span>{control}</label>",
        escape_text(label)
    )
}

fn static_form_input_attrs(field: &StaticFormField) -> String {
    let mut attrs = format!(" name=\"{}\"", escape_attr(&field.name));
    if let Some(placeholder) = field.placeholder.as_deref() {
        attrs.push_str(&format!(" placeholder=\"{}\"", escape_attr(placeholder)));
    }
    if let Some(value) = field.value.as_deref() {
        if !matches!(
            field.kind,
            StaticFormFieldKind::Textarea | StaticFormFieldKind::Checkbox
        ) {
            attrs.push_str(&format!(" value=\"{}\"", escape_attr(value)));
        }
    }
    if field.required {
        attrs.push_str(" required");
    }
    if let Some(max_length) = field.max_length {
        attrs.push_str(&format!(" maxlength=\"{}\"", max_length));
    }
    attrs
}

fn site_semantic_data_attrs(identifier: &str) -> String {
    if let Some(rest) = identifier.strip_prefix("site-sidebar-item:") {
        let mut parts = rest.split(':');
        let level = parts.next().unwrap_or("0");
        let active = parts.next().unwrap_or("false");
        let group = parts.next().unwrap_or("false");
        let index = parts.next().unwrap_or("0");
        return format!(
            " data-fission-site-sidebar-level=\"{}\" data-fission-site-sidebar-active=\"{}\" data-fission-site-sidebar-group=\"{}\" data-fission-site-sidebar-index=\"{}\"",
            escape_attr(level),
            escape_attr(active),
            escape_attr(group),
            escape_attr(index)
        );
    }
    if let Some(rest) = identifier.strip_prefix("site-nav-item:") {
        let mut parts = rest.split(':');
        let depth = parts.next().unwrap_or("0");
        let has_children = parts.next().unwrap_or("false");
        let index = parts.next().unwrap_or("0");
        return format!(
            " data-fission-site-nav-depth=\"{}\" data-fission-site-nav-has-children=\"{}\" data-fission-site-nav-index=\"{}\"",
            escape_attr(depth),
            escape_attr(has_children),
            escape_attr(index)
        );
    }
    if let Some(rest) = identifier.strip_prefix("site-nav-menu:") {
        let mut parts = rest.split(':');
        let depth = parts.next().unwrap_or("0");
        let count = parts.next().unwrap_or("0");
        return format!(
            " data-fission-site-nav-menu-depth=\"{}\" data-fission-site-nav-menu-count=\"{}\"",
            escape_attr(depth),
            escape_attr(count)
        );
    }
    if let Some(rest) = identifier.strip_prefix("site-nav-label:") {
        let mut parts = rest.split(':');
        let depth = parts.next().unwrap_or("0");
        let has_children = parts.next().unwrap_or("false");
        let index = parts.next().unwrap_or("0");
        return format!(
            " data-fission-site-nav-label-depth=\"{}\" data-fission-site-nav-label-has-children=\"{}\" data-fission-site-nav-label-index=\"{}\"",
            escape_attr(depth),
            escape_attr(has_children),
            escape_attr(index)
        );
    }
    String::new()
}

fn push_paragraph_style(
    style: &mut Vec<String>,
    paragraph: Option<&fission_ir::op::TextParagraphStyle>,
) {
    if let Some(paragraph) = paragraph {
        style.push(format!(
            "text-align:{}",
            text_align_css(paragraph.text_align)
        ));
        if let Some(lines) = paragraph.max_lines {
            style.push("display:-webkit-box".to_string());
            style.push("-webkit-box-orient:vertical".to_string());
            style.push(format!("-webkit-line-clamp:{lines}"));
        }
        match paragraph.overflow {
            TextOverflow::Clip => style.push("overflow:hidden".to_string()),
            TextOverflow::Ellipsis => {
                style.push("overflow:hidden".to_string());
                style.push("text-overflow:ellipsis".to_string());
            }
            TextOverflow::Fade => style.push("overflow:hidden".to_string()),
            TextOverflow::Visible => {}
        }
    }
}

fn language_attribute(locale: Option<&str>) -> String {
    locale
        .filter(|locale| !locale.trim().is_empty())
        .map(|locale| format!(" lang=\"{}\"", escape_attr(locale)))
        .unwrap_or_default()
}

fn direction_attribute(paragraph: Option<&fission_ir::op::TextParagraphStyle>) -> &'static str {
    match paragraph.map(|style| style.text_direction) {
        Some(fission_ir::op::TextDirection::Ltr) => " dir=\"ltr\"",
        Some(fission_ir::op::TextDirection::Rtl) => " dir=\"rtl\"",
        Some(fission_ir::op::TextDirection::Auto) => " dir=\"auto\"",
        None => "",
    }
}

fn paragraph_needs_text_box(paragraph: Option<&fission_ir::op::TextParagraphStyle>) -> bool {
    matches!(
        paragraph.map(|style| style.text_align),
        Some(TextAlign::Center | TextAlign::Right | TextAlign::End | TextAlign::Justify)
    )
}

fn push_box_constraints(
    style: &mut Vec<String>,
    width: Option<f32>,
    height: Option<f32>,
    min_width: Option<f32>,
    max_width: Option<f32>,
    min_height: Option<f32>,
    max_height: Option<f32>,
) {
    push_optional_px(style, "width", width);
    push_optional_px(style, "height", height);
    push_optional_px(style, "min-width", min_width);
    push_optional_px(style, "max-width", max_width);
    push_optional_px(style, "min-height", min_height);
    push_optional_px(style, "max-height", max_height);
}

fn push_length_property(style: &mut Vec<String>, property: &str, value: Option<&Length>) {
    if let Some(value) = value {
        style.push(format!("{property}:{}", length_css(value)));
    }
}

fn length_css(length: &Length) -> String {
    match length {
        Length::Points(value) => format!("{}px", px(*value)),
        Length::Percent(value) => format!("{}%", px(*value)),
        Length::ViewportWidth(value) => format!("{}vw", px(*value)),
        Length::ViewportHeight(value) => format!("{}vh", px(*value)),
        Length::Add(left, right) => format!("calc({} + {})", length_css(left), length_css(right)),
        Length::Subtract(left, right) => {
            format!("calc({} - {})", length_css(left), length_css(right))
        }
        Length::Min(values) => format!(
            "min({})",
            values.iter().map(length_css).collect::<Vec<_>>().join(", ")
        ),
        Length::Max(values) => format!(
            "max({})",
            values.iter().map(length_css).collect::<Vec<_>>().join(", ")
        ),
        Length::Clamp {
            min,
            preferred,
            max,
        } => format!(
            "clamp({}, {}, {})",
            length_css(min),
            length_css(preferred),
            length_css(max)
        ),
        Length::FitContent(Some(limit)) => format!("fit-content({})", length_css(limit)),
        Length::FitContent(None) => "fit-content".into(),
        Length::MinContent => "min-content".into(),
        Length::MaxContent => "max-content".into(),
        Length::Auto => "auto".into(),
    }
}

fn push_padding(style: &mut Vec<String>, padding: [f32; 4]) {
    if padding.iter().any(|value| *value != 0.0) {
        style.push(format!(
            "padding:{}px {}px {}px {}px",
            px(padding[2]),
            px(padding[1]),
            px(padding[3]),
            px(padding[0])
        ));
    }
}

fn push_flex_item(style: &mut Vec<String>, flex_grow: f32, flex_shrink: f32) {
    if flex_grow != 0.0 {
        style.push(format!("flex-grow:{flex_grow}"));
    }
    if (flex_shrink - 1.0).abs() > f32::EPSILON {
        style.push(format!("flex-shrink:{flex_shrink}"));
    }
}

fn push_optional_px(style: &mut Vec<String>, name: &str, value: Option<f32>) {
    if let Some(value) = value {
        style.push(format!("{name}:{}px", px(value)));
    }
}

fn push_grid_placement(style: &mut Vec<String>, name: &str, value: GridPlacement) {
    match value {
        GridPlacement::Auto => {}
        GridPlacement::Line(line) => style.push(format!("{name}:{line}")),
        GridPlacement::Span(span) => style.push(format!("{name}:span {span}")),
    }
}

#[derive(Clone, Copy, Debug)]
enum CssAnimationProperty {
    Opacity,
    TranslateX { other_axis: f32 },
    TranslateY { other_axis: f32 },
    Scale,
    Rotation,
    Width,
    Height,
    CornerRadius,
}

impl CssAnimationProperty {
    fn property_name(self) -> &'static str {
        match self {
            Self::Opacity => "opacity",
            Self::TranslateX { .. } | Self::TranslateY { .. } => "translate",
            Self::Scale => "scale",
            Self::Rotation => "rotate",
            Self::Width => "width",
            Self::Height => "height",
            Self::CornerRadius => "border-radius",
        }
    }

    fn css_declaration(self, value: f32) -> String {
        match self {
            Self::Opacity => format!("opacity:{}", px(value)),
            Self::TranslateX { other_axis } => {
                format!("translate:{}px {}px", px(value), px(other_axis))
            }
            Self::TranslateY { other_axis } => {
                format!("translate:{}px {}px", px(other_axis), px(value))
            }
            Self::Scale => format!("scale:{}", px(value)),
            Self::Rotation => format!("rotate:{}deg", px(value)),
            Self::Width => format!("width:{}px", px(value)),
            Self::Height => format!("height:{}px", px(value)),
            Self::CornerRadius => format!("border-radius:{}px", px(value)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum CssColorAnimationProperty {
    BackgroundColor,
    BorderColor,
    TextColor,
}

impl CssColorAnimationProperty {
    fn property_name(self) -> &'static str {
        match self {
            Self::BackgroundColor => "background-color",
            Self::BorderColor => "border-color",
            Self::TextColor => "color",
        }
    }

    fn css_declaration(self, renderer: &HtmlRenderer<'_>, color: Color) -> String {
        match self {
            Self::BackgroundColor => format!("background-color:{}", renderer.color_css(color)),
            Self::BorderColor => format!("border-color:{}", renderer.color_css(color)),
            Self::TextColor => format!("color:{}", renderer.color_css(color)),
        }
    }
}

fn animation_start_value(request: &MotionTrack, base: f32) -> f32 {
    match &request.from {
        MotionStartValue::Explicit(value) => motion_expr_scalar(value, base),
        MotionStartValue::Current => base,
    }
}

fn animation_start_scalar(request: &MotionTrack, base: f32) -> Option<f32> {
    match &request.from {
        MotionStartValue::Explicit(value) => motion_expr_scalar_value(value),
        MotionStartValue::Current => Some(base),
    }
}

fn animation_start_color(request: &MotionTrack) -> Option<Color> {
    match &request.from {
        MotionStartValue::Explicit(value) => motion_expr_color_value(value),
        MotionStartValue::Current => None,
    }
}

fn motion_expr_scalar(expr: &MotionExpr, fallback: f32) -> f32 {
    motion_expr_scalar_value(expr).unwrap_or(fallback)
}

fn motion_expr_scalar_value(expr: &MotionExpr) -> Option<f32> {
    match expr {
        MotionExpr::Value(MotionValue::Scalar(value))
        | MotionExpr::Value(MotionValue::Px(value))
        | MotionExpr::Value(MotionValue::Deg(value)) => Some(*value),
        MotionExpr::Neg(value) => motion_expr_scalar_value(value).map(|value| -value),
        MotionExpr::Abs(value) => motion_expr_scalar_value(value).map(f32::abs),
        MotionExpr::Add(left, right) => {
            Some(motion_expr_scalar_value(left)? + motion_expr_scalar_value(right)?)
        }
        MotionExpr::Sub(left, right) => {
            Some(motion_expr_scalar_value(left)? - motion_expr_scalar_value(right)?)
        }
        MotionExpr::Mul(left, right) => {
            Some(motion_expr_scalar_value(left)? * motion_expr_scalar_value(right)?)
        }
        MotionExpr::Div(left, right) => {
            let right = motion_expr_scalar_value(right)?;
            if right.abs() <= f32::EPSILON {
                motion_expr_scalar_value(left)
            } else {
                Some(motion_expr_scalar_value(left)? / right)
            }
        }
        MotionExpr::Min(left, right) => {
            Some(motion_expr_scalar_value(left)?.min(motion_expr_scalar_value(right)?))
        }
        MotionExpr::Max(left, right) => {
            Some(motion_expr_scalar_value(left)?.max(motion_expr_scalar_value(right)?))
        }
        MotionExpr::Clamp { value, min, max } => Some(motion_expr_scalar_value(value)?.clamp(
            motion_expr_scalar_value(min)?,
            motion_expr_scalar_value(max)?,
        )),
        MotionExpr::Lerp { from, to, t } => {
            let from = motion_expr_scalar_value(from)?;
            let to = motion_expr_scalar_value(to)?;
            let t = motion_expr_scalar_value(t)?.clamp(0.0, 1.0);
            Some(from + (to - from) * t)
        }
        MotionExpr::MapRange {
            value,
            from_start,
            from_end,
            to_start,
            to_end,
            clamp,
        } => {
            let denominator = from_end - from_start;
            if denominator.abs() <= f32::EPSILON {
                return Some(*to_start);
            }
            let mut t = (motion_expr_scalar_value(value)? - from_start) / denominator;
            if *clamp {
                t = t.clamp(0.0, 1.0);
            }
            Some(*to_start + (*to_end - *to_start) * t)
        }
        _ => None,
    }
}

fn motion_expr_length_css(expr: &MotionExpr) -> Option<String> {
    match expr {
        MotionExpr::IntrinsicWidth | MotionExpr::IntrinsicHeight => Some("auto".to_string()),
        _ => motion_expr_scalar_value(expr).map(|value| format!("{}px", px(value))),
    }
}

fn motion_expr_color_value(expr: &MotionExpr) -> Option<Color> {
    match expr {
        MotionExpr::Value(MotionValue::Color(value)) => Some(*value),
        _ => None,
    }
}

fn interaction_predicate_id(expression: &MotionExpr) -> Option<WidgetId> {
    fn visit(expression: &MotionExpr, found: &mut Option<WidgetId>) -> bool {
        let MotionExpr::If {
            predicate,
            then_expr,
            else_expr,
        } = expression
        else {
            return true;
        };
        let id = match predicate {
            MotionPredicate::Hovered(id)
            | MotionPredicate::Pressed(id)
            | MotionPredicate::Focused(id)
            | MotionPredicate::Disabled(id) => *id,
        };
        if found.is_some_and(|found| found != id) {
            return false;
        }
        *found = Some(id);
        visit(then_expr, found) && visit(else_expr, found)
    }

    let mut found = None;
    visit(expression, &mut found).then_some(found).flatten()
}

fn select_interaction_expr(expression: &MotionExpr, pseudo: InteractionPseudo) -> &MotionExpr {
    match expression {
        MotionExpr::If {
            predicate,
            then_expr,
            else_expr,
        } => {
            if pseudo.matches(predicate) {
                select_interaction_expr(then_expr, pseudo)
            } else {
                select_interaction_expr(else_expr, pseudo)
            }
        }
        expression => expression,
    }
}

fn interaction_css_property(property: &MotionPropertyId) -> Option<&'static str> {
    match property {
        MotionPropertyId::Opacity => Some("opacity"),
        MotionPropertyId::Scale => Some("scale"),
        MotionPropertyId::BackgroundColor => Some("background-color"),
        MotionPropertyId::BackgroundFill => Some("background"),
        MotionPropertyId::BorderColor => Some("border-color"),
        MotionPropertyId::BorderWidth => Some("border-width"),
        MotionPropertyId::CornerRadius => Some("border-radius"),
        MotionPropertyId::PaddingLeft => Some("padding-left"),
        MotionPropertyId::PaddingRight => Some("padding-right"),
        MotionPropertyId::PaddingTop => Some("padding-top"),
        MotionPropertyId::PaddingBottom => Some("padding-bottom"),
        MotionPropertyId::BoxShadows => Some("box-shadow"),
        _ => None,
    }
}

fn interaction_transition_css(property: &str, transition: &MotionTransition) -> String {
    let (duration_ms, delay_ms, easing, _) = transition_css_parts(transition);
    format!(
        "{property} {duration_ms}ms {} {delay_ms}ms",
        easing_css(&easing)
    )
}

fn transition_css_parts(transition: &MotionTransition) -> (u64, u64, MotionEasing, bool) {
    match transition {
        MotionTransition::Instant => (0, 0, MotionEasing::Linear, false),
        MotionTransition::Tween {
            duration_ms,
            delay_ms,
            easing,
            repeat,
            ..
        } => (*duration_ms, *delay_ms, easing.clone(), *repeat),
        MotionTransition::Spring { delay_ms, .. } => (260, *delay_ms, MotionEasing::EaseOut, false),
    }
}

fn easing_css(easing: &MotionEasing) -> String {
    match easing {
        MotionEasing::Linear => "linear".to_string(),
        MotionEasing::EaseIn => "ease-in".to_string(),
        MotionEasing::EaseOut => "ease-out".to_string(),
        MotionEasing::EaseInOut => "ease-in-out".to_string(),
        MotionEasing::CubicBezier(x1, y1, x2, y2) => {
            format!(
                "cubic-bezier({},{},{},{})",
                px(*x1),
                px(*y1),
                px(*x2),
                px(*y2)
            )
        }
    }
}

fn raw_color_css(color: Color) -> String {
    if color.a == 255 {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    } else {
        format!(
            "rgba({},{},{},{:.3})",
            color.r,
            color.g,
            color.b,
            color.a as f32 / 255.0
        )
    }
}

fn grid_tracks(tracks: &[GridTrack]) -> String {
    tracks.iter().map(grid_track).collect::<Vec<_>>().join(" ")
}

fn grid_track(track: &GridTrack) -> String {
    match track {
        GridTrack::Points(value) => format!("{}px", px(*value)),
        GridTrack::Percent(value) => format!("{}%", px(*value)),
        GridTrack::Fr(value) => format!("{}fr", px(*value)),
        GridTrack::Auto => "auto".to_string(),
        GridTrack::MinContent => "min-content".to_string(),
        GridTrack::MaxContent => "max-content".to_string(),
        GridTrack::MinMax(min, max) => format!("minmax({}, {})", grid_track(min), grid_track(max)),
        GridTrack::Repeat { count, tracks } => {
            format!("repeat({count}, {})", grid_tracks(tracks))
        }
        GridTrack::AutoFit(track) => format!("repeat(auto-fit, {})", grid_track(track)),
        GridTrack::AutoFill(track) => format!("repeat(auto-fill, {})", grid_track(track)),
    }
}

fn transparent_list_layout_style(layout: &LayoutOp) -> Option<(&'static str, Vec<String>)> {
    match layout {
        LayoutOp::Flex {
            direction,
            wrap,
            flex_grow,
            flex_shrink,
            padding,
            gap,
            line_gap,
            align_items,
            justify_content,
        } => Some(flex_layout_style(
            *direction,
            *wrap,
            *flex_grow,
            *flex_shrink,
            *padding,
            *gap,
            *line_gap,
            *align_items,
            *justify_content,
        )),
        LayoutOp::Grid {
            columns,
            rows,
            column_gap,
            row_gap,
            padding,
        } => Some((
            "fission-site-grid",
            grid_layout_style(columns, rows, *column_gap, *row_gap, *padding),
        )),
        _ => None,
    }
}

fn flex_layout_style(
    direction: FlexDirection,
    wrap: FlexWrap,
    flex_grow: f32,
    flex_shrink: f32,
    padding: [f32; 4],
    gap: Option<f32>,
    line_gap: Option<f32>,
    align_items: AlignItems,
    justify_content: JustifyContent,
) -> (&'static str, Vec<String>) {
    let mut style = vec![
        "display:flex".to_string(),
        format!("flex-direction:{}", flex_direction(direction)),
        format!("flex-wrap:{}", flex_wrap(wrap)),
        format!("align-items:{}", align_items_css(align_items)),
        format!("justify-content:{}", justify_content_css(justify_content)),
    ];
    if let Some(gap) = gap {
        let main_property = match direction {
            FlexDirection::Row => "column-gap",
            FlexDirection::Column => "row-gap",
        };
        style.push(format!("{main_property}:{}px", px(gap)));
    }
    if let Some(line_gap) = line_gap.or(gap) {
        let line_property = match direction {
            FlexDirection::Row => "row-gap",
            FlexDirection::Column => "column-gap",
        };
        style.push(format!("{line_property}:{}px", px(line_gap)));
    }
    push_padding(&mut style, padding);
    push_flex_item(&mut style, flex_grow, flex_shrink);
    let class_name = match direction {
        FlexDirection::Column => "fission-site-column",
        FlexDirection::Row => "fission-site-row",
    };
    (class_name, style)
}

fn grid_layout_style(
    columns: &[GridTrack],
    rows: &[GridTrack],
    column_gap: Option<f32>,
    row_gap: Option<f32>,
    padding: [f32; 4],
) -> Vec<String> {
    let mut style = vec!["display:grid".to_string()];
    if !columns.is_empty() {
        style.push(format!("grid-template-columns:{}", grid_tracks(columns)));
    }
    if !rows.is_empty() {
        style.push(format!("grid-template-rows:{}", grid_tracks(rows)));
    }
    if let Some(gap) = column_gap {
        style.push(format!("column-gap:{}px", px(gap)));
    }
    if let Some(gap) = row_gap {
        style.push(format!("row-gap:{}px", px(gap)));
    }
    push_padding(&mut style, padding);
    style
}

fn flex_direction(direction: FlexDirection) -> &'static str {
    match direction {
        FlexDirection::Row => "row",
        FlexDirection::Column => "column",
    }
}

fn flex_wrap(wrap: FlexWrap) -> &'static str {
    match wrap {
        FlexWrap::NoWrap => "nowrap",
        FlexWrap::Wrap => "wrap",
        FlexWrap::WrapReverse => "wrap-reverse",
    }
}

fn align_items_css(align: AlignItems) -> &'static str {
    match align {
        AlignItems::Start => "flex-start",
        AlignItems::End => "flex-end",
        AlignItems::Center => "center",
        AlignItems::Stretch => "stretch",
        AlignItems::Baseline => "baseline",
    }
}

fn box_alignment_css(align: fission_ir::op::BoxAlignment) -> &'static str {
    match align {
        fission_ir::op::BoxAlignment::Start => "flex-start",
        fission_ir::op::BoxAlignment::Center => "center",
        fission_ir::op::BoxAlignment::End => "flex-end",
        fission_ir::op::BoxAlignment::Stretch => "stretch",
    }
}

fn justify_content_css(justify: JustifyContent) -> &'static str {
    match justify {
        JustifyContent::Start => "flex-start",
        JustifyContent::End => "flex-end",
        JustifyContent::Center => "center",
        JustifyContent::SpaceBetween => "space-between",
        JustifyContent::SpaceAround => "space-around",
        JustifyContent::SpaceEvenly => "space-evenly",
    }
}

fn is_native_control_semantics(semantics: &Semantics) -> bool {
    semantics.supports_text_editing()
        || matches!(
            semantics.role,
            Role::Checkbox | Role::Radio | Role::Switch | Role::Slider
        )
}

fn semantic_html_role(role: Role) -> Option<&'static str> {
    match role {
        Role::MenuItem => Some("menuitem"),
        Role::Image => Some("img"),
        // ARIA has no video role; a labelled group is what screen readers
        // actually announce usefully for a media surface.
        Role::Video => Some("group"),
        Role::Dialog => Some("dialog"),
        Role::Menu => Some("menu"),
        Role::ListBox => Some("listbox"),
        Role::Option => Some("option"),
        Role::ComboBox => Some("combobox"),
        Role::TabList => Some("tablist"),
        Role::Tab => Some("tab"),
        Role::TabPanel => Some("tabpanel"),
        Role::Alert => Some("alert"),
        Role::Group => Some("group"),
        Role::Separator => Some("separator"),
        Role::Tree => Some("tree"),
        Role::TreeItem => Some("treeitem"),
        Role::Toolbar => Some("toolbar"),
        Role::RadioGroup => Some("radiogroup"),
        Role::Table => Some("table"),
        Role::TableRow => Some("row"),
        Role::TableCell => Some("cell"),
        Role::ColumnHeader => Some("columnheader"),
        Role::ProgressBar => Some("progressbar"),
        Role::Tooltip => Some("tooltip"),
        Role::Status => Some("status"),
        Role::SpinButton => Some("spinbutton"),
        Role::Button
        | Role::Link
        | Role::Text
        | Role::TextInput
        | Role::Checkbox
        | Role::Radio
        | Role::Switch
        | Role::Slider
        | Role::Input
        | Role::List
        | Role::ListItem
        | Role::Generic => None,
    }
}

fn semantic_dom_id(ir: &CoreIR, node_id: WidgetId) -> String {
    ir.nodes
        .get(&node_id)
        .and_then(|node| match &node.op {
            Op::Semantics(semantics) => semantics.identifier.as_deref(),
            _ => None,
        })
        .and_then(markdown_heading_anchor)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("fission-semantic-{}", node_id.as_u128()))
}

fn semantic_html_attrs(ir: &CoreIR, node_id: WidgetId, semantics: &Semantics) -> String {
    let mut attrs = format!(" id=\"{}\"", escape_attr(&semantic_dom_id(ir, node_id)));
    if let Some(label) = semantics.label.as_deref() {
        attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
    }
    if let Some(identifier) = semantics.identifier.as_deref() {
        attrs.push_str(&format!(
            " data-fission-semantics=\"{}\"",
            escape_attr(identifier)
        ));
    }
    if semantics.disabled {
        attrs.push_str(" aria-disabled=\"true\"");
    }
    if semantics.read_only {
        attrs.push_str(" aria-readonly=\"true\"");
    }
    if semantics.focusable && !semantics.disabled {
        attrs.push_str(if semantics.is_sequentially_focusable() {
            " tabindex=\"0\""
        } else {
            " tabindex=\"-1\""
        });
    }
    if let Some(checked) = semantics.checked {
        attrs.push_str(&format!(" aria-checked=\"{checked}\""));
    }
    if let Some(selected) = semantics.selected {
        attrs.push_str(&format!(" aria-selected=\"{selected}\""));
    }
    if let Some(expanded) = semantics.expanded {
        attrs.push_str(&format!(" aria-expanded=\"{expanded}\""));
    }
    if let Some(popup) = semantics.has_popup {
        let popup = match popup {
            PopupKind::Menu => "menu",
            PopupKind::ListBox => "listbox",
            PopupKind::Tree => "tree",
            PopupKind::Grid => "grid",
            PopupKind::Dialog => "dialog",
        };
        attrs.push_str(&format!(" aria-haspopup=\"{popup}\""));
    }
    if let Some(orientation) = semantics.orientation {
        let orientation = match orientation {
            SemanticOrientation::Horizontal => "horizontal",
            SemanticOrientation::Vertical => "vertical",
        };
        attrs.push_str(&format!(" aria-orientation=\"{orientation}\""));
    }
    if semantics.modal {
        attrs.push_str(" aria-modal=\"true\"");
    }
    if semantics.required {
        attrs.push_str(" aria-required=\"true\"");
    }
    if matches!(
        semantics.validation_state,
        fission_ir::TextFieldValidationState::Invalid
    ) {
        attrs.push_str(" aria-invalid=\"true\"");
    }

    // A range is unreadable without its bounds, so report them wherever a role
    // carries one rather than only on a native range input.
    if let Some(value) = semantics.min_value {
        attrs.push_str(&format!(" aria-valuemin=\"{value}\""));
    }
    if let Some(value) = semantics.max_value {
        attrs.push_str(&format!(" aria-valuemax=\"{value}\""));
    }
    if let Some(value) = semantics.current_value {
        attrs.push_str(&format!(" aria-valuenow=\"{value}\""));
    }
    let mut relation = |name: &str, ids: &[WidgetId]| {
        if ids.is_empty() {
            return;
        }
        let value = ids
            .iter()
            .map(|id| semantic_dom_id(ir, *id))
            .collect::<Vec<_>>()
            .join(" ");
        attrs.push_str(&format!(" {name}=\"{}\"", escape_attr(&value)));
    };
    relation("aria-controls", &semantics.controls);
    relation("aria-labelledby", &semantics.labelled_by);
    relation("aria-describedby", &semantics.described_by);
    if let Some(active_descendant) = semantics.active_descendant {
        attrs.push_str(&format!(
            " aria-activedescendant=\"{}\"",
            escape_attr(&semantic_dom_id(ir, active_descendant))
        ));
    }
    attrs
}

fn html_text_input_type(semantics: &Semantics) -> &'static str {
    if semantics.masked {
        return "password";
    }
    match semantics.text_input_type {
        fission_ir::semantics::TextInputType::Number => "number",
        fission_ir::semantics::TextInputType::EmailAddress => "email",
        fission_ir::semantics::TextInputType::Url => "url",
        fission_ir::semantics::TextInputType::Phone => "tel",
        _ => "text",
    }
}

fn html_text_input_mode(semantics: &Semantics) -> &'static str {
    use fission_ir::semantics::TextInputType;

    match semantics.text_input_type {
        TextInputType::Number => "decimal",
        TextInputType::EmailAddress => "email",
        TextInputType::Url => "url",
        TextInputType::Phone => "tel",
        TextInputType::Text | TextInputType::Multiline | TextInputType::Name => "text",
    }
}

fn html_enter_key_hint(action: fission_ir::semantics::TextInputAction) -> &'static str {
    use fission_ir::semantics::TextInputAction;

    match action {
        TextInputAction::Done => "done",
        TextInputAction::Go | TextInputAction::Route => "go",
        TextInputAction::Search => "search",
        TextInputAction::Send | TextInputAction::EmergencyCall => "send",
        TextInputAction::Next => "next",
        TextInputAction::Previous => "previous",
        TextInputAction::Continue | TextInputAction::Join | TextInputAction::Newline => "enter",
    }
}

fn html_autocapitalize(capitalization: fission_ir::semantics::TextCapitalization) -> &'static str {
    use fission_ir::semantics::TextCapitalization;

    match capitalization {
        TextCapitalization::None => "none",
        TextCapitalization::Characters => "characters",
        TextCapitalization::Words => "words",
        TextCapitalization::Sentences => "sentences",
    }
}

fn text_validation_message_id(node_id: WidgetId) -> String {
    format!("fission-text-validation-{}", node_id.as_u128())
}

fn image_fit_css(fit: ImageFit) -> &'static str {
    match fit {
        ImageFit::Contain => "contain",
        ImageFit::Cover => "cover",
        ImageFit::Fill => "fill",
        ImageFit::None => "none",
    }
}

fn image_alignment_css(alignment: ImageAlignment) -> &'static str {
    match alignment {
        ImageAlignment::TopStart => "left top",
        ImageAlignment::TopCenter => "center top",
        ImageAlignment::TopEnd => "right top",
        ImageAlignment::CenterStart => "left center",
        ImageAlignment::Center => "center center",
        ImageAlignment::CenterEnd => "right center",
        ImageAlignment::BottomStart => "left bottom",
        ImageAlignment::BottomCenter => "center bottom",
        ImageAlignment::BottomEnd => "right bottom",
    }
}

fn text_align_css(align: TextAlign) -> &'static str {
    match align {
        TextAlign::Left => "left",
        TextAlign::Right => "right",
        TextAlign::Center => "center",
        TextAlign::Justify => "justify",
        TextAlign::Start => "start",
        TextAlign::End => "end",
    }
}

fn line_cap_css(line_cap: LineCap) -> &'static str {
    match line_cap {
        LineCap::Butt => "butt",
        LineCap::Round => "round",
        LineCap::Square => "square",
    }
}

fn line_join_css(line_join: LineJoin) -> &'static str {
    match line_join {
        LineJoin::Miter => "miter",
        LineJoin::Round => "round",
        LineJoin::Bevel => "bevel",
    }
}

fn matrix3d(values: &[f32; 16]) -> String {
    values
        .iter()
        .map(|value| px(*value))
        .collect::<Vec<_>>()
        .join(",")
}

fn css_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// The CSS line style for a border stroke.
///
/// CSS cannot take an arbitrary dash pattern for a border, so any dashed stroke
/// is written as `dashed` and the browser chooses the dash length.
fn border_line_style(stroke: &Stroke) -> &'static str {
    match &stroke.dash_array {
        Some(dashes) if !dashes.is_empty() => "dashed",
        _ => "solid",
    }
}

fn px(value: f32) -> String {
    if (value.fract()).abs() < 0.001 {
        format!("{}", value.round() as i32)
    } else {
        format!("{value:.3}")
    }
}

fn escape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_attr(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// A linear gradient's IR geometry expressed as CSS's angle and gradient line.
///
/// The IR places the gradient between two points normalized to the painted
/// bounds; CSS instead takes an angle and runs the gradient line through the
/// box centre, long enough that its ends touch the far corners. Each IR stop
/// offset is re-projected onto that line so a gradient that starts or ends
/// inside the box keeps its extent. The normalized points are treated as
/// square, so the result is exact for square boxes and for axis-aligned
/// gradients in any box; a diagonal gradient in a non-square box keeps its
/// normalized angle.
struct CssLinearGradientLine {
    angle_degrees: f32,
    /// CSS position of IR offset 0.
    origin: f32,
    /// CSS positions advanced per unit of IR offset.
    scale: f32,
}

impl CssLinearGradientLine {
    fn new(start: (f32, f32), end: (f32, f32)) -> Self {
        let (vx, vy) = (end.0 - start.0, end.1 - start.1);
        let length = (vx * vx + vy * vy).sqrt();
        if length <= f32::EPSILON {
            return Self {
                angle_degrees: 180.0,
                origin: 0.0,
                scale: 1.0,
            };
        }
        // CSS measures from "up", clockwise; the IR's y grows downward.
        let angle = vx.atan2(-vy);
        let (dx, dy) = (vx / length, vy / length);
        let line_length = dx.abs() + dy.abs();
        // Distance along the direction from the box centre to the start point.
        let start_from_centre = (start.0 - 0.5) * dx + (start.1 - 0.5) * dy;
        Self {
            angle_degrees: angle.to_degrees().rem_euclid(360.0),
            origin: 0.5 + start_from_centre / line_length,
            scale: length / line_length,
        }
    }

    fn css_position(&self, offset: f32) -> f32 {
        self.origin + offset * self.scale
    }
}

/// A sweep gradient's IR angles expressed as CSS conic-gradient terms.
struct CssSweep {
    from_degrees: f32,
    span_degrees: f32,
}

/// Converts IR sweep angles, clockwise from the positive x axis in radians, to
/// CSS's `from` angle, clockwise from "up", and the swept span that stop
/// offsets are laid along. Both are in screen space, so this is exact for any
/// box. A zero or negative span sweeps the full circle.
fn css_sweep(start_angle: f32, end_angle: f32) -> CssSweep {
    let span = (end_angle - start_angle).to_degrees();
    CssSweep {
        from_degrees: (start_angle.to_degrees() + 90.0).rem_euclid(360.0),
        span_degrees: if span > 0.0 { span } else { 360.0 },
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod gradient_geometry_tests {
    use super::{css_sweep, CssLinearGradientLine};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn a_full_width_horizontal_gradient_spans_the_css_line() {
        let line = CssLinearGradientLine::new((0.0, 0.5), (1.0, 0.5));
        assert!(close(line.angle_degrees, 90.0));
        assert!(close(line.css_position(0.0), 0.0));
        assert!(close(line.css_position(1.0), 1.0));
    }

    #[test]
    fn an_inset_gradient_keeps_its_extent() {
        let line = CssLinearGradientLine::new((0.25, 0.5), (0.75, 0.5));
        assert!(close(line.css_position(0.0), 0.25));
        assert!(close(line.css_position(1.0), 0.75));
    }

    #[test]
    fn a_reversed_vertical_gradient_points_up() {
        let line = CssLinearGradientLine::new((0.5, 1.0), (0.5, 0.0));
        assert!(close(line.angle_degrees, 0.0));
        assert!(close(line.css_position(0.0), 0.0));
        assert!(close(line.css_position(1.0), 1.0));
    }

    #[test]
    fn a_corner_to_corner_diagonal_reaches_both_corners() {
        let line = CssLinearGradientLine::new((0.0, 0.0), (1.0, 1.0));
        assert!(close(line.angle_degrees, 135.0));
        assert!(close(line.css_position(0.0), 0.0));
        assert!(close(line.css_position(1.0), 1.0));
    }

    #[test]
    fn a_sweep_starting_at_positive_x_starts_css_at_three_o_clock() {
        let sweep = css_sweep(0.0, std::f32::consts::PI);
        assert!(close(sweep.from_degrees, 90.0));
        assert!(close(sweep.span_degrees, 180.0));
    }

    #[test]
    fn a_degenerate_sweep_covers_the_full_circle() {
        assert!(close(css_sweep(1.0, 1.0).span_degrees, 360.0));
    }
}
