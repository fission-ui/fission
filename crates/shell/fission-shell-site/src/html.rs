use anyhow::{anyhow, bail, Result};
use fission_ir::op::{
    decode_inline_widget_marker, AlignItems, BoxShadow, Color, Fill, FlexDirection, FlexWrap,
    FontStyle, GridPlacement, GridTrack, ImageFit, ImageSource, JustifyContent, LayoutOp, LineCap,
    LineJoin, Op, PaintOp, Stroke, TextAlign, TextOverflow, TextRun,
};
use fission_ir::{semantics::ActionTrigger, CoreIR, CoreNode, NodeId, Role};
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
    pub server_action_tokens: BTreeMap<(NodeId, u128), String>,
    pub browser_action_bindings: bool,
    pub structured_data: Vec<String>,
    pub head_start_html: Vec<String>,
    pub head_end_html: Vec<String>,
    pub body_start_html: Vec<String>,
    pub body_end_html: Vec<String>,
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
        metadata.push_str(json);
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
        out
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
                match semantics.role {
                    Role::TextInput
                    | Role::Checkbox
                    | Role::Switch
                    | Role::Slider
                    | Role::Input => {
                        bail!(
                            "static site renderer cannot lower interactive semantic role {:?} on node {}; provide static content or move this route to a web target",
                            semantics.role,
                            node.id
                        );
                    }
                    _ => {}
                }
            }
            Op::Layout(LayoutOp::Embed { kind, .. }) => {
                bail!(
                    "static site renderer cannot lower embedded surface {:?} on node {}; provide a static fallback widget for the site target",
                    kind,
                    node.id
                );
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
    fn render_node(&mut self, node_id: NodeId) -> Result<String> {
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

    fn render_children(&mut self, children: &[NodeId], skip: &HashSet<NodeId>) -> Result<String> {
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
        style.extend(composite_style(node));
        let children = self.render_children(&node.children, &skip)?;
        let class_name = self.class_name(class_name, style);
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
                self.render_element("div", node, "fission-site-node fission-site-grid-item", style)
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
            LayoutOp::Embed { .. } => unreachable!("embed ops are rejected before rendering"),
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
                self.render_element("div", node, "fission-site-node fission-site-positioned", style)
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
            LayoutOp::Flyout { .. } => bail!(
                "static site renderer cannot lower flyout layout on node {}; render open dialog content as normal page content for site output",
                node.id
            ),
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
                let mut style = vec![
                    "display:inline".to_string(),
                    format!("white-space:{}", if *wrap { "pre-wrap" } else { "pre" }),
                ];
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
            PaintOp::DrawImage { request, fit, .. } => {
                let class_name = self.class_name(
                    "fission-site-img",
                    vec![
                        "width:100%".to_string(),
                        "height:100%".to_string(),
                        format!("object-fit:{}", image_fit_css(*fit)),
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
                fill: _,
                stroke: _,
            } => Ok(format!(
                "<span class=\"fission-site-svg\" data-fission-node=\"{}\">{}</span>",
                node.id, content
            )),
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
                    "<button class=\"fission-site-node fission-site-theme-toggle\" type=\"button\" data-fission-theme-toggle data-fission-node=\"{}\">{children}</button>",
                    node.id
                ));
            }
            if identifier == "site-search-trigger" {
                let children = self.render_children(&node.children, &HashSet::new())?;
                return Ok(format!(
                    "<button class=\"fission-site-node fission-site-search-trigger\" type=\"button\" data-fission-search-trigger data-fission-node=\"{}\">{children}</button>",
                    node.id
                ));
            }
            if identifier == "site-sidebar-toggle" {
                let children = self.render_children(&node.children, &HashSet::new())?;
                return Ok(format!(
                    "<button class=\"fission-site-node fission-site-sidebar-toggle\" type=\"button\" aria-expanded=\"false\" data-fission-sidebar-toggle data-fission-node=\"{}\">{children}</button>",
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
            if identifier.starts_with("site-") {
                return self.render_site_semantic_wrapper(
                    node,
                    identifier,
                    semantics.label.as_deref(),
                );
            }
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
                unreachable!("interactive roles are rejected before rendering")
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

    fn semantic_payload_children(&self, node: &CoreNode) -> Vec<NodeId> {
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
            ImageSource::Memory { .. } | ImageSource::SvgText { .. } => String::new(),
        }
    }

    fn coalesced_paint_style(
        &self,
        node: &CoreNode,
        skip: &mut HashSet<NodeId>,
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

fn site_semantic_data_attrs(identifier: &str) -> String {
    let Some(rest) = identifier.strip_prefix("site-sidebar-item:") else {
        return String::new();
    };
    let mut parts = rest.split(':');
    let level = parts.next().unwrap_or("0");
    let active = parts.next().unwrap_or("false");
    let group = parts.next().unwrap_or("false");
    let index = parts.next().unwrap_or("0");
    format!(
        " data-fission-site-sidebar-level=\"{}\" data-fission-site-sidebar-active=\"{}\" data-fission-site-sidebar-group=\"{}\" data-fission-site-sidebar-index=\"{}\"",
        escape_attr(level),
        escape_attr(active),
        escape_attr(group),
        escape_attr(index)
    )
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

fn composite_style(node: &CoreNode) -> Vec<String> {
    let mut style = Vec::new();
    if let Some(opacity) = node.composite.opacity.as_ref() {
        style.push(format!("opacity:{}", opacity.base));
    }
    if node.composite.clip_to_bounds {
        style.push("overflow:hidden".to_string());
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
    if translate_x != 0.0
        || translate_y != 0.0
        || (scale - 1.0).abs() > f32::EPSILON
        || rotation != 0.0
    {
        style.push(format!(
            "transform:translate({}px,{}px) scale({}) rotate({}rad)",
            px(translate_x),
            px(translate_y),
            scale,
            rotation
        ));
    }
    style
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

fn image_fit_css(fit: ImageFit) -> &'static str {
    match fit {
        ImageFit::Contain => "contain",
        ImageFit::Cover => "cover",
        ImageFit::Fill => "fill",
        ImageFit::None => "none",
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
    use fission_ir::{ActionEntry, ActionSet, CoreIR, CoreNode, NodeId, Op, Semantics};

    #[test]
    fn renders_text_from_core_ir() {
        let root = NodeId::explicit("root");
        let text = NodeId::explicit("text");
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
        let root = NodeId::explicit("root");
        let image = NodeId::explicit("image");
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
        let root = NodeId::explicit("root");
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
        let root = NodeId::explicit("server-action");
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
    fn browser_action_options_render_client_binding_attributes() {
        let root = NodeId::explicit("browser-action");
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
