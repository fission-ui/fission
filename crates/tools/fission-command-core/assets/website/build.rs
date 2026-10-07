fn main() {
    fission_design_system_codegen::generate(
        fission_design_system_codegen::Config::new("design/dsp.json")
            .out_file("website_design_system.rs")
            .type_name("WebsiteDesignSystem")
            .crate_path("fission::theme"),
    )
    .expect("failed to generate the website design system");
}
