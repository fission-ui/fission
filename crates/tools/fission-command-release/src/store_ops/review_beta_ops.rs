use super::*;

pub(super) fn app_store_reviews_list(
    project_dir: &Path,
    since: Option<String>,
    json_output: bool,
) -> Result<()> {
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let url = format!(
        "{APP_STORE_API}/v1/apps/{app_id}/customerReviews?limit=200&sort=-createdDate&fields[customerReviews]=rating,title,body,reviewerNickname,createdDate,territory,response"
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to list App Store customer reviews")?;
    let value = json_response(response, "App Store customer reviews list")?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    println!("App Store reviews for app {app_id}");
    if let Some(since) = since {
        println!("Requested window: {since} (App Store Connect returned newest-first; filter locally if needed)");
    }
    for review in value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = review.get("id").and_then(Value::as_str).unwrap_or("<id>");
        let attrs = review.get("attributes").unwrap_or(&Value::Null);
        let rating = attrs
            .get("rating")
            .and_then(Value::as_i64)
            .map(|rating| rating.to_string())
            .unwrap_or_else(|| "?".to_string());
        let title = attrs.get("title").and_then(Value::as_str).unwrap_or("");
        let body = attrs
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .replace('\n', " ");
        println!("{id} [{rating}/5] {title}: {body}");
    }
    Ok(())
}

pub(super) fn app_store_reviews_reply(
    project_dir: &Path,
    review: &str,
    message_file: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let reply = fs::read_to_string(message_file)
        .with_context(|| format!("failed to read {}", message_file.display()))?;
    let payload = app_store_review_response_payload(review, reply.trim());
    if dry_run {
        let value = json!({
            "provider": "app-store",
            "review": review,
            "reply_text_bytes": reply.len(),
            "payload": payload,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!("Would reply to App Store review {review}");
        }
        return Ok(());
    }
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let response = client
        .post(format!("{APP_STORE_API}/v1/customerReviewResponses"))
        .bearer_auth(token)
        .json(&payload)
        .send()
        .context("failed to reply to App Store review")?;
    let value = json_response(response, "App Store review reply")?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Replied to App Store review {review}");
    }
    Ok(())
}

pub(super) fn app_store_review_response_payload(review: &str, response_body: &str) -> Value {
    json!({
        "data": {
            "type": "customerReviewResponses",
            "attributes": {
                "responseBody": response_body,
            },
            "relationships": {
                "review": {
                    "data": {
                        "type": "customerReviews",
                        "id": review,
                    }
                }
            }
        }
    })
}

pub(super) fn play_reviews_list(
    project_dir: &Path,
    since: Option<String>,
    json_output: bool,
) -> Result<()> {
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play reviews")?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let url =
        format!("{PLAY_API}/androidpublisher/v3/applications/{package_name}/reviews?maxResults=50");
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to list Google Play reviews")?;
    let value = json_response(response, "Google Play reviews list")?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    println!("Google Play reviews for {package_name}");
    if let Some(since) = since {
        println!("Requested window: {since} (Google Play API pagination is returned newest-first; filter locally if needed)");
    }
    for review in value
        .get("reviews")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = review
            .get("reviewId")
            .and_then(Value::as_str)
            .unwrap_or("<id>");
        let author = review
            .get("authorName")
            .and_then(Value::as_str)
            .unwrap_or("<anonymous>");
        let user = latest_user_comment(review);
        let rating = user
            .and_then(|comment| comment.get("starRating"))
            .and_then(Value::as_i64)
            .map(|rating| rating.to_string())
            .unwrap_or_else(|| "?".to_string());
        let text = user
            .and_then(|comment| comment.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .replace('\n', " ");
        println!("{id} [{rating}/5] {author}: {text}");
    }
    Ok(())
}

pub(super) fn play_reviews_reply(
    project_dir: &Path,
    review: &str,
    message_file: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play reviews")?;
    let reply = fs::read_to_string(message_file)
        .with_context(|| format!("failed to read {}", message_file.display()))?;
    if dry_run {
        let value = json!({
            "provider": "play-store",
            "package_name": package_name,
            "review": review,
            "reply_text_bytes": reply.len(),
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!("Would reply to Google Play review {review} for {package_name}");
        }
        return Ok(());
    }
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/reviews/{review}:reply"
    );
    let response = client
        .post(url)
        .bearer_auth(token)
        .json(&json!({ "replyText": reply.trim() }))
        .send()
        .context("failed to reply to Google Play review")?;
    let value = json_response(response, "Google Play review reply")?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Replied to Google Play review {review}");
    }
    Ok(())
}

