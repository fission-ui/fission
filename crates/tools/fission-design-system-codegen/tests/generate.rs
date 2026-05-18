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
}
