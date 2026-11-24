use super::*;

#[test]
fn rich_text_hyperlink_wraps_its_visible_words_once_with_complete_metadata() {
    let rendered = render_test_widget(RichText::new(vec![
        fission_core::ui::RichTextRun::new("Read "),
        fission_core::ui::RichTextRun::new("guide")
            .semantics_identifier("inline-guide")
            .hyperlink(
                fission_ir::Hyperlink::new("https://example.com/guide?part=1&lang=en")
                    .target(fission_ir::LinkTarget::NewWindow)
                    .rel("external")
                    .download("guide.html"),
            ),
        fission_core::ui::RichTextRun::new(" next."),
    ]));
    assert_eq!(
        rendered.body_html.matches("<a ").count(),
        1,
        "no trailing empty semantic anchor"
    );
    let anchor = rendered
        .body_html
        .split_once("<a ")
        .unwrap()
        .1
        .split_once("</a>")
        .unwrap()
        .0;
    let (attributes, contents) = anchor.split_once('>').unwrap();
    assert!(attributes.contains("href=\"https://example.com/guide?part=1&amp;lang=en\""));
    assert!(attributes.contains("target=\"_blank\""));
    assert!(attributes.contains("rel=\"external\""));
    assert!(attributes.contains("download=\"guide.html\""));
    assert!(attributes.contains("data-fission-semantics=\"inline-guide\""));
    assert!(
        contents.contains(">guide</span>"),
        "the visible word must be inside the anchor: {contents}"
    );
    assert!(!contents.contains("Read ") && !contents.contains(" next."));
}

#[test]
fn rich_text_hyperlink_does_not_remove_real_inline_widgets() {
    let rendered = render_test_widget(RichText::from_spans([
        RichTextChild::from(RichTextSpan::new("Before ")),
        RichTextChild::from(
            RichTextSpan::new("guide").hyperlink(fission_ir::Hyperlink::new("/guide")),
        ),
        RichTextChild::from(WidgetSpan::new(Text::new("badge"), 40.0, 16.0)),
        RichTextChild::from(RichTextSpan::new(" after")),
    ]));
    assert_eq!(rendered.body_html.matches("<a ").count(), 1);
    let before = rendered.body_html.find(">Before </span>").unwrap();
    let guide = rendered.body_html.find(">guide</span>").unwrap();
    let badge = rendered.body_html.find(">badge</span>").unwrap();
    let after = rendered.body_html.find("> after</span>").unwrap();
    assert!(
        before < guide && guide < badge && badge < after,
        "real inline children retain their authored order"
    );
}

#[test]
fn rich_text_hyperlink_keeps_different_styles_in_one_anchor() {
    let rendered = render_test_widget(RichText::from_spans([
        RichTextSpan::new("Read ")
            .semantics_identifier("styled-guide")
            .hyperlink(fission_ir::Hyperlink::new("/guide"))
            .children([
                RichTextSpan::new("the guide").weight(700),
                RichTextSpan::new(" carefully").italic(true),
            ]),
        RichTextSpan::new(" outside"),
    ]));
    let body = &rendered.body_html;
    assert_eq!(
        body.matches("<a ").count(),
        1,
        "one link, not one per style"
    );
    assert_eq!(
        body.matches("data-fission-semantics=\"styled-guide\"")
            .count(),
        1
    );
    let anchor = body
        .split_once("<a ")
        .unwrap()
        .1
        .split_once("</a>")
        .unwrap()
        .0;
    assert!(anchor.contains("href=\"guide\""));
    for words in ["Read ", "the guide", " carefully"] {
        assert!(anchor.contains(&format!(">{words}</span>")), "{anchor}");
    }
    assert!(!anchor.contains(" outside"));
    assert!(rendered.css.contains("font-weight:700"));
    assert!(rendered.css.contains("font-style:italic"));
}

#[test]
fn rich_text_nested_hyperlinks_split_outer_anchor_without_nesting() {
    let rendered = render_test_widget(RichText::from_spans([RichTextSpan::new("Outer before ")
        .hyperlink(fission_ir::Hyperlink::new("/outer"))
        .children([
            RichTextSpan::new("inner").hyperlink(fission_ir::Hyperlink::new("/inner")),
            RichTextSpan::new(" outer after"),
        ])]));
    let body = &rendered.body_html;
    assert_eq!(body.matches("<a ").count(), 3, "{body}");
    let anchors = body
        .split("<a ")
        .skip(1)
        .map(|fragment| {
            let (anchor, after) = fragment.split_once("</a>").unwrap();
            assert!(!after.contains("</a>"), "anchors must not nest: {body}");
            anchor
        })
        .collect::<Vec<_>>();
    for (anchor, destination, words) in [
        (anchors[0], "outer", "Outer before "),
        (anchors[1], "inner", "inner"),
        (anchors[2], "outer", " outer after"),
    ] {
        assert!(
            anchor.contains(&format!("href=\"{destination}\"")),
            "{anchor}"
        );
        assert!(anchor.contains(&format!(">{words}</span>")), "{anchor}");
    }
}

