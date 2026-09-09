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
            && generated.contains("action_stack_breakpoint: 640.0"),
        "modal responsive geometry must survive DSP code generation"
    );
    assert!(
        generated.contains("(fission_theme::ComponentSize::Sm")
            && generated.contains("(fission_theme::ComponentSize::Md"),
        "card density recipes must survive DSP code generation"
    );
    assert!(
        generated.contains("footer_style: fission_theme::ResolvedComponentStyle"),
        "card footer anatomy must survive DSP code generation"
    );
    assert!(
        generated.contains("menu: fission_theme::MenuTheme")
            && generated.contains("destructive_item_states: fission_theme::ComponentStateStyles")
            && generated.contains("group_label_style: fission_theme::ResolvedComponentStyle"),
        "menu anatomy and state recipes must survive DSP code generation"
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
}
