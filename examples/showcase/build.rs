use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=design/dsp.json");
    println!("cargo:rerun-if-changed=design/tokens.json");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    fission_design_system_codegen::generate(fission_design_system_codegen::Config {
        dsp_path: manifest_dir.join("design/dsp.json"),
        out_file: "showcase_design_system.rs".into(),
        type_name: "ShowcaseDesignSystem".into(),
        crate_path: "fission::theme".into(),
    })
    .expect("failed to generate ShowcaseDesignSystem from design/dsp.json");
}