#[test]
fn markdown_hyperlink_keeps_different_styles_in_one_anchor() {
    let rendered = render_test_widget(RichText::from_spans([RichTextSpan::new("Read ")
        .semantics_identifier("markdown-link:/guide")
        .children([RichTextSpan::new("the guide").weight(700)])]));
    let body = &rendered.body_html;
    assert_eq!(body.matches("<a ").count(), 1, "{body}");
    let anchor = body
        .split_once("<a ")
        .unwrap()
        .1
        .split_once("</a>")
        .unwrap()
        .0;
    assert!(anchor.contains("fission-site-markdown-link"));
    assert!(anchor.contains("href=\"guide\""));
    assert!(anchor.contains(">Read </span>") && anchor.contains(">the guide</span>"));
}

#[test]
fn generic_semantics_hyperlinks_render_complete_anchor_metadata() {
    let rendered = render_test_widget(
        SemanticsRegion::new(Text::new("Open details"))
            .label("Open details")
            .hyperlink(
                fission_ir::Hyperlink::new("/details/42")
                    .target(fission_ir::LinkTarget::NewWindow)
                    .rel("external")
                    .download("details.html"),
            )
            .popover_target("details-popover", fission_ir::PopoverAction::Show),
    );

    assert!(rendered.html.contains("<a "));
    assert!(rendered.html.contains("href=\"details/42\""));
    assert!(rendered.html.contains("target=\"_blank\""));
    assert!(rendered.html.contains("rel=\"external\""));
    assert!(rendered.html.contains("download=\"details.html\""));
    assert!(rendered.html.contains("popovertarget=\"details-popover\""));
    assert!(rendered.html.contains("popovertargetaction=\"show\""));
}

