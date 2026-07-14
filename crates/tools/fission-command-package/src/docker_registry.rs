use super::*;

pub(super) fn publish(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = docker_registry_config(config, &options.site)?;
    let metadata_tags = docker_metadata_tags(manifest)?;
    let push_tags = options
        .deploy
        .as_ref()
        .filter(|tag| !tag.trim().is_empty())
        .map(|tag| vec![tag.clone()])
        .or_else(|| non_empty_vec(cfg.tags))
        .unwrap_or_else(|| metadata_tags.clone());
    if push_tags.is_empty() {
        bail!("docker-registry publish requires [package.docker].tags, [distribution.docker_registry.<site>].tags, or image-metadata.json tags");
    }
    if find_in_path("docker").is_none() {
        bail!("docker was not found on PATH; run readiness distribute for remediation");
    }

    if options.dry_run {
        println!("Would push Docker image tags: {}", push_tags.join(", "));
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: "docker-registry".to_string(),
            site: options.site.clone(),
            action: "publish".to_string(),
            artifact_manifest: Some(artifact_path.display().to_string()),
            deployment_id: None,
            canonical_url: push_tags.first().cloned(),
            preview_url: None,
            custom_domain: None,
            status: "dry-run".to_string(),
            stdout: None,
            stderr: None,
            manual_follow_up: Vec::new(),
        });
    }

    let source_tag = metadata_tags
        .first()
        .cloned()
        .or_else(|| push_tags.first().cloned())
        .context("docker image metadata did not contain a usable source tag")?;
    let mut stdout = String::new();
    let mut stderr = String::new();
    for tag in &push_tags {
        if tag != &source_tag && !metadata_tags.iter().any(|item| item == tag) {
            let output = Command::new("docker")
                .args(["tag", source_tag.as_str(), tag.as_str()])
                .output()
                .with_context(|| format!("failed to run docker tag {source_tag} {tag}"))?;
            stdout.push_str(&String::from_utf8_lossy(&output.stdout));
            stderr.push_str(&String::from_utf8_lossy(&output.stderr));
            if !output.status.success() {
                bail!("docker tag failed with {}", output.status);
            }
        }
        let output = Command::new("docker")
            .args(["push", tag.as_str()])
            .output()
            .with_context(|| format!("failed to run docker push {tag}"))?;
        stdout.push_str(&String::from_utf8_lossy(&output.stdout));
        stderr.push_str(&String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            bail!("docker push {tag} failed with {}", output.status);
        }
    }

    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "docker-registry".to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: push_tags.first().cloned(),
        canonical_url: push_tags.first().cloned(),
        preview_url: None,
        custom_domain: None,
        status: "published".to_string(),
        stdout: Some(stdout),
        stderr: (!stderr.trim().is_empty()).then_some(stderr),
        manual_follow_up: vec![
            "Keep the artifact manifest and pushed image digest with the release record."
                .to_string(),
        ],
    })
}

pub(super) fn status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = docker_registry_config(config, &options.site)?;
    let (tags, artifact_manifest) = resolve_docker_status_tags(cfg, options.artifact.as_deref())?;
    if find_in_path("docker").is_none() {
        bail!("docker was not found on PATH; install Docker and authenticate with docker login");
    }

    let statuses = tags
        .iter()
        .map(|tag| inspect_docker_manifest_tag(tag))
        .collect::<Result<Vec<_>>>()?;
    let report = json!({ "tags": statuses });
    let status = docker_manifest_report_status(&statuses);
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "docker-registry".to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest,
        deployment_id: statuses.iter().find_map(|item| item.digest.clone()),
        canonical_url: tags.first().cloned(),
        preview_url: None,
        custom_domain: None,
        status: status.to_string(),
        stdout: Some(serde_json::to_string_pretty(&report)?),
        stderr: None,
        manual_follow_up: if status == "ok" {
            Vec::new()
        } else {
            vec!["Run `docker login`, verify registry permissions, and confirm the configured tags were pushed.".to_string()]
        },
    })
}

