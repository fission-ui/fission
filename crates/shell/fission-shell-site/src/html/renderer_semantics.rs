use super::*;

impl HtmlRenderer<'_> {
    pub(super) fn render_transparent_list_layout(
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

    pub(super) fn render_native_control_semantics(
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
            Role::Radio => {
                if semantics.checked.unwrap_or(false) {
                    attrs.push_str(" checked");
                }

                Ok(format!(
                    "<label class=\"fission-site-node fission-site-control\" data-fission-node=\"{}\"><input class=\"fission-site-radio\" type=\"radio\"{attrs}><span class=\"fission-site-control-label\">{}</span>{children}</label>",
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

    pub(super) fn native_control_attrs(&self, node: &CoreNode, semantics: &Semantics) -> String {
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

    pub(super) fn render_server_action_semantics(
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

    pub(super) fn render_browser_action_semantics(
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

    pub(super) fn render_markdown_table(&mut self, node: &CoreNode) -> Result<String> {
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

    pub(super) fn render_markdown_table_row(
        &mut self,
        node: &CoreNode,
        row_kind: &str,
    ) -> Result<String> {
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

    pub(super) fn render_markdown_table_cell(
        &mut self,
        node: &CoreNode,
        cell_kind: &str,
    ) -> Result<String> {
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

    pub(super) fn render_markdown_code_block(
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

    pub(super) fn render_static_form(
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

    pub(super) fn render_static_form_field(&self, field: &StaticFormField) -> String {
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

    pub(super) fn render_site_semantic_wrapper(
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
        let children = self.render_children(&node.children, &HashSet::new())?;
        Ok(format!(
            "<{tag} class=\"fission-site-node fission-site-semantics {class_name}\"{attrs} data-fission-node=\"{}\">{children}</{tag}>",
            node.id,
        ))
    }

    pub(super) fn render_semantic_payload_children(&mut self, node: &CoreNode) -> Result<String> {
        let children = self.semantic_payload_children(node);
        self.render_children(&children, &HashSet::new())
    }

    pub(super) fn semantic_payload_children(&self, node: &CoreNode) -> Vec<WidgetId> {
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

    pub(super) fn semantic_identifier(&self, node_id: WidgetId) -> Option<&str> {
        let node = self.ir.nodes.get(&node_id)?;
        let Op::Semantics(semantics) = &node.op else {
            return None;
        };
        semantics.identifier.as_deref()
    }

    pub(super) fn rich_text_annotations(&self, node_id: WidgetId) -> Option<&[RichTextAnnotation]> {
        self.ir
            .custom_render_objects
            .get(&node_id)?
            .downcast_ref::<Vec<RichTextAnnotation>>()
            .map(Vec::as_slice)
    }

    pub(super) fn subtree_has_rich_text_annotation(
        &self,
        node: &CoreNode,
        identifier: &str,
    ) -> bool {
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

    pub(super) fn render_semantic_link(
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

    pub(super) fn link_destination_attrs(&self, target: &str) -> String {
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

    pub(super) fn resolve_link_href(&self, target: &str) -> String {
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

    pub(super) fn resolve_asset_src(&self, source: &str) -> String {
        if source.starts_with('/') {
            relative_href_for_route(&self.options.current_route_path, source)
        } else {
            source.to_string()
        }
    }

    pub(super) fn resolve_image_src(&self, source: &ImageSource) -> String {
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

    pub(super) fn coalesced_paint_style(
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
            ));
            skip.insert(*child_id);
        }
        if !shadows.is_empty() {
            style.push(format!("box-shadow:{}", shadows.join(",")));
        }
        Ok(style)
    }

    pub(super) fn draw_rect_style(
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
            style.push(format!("box-shadow:{}", self.box_shadow_css(shadow)));
        }
        style
    }

    pub(super) fn box_shadow_css(&self, shadow: &BoxShadow) -> String {
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

    pub(super) fn text_style(
        &self,
        size: f32,
        color: Color,
        underline: bool,
        wrap: bool,
    ) -> Vec<String> {
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

    pub(super) fn render_text_run(&mut self, run: &TextRun) -> String {
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

    pub(super) fn render_rich_text_runs(
        &mut self,
        node: &CoreNode,
        runs: &[TextRun],
        annotations: &[RichTextAnnotation],
    ) -> Result<String> {
        let mut content = String::new();
        let mut rendered_inline_children = HashSet::new();
        let mut run_start = 0usize;
        for run in runs {
            let run_end = run_start.saturating_add(run.text.len());
            if run.text.is_empty() {
                if let Some(marker) = decode_inline_widget_marker(run.style.font_family.as_deref())
                {
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
                let mut rendered = self.render_text_run(&segment);
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
                for annotation in segment_annotations {
                    rendered = self.render_rich_text_annotation(rendered, annotation);
                }
                content.push_str(&rendered);
            }
            run_start = run_end;
        }
        for child_id in &node.children {
            if rendered_inline_children.insert(*child_id) {
                content.push_str(&self.render_node(*child_id)?);
            }
        }
        Ok(content)
    }

    pub(super) fn render_rich_text_annotation(
        &self,
        content: String,
        annotation: &RichTextAnnotation,
    ) -> String {
        let mut attrs = String::new();
        if let Some(label) = annotation.semantics_label.as_deref() {
            attrs.push_str(&format!(" aria-label=\"{}\"", escape_attr(label)));
        }
        if let Some(identifier) = annotation.semantics_identifier.as_deref() {
            attrs.push_str(&format!(
                " data-fission-semantics=\"{}\"",
                escape_attr(identifier)
            ));
            if let Some(target) = identifier.strip_prefix("markdown-link:") {
                let link_attrs = self.link_destination_attrs(target);
                return format!(
                    "<a class=\"fission-site-link fission-site-markdown-link\"{link_attrs}{attrs}>{content}</a>",
                );
            }
        }
        if annotation.spell_out.unwrap_or(false) {
            attrs.push_str(" role=\"text\"");
        }
        if attrs.is_empty() {
            content
        } else {
            format!("<span{attrs}>{content}</span>")
        }
    }

    pub(super) fn fill_css(&self, fill: &Fill) -> String {
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

    pub(super) fn stroke_css(&self, stroke: &Stroke) -> String {
        match &stroke.fill {
            Fill::Solid(color) => self.color_css(*color),
            fill => self.fill_css(fill),
        }
    }

    pub(super) fn svg_paint_style(
        &self,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
    ) -> Vec<String> {
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

    pub(super) fn color_css(&self, color: Color) -> String {
        self.options
            .css_variables
            .color_var(color)
            .map(|name| format!("var(--fs-color-{name})"))
            .unwrap_or_else(|| raw_color_css(color))
    }

    pub(super) fn font_family_css(&self, family: &str) -> String {
        self.options
            .css_variables
            .font_var(family)
            .map(|name| format!("var(--fs-font-{name})"))
            .unwrap_or_else(|| family.to_string())
    }
}
