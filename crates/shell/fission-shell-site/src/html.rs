use anyhow::{anyhow, bail, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use fission_core::{
    registry::{VideoRegistration, WebRegistration},
    MotionDeclaration, MotionDeclarationKind, MotionEasing, MotionExpr, MotionPredicate,
    MotionPropertyId, MotionStartValue, MotionTrack, MotionTransition, MotionValue,
};
use fission_ir::op::{
    decode_inline_widget_marker, AlignItems, BoxShadow, Color, CompositeScalar, EmbedKind, Fill,
    FlexDirection, FlexWrap, FontStyle, GridPlacement, GridTrack, ImageAlignment, ImageFit,
    ImageSource, JustifyContent, LayoutOp, Length, LineCap, LineJoin, Op, Overflow, PaintOp,
    RichTextAnnotation, Stroke, TextAlign, TextOverflow, TextRun,
};
use fission_ir::{semantics::ActionTrigger, CoreIR, CoreNode, Role, Semantics, WidgetId};
use fission_theme::{DesignMode, PackagedFont, PackagedFontStyle, Theme};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug)]
pub struct HtmlRenderOptions {
    pub lang: String,
    pub document_title: String,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub site_name: Option<String>,
    pub favicon_href: Option<String>,
    pub stylesheet_href: String,
    pub root_class: String,
    pub current_route_path: String,
    pub css_variables: CssVariableMap,
    pub default_theme_mode: Option<DesignMode>,
    pub theme_switching: bool,
    pub code_highlighting: CodeHighlightingOptions,
    pub search_script_href: Option<String>,
    pub server_action_post_path: Option<String>,
    pub server_action_tokens: BTreeMap<(WidgetId, u128), String>,
    pub browser_action_bindings: bool,
    pub structured_data: Vec<String>,
    pub head_start_html: Vec<String>,
    pub head_end_html: Vec<String>,
    pub body_start_html: Vec<String>,
    pub body_end_html: Vec<String>,
    pub motion_declarations: Vec<MotionDeclaration>,
    pub video_registrations: BTreeMap<WidgetId, VideoRegistration>,
    pub web_registrations: BTreeMap<WidgetId, WebRegistration>,
    /// Font faces embedded by the selected design system.
    pub font_faces: &'static [PackagedFont],
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
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeHighlightingOptions {
    pub enabled: bool,
    pub stylesheet_href: String,
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
pub struct RenderedHtml {
    pub html: String,
    pub body_html: String,
    pub css: String,
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

pub fn render_ir_to_html(ir: &CoreIR, options: &HtmlRenderOptions) -> Result<RenderedHtml> {
    let mut registry = StyleRegistry::default();
    render_ir_to_html_with_styles(ir, options, &mut registry)
}

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
    for font in options.font_faces {
        styles.raw_rule(
            format!(
                "fission-font-{}-{}-{:?}",
                font.family, font.weight, font.style
            ),
            packaged_font_css(font),
        );
    }
    let mut renderer = HtmlRenderer {
        ir,
        options,
        styles,
        has_code_blocks: false,
    };
    renderer.register_interaction_motion_styles();
    let body = renderer.render_node(root)?;
    let has_code_blocks = renderer.has_code_blocks;
    let body_html = format!(
        "<div class=\"{}\">{body}</div>",
        escape_attr(&options.root_class)
    );
    let html = render_document(&body_html, options, has_code_blocks);
    Ok(RenderedHtml {
        html,
        body_html,
        css: renderer.styles.to_css(),
    })
}

fn packaged_font_css(font: &PackagedFont) -> String {
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
    format!(
        "@font-face{{font-family:'{}';font-style:{style};font-weight:{};font-display:swap;{axes}src:url(data:font/{};base64,{}) format('{}')}}",
        escape_attr(font.family),
        font.weight,
        font_mime(font.format),
        BASE64_STANDARD.encode(font.data),
        escape_attr(font.format),
    )
}

fn font_mime(format: &str) -> &'static str {
    match format.to_ascii_lowercase().as_str() {
        "woff2" => "woff2",
        "woff" => "woff",
        "opentype" | "otf" => "otf",
        "truetype" | "ttf" => "ttf",
        _ => "octet-stream",
    }
}

fn render_document(body_html: &str, options: &HtmlRenderOptions, has_code_blocks: bool) -> String {
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
    format!(
        "<!doctype html>\n<html lang=\"{}\"{theme_attr}>\n  <head>{head_start_html}\n    <meta charset=\"utf-8\">\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">{metadata}\n    <title>{}</title>\n    <link rel=\"stylesheet\" href=\"{}\">{code_highlighting_assets}{search_script}{enhancement_script}{head_end_html}\n  </head>\n  <body>{body_start_html}\n    {body_html}{body_end_html}\n  </body>\n</html>\n",
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
pub struct CssVariableMap {
    color_vars: Vec<(Color, &'static str)>,
    font_vars: Vec<(String, &'static str)>,
}

impl CssVariableMap {
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
pub struct StyleRegistry {
    style_to_class: BTreeMap<String, String>,
    class_to_style: BTreeMap<String, String>,
    raw_rules: BTreeMap<String, String>,
}

impl StyleRegistry {
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

    pub fn raw_rule(&mut self, key: impl Into<String>, rule: impl Into<String>) {
        self.raw_rules.insert(key.into(), rule.into());
    }
}

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
        match &node.op {
            Op::Semantics(semantics) => {
                if !semantics.actions.entries.is_empty() && !allow_server_actions {
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

mod renderer;
use renderer::{HtmlRenderer, InteractionPseudo};

mod renderer_semantics;

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
            align_items,
            justify_content,
        } => Some(flex_layout_style(
            *direction,
            *wrap,
            *flex_grow,
            *flex_shrink,
            *padding,
            *gap,
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
        style.push(format!("gap:{}px", px(gap)));
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

fn is_native_control_role(role: Role) -> bool {
    matches!(
        role,
        Role::TextInput | Role::Checkbox | Role::Radio | Role::Switch | Role::Slider | Role::Input
    )
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

#[cfg(test)]
#[path = "html_tests.rs"]
mod tests;