pub(super) fn readiness(
    site: &str,
    artifact: Option<&Path>,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = docker_registry_config(config, site)?;
    checks.push(check_tool(
        "release.docker_registry.docker_available",
        "docker",
        "Install Docker, authenticate with `docker login`, and ensure the Docker engine is reachable.",
    ));
    let configured_tags = cfg
        .tags
        .as_ref()
        .map(|tags| tags.iter().filter(|tag| !tag.trim().is_empty()).count())
        .unwrap_or(0);
    let metadata_tags = artifact
        .and_then(|artifact| read_artifact_manifest(artifact).ok())
        .and_then(|manifest| docker_metadata_tags(&manifest).ok())
        .unwrap_or_default();
    checks.push(check(
        "release.docker_registry.tags_available",
        CheckSeverity::Error,
        if configured_tags > 0 || !metadata_tags.is_empty() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "Docker image tags are available",
        Some(format!(
            "configured tags: {configured_tags}, package tags: {}",
            metadata_tags.len()
        )),
        vec!["Set [package.docker].tags before packaging, set [distribution.docker_registry.<site>].tags, or rebuild the Docker image package."],
    ));
    if let Some(path) = artifact {
        let manifest = read_artifact_manifest(path)?;
        checks.push(check(
            "release.docker_registry.artifact_is_docker_image",
            CheckSeverity::Error,
            if manifest.format == PackageFormat::DockerImage.as_str() {
                CheckStatus::Passed
            } else {
                CheckStatus::Failed
            },
            "artifact manifest describes a docker-image package",
            Some(format!("format = {}", manifest.format)),
            vec!["Run `fission package --target ssr --format docker-image --release` or `fission package --target static-site --format docker-image --release`."],
        ));
        checks.push(check_path(
            "release.docker_registry.image_metadata_exists",
            Path::new(&manifest.root_dir).join("image-metadata.json"),
            "image metadata exists",
            "Rebuild the Docker image package so image-metadata.json is present.",
        ));
    }
    Ok(())
}

