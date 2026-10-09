use std::time::Duration;
use serde::{Deserialize, Serialize};

// Advance this commit only when upstream changes have actually been integrated.
pub const INTEGRATED_COMMIT: &str = "32b265d50e887f875f59fd45d60cd41eefaa2e0a";
const REPOSITORY: &str = "kazu00001/Whispera-K";

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub version: Option<String>,
    pub notes: String,
    pub url: String,
    pub phase: String,
    pub error: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    draft: bool,
    prerelease: bool,
}

#[derive(Deserialize)]
struct Comparison { status: String, ahead_by: u64 }

pub fn trusted_release(url: &str) -> bool {
    url.starts_with("https://github.com/kazu00001/Whispera-K/releases/tag/")
        && !url.contains(['?', '#', '\\'])
}

fn classify(release: Release, comparison: Comparison) -> Result<Status, String> {
    if release.draft || release.prerelease || !trusted_release(&release.html_url) {
        return Err("Publicacion de Whispera-K no valida".into());
    }
    let phase = match comparison.status.as_str() {
        "ahead" if comparison.ahead_by > 0 => "available",
        "identical" | "behind" => "current",
        _ => return Err("No se pudo confirmar la relacion con el codigo integrado".into()),
    };
    Ok(Status {version: Some(release.tag_name), notes: release.body.unwrap_or_default(),
        url: release.html_url, phase: phase.into(), error: String::new()})
}

async fn read<T: serde::de::DeserializeOwned>(client: &reqwest::Client, url: &str) -> Result<T, String> {
    let mut response = client.get(url).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 1_048_576 { return Err("Respuesta de GitHub demasiado grande".into()); }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

pub async fn check() -> Result<Status, String> {
    let client = reqwest::Client::builder().user_agent("Whispera-Upstream-Updates")
        .connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;
    let release: Release = read(&client, &format!("https://api.github.com/repos/{REPOSITORY}/releases/latest")).await?;
    // Encode the tag as a path component rather than interpolating untrusted URL syntax.
    let mut url = reqwest::Url::parse(&format!("https://api.github.com/repos/{REPOSITORY}/compare/")).unwrap();
    url.path_segments_mut().map_err(|_| "Ruta de GitHub invalida")?.pop_if_empty()
        .push(&format!("{INTEGRATED_COMMIT}...{}", release.tag_name));
    url.query_pairs_mut().append_pair("per_page", "1");
    let comparison = read(&client, url.as_str()).await?;
    classify(release, comparison)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Read-only requests to the public GitHub API"]
    fn live_upstream_release_is_compared_to_integrated_code() {
        let status=tauri::async_runtime::block_on(check()).unwrap();
        assert!(matches!(status.phase.as_str(), "available" | "current"));
        assert!(trusted_release(&status.url));
        println!("Whispera-K {:?}: {}", status.version, status.phase);
    }
    fn release() -> Release { Release {tag_name:"v0.2.23".into(), html_url:"https://github.com/kazu00001/Whispera-K/releases/tag/v0.2.23".into(), body:Some("Changes".into()), draft:false, prerelease:false} }
    #[test] fn equal_version_numbers_do_not_hide_unmerged_commits() {
        let status = classify(release(), Comparison {status:"ahead".into(), ahead_by:2}).unwrap();
        assert_eq!(status.phase, "available");
        assert_eq!(status.version.as_deref(), Some("v0.2.23"));
    }
    #[test] fn integrated_and_older_releases_are_not_new() {
        for relation in ["identical", "behind"] {
            assert_eq!(classify(release(), Comparison {status:relation.into(), ahead_by:0}).unwrap().phase, "current");
        }
    }
    #[test] fn divergent_history_and_untrusted_links_are_not_updates() {
        assert!(classify(release(), Comparison {status:"diverged".into(), ahead_by:3}).is_err());
        for url in ["https://github.com.evil.test/kazu00001/Whispera-K/releases/tag/v1", "https://github.com/SikJa/Whispera/releases/tag/v1", "https://github.com/kazu00001/Whispera-K/releases/tag/v1?redirect=x"] { assert!(!trusted_release(url)); }
        let mut r=release(); r.draft=true;
        assert!(classify(r, Comparison {status:"ahead".into(), ahead_by:1}).is_err());
    }
}
