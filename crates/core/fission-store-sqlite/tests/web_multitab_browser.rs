use anyhow::Result;
use fission_test_driver::{run_browser_smoke, BrowserTestOptions};

#[test]
fn sqlite_store_is_shared_across_browser_contexts() -> Result<()> {
    let Ok(url) = std::env::var("FISSION_SQLITE_WEB_TEST_URL") else {
        eprintln!("set FISSION_SQLITE_WEB_TEST_URL to run the Web SQLite browser test");
        return Ok(());
    };

    let report = run_browser_smoke(BrowserTestOptions::new(url))?;
    assert_eq!(report.title, "Fission SQLite Web multi-context test");
    assert_eq!(report.body_text_len, "parent,peer".len());
    Ok(())
}