fn docker_metadata_tags(manifest: &ArtifactManifest) -> Result<Vec<String>> {
    let metadata_path = Path::new(&manifest.root_dir).join("image-metadata.json");
    let data = fs::read_to_string(&metadata_path)
        .with_context(|| format!("failed to read {}", metadata_path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", metadata_path.display()))?;
    Ok(value
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter(|tag| !tag.trim().is_empty())
        .map(str::to_string)
        .collect())
}

fn non_empty_vec(values: Option<Vec<String>>) -> Option<Vec<String>> {
    let values = values?
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn resolve_docker_status_tags(
    cfg: DockerRegistryConfig,
    artifact: Option<&Path>,
) -> Result<(Vec<String>, Option<String>)> {
    if let Some(tags) = non_empty_vec(cfg.tags) {
        return Ok((tags, artifact.map(|path| path.display().to_string())));
    }
    let Some(artifact) = artifact else {
        bail!("docker-registry status requires distribution.docker_registry.<site>.tags or --artifact pointing at a docker-image artifact manifest");
    };
    let manifest = read_artifact_manifest(artifact)?;
    let tags = non_empty_vec(Some(docker_metadata_tags(&manifest)?))
        .context("docker-registry status requires configured tags or image-metadata.json tags")?;
    Ok((tags, Some(artifact.display().to_string())))
}

#[derive(Clone, Debug, Serialize)]
struct DockerManifestTagStatus {
    tag: String,
    status: String,
    digest: Option<String>,
    error: Option<String>,
}

fn inspect_docker_manifest_tag(tag: &str) -> Result<DockerManifestTagStatus> {
    let buildx = Command::new("docker")
        .args(["buildx", "imagetools", "inspect", tag])
        .output()
        .with_context(|| format!("failed to inspect Docker image tag {tag}"))?;
    if buildx.status.success() {
        let stdout = String::from_utf8_lossy(&buildx.stdout);
        return Ok(DockerManifestTagStatus {
            tag: tag.to_string(),
            status: "ok".to_string(),
            digest: extract_docker_manifest_digest(&stdout),
            error: None,
        });
    }

    let manifest = Command::new("docker")
        .args(["manifest", "inspect", "--verbose", tag])
        .output()
        .with_context(|| format!("failed to inspect Docker manifest for {tag}"))?;
    if manifest.status.success() {
        let stdout = String::from_utf8_lossy(&manifest.stdout);
        return Ok(DockerManifestTagStatus {
            tag: tag.to_string(),
            status: "ok".to_string(),
            digest: extract_docker_manifest_digest(&stdout),
            error: None,
        });
    }

    let buildx_err = String::from_utf8_lossy(&buildx.stderr);
    let manifest_err = String::from_utf8_lossy(&manifest.stderr);
    Ok(DockerManifestTagStatus {
        tag: tag.to_string(),
        status: "missing".to_string(),
        digest: None,
        error: Some(
            [buildx_err.trim(), manifest_err.trim()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .filter(|message| !message.is_empty()),
    })
}

fn docker_manifest_report_status(statuses: &[DockerManifestTagStatus]) -> &'static str {
    if statuses.iter().all(|item| item.status == "ok") {
        "ok"
    } else if statuses.iter().any(|item| item.status == "ok") {
        "partial"
    } else {
        "failed"
    }
}

fn extract_docker_manifest_digest(output: &str) -> Option<String> {
    if let Some(digest) = output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Digest:")
            .map(str::trim)
            .filter(|value| is_sha256_digest(value))
            .map(str::to_string)
    }) {
        return Some(digest);
    }

    let value: Value = serde_json::from_str(output).ok()?;
    descriptor_digest_from_json(&value)
}

fn descriptor_digest_from_json(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["Descriptor", "descriptor"] {
                if let Some(digest) = map
                    .get(key)
                    .and_then(|descriptor| descriptor.get("digest"))
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256_digest(value))
                {
                    return Some(digest.to_string());
                }
            }
            map.get("digest")
                .and_then(Value::as_str)
                .filter(|value| is_sha256_digest(value))
                .map(str::to_string)
        }
        Value::Array(items) => items.iter().find_map(descriptor_digest_from_json),
        _ => None,
    }
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("sha256:") else {
        return false;
    };
    rest.len() == 64 && rest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_tags_are_read_from_image_metadata() {
        let root = std::env::temp_dir().join(format!(
            "fission-docker-registry-metadata-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("image-metadata.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": 1,
                "tags": ["registry.example.com/app:1.2.3", "registry.example.com/app:latest"]
            }))
            .unwrap(),
        )
        .unwrap();
        let manifest = ArtifactManifest {
            schema_version: 1,
            created_at_unix_seconds: 0,
            project: ArtifactProject {
                app_id: "com.example.app".to_string(),
                name: "app".to_string(),
                build: Some(42),
                version: Some("1.2.3".to_string()),
            },
            target: "ssr".to_string(),
            format: "docker-image".to_string(),
            profile: "release".to_string(),
            root_dir: root.display().to_string(),
            source_config: Vec::new(),
            artifacts: Vec::new(),
            icon_manifest: None,
            signing: None,
            notarization: None,
            validation: ArtifactValidation {
                state: "passed".to_string(),
                checks: Vec::new(),
            },
        };

        let tags = docker_metadata_tags(&manifest).unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], "registry.example.com/app:1.2.3");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn status_tags_fall_back_to_artifact_image_metadata() {
        let root = std::env::temp_dir().join(format!(
            "fission-docker-registry-status-tags-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("image-root")).unwrap();
        fs::write(
            root.join("image-root/image-metadata.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": 1,
                "tags": ["registry.example.com/app:1.2.3"]
            }))
            .unwrap(),
        )
        .unwrap();
        let artifact_path = root.join("artifact-manifest.json");
        fs::write(
            &artifact_path,
            serde_json::to_vec_pretty(&ArtifactManifest {
                schema_version: 1,
                created_at_unix_seconds: 0,
                project: ArtifactProject {
                    app_id: "com.example.app".to_string(),
                    name: "app".to_string(),
                    build: Some(42),
                    version: Some("1.2.3".to_string()),
                },
                target: "ssr".to_string(),
                format: "docker-image".to_string(),
                profile: "release".to_string(),
                root_dir: root.join("image-root").display().to_string(),
                source_config: Vec::new(),
                artifacts: Vec::new(),
                icon_manifest: None,
                signing: None,
                notarization: None,
                validation: ArtifactValidation {
                    state: "passed".to_string(),
                    checks: Vec::new(),
                },
            })
            .unwrap(),
        )
        .unwrap();

        let (tags, artifact_manifest) =
            resolve_docker_status_tags(DockerRegistryConfig::default(), Some(&artifact_path))
                .unwrap();
        assert_eq!(tags, vec!["registry.example.com/app:1.2.3"]);
        assert_eq!(
            artifact_manifest.unwrap(),
            artifact_path.display().to_string()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn digest_is_read_from_buildx_output() {
        let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let output = format!(
            "Name: registry.example.com/app:1.2.3\nMediaType: application/vnd.oci.image.index.v1+json\nDigest: {digest}\n"
        );

        assert_eq!(
            extract_docker_manifest_digest(&output).as_deref(),
            Some(digest)
        );
    }

    #[test]
    fn digest_is_read_from_verbose_manifest_json() {
        let digest = "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let output = serde_json::json!([
            {
                "Ref": "registry.example.com/app:1.2.3",
                "Descriptor": {
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "digest": digest,
                    "size": 1234
                }
            }
        ])
        .to_string();

        assert_eq!(
            extract_docker_manifest_digest(&output).as_deref(),
            Some(digest)
        );
    }

    #[test]
    fn docker_manifest_report_status_tracks_partial_failure() {
        let statuses = vec![
            DockerManifestTagStatus {
                tag: "registry.example.com/app:1.2.3".to_string(),
                status: "ok".to_string(),
                digest: Some(
                    "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                ),
                error: None,
            },
            DockerManifestTagStatus {
                tag: "registry.example.com/app:latest".to_string(),
                status: "missing".to_string(),
                digest: None,
                error: Some("manifest unknown".to_string()),
            },
        ];

        assert_eq!(docker_manifest_report_status(&statuses), "partial");
    }
}
