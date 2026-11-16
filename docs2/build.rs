use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=design/dsp.json");
    println!("cargo:rerun-if-changed=design/tokens.json");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    fission_design_system_codegen::generate(
        fission_design_system_codegen::Config::new(manifest_dir.join("design/dsp.json"))
            .out_file("docs_design_system.rs")
            .type_name("DocsDesignSystem")
            .crate_path("fission::theme"),
    )
    .expect("failed to generate the documentation design system");
}