pub(super) fn play_beta_groups_list(project_dir: &Path, json_output: bool) -> Result<()> {
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play beta groups")?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let mut tracks = Vec::new();
    for track in ["internal", "closed", "open"] {
        let url = format!(
            "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/testers/{track}"
        );
        let response = client
            .get(url)
            .bearer_auth(&token)
            .send()
            .with_context(|| format!("failed to get Google Play testers for {track}"))?;
        let value = json_response(response, "Google Play testers get")?;
        tracks.push(json!({
            "track": track,
            "googleGroups": value.get("googleGroups").cloned().unwrap_or_else(|| json!([]))
        }));
    }
    let value = json!({ "package_name": package_name, "tracks": tracks });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Google Play tester groups for {package_name}");
        for track in value
            .get("tracks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = track
                .get("track")
                .and_then(Value::as_str)
                .unwrap_or("<track>");
            let groups = track
                .get("googleGroups")
                .and_then(Value::as_array)
                .map(|groups| {
                    groups
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            println!("{name}: {groups}");
        }
    }
    Ok(())
}

pub(super) fn app_store_beta_groups_list(project_dir: &Path, json_output: bool) -> Result<()> {
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let value = app_store_beta_groups(&client, &token, &app_id)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("App Store TestFlight groups for app {app_id}");
        for group in value
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = group.get("id").and_then(Value::as_str).unwrap_or("<id>");
            let attrs = group.get("attributes").unwrap_or(&Value::Null);
            let name = attrs
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<name>");
            let public_link = attrs
                .get("publicLink")
                .and_then(Value::as_str)
                .unwrap_or("");
            println!("{id} {name} {public_link}");
        }
    }
    Ok(())
}

pub(super) fn app_store_beta_testers_import(
    project_dir: &Path,
    group: Option<&str>,
    csv: &Path,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("beta testers import mutates provider tester state; pass --yes after reviewing the CSV and group");
    }
    let group = group.context("App Store tester import requires --group <group-id-or-name>")?;
    let testers = read_app_store_tester_csv(csv)?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let group_id = resolve_app_store_beta_group(&client, &token, &app_id, group)?;
    if dry_run {
        let value = json!({
            "provider": "app-store",
            "app_id": app_id,
            "group": group,
            "group_id": group_id,
            "testers": testers,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "Would import {} App Store TestFlight tester(s) into group {group}",
                testers.len()
            );
        }
        return Ok(());
    }
    let mut responses = Vec::new();
    for tester in &testers {
        let response = client
            .post(format!("{APP_STORE_API}/v1/betaTesters"))
            .bearer_auth(&token)
            .json(&app_store_beta_tester_payload(tester, &group_id))
            .send()
            .with_context(|| format!("failed to create App Store beta tester {}", tester.email))?;
        responses.push(json_response(response, "App Store beta tester create")?);
    }
    let value = json!({
        "provider": "app-store",
        "app_id": app_id,
        "group": group,
        "group_id": group_id,
        "created": responses.len(),
        "responses": responses,
        "status": "imported"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!(
            "Imported {} App Store TestFlight tester(s) into group {group}",
            responses.len()
        );
    }
    Ok(())
}

