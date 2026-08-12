#[test]
fn bundled_lock_is_well_formed() {
    fission_skia_artifacts::validate_bundled_lock().unwrap();
}
