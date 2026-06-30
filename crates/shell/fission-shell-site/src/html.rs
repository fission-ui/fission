use anyhow::{anyhow, bail, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use fission_core::{
    registry::{VideoRegistration, WebRegistration},
    MotionDeclaration, MotionDeclarationKind, MotionEasing, MotionExpr, MotionPropertyId,
    MotionStartValue, MotionTrack, MotionTransition, MotionValue,
};
use fission_ir::op::{
    decode_inline_widget_marker, AlignItems, BoxShadow, Color, CompositeScalar, EmbedKind, Fill,
    FlexDirection, FlexWrap, FontStyle, GridPlacement, GridTrack, ImageAlignment, ImageFit,
    ImageSource, JustifyContent, LayoutOp, LineCap, LineJoin, Op, PaintOp, Stroke, TextAlign,
    TextOverflow, TextRun,
};
use fission_ir::{semantics::ActionTrigger, CoreIR, CoreNode, Role, Semantics, WidgetId};
use fission_theme::{DesignMode, Theme};
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
    let mut renderer = HtmlRenderer {
        ir,
        options,
        styles,
        has_code_blocks: false,
    };
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

struct HtmlRenderer<'a> {
    ir: &'a CoreIR,
    options: &'a HtmlRenderOptions,
    styles: &'a mut StyleRegistry,
    has_code_blocks: bool,
}

