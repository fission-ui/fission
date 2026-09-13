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
    // A partial design system that inherits from the default one, so tests can
    // check what inheritance keeps. It is not part of the crate's API.
    fission_design_system_codegen::generate(
        fission_design_system_codegen::Config::new("tests/fixtures/inheriting/dsp.json")
            .out_file("generated_inheritance_fixture_design_system.rs")
            .type_name("InheritanceFixtureDesignSystem")
            .crate_path("fission_theme"),
    )
    .unwrap_or_else(|error| panic!("failed to generate the inheritance fixture: {error}"));
}

fn generate(directory: &str, out_file: &str, type_name: &str) {
    println!("cargo:rerun-if-changed=design/{directory}/dsp.json");
    println!("cargo:rerun-if-changed=design/{directory}/tokens.json");
    fission_design_system_codegen::generate(
        fission_design_system_codegen::Config::new(format!("design/{directory}/dsp.json"))
            .out_file(out_file)
            .type_name(type_name)
            .crate_path("crate")
            // Fission supplies these, so they are the complete authority for any
            // application that selects one. An omitted recipe must fail the build
            // rather than be inherited or fall back to the codegen's own geometry.
            .require_complete_components(),
    )
    .unwrap_or_else(|error| {
        panic!("failed to generate {directory} Fission design system: {error}")
    });
}
