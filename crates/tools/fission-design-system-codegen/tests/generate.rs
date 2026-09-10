use std::path::PathBuf;

#[test]
fn generates_rust_for_fission_dsp_package() {
    let out_dir =
        std::env::temp_dir().join(format!("fission-dsp-codegen-test-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    std::env::set_var("OUT_DIR", &out_dir);

    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();
    let dsp_path = repo.join("crates/core/fission-theme/design/default/dsp.json");

    let out = fission_design_system_codegen::generate(fission_design_system_codegen::Config {
        dsp_path,
        out_file: "generated.rs".into(),
        type_name: "GeneratedDesignSystem".into(),
        crate_path: "fission_theme".into(),
    })
    .unwrap();
    let generated = std::fs::read_to_string(out).unwrap();

    assert!(generated.contains("pub struct GeneratedDesignSystem"));
    assert!(generated.contains("impl fission_theme::DesignSystem for GeneratedDesignSystem"));
    assert!(generated.contains("color.teal.700"));
    assert!(generated.contains("marketing_hero"));
    assert!(generated.contains("fission_theme::ButtonHierarchy::Outline"));
    assert!(
        generated.contains("padding: Some([16.0, 16.0, 16.0, 16.0])"),
        "modal container padding must survive DSP code generation"
    );
    assert!(
        generated.contains("gap: Some(16.0)"),
        "modal container gap must survive DSP code generation"
    );
    assert!(
        generated.contains("title_style: fission_theme::ResolvedComponentStyle")
            && generated.contains("description_style: fission_theme::ResolvedComponentStyle")
            && generated.contains("content_style: fission_theme::ResolvedComponentStyle")
            && generated.contains("close_button_style: fission_theme::ResolvedComponentStyle"),
        "modal retained anatomy recipes must survive DSP code generation"
    );
    assert!(
        generated.contains("viewport_margin: 16.0")
            && generated.contains("action_stack_breakpoint: 640.0")
            && generated.contains("motion_duration_ms: 100")
            && generated.contains("motion_initial_scale: 0.95"),
        "modal responsive geometry must survive DSP code generation"
    );
    assert!(
        generated.contains("margin: Some([-16.0, -16.0, 0.0, -16.0])"),
        "modal full-bleed footer geometry must survive DSP code generation"
    );
    assert!(
        generated.contains("(fission_theme::ComponentSize::Sm")
            && generated.contains("(fission_theme::ComponentSize::Md"),
        "card density recipes must survive DSP code generation"
    );
    assert!(
        generated.contains("footer_style: fission_theme::ResolvedComponentStyle")
            && generated
                .contains("selected_indicator_style: fission_theme::ResolvedComponentStyle"),
        "card footer and selected-indicator anatomy must survive DSP code generation"
    );
    assert!(
        generated.contains("menu: fission_theme::MenuTheme")
            && generated.contains("trigger_sizes: vec!")
            && generated.contains("trigger_states: fission_theme::ComponentStateStyles")
            && generated.contains("destructive_item_states: fission_theme::ComponentStateStyles")
            && generated.contains("group_label_style: fission_theme::ResolvedComponentStyle"),
        "menu anatomy and state recipes must survive DSP code generation"
    );
    assert!(
        generated.contains("select: fission_theme::SelectTheme")
            && generated.contains("placeholder_style: fission_theme::ResolvedComponentStyle")
            && generated.contains("indicator_style: fission_theme::ResolvedComponentStyle")
            && generated.contains("padding: Some([10.0, 8.0, 3.0, 3.0])")
            && generated.contains("padding: Some([10.0, 8.0, 5.0, 5.0])"),
        "select trigger recipes must survive DSP code generation"
    );
    assert!(
        generated.contains("alert: fission_theme::AlertTheme")
            && generated.contains("content_style: fission_theme::ResolvedComponentStyle")
            && generated.contains("success_style: fission_theme::ResolvedComponentStyle"),
        "alert anatomy and tone recipes must survive DSP code generation"
    );
    assert!(
        generated.contains("pagination: fission_theme::PaginationTheme")
            && generated.contains("selected_style: fission_theme::ResolvedComponentStyle")
            && generated.contains("ellipsis_style: fission_theme::ResolvedComponentStyle"),
        "pagination geometry and current-page recipes must survive DSP code generation"
    );
    assert!(
        generated.contains("width: 1.0"),
        "component borders must survive DSP code generation"
    );
    assert!(
        generated.contains("empty_state: fission_theme::EmptyStateTheme")
            && generated.contains("min_height: Some(160.0)")
            && generated.contains("narrow_breakpoint: 640.0")
            && generated.contains("narrow_surface_style: fission_theme::ResolvedComponentStyle")
            && generated.contains("min_height: Some(128.0)")
            && generated.contains("border_dash: Some(vec![4.0,4.0])"),
        "empty-state geometry must survive DSP code generation"
    );
    assert!(
        generated.contains("avatar_group: fission_theme::AvatarGroupTheme")
            && generated.contains("overlap: 10.0")
            && generated.contains("max_visible: 4"),
        "avatar-group geometry must survive DSP code generation"
    );
    assert!(
        generated.contains("opacity: Some(0.5)") && generated.contains("translate_y: Some(2.0)"),
        "state opacity and visual translation must survive DSP code generation"
    );
}

#[test]
fn bundled_packages_declare_the_complete_component_recipes() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();

    for package in [
        "default",
        "cupertino",
        "fluent2",
        "liquid-glass",
        "material3",
    ] {
        let path = repo
            .join("crates/core/fission-theme/design")
            .join(package)
            .join("dsp.json");
        let raw = std::fs::read_to_string(&path).unwrap();
        let dsp: serde_json::Value = serde_json::from_str(&raw).unwrap();

        for pointer in [
            "/components/button/hierarchies/outline",
            "/components/select/sizes",
            "/components/menu/trigger/sizes",
            "/components/menu/separator/margin",
            "/components/alert/surface/min_height",
            "/components/empty_state/surface/min_height",
            "/components/empty_state/narrow_surface/min_height",
            "/components/empty_state/narrow_breakpoint",
            "/components/card/sizes",
            "/components/card/footer",
            "/components/card/interaction/selected_indicator",
            "/components/modal/footer/margin",
            "/components/modal/motion_duration",
            "/components/modal/motion_initial_scale",
        ] {
            assert!(
                dsp.pointer(pointer).is_some(),
                "{} must declare {} explicitly",
                path.display(),
                pointer
            );
        }

        let outline = dsp
            .pointer("/components/button/hierarchies/outline")
            .unwrap();
        let menu_trigger = dsp.pointer("/components/menu/trigger/states").unwrap();
        for state in ["default", "hover", "active", "focus", "disabled"] {
            assert_eq!(
                menu_trigger.get(state),
                outline.get(state),
                "{} menu trigger {} state must preserve its Outline button recipe",
                path.display(),
                state
            );
        }
        assert_eq!(
            menu_trigger.get("selected"),
            outline.get("hover"),
            "{} expanded menu trigger must layer its Outline hover recipe",
            path.display()
        );
    }
}
