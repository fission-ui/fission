use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let dsp_path = manifest_dir.join("../../crates/core/fission-theme/design/default/dsp.json");
    // An application design system customizes the components it cares about
    // and inherits the rest from Fission's default design system.
    fission_design_system_codegen::generate(
        fission_design_system_codegen::Config::new(dsp_path)
            .out_file("todo_design_system.rs")
            .type_name("TodoDesignSystem")
            .crate_path("fission::theme"),
    )
    .expect("failed to generate TodoDesignSystem from DSP JSON");
}
