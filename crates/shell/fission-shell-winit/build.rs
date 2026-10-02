fn main() {
    println!("cargo:rerun-if-env-changed=FISSION_WEB_TEST_CONTROL");
    println!("cargo:rustc-check-cfg=cfg(fission_web_test_control)");

    if std::env::var_os("FISSION_WEB_TEST_CONTROL").is_some() {
        println!("cargo:rustc-cfg=fission_web_test_control");
    }
}
