use super::*;
use anyhow::{bail, Context, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

const PLAY_API: &str = "https://androidpublisher.googleapis.com";
const GOOGLE_PLAY_SCOPE: &str = "https://www.googleapis.com/auth/androidpublisher";
const GOOGLE_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Deserialize, Default)]
struct ReleaseProviderToml {
    distribution: Option<DistributionToml>,
}

#[derive(Debug, Deserialize, Default)]
struct DistributionToml {
    play_store: Option<PlayStoreConfig>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct PlayStoreConfig {
    package_name: Option<String>,
    service_account: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleServiceAccount {
    client_email: String,
    private_key: String,
    #[serde(default)]
    token_uri: Option<String>,
}

#[derive(Debug, Serialize)]
struct GoogleJwtClaims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
}

pub(super) fn reviews_list(
    provider: publish::DistributionProvider,
    since: Option<String>,
    project_dir: &Path,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => {
            play_reviews_list(project_dir, since, json_output)
        }
        _ => unsupported_reviews(provider, "list"),
    }
}

pub(super) fn reviews_reply(
    provider: publish::DistributionProvider,
    review: &str,
    message_file: &Path,
    project_dir: &Path,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    match provider {
        publish::DistributionProvider::PlayStore => {
            play_reviews_reply(project_dir, review, message_file, dry_run, json_output)
        }
        _ => unsupported_reviews(provider, "reply"),
    }
}

fn play_reviews_list(project_dir: &Path, since: Option<String>, json_output: bool) -> Result<()> {
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

fn play_reviews_reply(
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

fn unsupported_reviews(provider: publish::DistributionProvider, action: &str) -> Result<()> {
    bail!(
        "{} review {} is not exposed by the current provider API backend; Google Play review list/reply is implemented",
        provider.as_str(),
        action
    )
}

fn latest_user_comment(review: &Value) -> Option<&Value> {
    review
        .get("comments")
        .and_then(Value::as_array)
        .and_then(|comments| {
            comments
                .iter()
                .rev()
                .find_map(|comment| comment.get("userComment"))
        })
}

fn play_config(project_dir: &Path) -> Result<PlayStoreConfig> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let root: ReleaseProviderToml =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(root
        .distribution
        .and_then(|distribution| distribution.play_store)
        .unwrap_or_default())
}

fn google_play_access_token(cfg: &PlayStoreConfig, client: &Client) -> Result<String> {
    if let Some(token) = env_value("PLAY_STORE_ACCESS_TOKEN") {
        return Ok(token);
    }
    let secret_source = env_value("PLAY_STORE_SERVICE_ACCOUNT_JSON")
        .or_else(|| env_value("GOOGLE_APPLICATION_CREDENTIALS"))
        .or_else(|| cfg.service_account.clone())
        .or_else(|| {
            provider_secret(publish::DistributionProvider::PlayStore, &[])
                .ok()
                .flatten()
        });
    let Some(source) = secret_source else {
        bail!("Google Play credentials are missing; set PLAY_STORE_SERVICE_ACCOUNT_JSON, PLAY_STORE_ACCESS_TOKEN, GOOGLE_APPLICATION_CREDENTIALS, or import play-store credentials")
    };
    if looks_like_bearer_token(&source) {
        return Ok(source);
    }
    let service_account = load_google_service_account(&source)?;
    service_account_access_token(&service_account, client)
}

fn service_account_access_token(account: &GoogleServiceAccount, client: &Client) -> Result<String> {
    let token_uri = account.token_uri.as_deref().unwrap_or(GOOGLE_TOKEN_URI);
    let iat = now_unix_seconds();
    let claims = GoogleJwtClaims {
        iss: &account.client_email,
        scope: GOOGLE_PLAY_SCOPE,
        aud: token_uri,
        iat,
        exp: iat + 3600,
    };
    let key = EncodingKey::from_rsa_pem(account.private_key.as_bytes())
        .context("failed to parse Google service account private_key as RSA PEM")?;
    let jwt = encode(&Header::new(Algorithm::RS256), &claims, &key)
        .context("failed to sign Google service account JWT")?;
    let response = client
        .post(token_uri)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", jwt.as_str()),
        ])
        .send()
        .context("failed to exchange Google service account JWT")?;
    let token: OAuthTokenResponse = response
        .error_for_status()
        .context("Google OAuth token exchange failed")?
        .json()
        .context("failed to parse Google OAuth token response")?;
    Ok(token.access_token)
}

fn load_google_service_account(source: &str) -> Result<GoogleServiceAccount> {
    let text = if source.trim_start().starts_with('{') {
        source.to_string()
    } else {
        fs::read_to_string(source)
            .with_context(|| format!("failed to read Google service account JSON from {source}"))?
    };
    serde_json::from_str(&text).context("failed to parse Google service account JSON")
}

fn json_response(response: Response, operation: &str) -> Result<Value> {
    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        bail!("{operation} failed with {status}: {text}");
    }
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {operation} response: {text}"))
}

fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent("fission-cli-release/0.1")
        .build()
        .context("failed to build release HTTP client")
}

fn looks_like_bearer_token(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.starts_with('{') && !Path::new(trimmed).exists() && trimmed.matches('.').count() >= 2
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_user_comment_uses_newest_user_comment() {
        let value = json!({
            "comments": [
                {"userComment": {"text": "old", "starRating": 2}},
                {"developerComment": {"text": "reply"}},
                {"userComment": {"text": "new", "starRating": 4}}
            ]
        });
        let comment = latest_user_comment(&value).unwrap();
        assert_eq!(comment.get("text").and_then(Value::as_str), Some("new"));
    }
}
