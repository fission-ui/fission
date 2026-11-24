//! Checks the generated deployment workflow inside an isolated project fixture.

use super::*;

#[test]
fn github_pages_setup_writes_workflow() {
    let dir = unique_dir("github-setup");
    write_minimal_site(&dir);
    let config = load_publish_manifest(&dir).unwrap();
    setup_github_pages(
        &DistributeOptions {
            project_dir: dir.clone(),
            provider: DistributionProvider::GithubPages,
            action: DistributeAction::Setup,
            target: None,
            format: None,
            artifact: None,
            site: "production".to_string(),
            deploy: None,
            track: None,
            locales: Vec::new(),
            dry_run: false,
            yes: true,
            json: false,
        },
        &config,
    )
    .unwrap();
    let workflow = fs::read_to_string(dir.join(".github/workflows/fission-pages.yml")).unwrap();
    assert!(workflow.contains("actions/upload-pages-artifact"));
    assert!(workflow.contains("actions/deploy-pages"));
    assert!(workflow.contains("fission package"));
}