pub(super) fn app_store_beta_testers_export(
    project_dir: &Path,
    group: Option<&str>,
    output: &Path,
    json_output: bool,
) -> Result<()> {
    let group = group.context("App Store tester export requires --group <group-id-or-name>")?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let group_id = resolve_app_store_beta_group(&client, &token, &app_id, group)?;
    let url = format!(
        "{APP_STORE_API}/v1/betaGroups/{group_id}/betaTesters?limit=200&fields[betaTesters]=email,firstName,lastName,inviteType,state"
    );
    let response = client
        .get(url)
        .bearer_auth(&token)
        .send()
        .context("failed to list App Store beta testers")?;
    let value = json_response(response, "App Store beta testers list")?;
    let testers = value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(app_store_tester_from_value)
        .collect::<Vec<_>>();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut csv = String::from("email,first_name,last_name\n");
    for tester in &testers {
        csv.push_str(&format!(
            "{},{},{}\n",
            csv_cell(&tester.email),
            csv_cell(tester.first_name.as_deref().unwrap_or("")),
            csv_cell(tester.last_name.as_deref().unwrap_or(""))
        ));
    }
    fs::write(output, csv)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "provider": "app-store",
                "app_id": app_id,
                "group": group,
                "group_id": group_id,
                "output": output,
                "count": testers.len()
            }))?
        );
    } else {
        println!(
            "Exported {} App Store TestFlight tester(s) to {}",
            testers.len(),
            output.display()
        );
    }
    Ok(())
}

pub(super) fn app_store_beta_distribute(
    project_dir: &Path,
    artifact: &Path,
    group: Option<&str>,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("App Store TestFlight distribution mutates beta group build assignment; pass --yes after reviewing the group and build");
    }
    let group = group.context("App Store beta distribution requires --group <group-id-or-name>")?;
    let cfg = app_store_config(project_dir)?;
    let client = http_client()?;
    let token = app_store_access_token(&cfg)?;
    let app_id = app_store_app_id(&cfg, &client, &token)?;
    let group_id = resolve_app_store_beta_group(&client, &token, &app_id, group)?;
    let artifact_build = app_store_artifact_build_number(artifact)?;
    let build = resolve_app_store_beta_build(&client, &token, &app_id, artifact_build.as_deref())?;
    let payload = app_store_beta_build_assignment_payload(&build.id);

    if dry_run {
        let value = json!({
            "provider": "app-store",
            "app_id": app_id,
            "group": group,
            "group_id": group_id,
            "artifact_manifest": artifact.display().to_string(),
            "build": build,
            "payload": payload,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "Would assign App Store build {} to TestFlight group {group}",
                build.version
            );
        }
        return Ok(());
    }

    ensure_app_store_build_assignable(&build)?;
    let response = client
        .post(format!(
            "{APP_STORE_API}/v1/betaGroups/{group_id}/relationships/builds"
        ))
        .bearer_auth(&token)
        .json(&payload)
        .send()
        .context("failed to assign App Store build to TestFlight group")?;
    let status = response.status();
    let text = response.text()?;
    let value = if status.is_success() {
        if text.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&text).with_context(|| {
                format!("failed to parse TestFlight assignment response: {text}")
            })?
        }
    } else if status.as_u16() == 409 {
        json!({
            "warning": "already-assigned",
            "provider_response": text
        })
    } else {
        bail!("App Store TestFlight build assignment failed with {status}: {text}");
    };
    let summary = json!({
        "provider": "app-store",
        "app_id": app_id,
        "group": group,
        "group_id": group_id,
        "artifact_manifest": artifact.display().to_string(),
        "build": build,
        "response": value,
        "status": if status.as_u16() == 409 { "already-assigned" } else { "assigned" }
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!(
            "Assigned App Store build {} to TestFlight group {group}",
            build.version
        );
    }
    Ok(())
}