#[test]
fn navigational_link_is_static_html_not_an_interactive_action() {
    let rendered = render_test_component(|| fission_widgets::Link::to("Next", "/next").into());
    assert!(rendered.html.contains("<a "));
    assert!(rendered.html.contains("href=\"next\""));
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

#[test]
fn browser_action_options_bind_text_controls_without_exposing_action_metadata() {
    let input = WidgetId::explicit("browser-text-input");
    let mut semantics = Semantics {
        role: Role::TextInput,
        value: Some("before".into()),
        ..Default::default()
    };
    semantics.actions = ActionSet {
        entries: vec![ActionEntry {
            trigger: ActionTrigger::TextChanged,
            action_id: 11,
            payload_data: Some(br#"["smtp_host","secret-marker-8bd4"]"#.to_vec()),
        }],
    };
    let mut ir = CoreIR::new();
    ir.nodes.insert(
        input,
        CoreNode {
            id: input,
            op: Op::Semantics(semantics),
            composite: Default::default(),
            children: Vec::new(),
            parent: None,
            hash: 0,
        },
    );
    ir.set_root(input);

    let interactive = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            browser_action_bindings: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(interactive
        .html
        .contains("data-fission-browser-text-action=\"true\""));
    assert!(interactive.html.contains(&format!(
        "data-fission-action-target=\"{}\"",
        input.as_u128()
    )));
    assert!(!interactive.html.contains("smtp_host"));
    assert!(!interactive.html.contains("secret-marker-8bd4"));
    assert!(!interactive.html.contains("data-fission-action-payload"));
    assert!(!interactive.html.contains("data-fission-action-id"));
    assert!(!interactive
        .html
        .contains(&hex_encode(br#"["smtp_host","secret-marker-8bd4"]"#)));

    let server_only = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            server_action_post_path: Some("/__fission/action".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!server_only
        .html
        .contains("data-fission-browser-text-action"));
}

#[test]
fn browser_action_options_do_not_bind_non_dispatchable_text_controls() {
    for (disabled, read_only) in [(true, false), (false, true)] {
        let input = WidgetId::explicit(if disabled {
            "disabled-browser-text-input"
        } else {
            "readonly-browser-text-input"
        });
        let mut semantics = Semantics {
            role: Role::TextInput,
            disabled,
            read_only,
            ..Default::default()
        };
        semantics.actions = ActionSet {
            entries: vec![ActionEntry {
                trigger: ActionTrigger::TextChanged,
                action_id: 17,
                payload_data: Some(vec![1, 2, 3]),
            }],
        };
        let mut ir = CoreIR::new();
        ir.add_node(input, Op::Semantics(semantics), Vec::new());
        ir.set_root(input);

        let rendered = render_ir_to_html(
            &ir,
            &HtmlRenderOptions {
                browser_action_bindings: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!rendered.html.contains("data-fission-browser-text-action"));
        assert!(!rendered.html.contains("data-fission-action-id"));
        assert!(!rendered.html.contains("data-fission-action-payload"));
    }
}

#[test]
fn browser_action_options_require_text_editing_capability() {
    let node = WidgetId::explicit("generic-text-change-node");
    let mut semantics = Semantics {
        role: Role::Generic,
        ..Default::default()
    };
    semantics.actions = ActionSet {
        entries: vec![ActionEntry {
            trigger: ActionTrigger::TextChanged,
            action_id: 21,
            payload_data: Some(vec![4, 5, 6]),
        }],
    };
    let mut ir = CoreIR::new();
    ir.add_node(node, Op::Semantics(semantics), Vec::new());
    ir.set_root(node);

    let rendered = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            browser_action_bindings: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!rendered.html.contains("data-fission-browser-text-action"));
    assert!(!rendered.html.contains("data-fission-action-id"));
    assert!(!rendered.html.contains("data-fission-action-payload"));
}

#[test]
fn editable_combobox_renders_as_a_native_text_control() {
    let node = WidgetId::explicit("editable-combobox");
    let mut semantics = Semantics {
        role: Role::ComboBox,
        text_editable: true,
        label: Some("Destination".into()),
        value: Some("London".into()),
        expanded: Some(false),
        ..Default::default()
    };
    semantics.actions = ActionSet {
        entries: vec![ActionEntry {
            trigger: ActionTrigger::TextChanged,
            action_id: 22,
            payload_data: None,
        }],
    };
    let mut ir = CoreIR::new();
    ir.add_node(node, Op::Semantics(semantics), Vec::new());
    ir.set_root(node);

    let rendered = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            browser_action_bindings: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(rendered.html.contains("<input"));
    assert!(rendered.html.contains("role=\"combobox\""));
    assert!(rendered.html.contains("value=\"London\""));
    assert!(rendered
        .html
        .contains("data-fission-browser-text-action=\"true\""));
}

#[test]
fn noneditable_combobox_uses_a_hidden_successful_form_control() {
    let node = WidgetId::explicit("noneditable-combobox");
    let mut ir = CoreIR::new();
    ir.add_node(
        node,
        Op::Semantics(Semantics {
            role: Role::ComboBox,
            label: Some("Destination".into()),
            value: Some("London".into()),
            text_field_name: Some("destination".into()),
            text_form_id: Some("journey".into()),
            form_value: Some("lhr".into()),
            ..Default::default()
        }),
        Vec::new(),
    );
    ir.set_root(node);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered
        .html
        .contains("<input type=\"hidden\" name=\"destination\" value=\"lhr\" form=\"journey\">"));
    assert!(rendered.html.contains("role=\"combobox\""));

    let mut disabled_ir = CoreIR::new();
    disabled_ir.add_node(
        node,
        Op::Semantics(Semantics {
            role: Role::ComboBox,
            disabled: true,
            text_field_name: Some("destination".into()),
            text_form_id: Some("journey".into()),
            form_value: Some("lhr".into()),
            ..Default::default()
        }),
        Vec::new(),
    );
    disabled_ir.set_root(node);
    let disabled = render_ir_to_html(&disabled_ir, &HtmlRenderOptions::default()).unwrap();
    assert!(!disabled.html.contains("name=\"destination\""));
}

#[test]
fn retained_select_with_machine_values_renders_a_native_form_control() {
    let rendered = render_test_component(|| {
        Select {
            id: WidgetId::explicit("region"),
            selected_label: Some("Europe West".into()),
            items: vec![
                SelectItem {
                    label: "Europe West".into(),
                    icon: None,
                    on_select: ActionEnvelope {
                        id: ActionId::from_name("region.eu-west"),
                        payload: Vec::new(),
                    },
                    semantics_identifier: Some("region.eu-west".into()),
                    form_value: Some("eu-west".into()),
                },
                SelectItem {
                    label: "US East".into(),
                    icon: None,
                    on_select: ActionEnvelope {
                        id: ActionId::from_name("region.us-east"),
                        payload: Vec::new(),
                    },
                    semantics_identifier: Some("region.us-east".into()),
                    form_value: Some("us-east".into()),
                },
            ],
            name: Some("default_region".into()),
            form_id: Some("connect".into()),
            form_value: Some("eu-west".into()),
            ..Default::default()
        }
        .into()
    });

    assert!(rendered.html.contains("<select"));
    assert!(rendered.html.contains("name=\"default_region\""));
    assert!(rendered.html.contains("form=\"connect\""));
    assert!(rendered
        .html
        .contains("<option value=\"eu-west\" selected>Europe West</option>"));
    assert!(rendered
        .html
        .contains("<option value=\"us-east\">US East</option>"));
    assert!(!rendered.html.contains("type=\"hidden\""));
    assert!(
        crate::build::site_base_css().contains("select.fission-site-input {\n  appearance: auto;")
    );
}

#[test]
fn alert_semantics_render_as_an_announcing_region() {
    let node = WidgetId::explicit("status-alert");
    let mut ir = CoreIR::new();
    ir.add_node(
        node,
        Op::Semantics(Semantics {
            role: Role::Alert,
            ..Default::default()
        }),
        Vec::new(),
    );
    ir.set_root(node);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("role=\"alert\""));
}

#[test]
fn composite_semantics_render_complete_aria_state_and_relationships() {
    let root = WidgetId::from_u128(1);
    let listbox = WidgetId::from_u128(2);
    let label = WidgetId::from_u128(3);
    let description = WidgetId::from_u128(4);
    let option = WidgetId::from_u128(5);
    let combo = WidgetId::from_u128(6);
    let dialog = WidgetId::from_u128(7);
    let mut ir = CoreIR::new();
    ir.add_node(
        label,
        Op::Semantics(Semantics {
            role: Role::Text,
            label: Some("Destination".into()),
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        description,
        Op::Semantics(Semantics {
            role: Role::Text,
            label: Some("Choose one destination".into()),
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        option,
        Op::Semantics(Semantics {
            role: Role::Option,
            label: Some("London".into()),
            selected: Some(true),
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        listbox,
        Op::Semantics(Semantics {
            role: Role::ListBox,
            orientation: Some(SemanticOrientation::Vertical),
            ..Semantics::default()
        }),
        vec![option],
    );
    ir.add_node(
        combo,
        Op::Semantics(Semantics {
            role: Role::ComboBox,
            label: Some("Destination".into()),
            focusable: true,
            sequential_focusable: false,
            expanded: Some(false),
            has_popup: Some(PopupKind::ListBox),
            required: true,
            validation_state: fission_ir::TextFieldValidationState::Invalid,
            controls: vec![listbox],
            labelled_by: vec![label],
            described_by: vec![description],
            active_descendant: Some(option),
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        dialog,
        Op::Semantics(Semantics {
            role: Role::Dialog,
            modal: true,
            ..Semantics::default()
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        vec![label, description, combo, listbox, dialog],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    let combo_start = rendered
        .html
        .find("id=\"fission-semantic-6\"")
        .expect("combobox semantic element");
    let combo_end = combo_start
        + rendered.html[combo_start..]
            .find('>')
            .expect("combobox opening tag");
    let combo_attrs = &rendered.html[combo_start..combo_end];
    for expected in [
        "tabindex=\"-1\"",
        "aria-expanded=\"false\"",
        "aria-haspopup=\"listbox\"",
        "aria-required=\"true\"",
        "aria-invalid=\"true\"",
        "aria-controls=\"fission-semantic-2\"",
        "aria-labelledby=\"fission-semantic-3\"",
        "aria-describedby=\"fission-semantic-4\"",
        "aria-activedescendant=\"fission-semantic-5\"",
        "role=\"combobox\"",
    ] {
        assert!(
            combo_attrs.contains(expected),
            "missing {expected}: {combo_attrs}"
        );
    }
    assert!(rendered.html.contains("aria-orientation=\"vertical\""));
    assert!(rendered.html.contains("aria-selected=\"true\""));
    assert!(rendered.html.contains("aria-modal=\"true\""));
}

#[test]
fn semantic_list_absorbs_column_layout_for_direct_list_items() {
    let rendered = render_test_widget(
        SemanticsRegion::new(Column {
            gap: Some(8.0),
            children: vec![
                generic_list_item("First item"),
                generic_list_item("Second item"),
            ],
            ..Default::default()
        })
        .identifier("sample-list")
        .label("Sample items")
        .role(Role::List),
    );

    let (opening, contents) = rendered_list_parts(&rendered.html);
    assert!(opening.contains("fission-site-column"));
    assert!(opening.contains("data-fission-semantics=\"sample-list\""));
    assert!(opening.contains("aria-label=\"Sample items\""));
    assert!(contents.starts_with("<li "));
    assert_eq!(contents.matches("<li ").count(), 2);
    assert!(contents.contains("</li><li "));
    assert!(!contents.starts_with("<div"));
    assert!(rendered.css.contains("display:flex"));
    assert!(rendered.css.contains("gap:8px"));
}

#[test]
fn semantic_list_absorbs_grid_layout_for_direct_list_items() {
    let rendered = render_test_widget(
        SemanticsRegion::new(Grid {
            columns: vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)],
            column_gap: Some(12.0),
            children: vec![
                generic_list_item("First result"),
                generic_list_item("Second result"),
            ],
            ..Default::default()
        })
        .role(Role::List),
    );

    let (opening, contents) = rendered_list_parts(&rendered.html);
    assert!(opening.contains("fission-site-grid"));
    assert!(contents.starts_with("<li "));
    assert_eq!(contents.matches("<li ").count(), 2);
    assert!(contents.contains("</li><li "));
    assert!(!contents.starts_with("<div"));
    assert!(rendered.css.contains("display:grid"));
    assert!(rendered.css.contains("grid-template-columns:1fr 1fr"));
    assert!(rendered.css.contains("column-gap:12px"));
}

#[test]
fn semantic_list_preserves_layout_inside_meaningful_list_item() {
    let nested_item = SemanticsRegion::new(Column {
        children: vec![Text::new("Nested item detail").into()],
        ..Default::default()
    })
    .identifier("nested-item")
    .role(Role::ListItem);
    let rendered = render_test_widget(
        SemanticsRegion::new(Column {
            children: vec![nested_item.into()],
            ..Default::default()
        })
        .role(Role::List),
    );

    let (_, contents) = rendered_list_parts(&rendered.html);
    assert!(contents.starts_with("<li "));
    assert!(contents.contains("data-fission-semantics=\"nested-item\""));
    let (_, item_contents) = contents.split_once('>').expect("list item opening tag");
    assert!(item_contents.starts_with("<div "));
    assert!(item_contents.contains("fission-site-column"));
}

#[test]
fn markdown_table_separates_header_and_body_rows() {
    let rendered = render_test_component(|| {
        MarkdownContent::new("| Package | Status |\n| --- | --- |\n| example | Ready |").into()
    });

    assert!(rendered.html.contains("<table "));
    assert!(rendered.html.contains("<thead><tr "));
    assert!(rendered.html.contains("<th "));
    assert!(rendered.html.contains("</thead><tbody><tr "));
    assert!(rendered.html.contains("<td "));
}

#[test]
fn markdown_rich_text_preserves_inline_links_and_emphasis() {
    let rendered = render_test_component(|| {
        MarkdownContent::new(
            "Read the [support guide](/support) and **keep this visible** with `sample-code`.",
        )
        .into()
    });

    assert!(rendered
        .body_html
        .contains("class=\"fission-site-link fission-site-markdown-link\""));
    assert!(rendered.html.contains("href=\"support\""));
    assert_eq!(
        rendered
            .body_html
            .matches("class=\"fission-site-link fission-site-markdown-link\"")
            .count(),
        1
    );
    let (_, link_and_after) = rendered
        .body_html
        .split_once("class=\"fission-site-link fission-site-markdown-link\"")
        .expect("inline Markdown link");
    let (_, link_content) = link_and_after
        .split_once('>')
        .expect("inline Markdown link opening tag");
    let (link_content, _) = link_content
        .split_once("</a>")
        .expect("inline Markdown link closing tag");
    assert!(link_content.contains("support guide"));
    assert!(!link_content.contains("Read the"));
    assert!(!link_content.contains("keep this visible"));
    assert!(rendered.css.contains("font-weight:700"));
    assert!(rendered.html.contains("sample-code"));
    assert!(rendered.css.contains("background:"));
}

#[test]
fn external_markdown_links_preserve_targets_and_add_safe_relationships() {
    let rendered = render_test_component(|| {
        MarkdownContent::new(
            "Visit the [reference](HTTPS://example.com/docs) or its [mirror](//cdn.example.com/docs).",
        )
        .into()
    });

    assert!(rendered
        .body_html
        .contains("href=\"HTTPS://example.com/docs\" rel=\"noopener noreferrer\""));
    assert!(rendered
        .body_html
        .contains("href=\"//cdn.example.com/docs\" rel=\"noopener noreferrer\""));
    assert!(!rendered.body_html.contains("target=\"_blank\""));
}

#[test]
fn rich_text_annotations_keep_inline_widgets_in_text_order() {
    let rendered = render_test_widget(RichText::from_span(
        RichTextSpan::new("Before ")
            .semantics_identifier("markdown-link:/guide")
            .children(vec![
                RichTextChild::from(WidgetSpan::new(Text::new("badge"), 40.0, 16.0)),
                RichTextChild::from(RichTextSpan::new(" after").weight(700)),
            ]),
    ));

    let before = rendered
        .body_html
        .find("Before ")
        .expect("text before marker");
    let badge = rendered
        .body_html
        .find("badge")
        .expect("inline widget marker payload");
    let after = rendered
        .body_html
        .find(" after")
        .expect("text after marker");
    assert!(before < badge);
    assert!(badge < after);
    assert_eq!(rendered.body_html.matches("href=\"guide\"").count(), 2);
}

#[test]
fn nested_rich_text_annotations_keep_parent_semantics_outermost() {
    let rendered = render_test_widget(RichText::from_span(
        RichTextSpan::new("")
            .semantics_label("Read documentation")
            .children([
                RichTextSpan::new("documentation").semantics_identifier("markdown-link:/docs")
            ]),
    ));

    assert!(rendered.body_html.contains(
        "<span aria-label=\"Read documentation\"><a class=\"fission-site-link fission-site-markdown-link\" href=\"docs\""
    ));
}

#[test]
fn disclosure_summary_is_native_only_inside_its_matching_disclosure() {
    let rendered = render_test_widget(Column {
        children: vec![
            SemanticsRegion::new(Column {
                children: vec![
                    SemanticsRegion::new(Text::new("Show source"))
                        .identifier("site-disclosure-summary:source")
                        .into(),
                    Text::new("Source contents").into(),
                ],
                ..Default::default()
            })
            .identifier("site-disclosure:source")
            .into(),
            SemanticsRegion::new(Text::new("Not a disclosure trigger"))
                .identifier("site-disclosure-summary:other")
                .into(),
        ],
        ..Default::default()
    });

    assert_eq!(rendered.body_html.matches("<details ").count(), 1);
    assert_eq!(rendered.body_html.matches("<summary ").count(), 1);
    assert!(rendered.body_html.contains("Source contents"));
    assert!(rendered.body_html.contains("Not a disclosure trigger"));
}

#[test]
fn client_action_semantics_render_generic_site_button() {
    let rendered = render_test_widget(
        SemanticsRegion::new(Text::new("Open preferences"))
            .identifier("site-client-action:open-preferences")
            .label("Open preferences")
            .role(Role::Generic),
    );

    assert!(rendered.html.contains("<button "));
    assert!(rendered
        .html
        .contains("data-fission-client-action=\"open-preferences\""));
    assert!(rendered
        .html
        .contains("data-fission-semantics=\"site-client-action:open-preferences\""));
    assert!(rendered.html.contains("aria-label=\"Open preferences\""));
    assert!(!rendered.html.contains(" disabled"));
}

#[test]
fn site_address_and_current_page_link_use_native_html_semantics() {
    let rendered = render_test_widget(Column {
        children: vec![
            SemanticsRegion::new(Text::new("Example Company"))
                .identifier("site-address")
                .role(Role::Generic)
                .into(),
            SemanticsRegion::new(Text::new("Home"))
                .identifier("site-link:/")
                .role(Role::Link)
                .into(),
        ],
        ..Default::default()
    });

    assert!(rendered.html.contains("<address "));
    assert!(rendered
        .html
        .contains("data-fission-semantics=\"site-address\""));
    assert!(rendered.html.contains("aria-current=\"page\""));
}

#[test]
fn responsive_css_preserves_first_match_precedence() {
    let root = WidgetId::explicit("responsive");
    let first = WidgetId::explicit("first");
    let second = WidgetId::explicit("second");
    let fallback = WidgetId::explicit("fallback");
    let mut ir = CoreIR::new();
    for child in [first, second, fallback] {
        ir.add_node(
            child,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            Vec::new(),
        );
    }
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Responsive {
            query: fission_ir::op::ResponsiveQuery::Viewport,
            cases: vec![
                fission_ir::op::ResponsiveCondition {
                    min_width: None,
                    max_width: Some(900.0),
                },
                fission_ir::op::ResponsiveCondition {
                    min_width: None,
                    max_width: Some(600.0),
                },
            ],
        }),
        vec![first, second, fallback],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    let later_case = rendered
        .css
        .find("(max-width:599.990px)")
        .expect("later responsive case");
    let first_case = rendered
        .css
        .find("(max-width:899.990px)")
        .expect("first responsive case");

    assert!(
        later_case < first_case,
        "the first case must be emitted last so equal-specificity CSS wins"
    );
}

#[test]
fn responsive_heading_keeps_only_the_selected_branch_visible() {
    let heading = WidgetId::explicit("responsive-heading");
    let responsive = WidgetId::explicit("responsive-heading-layout");
    let compact = WidgetId::explicit("compact-heading");
    let wide = WidgetId::explicit("wide-heading");
    let mut ir = CoreIR::new();
    for child in [compact, wide] {
        ir.add_node(
            child,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            Vec::new(),
        );
    }
    ir.add_node(
        responsive,
        Op::Layout(LayoutOp::Responsive {
            query: fission_ir::op::ResponsiveQuery::Viewport,
            cases: vec![fission_ir::op::ResponsiveCondition {
                min_width: Some(700.0),
                max_width: None,
            }],
        }),
        vec![wide, compact],
    );
    ir.add_node(
        heading,
        Op::Semantics(Semantics {
            identifier: Some("site-heading-1:responsive".into()),
            ..Default::default()
        }),
        vec![responsive],
    );
    ir.set_root(heading);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("<h1"));
    assert!(rendered
        .html
        .contains("<span class=\"fission-site-node fission-site-responsive"));
    assert!(!rendered.html.contains(
        "<h1 class=\"fission-site-node fission-site-semantics site-heading-1-responsive\"><div"
    ));
    assert!(rendered.css.contains("display:none"));
}

#[test]
fn site_shell_sizes_stretched_container_query_children() {
    let root = WidgetId::explicit("stretch-box");
    let responsive = WidgetId::explicit("responsive");
    let fallback = WidgetId::explicit("fallback");
    let background = WidgetId::explicit("background");
    let mut ir = CoreIR::new();
    ir.add_node(
        fallback,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        Vec::new(),
    );
    ir.add_node(
        responsive,
        Op::Layout(LayoutOp::Responsive {
            query: fission_ir::op::ResponsiveQuery::Container,
            cases: Vec::new(),
        }),
        vec![fallback],
    );
    ir.add_node(
        background,
        Op::Paint(PaintOp::DrawRect {
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            corner_radii: None,
            border_sides: None,
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle {
                alignment: fission_ir::op::BoxAlignment::Stretch,
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![responsive, background],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered
        .html
        .contains("fission-site-box-stretch-auto-width"));
    assert!(rendered.html.contains("fission-site-responsive"));
    assert!(rendered.html.contains("container-type:inline-size"));
    assert!(crate::site_base_css()
        .contains(".fission-site-box-stretch-auto-width > .fission-site-node"));
}

#[test]
fn site_shell_preserves_explicit_width_on_stretch_children() {
    let root = WidgetId::explicit("stretch-box-static");
    let child = WidgetId::explicit("explicit-child");
    let mut ir = CoreIR::new();
    ir.add_node(
        child,
        Op::Layout(LayoutOp::Box {
            width: Some(240.0),
            height: Some(80.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle {
                alignment: fission_ir::op::BoxAlignment::Stretch,
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![child],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(!rendered
        .html
        .contains("fission-site-box-stretch-auto-width"));
    assert!(rendered.css.contains("width:240px"));
}

#[test]
fn site_shell_stretches_auto_width_grid_children() {
    let root = WidgetId::explicit("stretch-box-grid");
    let child = WidgetId::explicit("auto-grid");
    let background = WidgetId::explicit("grid-background");
    let mut ir = CoreIR::new();
    ir.add_node(
        child,
        Op::Layout(LayoutOp::Grid {
            columns: vec![fission_ir::op::GridTrack::Fr(1.0)],
            rows: Vec::new(),
            column_gap: None,
            row_gap: None,
            padding: [0.0; 4],
        }),
        Vec::new(),
    );
    ir.add_node(
        background,
        Op::Paint(PaintOp::DrawRect {
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            corner_radii: None,
            border_sides: None,
        }),
        Vec::new(),
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle {
                alignment: fission_ir::op::BoxAlignment::Stretch,
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![child, background],
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered
        .html
        .contains("fission-site-box-stretch-auto-width"));
}

#[test]
fn site_shell_preserves_intrinsic_width_inside_links() {
    let link = WidgetId::explicit("site-link");
    let container = WidgetId::explicit("link-container");
    let child = WidgetId::explicit("link-label");
    let mut ir = CoreIR::new();
    ir.add_node(
        child,
        Op::Paint(PaintOp::DrawText {
            text: "Open details".into(),
            size: 16.0,
            color: Color::BLACK,
            underline: false,
            locale: None,
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
        container,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle {
                alignment: fission_ir::op::BoxAlignment::Stretch,
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![child],
    );
    ir.add_node(
        link,
        Op::Semantics(Semantics {
            role: Role::Link,
            identifier: Some("site-link:#details".into()),
            ..Default::default()
        }),
        vec![container],
    );
    ir.set_root(link);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("href=\"#details\""));
    assert!(!rendered
        .html
        .contains("fission-site-box-stretch-auto-width"));
}

#[test]
fn site_semantics_emit_native_landmarks_headings_and_anchors() {
    let root = WidgetId::explicit("root");
    let heading_layout = WidgetId::explicit("semantic-heading-layout");
    let heading_text = WidgetId::explicit("semantic-heading-text");
    let identifiers = [
        "site-header",
        "site-main",
        "site-navigation",
        "site-section:features",
        "site-heading-2:page-title",
        "site-anchor:details",
        "site-footer",
    ];
    let mut ir = CoreIR::new();
    let mut semantic_nodes = Vec::new();
    let node_ids = [
        "semantic-header",
        "semantic-main",
        "semantic-navigation",
        "semantic-section",
        "semantic-heading",
        "semantic-anchor",
        "semantic-footer",
    ];
    ir.add_node(
        heading_text,
        Op::Paint(PaintOp::DrawText {
            text: "Page heading".into(),
            size: 24.0,
            color: Color::BLACK,
            underline: false,
            locale: None,
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
        heading_layout,
        Op::Layout(LayoutOp::StyledBox {
            style: fission_ir::op::BoxStyle {
                alignment: fission_ir::op::BoxAlignment::Stretch,
                ..Default::default()
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![heading_text],
    );
    for (identifier, node_id) in identifiers.into_iter().zip(node_ids) {
        let id = WidgetId::explicit(node_id);
        let node_children = if identifier == "site-heading-2:page-title" {
            vec![heading_layout]
        } else {
            Vec::new()
        };
        ir.add_node(
            id,
            Op::Semantics(Semantics {
                role: Role::Generic,
                identifier: Some(identifier.to_string()),
                ..Default::default()
            }),
            node_children,
        );
        semantic_nodes.push(id);
    }
    ir.add_node(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        semantic_nodes,
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("<header "));
    assert!(rendered.html.contains("<main "));
    assert!(rendered.html.contains("<nav "));
    assert!(rendered.html.contains("<section "));
    assert!(rendered.html.contains("id=\"features\""));
    assert!(rendered.html.contains("<h2 "));
    assert!(rendered.html.contains("id=\"page-title\""));
    assert!(rendered.html.contains("Page heading"));
    let heading_html = rendered
        .html
        .split_once("<h2 ")
        .and_then(|(_, rest)| rest.split_once("</h2>"))
        .map(|(heading, _)| heading)
        .expect("rendered h2");
    assert!(!heading_html.contains("<div"));
    assert!(rendered.html.contains("id=\"details\""));
    assert!(rendered.html.contains("<footer "));
    assert!(crate::site_base_css().contains("h2.fission-site-semantics"));
}

#[test]
fn site_link_semantics_emit_an_ordinary_anchor() {
    let root = WidgetId::explicit("site-link");
    let mut ir = CoreIR::new();
    ir.add_node(
        root,
        Op::Semantics(Semantics {
            role: Role::Link,
            identifier: Some("site-link:#details".into()),
            label: Some("View details".into()),
            ..Default::default()
        }),
        Vec::new(),
    );
    ir.set_root(root);

    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert!(rendered.html.contains("<a "));
    assert!(rendered.html.contains("href=\"#details\""));
    assert!(rendered.html.contains("aria-label=\"View details\""));
    assert!(!rendered.html.contains("<form"));
}
