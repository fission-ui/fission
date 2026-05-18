fn main() {
    println!("cargo:rerun-if-changed=design/default/dsp.json");
    println!("cargo:rerun-if-changed=design/default/tokens.json");
    fission_design_system_codegen::generate(fission_design_system_codegen::Config {
        dsp_path: "design/default/dsp.json".into(),
        out_file: "generated_default_design_system.rs".into(),
        type_name: "FissionDefaultDesignSystem".into(),
        crate_path: "crate".into(),
    })
    .expect("failed to generate default Fission design system");
}
