use super::*;

pub(super) fn unique_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fission-publish-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

pub(super) fn write_minimal_site(dir: &Path) {
    fs::create_dir_all(dir.join("content")).unwrap();
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::write(
        dir.join("assets/app-icon.png"),
        [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H',
            b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, b'I', b'D', b'A', b'T', 0x78,
            0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00,
            0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82,
        ],
    )
    .unwrap();
    fs::write(
        dir.join("fission.toml"),
        r#"targets = ["static-site"]

[app]
name = "site-demo"
app_id = "com.example.site_demo"

[site]
title = "Site Demo"
out_dir = "dist/site"
generate_sitemap = false
generate_robots = false

[distribution.github_pages.production]
owner = "example"
repo = "site-demo"
mode = "actions"
site_kind = "project"
base_path = "/site-demo/"

[distribution.github_releases.production]
owner = "example"
repo = "site-demo"
tag = "v1.2.3"
name = "Site Demo 1.2.3"
draft = true
prerelease = false
replace_assets = true
upload_artifact_manifest = true
"#,
    )
    .unwrap();
    fs::write(
        dir.join("content/index.md"),
        "---\ntitle: Home\n---\n# Home\n",
    )
    .unwrap();
}
