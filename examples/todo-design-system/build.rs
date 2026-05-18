use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let dsp_path = manifest_dir.join("../../crates/core/fission-theme/design/default/dsp.json");
    fission_design_system_codegen::generate(fission_design_system_codegen::Config {
        dsp_path,
        out_file: "todo_design_system.rs".into(),
        type_name: "TodoDesignSystem".into(),
        crate_path: "fission::theme".into(),
    })
    .expect("failed to generate TodoDesignSystem from DSP JSON");
}
