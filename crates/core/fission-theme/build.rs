fn main() {
    generate(
        "default",
        "generated_default_design_system.rs",
        "FissionDefaultDesignSystem",
    );
    generate(
        "material3",
        "generated_material3_design_system.rs",
        "FissionMaterialDesign3DesignSystem",
    );
    generate(
        "fluent2",
        "generated_fluent2_design_system.rs",
        "FissionFluent2DesignSystem",
    );
    generate(
        "liquid-glass",
        "generated_liquid_glass_design_system.rs",
        "FissionLiquidGlassDesignSystem",
    );
    generate(
        "cupertino",
        "generated_cupertino_design_system.rs",
        "FissionCupertinoDesignSystem",
    );
}

fn generate(directory: &str, out_file: &str, type_name: &str) {
    println!("cargo:rerun-if-changed=design/{directory}/dsp.json");
    println!("cargo:rerun-if-changed=design/{directory}/tokens.json");
    fission_design_system_codegen::generate(fission_design_system_codegen::Config {
        dsp_path: format!("design/{directory}/dsp.json").into(),
        out_file: out_file.into(),
        type_name: type_name.into(),
        crate_path: "crate".into(),
    })
    .unwrap_or_else(|error| {
        panic!("failed to generate {directory} Fission design system: {error}")
    });
}