pub(super) fn play_beta_groups_sync(
    project_dir: &Path,
    source: &Path,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("beta groups sync mutates provider tester-group state; pass --yes after reviewing the track/group mapping");
    }
    let source = if source.is_absolute() {
        source.to_path_buf()
    } else {
        project_dir.join(source)
    };
    let root = read_release_provider_toml_from_path(&source)?;
    let tracks = root
        .beta
        .and_then(|beta| beta.play_store)
        .map(|play| play.tracks)
        .unwrap_or_default();
    if tracks.is_empty() {
        bail!(
            "{} does not contain [beta.play_store.tracks.<track>] entries",
            source.display()
        );
    }
    let updates = tracks
        .into_iter()
        .map(|(track, config)| {
            let mut groups = config.groups;
            if let Some(group) = config.group {
                groups.push(group);
            }
            groups.retain(|group| !group.trim().is_empty());
            groups.sort();
            groups.dedup();
            if groups.is_empty() {
                bail!("beta.play_store.tracks.{track} must set group or groups");
            }
            if config
                .tester_source
                .as_deref()
                .is_some_and(|source| source != "google_group")
            {
                bail!("Google Play beta group sync supports tester_source = \"google_group\" for track {track}");
            }
            Ok((track, groups))
        })
        .collect::<Result<Vec<_>>>()?;
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play beta group sync")?;
    if dry_run {
        let value = json!({
            "provider": "play-store",
            "package_name": package_name,
            "tracks": updates.iter().map(|(track, groups)| json!({"track": track, "googleGroups": groups})).collect::<Vec<_>>(),
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "Would sync Google Play tester groups for {} track(s)",
                updates.len()
            );
        }
        return Ok(());
    }
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let mut responses = Vec::new();
    for (track, groups) in &updates {
        let url = format!(
            "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/testers/{track}"
        );
        let response = client
            .put(url)
            .bearer_auth(&token)
            .json(&json!({ "googleGroups": groups }))
            .send()
            .with_context(|| format!("failed to update Google Play testers for {track}"))?;
        responses.push(json_response(response, "Google Play testers update")?);
    }
    validate_play_edit(&client, &token, package_name, &edit_id)?;
    commit_play_edit(&client, &token, package_name, &edit_id)?;
    let value = json!({
        "provider": "play-store",
        "package_name": package_name,
        "tracks": updates.iter().map(|(track, groups)| json!({"track": track, "googleGroups": groups})).collect::<Vec<_>>(),
        "responses": responses,
        "status": "synced"
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!(
            "Synced Google Play tester groups for {} track(s)",
            updates.len()
        );
    }
    Ok(())
}

pub(super) fn play_beta_testers_import(
    project_dir: &Path,
    track: Option<&str>,
    csv: &Path,
    dry_run: bool,
    yes: bool,
    json_output: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("beta testers import mutates provider tester state; pass --yes after reviewing the CSV and track");
    }
    let track = track.context("Google Play tester import requires --track internal|closed|open")?;
    let groups = read_google_group_csv(csv)?;
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play beta testers")?;
    if dry_run {
        let value = json!({
            "provider": "play-store",
            "package_name": package_name,
            "track": track,
            "googleGroups": groups,
            "status": "dry-run"
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "Would set {} Google Groups on Play track {track}",
                value
                    .get("googleGroups")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0)
            );
        }
        return Ok(());
    }
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/testers/{track}"
    );
    let response = client
        .put(url)
        .bearer_auth(&token)
        .json(&json!({ "googleGroups": groups }))
        .send()
        .context("failed to update Google Play testers")?;
    let value = json_response(response, "Google Play testers update")?;
    validate_play_edit(&client, &token, package_name, &edit_id)?;
    commit_play_edit(&client, &token, package_name, &edit_id)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Updated Google Play tester groups for {package_name} track {track}");
    }
    Ok(())
}

pub(super) fn play_beta_testers_export(
    project_dir: &Path,
    track: Option<&str>,
    output: &Path,
    json_output: bool,
) -> Result<()> {
    let track = track.context("Google Play tester export requires --track internal|closed|open")?;
    let cfg = play_config(project_dir)?;
    let package_name = cfg
        .package_name
        .as_deref()
        .context("distribution.play_store.package_name is required for Play beta testers")?;
    let client = http_client()?;
    let token = google_play_access_token(&cfg, &client)?;
    let edit_id = create_play_edit(&client, &token, package_name)?;
    let url = format!(
        "{PLAY_API}/androidpublisher/v3/applications/{package_name}/edits/{edit_id}/testers/{track}"
    );
    let response = client
        .get(url)
        .bearer_auth(&token)
        .send()
        .context("failed to get Google Play testers")?;
    let value = json_response(response, "Google Play testers get")?;
    let groups = value
        .get("googleGroups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, groups.join("\n") + "\n")?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "provider": "play-store",
                "package_name": package_name,
                "track": track,
                "output": output,
                "googleGroups": groups
            }))?
        );
    } else {
        println!(
            "Exported {} Google Groups to {}",
            groups.len(),
            output.display()
        );
    }
    Ok(())
}
