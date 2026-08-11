use super::*;

pub(super) struct HtmlRenderer<'a> {
    pub(super) ir: &'a CoreIR,
    pub(super) options: &'a HtmlRenderOptions,
    pub(super) styles: &'a mut StyleRegistry,
    pub(super) has_code_blocks: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum InteractionPseudo {
    Hover,
    Focused,
    Pressed,
}

impl InteractionPseudo {
    pub(super) fn selector(self) -> &'static str {
        match self {
            Self::Hover => ":hover",
            Self::Focused => ":focus",
            Self::Pressed => ":active",
        }
    }

    pub(super) fn matches(self, predicate: &MotionPredicate) -> bool {
        matches!(
            (self, predicate),
            (Self::Hover, MotionPredicate::Hovered(_))
                | (Self::Focused, MotionPredicate::Focused(_))
                | (Self::Pressed, MotionPredicate::Pressed(_))
        )
    }
}

impl HtmlRenderer<'_> {
    pub(super) fn register_interaction_motion_styles(&mut self) {
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

    pub(super) fn first_styled_box_descendant(&self, root: WidgetId) -> Option<WidgetId> {
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

    pub(super) fn interaction_css_value(
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

    pub(super) fn render_node(&mut self, node_id: WidgetId) -> Result<String> {
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

    pub(super) fn render_children(
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

    pub(super) fn render_element(
        &mut self,
        tag: &str,
        node: &CoreNode,
        class_name: &str,
        style: Vec<String>,
    ) -> Result<String> {
        self.render_element_with_attrs(tag, node, node.id, class_name, style, "")
    }

    pub(super) fn render_element_with_attrs(
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

    pub(super) fn stretches_auto_width_content_child(&self, node: &CoreNode) -> bool {
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

    pub(super) fn parent_uses_intrinsic_inline_sizing(&self, node: &CoreNode) -> bool {
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

    pub(super) fn class_name(&mut self, base: &str, style: Vec<String>) -> String {
        if let Some(generated) = self.styles.class_for(style) {
            format!("{base} {generated}")
        } else {
            base.to_string()
        }
    }

    pub(super) fn composite_style(&mut self, node: &CoreNode) -> (Vec<String>, bool) {
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

    pub(super) fn node_motion_style(&mut self, target: WidgetId) -> (Vec<String>, bool) {
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

    pub(super) fn animation_request(
        &self,
        scalar: &CompositeScalar,
        property: MotionPropertyId,
    ) -> Option<MotionTrack> {
        let target = scalar.motion_target?;
        self.motion_track_for_target(target, property)
    }

    pub(super) fn motion_track_for_target(
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

    pub(super) fn animation_css(
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

    pub(super) fn register_animation_keyframes(
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

    pub(super) fn color_animation_css(
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

    pub(super) fn register_color_animation_keyframes(
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

    pub(super) fn render_layout(&mut self, node: &CoreNode, layout: &LayoutOp) -> Result<String> {
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
                                "<div class=\"{} {}-{}\">{html}</div>",
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
                // Emit earlier cases later so equal-specificity CSS preserves
                // Fission's documented first-match precedence.
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
                    let body =
                        format!("{selector}{{display:none}}.{root_class}-{index}{{display:block}}");
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
                    fission_ir::op::ResponsiveQuery::Container => {
                        " style=\"container-type:inline-size\""
                    }
                };
                Ok(format!(
                    "<div class=\"fission-site-node fission-site-responsive {root_class}\"{container_style} data-fission-node=\"{}\">{children}</div>",
                    node.id
                ))
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
            LayoutOp::Clip { path } => {
                let mut style = vec!["overflow:hidden".to_string()];
                if let Some(path) = path {
                    style.push(format!("clip-path:path('{}')", css_string(path)));
                }
                self.render_element("div", node, "fission-site-node fission-site-clip", style)
            }
        }
    }

    pub(super) fn render_embed(
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

    pub(super) fn render_embed_fallback(
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

    pub(super) fn render_paint(&mut self, node: &CoreNode, paint: &PaintOp) -> Result<String> {
        match paint {
            PaintOp::BackdropFilter {
                filter,
                corner_radius,
            } => {
                let mut style = match filter {
                    fission_ir::op::BackdropFilter::Blur(sigma) => vec![
                        format!("backdrop-filter:blur({}px)", px(*sigma)),
                        format!("-webkit-backdrop-filter:blur({}px)", px(*sigma)),
                    ],
                };
                if *corner_radius > 0.0 {
                    style.push(format!("border-radius:{}px", px(*corner_radius)));
                    style.push("overflow:hidden".into());
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
                let annotations = self
                    .rich_text_annotations(node.id)
                    .map(|annotations| annotations.to_vec())
                    .unwrap_or_default();
                let content = self.render_rich_text_runs(node, runs, &annotations)?;
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

    pub(super) fn render_semantics(&mut self, node: &CoreNode) -> Result<String> {
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
            Role::Link => "a",
            Role::MenuItem => "button",
            Role::Image => "figure",
            Role::List => "ul",
            Role::ListItem => "li",
            Role::Dialog => "section",
            Role::Text | Role::Generic => "div",
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
        if tag == "ul" {
            if let Some(html) = self.render_transparent_list_layout(node, &attrs)? {
                return Ok(html);
            }
        }
        let children = self.render_children(&node.children, &HashSet::new())?;
        Ok(format!(
            "<{tag} class=\"fission-site-node fission-site-semantics\"{attrs} data-fission-node=\"{}\">{children}</{tag}>",
            node.id
        ))
    }
}