impl HtmlRenderer<'_> {
    fn render_node(&mut self, node_id: WidgetId) -> Result<String> {
        let node = self
            .ir
            .nodes
            .get(&node_id)
            .ok_or_else(|| anyhow!("site render failed: missing IR node {node_id}"))?;
        match &node.op {
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
        mut style: Vec<String>,
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
            "<{tag} class=\"{}\" data-fission-node=\"{}\">{children}</{tag}>",
            escape_attr(&class_name),
            node.id
        ))
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

        if !animations.is_empty() {
            style.push(format!("animation:{}", animations.join(",")));
            self.styles.raw_rule(
                "fission-site-reduced-motion-animations",
                "@media (prefers-reduced-motion:reduce){.fission-site-animated{animation:none!important;}}\n",
            );
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
            self.styles.raw_rule(
                "fission-site-reduced-motion-animations",
                "@media (prefers-reduced-motion:reduce){.fission-site-animated{animation:none!important;}}\n",
            );
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
            LayoutOp::Flex {
                direction,
                wrap,
                flex_grow,
                flex_shrink,
                padding,
                gap,
                align_items,
                justify_content,
            } => {
                let mut style = vec![
                    "display:flex".to_string(),
                    format!("flex-direction:{}", flex_direction(*direction)),
                    format!("flex-wrap:{}", flex_wrap(*wrap)),
                    format!("align-items:{}", align_items_css(*align_items)),
                    format!("justify-content:{}", justify_content_css(*justify_content)),
                ];
                if let Some(gap) = gap {
                    style.push(format!("gap:{}px", px(*gap)));
                }
                push_padding(&mut style, *padding);
                push_flex_item(&mut style, *flex_grow, *flex_shrink);
                let class_name = match direction {
                    FlexDirection::Column => "fission-site-node fission-site-column",
                    FlexDirection::Row => "fission-site-node fission-site-row",
                };
                self.render_element("div", node, class_name, style)
            }
            LayoutOp::Grid {
                columns,
                rows,
                column_gap,
                row_gap,
                padding,
            } => {
                let mut style = vec!["display:grid".to_string()];
                if !columns.is_empty() {
                    style.push(format!("grid-template-columns:{}", grid_tracks(columns)));
                }
                if !rows.is_empty() {
                    style.push(format!("grid-template-rows:{}", grid_tracks(rows)));
                }
                if let Some(gap) = column_gap {
                    style.push(format!("column-gap:{}px", px(*gap)));
                }
                if let Some(gap) = row_gap {
                    style.push(format!("row-gap:{}px", px(*gap)));
                }
                push_padding(&mut style, *padding);
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
                    "align-items:center".to_string(),
                    "justify-content:center".to_string(),
                ],
            ),
            LayoutOp::Flyout { anchor, content } => {
                let children = self.render_children(&node.children, &HashSet::new())?;
                let class_name = self.class_name(
                    "fission-site-node fission-site-flyout",
                    vec![
                        "position:absolute".to_string(),
                        "z-index:1000".to_string(),
                        "inset:auto".to_string(),
                    ],
                );
                Ok(format!(
                    "<div class=\"{}\" data-fission-flyout-anchor=\"{}\" data-fission-flyout-content=\"{}\" data-fission-node=\"{}\">{children}</div>",
                    escape_attr(&class_name),
                    anchor,
                    content,
                    node.id
                ))
            }
            LayoutOp::Transform { transform } => self.render_element(
                "div",
                node,
                "fission-site-node fission-site-transform",
                vec![format!("transform:matrix3d({})", matrix3d(transform))],
            ),
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
                    Ok(format!(
                        "<iframe class=\"{}\" src=\"{}\" title=\"{}\" loading=\"lazy\" referrerpolicy=\"strict-origin-when-cross-origin\" data-fission-web-view=\"{}\" data-fission-node=\"{}\"></iframe>",
                        escape_attr(&class_name),
                        escape_attr(&web.url),
                        "Embedded web content",
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
            PaintOp::DrawRect {
                fill,
                stroke,
                corner_radius,
                shadow,
            } => {
                let mut style = self.draw_rect_style(
                    fill.as_ref(),
                    stroke.as_ref(),
                    *corner_radius,
                    shadow.as_ref(),
                );
                style.push("min-height:1px".to_string());
                self.render_element("div", node, "fission-site-node fission-site-rect", style)
            }
            PaintOp::DrawText {
                text,
                size,
                color,
                underline,
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
                Ok(format!(
                    "<span class=\"{}\" data-fission-node=\"{}\">{}</span>",
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
                let mut content = String::new();
                for run in runs {
                    if decode_inline_widget_marker(run.style.font_family.as_deref()).is_some() {
                        continue;
                    }
                    content.push_str(&self.render_text_run(run));
                }
                content.push_str(&self.render_children(&node.children, &HashSet::new())?);
                let class_name = self.class_name("fission-site-rich-text", style);
                Ok(format!(
                    "<span class=\"{}\" data-fission-node=\"{}\">{content}</span>",
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
            PaintOp::DrawPath { path, fill, stroke } => {
                let path_class = self.class_name(
                    "fission-site-svg-path",
                    self.svg_paint_style(fill.as_ref(), stroke.as_ref()),
                );
                Ok(format!(
                    "<svg class=\"fission-site-svg\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" data-fission-node=\"{}\"><path class=\"{}\" d=\"{}\"></path></svg>",
                    node.id,
                    escape_attr(&path_class),
                    escape_attr(path)
                ))
            }
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
        if let Some(identifier) = semantics.identifier.as_deref() {
            if let Some(target) = identifier.strip_prefix("site-route:") {
                return self.render_semantic_link(
                    node,
                    target,
                    semantics.label.as_deref(),
                    "fission-site-route-link",
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
                return self.render_semantic_link(
                    node,
                    target,
                    semantics.label.as_deref(),
                    "fission-site-markdown-link",
                );
            }
            if identifier == "site-theme-toggle" {
                let children = self.render_children(&node.children, &HashSet::new())?;
                return Ok(format!(
                    "<button class=\"fission-site-node fission-site-theme-toggle\" type=\"button\" aria-label=\"Toggle color theme\" data-fission-theme-toggle data-fission-node=\"{}\">{children}</button>",
                    node.id
                ));
            }
            if identifier == "site-search-trigger" {
                let children = self.render_children(&node.children, &HashSet::new())?;
                return Ok(format!(
                    "<button class=\"fission-site-node fission-site-search-trigger\" type=\"button\" aria-label=\"Search documentation\" data-fission-search-trigger data-fission-node=\"{}\">{children}</button>",
                    node.id
                ));
            }
            if identifier == "site-sidebar-toggle" {
                let children = self.render_children(&node.children, &HashSet::new())?;
                return Ok(format!(
                    "<button class=\"fission-site-node fission-site-sidebar-toggle\" type=\"button\" aria-label=\"Open documentation navigation\" aria-expanded=\"false\" data-fission-sidebar-toggle data-fission-node=\"{}\">{children}</button>",
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
        if is_native_control_role(semantics.role) {
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
            Role::Image => "figure",
            Role::List => "ul",
            Role::ListItem => "li",
            Role::Dialog => "section",
            Role::Text | Role::Generic => "div",
            Role::TextInput | Role::Checkbox | Role::Switch | Role::Slider | Role::Input => {
                unreachable!("interactive controls are rendered before generic semantics")
            }
        };
        let tag = semantics
            .identifier
            .as_deref()
            .and_then(markdown_heading_tag)
            .unwrap_or(tag);
        let mut attrs = String::new();
        if let Some(label) = &semantics.label {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        if let Some(identifier) = &semantics.identifier {
            attrs.push_str(&format!(
                " data-fission-semantics=\"{}\"",
                escape_attr(identifier)
            ));
            if let Some(anchor) = markdown_heading_anchor(identifier) {
                attrs.push_str(&format!(" id=\"{}\"", escape_attr(anchor)));
            }
        }
        if tag == "button" {
            attrs.push_str(" type=\"button\" disabled");
        }
        let children = self.render_children(&node.children, &HashSet::new())?;
        Ok(format!(
            "<{tag} class=\"fission-site-node fission-site-semantics\"{attrs} data-fission-node=\"{}\">{children}</{tag}>",
            node.id
        ))
    }

    fn render_native_control_semantics(
        &mut self,
        node: &CoreNode,
        semantics: &Semantics,
    ) -> Result<String> {
        let mut attrs = self.native_control_attrs(node, semantics);
        let children = self.render_children(&node.children, &HashSet::new())?;
        let label_text = semantics.label.as_deref().unwrap_or_default();
        match semantics.role {
            Role::TextInput | Role::Input if semantics.multiline => {
                let value = semantics.value.as_deref().unwrap_or_default();
                Ok(format!(
                    "<label class=\"fission-site-node fission-site-control\" data-fission-node=\"{}\"><span class=\"fission-site-control-label\">{}</span><textarea class=\"fission-site-input\"{attrs}>{}</textarea>{children}</label>",
                    node.id,
                    escape_text(label_text),
                    escape_text(value)
                ))
            }
            Role::TextInput | Role::Input => {
                attrs.push_str(&format!(
                    " type=\"{}\" value=\"{}\"",
                    html_text_input_type(semantics),
                    escape_attr(semantics.value.as_deref().unwrap_or_default())
                ));
                Ok(format!(
                    "<label class=\"fission-site-node fission-site-control\" data-fission-node=\"{}\"><span class=\"fission-site-control-label\">{}</span><input class=\"fission-site-input\"{attrs}>{children}</label>",
                    node.id,
                    escape_text(label_text)
                ))
            }
            Role::Checkbox | Role::Switch => {
                if semantics.role == Role::Switch {
                    attrs.push_str(" role=\"switch\"");
                }
                if semantics.checked.unwrap_or(false) {
                    attrs.push_str(" checked");
                }
                Ok(format!(
                    "<label class=\"fission-site-node fission-site-control\" data-fission-node=\"{}\"><input class=\"fission-site-checkbox\" type=\"checkbox\"{attrs}><span class=\"fission-site-control-label\">{}</span>{children}</label>",
                    node.id,
                    escape_text(label_text)
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
        let mut attrs = format!(" data-fission-node=\"{}\"", node.id);
        if let Some(label) = &semantics.label {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        if let Some(identifier) = &semantics.identifier {
            attrs.push_str(&format!(
                " data-fission-semantics=\"{}\" name=\"{}\"",
                escape_attr(identifier),
                escape_attr(identifier)
            ));
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
        if let Some(max_length) = semantics.max_length {
            attrs.push_str(&format!(" maxlength=\"{max_length}\""));
        }
        attrs
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
        let mut attrs = String::new();
        if let Some(label) = &semantics.label {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        if let Some(identifier) = &semantics.identifier {
            attrs.push_str(&format!(
                " data-fission-semantics=\"{}\"",
                escape_attr(identifier)
            ));
        }
        Ok(Some(format!(
            "<form class=\"fission-site-node fission-server-action-form\" method=\"post\" action=\"{}\" data-fission-node=\"{}\"><input type=\"hidden\" name=\"token\" value=\"{}\"><button class=\"fission-site-node fission-site-semantics fission-server-action\" type=\"submit\"{attrs}>{children}</button></form>",
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
        let Some(action) = semantics
            .actions
            .entries
            .iter()
            .find(|entry| entry.trigger == ActionTrigger::Default)
        else {
            return Ok(None);
        };
        let Some(payload) = action.payload_data.as_ref() else {
            return Ok(None);
        };
        let children = self.render_children(&node.children, &HashSet::new())?;
        let mut attrs = String::new();
        if let Some(label) = &semantics.label {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        if let Some(identifier) = &semantics.identifier {
            attrs.push_str(&format!(
                " data-fission-semantics=\"{}\"",
                escape_attr(identifier)
            ));
        }
        attrs.push_str(" role=\"button\"");
        attrs.push_str(&format!(
            " data-fission-browser-action=\"true\" data-fission-action-id=\"{}\" data-fission-action-target=\"{}\" data-fission-action-payload=\"{}\"",
            action.action_id,
            node.id.as_u128(),
            hex_encode(payload)
        ));
        Ok(Some(format!(
            "<button class=\"fission-site-node fission-site-semantics fission-browser-action\" type=\"button\"{attrs} data-fission-node=\"{}\">{children}</button>",
            node.id
        )))
    }

    fn render_markdown_table(&mut self, node: &CoreNode) -> Result<String> {
        let children = self.render_semantic_payload_children(node)?;
        Ok(format!(
            "<div class=\"fission-site-markdown-table-wrap\" data-fission-node=\"{}\"><table class=\"fission-site-markdown-table\"><tbody>{children}</tbody></table></div>",
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
        attrs.push_str(&site_semantic_data_attrs(identifier));
        let children = self.render_children(&node.children, &HashSet::new())?;
        Ok(format!(
            "<div class=\"fission-site-node fission-site-semantics {class_name}\"{attrs} data-fission-node=\"{}\">{children}</div>",
            node.id
        ))
    }

    fn render_semantic_payload_children(&mut self, node: &CoreNode) -> Result<String> {
        let children = self.semantic_payload_children(node);
        self.render_children(&children, &HashSet::new())
    }

    fn semantic_payload_children(&self, node: &CoreNode) -> Vec<WidgetId> {
        if node.children.len() == 1 {
            if let Some(child) = self.ir.nodes.get(&node.children[0]) {
                match child.op {
                    Op::Layout(_) | Op::Structural(_) => return child.children.clone(),
                    _ => {}
                }
            }
        }
        node.children.clone()
    }

    fn render_semantic_link(
        &mut self,
        node: &CoreNode,
        target: &str,
        label: Option<&str>,
        link_class: &str,
    ) -> Result<String> {
        let mut attrs = format!(" href=\"{}\"", escape_attr(&self.resolve_link_href(target)));
        if let Some(label) = label {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        attrs.push_str(&format!(
            " data-fission-current-route=\"{}\"",
            escape_attr(&self.options.current_route_path)
        ));
        let children = self.render_children(&node.children, &HashSet::new())?;
        Ok(format!(
            "<a class=\"fission-site-node fission-site-link {link_class}\"{attrs} data-fission-node=\"{}\">{children}</a>",
            node.id
        ))
    }

    fn resolve_link_href(&self, target: &str) -> String {
        if target.starts_with('#')
            || target.starts_with("http://")
            || target.starts_with("https://")
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
        for child_id in &node.children {
            let Some(child) = self.ir.nodes.get(child_id) else {
                continue;
            };
            if let Op::Paint(PaintOp::DrawRect {
                fill,
                stroke,
                corner_radius,
                shadow,
            }) = &child.op
            {
                style.extend(self.draw_rect_style(
                    fill.as_ref(),
                    stroke.as_ref(),
                    *corner_radius,
                    shadow.as_ref(),
                ));
                skip.insert(*child_id);
            }
        }
        Ok(style)
    }

    fn draw_rect_style(
        &self,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
        corner_radius: f32,
        shadow: Option<&BoxShadow>,
    ) -> Vec<String> {
        let mut style = Vec::new();
        if let Some(fill) = fill {
            style.push(format!("background:{}", self.fill_css(fill)));
        }
        if let Some(stroke) = stroke {
            style.push(format!(
                "border:{}px solid {}",
                px(stroke.width),
                self.stroke_css(stroke)
            ));
        }
        if corner_radius > 0.0 {
            style.push(format!("border-radius:{}px", px(corner_radius)));
        }
        if let Some(shadow) = shadow {
            style.push(format!(
                "box-shadow:{}px {}px {}px {}",
                px(shadow.offset.0),
                px(shadow.offset.1),
                px(shadow.blur_radius),
                self.color_css(shadow.color)
            ));
        }
        style
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
        let mut style = vec![
            format!("font-size:{}px", px(run.style.font_size)),
            format!("color:{}", self.color_css(run.style.color)),
            format!("font-weight:{}", run.style.font_weight),
            format!("letter-spacing:{}px", px(run.style.letter_spacing)),
        ];
        if run.style.underline {
            style.push("text-decoration:underline".to_string());
        }
        if let Some(family) = &run.style.font_family {
            style.push(format!("font-family:{}", self.font_family_css(family)));
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
        let class_name = self.class_name("fission-site-text-run", style);
        format!(
            "<span class=\"{}\">{}</span>",
            escape_attr(&class_name),
            escape_text(&run.text)
        )
    }

    fn fill_css(&self, fill: &Fill) -> String {
        match fill {
            Fill::Solid(color) => self.color_css(*color),
            Fill::LinearGradient {
                start: _,
                end,
                stops,
            } => {
                let angle = if end.0.abs() >= end.1.abs() {
                    "90deg"
                } else {
                    "180deg"
                };
                let stops = stops
                    .iter()
                    .map(|(offset, color)| {
                        format!("{} {}%", self.color_css(*color), (offset * 100.0).round())
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("linear-gradient({angle},{stops})")
            }
            Fill::RadialGradient { stops, .. } => {
                let stops = stops
                    .iter()
                    .map(|(offset, color)| {
                        format!("{} {}%", self.color_css(*color), (offset * 100.0).round())
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("radial-gradient(circle,{stops})")
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
    tracks
        .iter()
        .map(|track| match track {
            GridTrack::Points(value) => format!("{}px", px(*value)),
            GridTrack::Percent(value) => format!("{}%", px(*value)),
            GridTrack::Fr(value) => format!("{}fr", px(*value)),
            GridTrack::Auto => "auto".to_string(),
            GridTrack::MinContent => "min-content".to_string(),
            GridTrack::MaxContent => "max-content".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
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
        Role::TextInput | Role::Checkbox | Role::Switch | Role::Slider | Role::Input
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
mod tests {
    use super::*;
    use fission_ir::{
        ActionEntry, ActionSet, CompositeScalar, CompositeStyle, CoreIR, CoreNode, Op, Semantics,
        WidgetId,
    };

    #[test]
    fn renders_text_from_core_ir() {
        let root = WidgetId::explicit("root");
        let text = WidgetId::explicit("text");
        let mut ir = CoreIR::new();
        ir.add_node(
            text,
            Op::Paint(PaintOp::DrawText {
                text: "Hello <site>".into(),
                size: 16.0,
                color: Color::BLACK,
                underline: false,
                wrap: true,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style: None,
            }),
            Vec::new(),
        );
        ir.add_node(
            root,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            vec![text],
        );
        ir.set_root(root);

        let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
        assert!(rendered.html.contains("Hello &lt;site&gt;"));
        assert!(!rendered.html.contains("style=\""));
        assert!(rendered.css.contains(".fs_"));
    }

    #[test]
    fn renders_typed_image_sources_to_img_elements() {
        let root = WidgetId::explicit("root");
        let image = WidgetId::explicit("image");
        let mut ir = CoreIR::new();
        ir.add_node(
            image,
            Op::Paint(PaintOp::DrawImage {
                request: fission_ir::op::ImageRequest {
                    source: ImageSource::Network {
                        url: "https://cdn.example.com/product.webp".into(),
                        headers: Vec::new(),
                        cache_policy: fission_ir::op::ImageCachePolicy::Default,
                    },
                    semantic_label: Some("Product photo".into()),
                    ..Default::default()
                },
                fit: ImageFit::Cover,
                alignment: fission_ir::op::ImageAlignment::Center,
            }),
            Vec::new(),
        );
        ir.add_node(
            root,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            vec![image],
        );
        ir.set_root(root);

        let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

        assert!(rendered
            .html
            .contains("src=\"https://cdn.example.com/product.webp\""));
        assert!(rendered.html.contains("alt=\"Product photo\""));
        assert!(rendered.css.contains("object-fit:cover"));
    }

    #[test]
    fn data_images_alignment_and_svg_dash_arrays_lower_to_html() {
        let root = WidgetId::explicit("root");
        let image = WidgetId::explicit("image");
        let path = WidgetId::explicit("path");
        let mut ir = CoreIR::new();
        ir.add_node(
            image,
            Op::Paint(PaintOp::DrawImage {
                request: fission_ir::op::ImageRequest {
                    source: ImageSource::SvgText {
                        content: "<svg viewBox=\"0 0 1 1\"></svg>".into(),
                    },
                    semantic_label: Some("Inline icon".into()),
                    ..Default::default()
                },
                fit: ImageFit::Contain,
                alignment: ImageAlignment::BottomEnd,
            }),
            Vec::new(),
        );
        ir.add_node(
            path,
            Op::Paint(PaintOp::DrawPath {
                path: "M0 0 L10 10".into(),
                fill: None,
                stroke: Some(Stroke {
                    fill: Fill::Solid(Color::BLACK),
                    width: 2.0,
                    dash_array: Some(vec![4.0, 2.0]),
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                }),
            }),
            Vec::new(),
        );
        ir.add_node(
            root,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            vec![image, path],
        );
        ir.set_root(root);

        let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

        assert!(rendered.html.contains("data:image/svg+xml;base64,"));
        assert!(rendered.css.contains("object-position:right bottom"));
        assert!(rendered.css.contains("stroke-dasharray:4 2"));
    }

    #[test]
    fn embeds_and_native_controls_lower_without_static_rejection() {
        let root = WidgetId::explicit("root");
        let video_node = WidgetId::explicit("video-node");
        let video_widget = WidgetId::explicit("video-widget");
        let input = WidgetId::explicit("search-input");
        let mut ir = CoreIR::new();
        ir.add_node(
            video_node,
            Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Video,
                widget_id: video_widget,
                width: Some(320.0),
                height: Some(180.0),
            }),
            Vec::new(),
        );
        ir.add_node(
            input,
            Op::Semantics(Semantics {
                role: Role::TextInput,
                label: Some("Search".into()),
                identifier: Some("search".into()),
                value: Some("fission".into()),
                ..Default::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            root,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            vec![video_node, input],
        );
        ir.set_root(root);
        let mut options = HtmlRenderOptions::default();
        options.video_registrations.insert(
            video_widget,
            VideoRegistration {
                node_id: video_widget,
                source: "/media/demo.mp4".into(),
                autoplay: true,
                loop_playback: true,
            },
        );

        let rendered = render_ir_to_html(&ir, &options).unwrap();

        assert!(rendered.html.contains("<video"));
        assert!(rendered.html.contains("src=\"media/demo.mp4\""));
        assert!(rendered.html.contains("autoplay muted"));
        assert!(rendered.html.contains("loop"));
        assert!(rendered.html.contains("<input"));
        assert!(rendered.html.contains("value=\"fission\""));
    }

    #[test]
    fn layout_and_paint_motion_lower_to_css_keyframes() {
        let root = WidgetId::explicit("root");
        let panel = WidgetId::explicit("motion-panel");
        let mut ir = CoreIR::new();
        ir.add_node(
            panel,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 7 }),
            Vec::new(),
        );
        ir.add_node(
            root,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            vec![panel],
        );
        ir.set_root(root);
        let options = HtmlRenderOptions {
            motion_declarations: vec![MotionDeclaration {
                id: panel,
                kind: MotionDeclarationKind::Tracks {
                    tracks: vec![
                        MotionTrack {
                            property: MotionPropertyId::Width,
                            phase: fission_core::MotionPhase::Layout,
                            from: MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Px(
                                0.0,
                            ))),
                            to: MotionExpr::Value(MotionValue::Px(240.0)),
                            transition: MotionTransition::tween(180, MotionEasing::EaseOut),
                        },
                        MotionTrack {
                            property: MotionPropertyId::BackgroundColor,
                            phase: fission_core::MotionPhase::Paint,
                            from: MotionStartValue::Explicit(MotionExpr::Value(
                                MotionValue::Color(Color::WHITE),
                            )),
                            to: MotionExpr::Value(MotionValue::Color(Color::BLACK)),
                            transition: MotionTransition::tween(180, MotionEasing::EaseOut),
                        },
                    ],
                },
            }],
            ..Default::default()
        };

        let rendered = render_ir_to_html(&ir, &options).unwrap();

        assert!(rendered.html.contains("fission-site-animated"));
        assert!(rendered.css.contains("width:0px"));
        assert!(rendered.css.contains("width:240px"));
        assert!(rendered.css.contains("background-color:#ffffff"));
        assert!(rendered.css.contains("background-color:#000000"));
        assert!(rendered.css.matches("@keyframes fission_anim_").count() >= 2);
    }

    #[test]
    fn style_registry_deduplicates_normalized_styles() {
        let mut styles = StyleRegistry::default();
        let first = styles
            .class_for(vec!["color:red".to_string(), "display:block".to_string()])
            .unwrap();
        let second = styles
            .class_for(vec!["display:block;".to_string(), "color:red".to_string()])
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(styles.to_css().matches(".fs_").count(), 1);
    }

    #[test]
    fn style_registry_keeps_last_declaration_for_duplicate_properties() {
        let mut styles = StyleRegistry::default();
        let class_name = styles
            .class_for(vec![
                "overflow:auto".to_string(),
                "display:block".to_string(),
                "overflow:hidden".to_string(),
            ])
            .unwrap();
        let css = styles.to_css();

        assert!(css.contains(&format!(".{class_name}")));
        assert!(css.contains("display:block;overflow:hidden"));
        assert!(!css.contains("overflow:auto"));
    }

    #[test]
    fn repeated_rotation_animation_lowers_to_css_keyframes() {
        let root = WidgetId::explicit("root");
        let spinner = WidgetId::explicit("spinner-node");
        let target = WidgetId::explicit("spinner-animation");
        let mut ir = CoreIR::new();
        ir.nodes.insert(
            spinner,
            CoreNode {
                id: spinner,
                op: Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 7 }),
                composite: CompositeStyle {
                    rotation: Some(CompositeScalar::new(0.0).motion(target)),
                    ..Default::default()
                },
                children: Vec::new(),
                parent: None,
                hash: 0,
            },
        );
        ir.add_node(
            root,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            vec![spinner],
        );
        ir.set_root(root);
        let options = HtmlRenderOptions {
            motion_declarations: vec![MotionDeclaration {
                id: target,
                kind: MotionDeclarationKind::Tracks {
                    tracks: vec![MotionTrack {
                        property: MotionPropertyId::Rotation,
                        phase: fission_core::MotionPhase::Composite,
                        from: MotionStartValue::Explicit(MotionExpr::Value(MotionValue::Deg(0.0))),
                        to: MotionExpr::Value(MotionValue::Deg(360.0)),
                        transition: MotionTransition::tween(7000, MotionEasing::Linear)
                            .repeat(true)
                            .delay_ms(120),
                    }],
                },
            }],
            ..Default::default()
        };

        let rendered = render_ir_to_html(&ir, &options).unwrap();

        assert!(rendered.html.contains("fission-site-animated"));
        assert!(rendered.css.contains("@keyframes fission_anim_"));
        assert!(rendered.css.contains("rotate:0deg"));
        assert!(rendered.css.contains("rotate:360deg"));
        assert!(rendered
            .css
            .contains("7000ms linear 120ms infinite normal both"));
        assert!(rendered.css.contains("prefers-reduced-motion:reduce"));
    }

    #[test]
    fn scale_and_opacity_animations_share_one_dom_node() {
        let root = WidgetId::explicit("root");
        let pulse = WidgetId::explicit("pulse-node");
        let target = WidgetId::explicit("pulse-animation");
        let mut ir = CoreIR::new();
        ir.nodes.insert(
            pulse,
            CoreNode {
                id: pulse,
                op: Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 7 }),
                composite: CompositeStyle {
                    opacity: Some(CompositeScalar::new(0.72).motion(target)),
                    scale: Some(CompositeScalar::new(0.92).motion(target)),
                    ..Default::default()
                },
                children: Vec::new(),
                parent: None,
                hash: 0,
            },
        );
        ir.add_node(
            root,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            vec![pulse],
        );
        ir.set_root(root);
        let options = HtmlRenderOptions {
            motion_declarations: vec![MotionDeclaration {
                id: target,
                kind: MotionDeclarationKind::Tracks {
                    tracks: vec![
                        MotionTrack {
                            property: MotionPropertyId::Opacity,
                            phase: fission_core::MotionPhase::Composite,
                            from: MotionStartValue::Explicit(MotionExpr::Value(
                                MotionValue::Scalar(0.72),
                            )),
                            to: MotionExpr::Value(MotionValue::Scalar(1.0)),
                            transition: MotionTransition::tween(1400, MotionEasing::EaseInOut)
                                .repeat(true),
                        },
                        MotionTrack {
                            property: MotionPropertyId::Scale,
                            phase: fission_core::MotionPhase::Composite,
                            from: MotionStartValue::Explicit(MotionExpr::Value(
                                MotionValue::Scalar(0.92),
                            )),
                            to: MotionExpr::Value(MotionValue::Scalar(1.08)),
                            transition: MotionTransition::tween(1400, MotionEasing::EaseInOut)
                                .repeat(true),
                        },
                    ],
                },
            }],
            ..Default::default()
        };

        let rendered = render_ir_to_html(&ir, &options).unwrap();

        assert!(rendered.css.contains("opacity:0.720"));
        assert!(rendered.css.contains("opacity:1"));
        assert!(rendered.css.contains("scale:0.920"));
        assert!(rendered.css.contains("scale:1.080"));
        assert!(rendered.css.matches("@keyframes fission_anim_").count() >= 2);
        assert!(rendered.css.contains(",fission_anim_"));
    }

    #[test]
    fn centered_rich_text_lowers_to_width_bearing_block() {
        let root = WidgetId::explicit("root");
        let text = WidgetId::explicit("centered-text");
        let mut ir = CoreIR::new();
        ir.add_node(
            text,
            Op::Paint(PaintOp::DrawRichText {
                runs: vec![TextRun {
                    text: "Centered\ncopy".into(),
                    style: fission_ir::op::TextStyle {
                        font_size: 24.0,
                        color: Color::WHITE,
                        underline: false,
                        font_family: None,
                        locale: None,
                        font_weight: 700,
                        font_style: FontStyle::Normal,
                        line_height: Some(28.0),
                        letter_spacing: 0.0,
                        background_color: None,
                    },
                }],
                wrap: true,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style: Some(fission_ir::op::TextParagraphStyle {
                    text_align: TextAlign::Center,
                    ..Default::default()
                }),
            }),
            Vec::new(),
        );
        ir.add_node(
            root,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            vec![text],
        );
        ir.set_root(root);

        let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

        assert!(rendered.css.contains("display:block"));
        assert!(rendered.css.contains("width:100%"));
        assert!(rendered.css.contains("text-align:center"));
        assert!(!rendered
            .css
            .contains("display:inline;white-space:pre-wrap;text-align:center"));
    }

    #[test]
    fn relative_hrefs_are_derived_from_current_route() {
        assert_eq!(
            relative_href_for_route("/docs/learn/quickstart/", "/reference/widgets/button/#api"),
            "../../../reference/widgets/button/#api"
        );
        assert_eq!(
            relative_href_for_route("/", "/docs/learn/overview/"),
            "docs/learn/overview/"
        );
        assert_eq!(
            relative_href_for_route("/docs/learn/quickstart/", "/"),
            "../../../"
        );
    }

    #[test]
    fn rejects_interactive_actions() {
        let root = WidgetId::explicit("root");
        let mut semantics = Semantics::default();
        semantics.actions = ActionSet {
            entries: vec![ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Default,
                action_id: 1,
                payload_data: None,
            }],
        };
        let mut ir = CoreIR::new();
        ir.nodes.insert(
            root,
            CoreNode {
                id: root,
                op: Op::Semantics(semantics),
                composite: Default::default(),
                children: Vec::new(),
                parent: None,
                hash: 0,
            },
        );
        ir.set_root(root);
        let error = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap_err();
        assert!(error.to_string().contains("interactive actions"));
    }

    #[test]
    fn server_action_options_render_signed_post_form() {
        let root = WidgetId::explicit("server-action");
        let mut semantics = Semantics {
            role: Role::Button,
            ..Default::default()
        };
        semantics.actions = ActionSet {
            entries: vec![ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Default,
                action_id: 7,
                payload_data: Some(vec![1, 2, 3]),
            }],
        };
        let mut ir = CoreIR::new();
        ir.nodes.insert(
            root,
            CoreNode {
                id: root,
                op: Op::Semantics(semantics),
                composite: Default::default(),
                children: Vec::new(),
                parent: None,
                hash: 0,
            },
        );
        ir.set_root(root);
        let mut options = HtmlRenderOptions {
            server_action_post_path: Some("/__fission/action".to_string()),
            ..Default::default()
        };
        options
            .server_action_tokens
            .insert((root, 7), "signed-token".to_string());

        let rendered = render_ir_to_html(&ir, &options).unwrap();
        assert!(rendered.html.contains("method=\"post\""));
        assert!(rendered.html.contains("action=\"/__fission/action\""));
        assert!(rendered.html.contains("name=\"token\""));
        assert!(rendered.html.contains("signed-token"));
    }

    #[test]
    fn site_form_semantics_render_native_post_form() {
        let root = WidgetId::explicit("static-form");
        let semantics = Semantics {
            identifier: Some("site-form:contact".to_string()),
            label: Some("Contact".to_string()),
            value: Some(
                r#"{
                    "action": "/contact/submit",
                    "method": "post",
                    "submitLabel": "Send",
                    "fields": [
                        {"kind": "email", "name": "email", "label": "Email", "required": true, "maxLength": 320},
                        {"kind": "textarea", "name": "message", "label": "Message", "rows": 4},
                        {"kind": "checkbox", "name": "agree", "label": "Agree"}
                    ]
                }"#
                .to_string(),
            ),
            ..Default::default()
        };
        let mut ir = CoreIR::new();
        ir.nodes.insert(
            root,
            CoreNode {
                id: root,
                op: Op::Semantics(semantics),
                composite: Default::default(),
                children: Vec::new(),
                parent: None,
                hash: 0,
            },
        );
        ir.set_root(root);

        let rendered = render_ir_to_html(
            &ir,
            &HtmlRenderOptions {
                current_route_path: "/contact/".to_string(),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(rendered.html.contains("<form"));
        assert!(rendered.html.contains("method=\"post\""));
        assert!(rendered.html.contains("action=\"../contact/submit\""));
        assert!(rendered
            .html
            .contains("data-fission-semantics=\"site-form:contact\""));
        assert!(rendered.html.contains("type=\"email\""));
        assert!(rendered.html.contains("name=\"message\""));
        assert!(rendered.html.contains("<textarea"));
        assert!(rendered.html.contains("type=\"checkbox\""));
        assert!(rendered.html.contains(">Send</button>"));
    }

    #[test]
    fn escape_script_data_is_case_insensitive() {
        let escaped = escape_script_data(
            "<script>{\"value\":\"</script\",\"alt\":\"</Script\",\"upper\":\"</SCRIPT\"}</script>",
        );
        assert_eq!(
            escaped,
            "<script>{\"value\":\"<\\/script\",\"alt\":\"<\\/script\",\"upper\":\"<\\/script\"}<\\/script>",
        );
        assert_eq!(escaped.matches("<\\/script").count(), 4);
    }

    #[test]
    fn browser_action_options_render_client_binding_attributes() {
        let root = WidgetId::explicit("browser-action");
        let mut semantics = Semantics {
            role: Role::Button,
            ..Default::default()
        };
        semantics.actions = ActionSet {
            entries: vec![ActionEntry {
                trigger: fission_ir::semantics::ActionTrigger::Default,
                action_id: 9,
                payload_data: Some(vec![0xde, 0xad]),
            }],
        };
        let mut ir = CoreIR::new();
        ir.nodes.insert(
            root,
            CoreNode {
                id: root,
                op: Op::Semantics(semantics),
                composite: Default::default(),
                children: Vec::new(),
                parent: None,
                hash: 0,
            },
        );
        ir.set_root(root);
        let options = HtmlRenderOptions {
            browser_action_bindings: true,
            ..Default::default()
        };

        let rendered = render_ir_to_html(&ir, &options).unwrap();

        assert!(rendered
            .html
            .contains("data-fission-browser-action=\"true\""));
        assert!(rendered.html.contains("data-fission-action-id=\"9\""));
        assert!(rendered
            .html
            .contains("data-fission-action-payload=\"dead\""));
    }
}
