use crate::stack::VStack;
use fission_core::op::Color;
use fission_core::ui::{
    Column, Container, Image, Node, RichText, RichTextRun, Row, Scroll, Text, TextFontStyle,
};
use fission_core::{BuildCtx, FlexDirection, View, Widget};
use fission_ir::op::{AlignItems, ImageFit};
use fission_ir::{Role, Semantics};
use rushdown::ast::{
    Arena, CodeBlock, CodeSpan, Heading, HtmlBlock, Image as MarkdownImage, KindData, Link,
    NodeRef, TableCellAlignment, Text as MarkdownText, TextQualifier,
};
use rushdown::parser::{self, Parser};
use rushdown::text::BasicReader;
use serde::{Deserialize, Serialize};

/// Parses Markdown and renders it with native Fission nodes.
///
/// This widget intentionally does not render Markdown to HTML or host a WebView.
/// The rushdown AST is converted directly into Text, RichText, layout, and
/// Container nodes so it stays in the Fission rendering pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkdownViewer {
    pub markdown: String,
    pub show_scrollbar: bool,
}

impl Default for MarkdownViewer {
    fn default() -> Self {
        Self {
            markdown: String::new(),
            show_scrollbar: true,
        }
    }
}

impl MarkdownViewer {
    pub fn new(markdown: impl Into<String>) -> Self {
        Self {
            markdown: markdown.into(),
            ..Default::default()
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for MarkdownViewer {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let parser = Parser::with_extensions(
            parser::Options::default(),
            parser::gfm(parser::GfmOptions::default()),
        );
        let mut reader = BasicReader::new(&self.markdown);
        let (arena, document_ref) = parser.parse(&mut reader);
        let renderer = MarkdownRenderer::new(&self.markdown, &arena, view);

        Scroll {
            child: Some(Box::new(renderer.document(document_ref))),
            direction: FlexDirection::Column,
            show_scrollbar: self.show_scrollbar,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node()
    }
}

#[derive(Clone, Copy)]
struct MarkdownPalette {
    text_primary: Color,
    text_secondary: Color,
    text_link: Color,
    border: Color,
    surface: Color,
    surface_raised: Color,
    primary_subtle: Color,
}

#[derive(Clone)]
struct InlineStyle {
    font_size: f32,
    color: Color,
    underline: bool,
    font_family: Option<String>,
    font_weight: Option<u16>,
    font_style: TextFontStyle,
    background_color: Option<Color>,
    link_destination: Option<String>,
}

struct MarkdownRenderer<'a> {
    source: &'a str,
    arena: &'a Arena,
    palette: MarkdownPalette,
    body_size: f32,
    line_height: f32,
    heading_family: String,
    heading_sizes: [f32; 6],
    heading_line_height: f32,
    image_width: f32,
    image_height: f32,
    code_family: String,
}

impl<'a> MarkdownRenderer<'a> {
    fn new<S: fission_core::AppState>(source: &'a str, arena: &'a Arena, view: &View<S>) -> Self {
        let tokens = &view.env.theme.tokens;
        Self {
            source,
            arena,
            palette: MarkdownPalette {
                text_primary: tokens.colors.text_primary,
                text_secondary: tokens.colors.text_secondary,
                text_link: tokens.colors.text_link,
                border: tokens.colors.border,
                surface: tokens.colors.surface,
                surface_raised: tokens.colors.surface_raised,
                primary_subtle: tokens.colors.primary_subtle,
            },
            body_size: tokens.typography.body_medium_size,
            line_height: tokens.typography.body_medium_size * tokens.typography.line_height_normal,
            heading_family: tokens.typography.font_family_serif.clone(),
            heading_sizes: [
                tokens.typography.heading1_size,
                tokens.typography.heading2_size,
                tokens.typography.heading_size,
                tokens.typography.font_size_xl,
                tokens.typography.font_size_lg,
                tokens.typography.font_size_base,
            ],
            heading_line_height: tokens.typography.line_height_heading,
            image_width: tokens.spacing.xxxxl * 8.0,
            image_height: tokens.spacing.xxxxl * 6.0,
            code_family: tokens.typography.font_family_mono.clone(),
        }
    }

    fn document(&self, document_ref: NodeRef) -> Node {
        VStack {
            spacing: Some(12.0),
            children: self.children_as_blocks(document_ref),
        }
        .into_node()
    }

    fn children_as_blocks(&self, node_ref: NodeRef) -> Vec<Node> {
        self.arena[node_ref]
            .children(self.arena)
            .filter_map(|child_ref| self.block(child_ref))
            .collect()
    }

    fn block(&self, node_ref: NodeRef) -> Option<Node> {
        match self.arena[node_ref].kind_data() {
            KindData::Document(_) => Some(self.document(node_ref)),
            KindData::Paragraph(_) => Some(self.paragraph(node_ref, self.body_size)),
            KindData::Heading(heading) => Some(self.heading(node_ref, heading)),
            KindData::ThematicBreak(_) => Some(self.divider()),
            KindData::CodeBlock(code) => Some(self.code_block(code)),
            KindData::Blockquote(_) => Some(self.blockquote(node_ref)),
            KindData::List(list) => Some(self.list(node_ref, list.is_ordered(), list.start())),
            KindData::Table(_) => Some(self.table(node_ref)),
            KindData::TableHeader(_) | KindData::TableBody(_) | KindData::TableRow(_) => {
                Some(self.readable_plain_block(node_ref))
            }
            KindData::TableCell(_) => Some(self.paragraph(node_ref, self.body_size)),
            KindData::HtmlBlock(html) => Some(self.html_block(html)),
            KindData::LinkReferenceDefinition(_) => None,
            _ => {
                let text = self.plain_text(node_ref);
                if text.trim().is_empty() {
                    None
                } else {
                    Some(self.text_node(text, self.body_size, self.palette.text_primary))
                }
            }
        }
    }

    fn heading(&self, node_ref: NodeRef, heading: &Heading) -> Node {
        let level = heading.level().clamp(1, 6);
        let size = self.heading_sizes[(level - 1) as usize];

        let mut style = self.inline_style(size);
        style.font_weight = Some(700);
        style.font_family = Some(self.heading_family.clone());

        RichText::new(self.inline_runs(node_ref, style))
            .strut_line_height(size * self.heading_line_height)
            .semantics_identifier(format!(
                "markdown-heading-{level}:{}",
                markdown_anchor(&self.plain_text(node_ref))
            ))
            .into_node()
    }

    fn paragraph(&self, node_ref: NodeRef, font_size: f32) -> Node {
        if let Some((image_ref, image)) = self.single_image(node_ref) {
            return self.image_block(image_ref, image);
        }
        let runs = self.inline_runs(node_ref, self.inline_style(font_size));
        if runs.is_empty() {
            self.text_node(
                self.plain_text(node_ref),
                font_size,
                self.palette.text_primary,
            )
        } else {
            RichText::new(runs).into_node()
        }
    }

    fn single_image(&self, node_ref: NodeRef) -> Option<(NodeRef, &MarkdownImage)> {
        let mut children = self.arena[node_ref].children(self.arena);
        let child_ref = children.next()?;
        if children.next().is_some() {
            return None;
        }
        match self.arena[child_ref].kind_data() {
            KindData::Image(image) => Some((child_ref, image)),
            _ => None,
        }
    }

    fn image_block(&self, _node_ref: NodeRef, image: &MarkdownImage) -> Node {
        let source = image.destination_str(self.source).to_string();
        Image {
            source,
            width: Some(self.image_width),
            height: Some(self.image_height),
            fit: Some(ImageFit::Contain),
            ..Default::default()
        }
        .into_node()
    }

    fn code_block(&self, code: &CodeBlock) -> Node {
        let text = code
            .value()
            .iter(self.source)
            .fold(String::new(), |mut out, line| {
                out.push_str(line.as_ref());
                out
            });
        let language = markdown_code_language(code.language_str(self.source).unwrap_or(""));
        let mut children = Vec::new();
        if !language.is_empty() {
            children.push(
                Text::new(language.to_string())
                    .size(11.0)
                    .color(self.palette.text_secondary)
                    .weight(600)
                    .into_node(),
            );
        }
        children.push(
            Text::new(text.clone())
                .size(13.0)
                .line_height(18.0)
                .family(self.code_family.clone())
                .color(self.palette.text_primary)
                .into_node(),
        );

        let code_content = Container::new(
            VStack {
                spacing: Some(6.0),
                children,
            }
            .into_node(),
        )
        .bg(self.palette.surface_raised)
        .border(self.palette.border.with_alpha(130), 1.0)
        .border_radius(8.0)
        .padding_all(12.0)
        .into_node();

        Column {
            children: vec![code_content],
            semantics: Some(markdown_code_semantics(language, text)),
            ..Default::default()
        }
        .into_node()
    }

    fn html_block(&self, html: &HtmlBlock) -> Node {
        let text = html
            .value()
            .iter(self.source)
            .fold(String::new(), |mut out, line| {
                out.push_str(line.as_ref());
                out
            });
        self.text_node(text, self.body_size, self.palette.text_secondary)
    }

    fn blockquote(&self, node_ref: NodeRef) -> Node {
        let content = VStack {
            spacing: Some(8.0),
            children: self.children_as_blocks(node_ref),
        }
        .into_node();

        Container::new(content)
            .bg(self.palette.primary_subtle.with_alpha(40))
            .border(self.palette.border.with_alpha(160), 1.0)
            .border_radius(8.0)
            .padding([14.0, 14.0, 10.0, 10.0])
            .into_node()
    }

    fn list(&self, node_ref: NodeRef, ordered: bool, start: u32) -> Node {
        let start = if ordered { start.max(1) } else { 0 };
        let children = self.arena[node_ref]
            .children(self.arena)
            .enumerate()
            .filter_map(|(index, item_ref)| {
                if !matches!(self.arena[item_ref].kind_data(), KindData::ListItem(_)) {
                    return self.block(item_ref);
                }
                Some(self.list_item(item_ref, ordered, start + index as u32))
            })
            .collect();

        VStack {
            spacing: Some(6.0),
            children,
        }
        .into_node()
    }

    fn list_item(&self, item_ref: NodeRef, ordered: bool, number: u32) -> Node {
        let marker = if ordered {
            format!("{number}.")
        } else {
            String::from("•")
        };

        let content = self.list_item_content(item_ref);
        Row {
            children: vec![
                Container::new(
                    Text::new(marker)
                        .size(self.body_size)
                        .color(self.palette.text_secondary)
                        .into_node(),
                )
                .width(if ordered { 30.0 } else { 18.0 })
                .into_node(),
                Container::new(content).flex_grow(1.0).into_node(),
            ],
            gap: Some(6.0),
            align_items: AlignItems::Start,
            ..Default::default()
        }
        .into_node()
    }

    fn list_item_content(&self, item_ref: NodeRef) -> Node {
        let mut blocks = self.children_as_blocks(item_ref);
        if blocks.len() == 1 {
            blocks.remove(0)
        } else {
            VStack {
                spacing: Some(8.0),
                children: blocks,
            }
            .into_node()
        }
    }

    fn table(&self, table_ref: NodeRef) -> Node {
        let mut rows = Vec::new();
        for section_ref in self.arena[table_ref].children(self.arena) {
            let is_header = matches!(
                self.arena[section_ref].kind_data(),
                KindData::TableHeader(_)
            );
            for row_ref in self.arena[section_ref].children(self.arena) {
                if matches!(self.arena[row_ref].kind_data(), KindData::TableRow(_)) {
                    rows.push(self.table_row(row_ref, is_header));
                }
            }
        }

        Container::new(
            Column {
                children: rows,
                semantics: Some(markdown_semantics("markdown-table")),
                gap: Some(0.0),
                ..Default::default()
            }
            .into_node(),
        )
        .border(self.palette.border, 1.0)
        .border_radius(8.0)
        .into_node()
    }

    fn table_row(&self, row_ref: NodeRef, is_header: bool) -> Node {
        let cells = self.arena[row_ref]
            .children(self.arena)
            .map(|cell_ref| self.table_cell(cell_ref, is_header))
            .collect();
        Row {
            children: cells,
            semantics: Some(markdown_semantics(if is_header {
                "markdown-table-row:header"
            } else {
                "markdown-table-row:body"
            })),
            gap: Some(0.0),
            align_items: AlignItems::Start,
            ..Default::default()
        }
        .into_node()
    }

    fn table_cell(&self, cell_ref: NodeRef, is_header: bool) -> Node {
        let alignment = match self.arena[cell_ref].kind_data() {
            KindData::TableCell(cell) => cell.alignment(),
            _ => TableCellAlignment::None,
        };
        let mut style = self.inline_style(self.body_size);
        if is_header {
            style.font_weight = Some(700);
        }

        let mut cell = Container::new(RichText::new(self.inline_runs(cell_ref, style)).into_node())
            .padding_all(8.0)
            .border(self.palette.border.with_alpha(120), 1.0)
            .flex_grow(1.0);
        if is_header {
            cell = cell.bg(self.palette.surface_raised);
        } else {
            cell = cell.bg(self.palette.surface);
        }

        match alignment {
            TableCellAlignment::Center | TableCellAlignment::Right => {
                // Text alignment is omitted until Fission exposes per-cell table layout hooks.
            }
            _ => {}
        }

        let alignment = match alignment {
            TableCellAlignment::Left => "left",
            TableCellAlignment::Center => "center",
            TableCellAlignment::Right => "right",
            TableCellAlignment::None => "none",
            _ => "none",
        };
        Row {
            children: vec![cell.into_node()],
            semantics: Some(markdown_semantics(format!(
                "markdown-table-cell:{}:{alignment}",
                if is_header { "header" } else { "body" }
            ))),
            flex_grow: 1.0,
            align_items: AlignItems::Start,
            ..Default::default()
        }
        .into_node()
    }

    fn readable_plain_block(&self, node_ref: NodeRef) -> Node {
        self.text_node(
            self.plain_text(node_ref),
            self.body_size,
            self.palette.text_primary,
        )
    }

    fn divider(&self) -> Node {
        Container::new(VStack::default().into_node())
            .height(1.0)
            .bg(self.palette.border.with_alpha(180))
            .into_node()
    }

    fn text_node(&self, text: String, size: f32, color: Color) -> Node {
        Text::new(text)
            .size(size)
            .line_height(self.line_height)
            .color(color)
            .into_node()
    }

    fn inline_style(&self, font_size: f32) -> InlineStyle {
        InlineStyle {
            font_size,
            color: self.palette.text_primary,
            underline: false,
            font_family: None,
            font_weight: None,
            font_style: TextFontStyle::Normal,
            background_color: None,
            link_destination: None,
        }
    }

    fn inline_runs(&self, node_ref: NodeRef, style: InlineStyle) -> Vec<RichTextRun> {
        let mut runs = Vec::new();
        for child_ref in self.arena[node_ref].children(self.arena) {
            self.push_inline_runs(child_ref, &style, &mut runs);
        }
        runs
    }

    fn push_inline_runs(
        &self,
        node_ref: NodeRef,
        style: &InlineStyle,
        runs: &mut Vec<RichTextRun>,
    ) {
        match self.arena[node_ref].kind_data() {
            KindData::Text(text) => self.push_text_run(text, style, runs),
            KindData::CodeSpan(code) => self.push_code_span(code, style, runs),
            KindData::Emphasis(_) => {
                let mut nested = style.clone();
                nested.font_style = TextFontStyle::Italic;
                self.push_children_inline(node_ref, &nested, runs);
            }
            KindData::Strong(_) => {
                let mut nested = style.clone();
                nested.font_weight = Some(700);
                self.push_children_inline(node_ref, &nested, runs);
            }
            KindData::Link(link) => self.push_link(node_ref, link, style, runs),
            KindData::Image(image) => self.push_image(node_ref, image, style, runs),
            KindData::RawHtml(raw_html) => {
                self.push_run(raw_html.str(self.source).into_owned(), style, runs)
            }
            KindData::Strikethrough(_) => self.push_children_inline(node_ref, style, runs),
            _ => {
                let text = self.plain_text(node_ref);
                if !text.is_empty() {
                    self.push_run(text, style, runs);
                }
            }
        }
    }

    fn push_children_inline(
        &self,
        node_ref: NodeRef,
        style: &InlineStyle,
        runs: &mut Vec<RichTextRun>,
    ) {
        for child_ref in self.arena[node_ref].children(self.arena) {
            self.push_inline_runs(child_ref, style, runs);
        }
    }

    fn push_text_run(&self, text: &MarkdownText, style: &InlineStyle, runs: &mut Vec<RichTextRun>) {
        let mut value = text.str(self.source).to_string();
        if text.has_qualifiers(TextQualifier::SOFT_LINE_BREAK)
            || text.has_qualifiers(TextQualifier::HARD_LINE_BREAK)
        {
            value.push('\n');
        }
        self.push_run(value, style, runs);
    }

    fn push_code_span(&self, code: &CodeSpan, style: &InlineStyle, runs: &mut Vec<RichTextRun>) {
        let mut code_style = style.clone();
        code_style.font_family = Some(self.code_family.clone());
        code_style.background_color = Some(self.palette.surface_raised);
        code_style.font_size = (style.font_size - 1.0).max(10.0);
        self.push_run(code.str(self.source).into_owned(), &code_style, runs);
    }

    fn push_link(
        &self,
        node_ref: NodeRef,
        link: &Link,
        style: &InlineStyle,
        runs: &mut Vec<RichTextRun>,
    ) {
        let mut link_style = style.clone();
        link_style.color = self.palette.text_link;
        link_style.underline = true;
        link_style.link_destination = Some(link.destination_str(self.source).to_string());
        self.push_children_inline(node_ref, &link_style, runs);
    }

    fn push_image(
        &self,
        node_ref: NodeRef,
        image: &MarkdownImage,
        style: &InlineStyle,
        runs: &mut Vec<RichTextRun>,
    ) {
        let alt = self.plain_text(node_ref);
        let label = if alt.trim().is_empty() {
            format!("[image: {}]", image.destination_str(self.source))
        } else {
            format!("[image: {alt}]")
        };
        self.push_run(label, style, runs);
    }

    fn push_run(&self, text: String, style: &InlineStyle, runs: &mut Vec<RichTextRun>) {
        if text.is_empty() {
            return;
        }

        let mut run = RichTextRun::new(text)
            .size(style.font_size)
            .color(style.color)
            .underline(style.underline)
            .line_height(self.line_height);
        if let Some(family) = &style.font_family {
            run = run.family(family.clone());
        }
        if let Some(weight) = style.font_weight {
            run = run.weight(weight);
        }
        if style.font_style == TextFontStyle::Italic {
            run = run.italic(true);
        }
        if let Some(background) = style.background_color {
            run = run.background_color(background);
        }
        if let Some(destination) = &style.link_destination {
            run = run.semantics_identifier(format!("markdown-link:{destination}"));
        }
        runs.push(run);
    }

    fn plain_text(&self, node_ref: NodeRef) -> String {
        let mut out = String::new();
        self.collect_plain_text(node_ref, &mut out);
        out
    }

    fn collect_plain_text(&self, node_ref: NodeRef, out: &mut String) {
        match self.arena[node_ref].kind_data() {
            KindData::Text(text) => {
                out.push_str(text.str(self.source));
                if text.has_qualifiers(TextQualifier::SOFT_LINE_BREAK)
                    || text.has_qualifiers(TextQualifier::HARD_LINE_BREAK)
                {
                    out.push('\n');
                }
            }
            KindData::CodeSpan(code) => out.push_str(code.str(self.source).as_ref()),
            KindData::CodeBlock(code) => {
                for line in code.value().iter(self.source) {
                    out.push_str(line.as_ref());
                }
            }
            KindData::HtmlBlock(html) => {
                for line in html.value().iter(self.source) {
                    out.push_str(line.as_ref());
                }
            }
            KindData::RawHtml(raw_html) => out.push_str(raw_html.str(self.source).as_ref()),
            KindData::ThematicBreak(_) => out.push_str("---"),
            KindData::Image(image) => {
                let before = out.len();
                self.collect_children_plain_text(node_ref, out);
                if out.len() == before {
                    out.push_str(image.destination_str(self.source));
                }
            }
            _ => self.collect_children_plain_text(node_ref, out),
        }
    }

    fn collect_children_plain_text(&self, node_ref: NodeRef, out: &mut String) {
        for child_ref in self.arena[node_ref].children(self.arena) {
            let before = out.len();
            self.collect_plain_text(child_ref, out);
            if out.len() > before && !out.ends_with('\n') {
                match self.arena[child_ref].kind_data() {
                    KindData::Paragraph(_)
                    | KindData::Heading(_)
                    | KindData::ListItem(_)
                    | KindData::TableCell(_) => out.push('\n'),
                    _ => {}
                }
            }
        }
    }
}

fn markdown_semantics(identifier: impl Into<String>) -> Semantics {
    Semantics {
        role: Role::Generic,
        identifier: Some(identifier.into()),
        ..Semantics::default()
    }
}

fn markdown_code_semantics(language: &str, code: String) -> Semantics {
    Semantics {
        role: Role::Generic,
        identifier: Some(format!("markdown-code-block:{language}")),
        label: Some(if language.is_empty() {
            "Code block".to_string()
        } else {
            format!("{language} code block")
        }),
        value: Some(code),
        ..Semantics::default()
    }
}

fn markdown_code_language(raw: &str) -> &str {
    raw.trim()
        .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
        .next()
        .unwrap_or("")
}

fn markdown_anchor(value: &str) -> String {
    let mut out = String::new();
    let mut previous_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            previous_dash = false;
        } else if !previous_dash && !out.is_empty() {
            out.push('-');
            previous_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
