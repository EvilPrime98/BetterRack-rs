//! Update check: on a packaged start, ask GitHub for the latest release
//! and offer to download its installer. A skipped version is remembered and not offered again.
//!
//! Network parts must run on the Tokio runtime (see [`crate::runtime::run`]).

use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

/// Override with `BETTERRACK_UPDATE_REPO=owner/name` (used for testing and forks).
const DEFAULT_REPO: &str = "EvilPrime98/betterrack-gpui";
const USER_AGENT: &str = "BetterRack";

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// A newer release with the installer for this platform.
#[derive(Debug, Clone)]
pub struct Update {
    pub version: String,
    pub asset_name: String,
    pub asset_url: String,
}

fn repo() -> String {
    std::env::var("BETTERRACK_UPDATE_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_string())
}

/// `1.2.3` / `v1.2.3` → numeric parts; anything unparsable counts as 0.
fn parts(v: &str) -> [u64; 3] {
    let mut out = [0; 3];
    for (slot, piece) in out.iter_mut().zip(v.trim_start_matches('v').split('.')) {
        *slot = piece
            .split(['-', '+'])
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
    }
    out
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    parts(candidate) > parts(current)
}

/// The Windows installer, or the Linux tarball.
fn asset_suffix() -> &'static str {
    if cfg!(windows) { ".exe" } else { ".tar.gz" }
}

fn pick(release: RawRelease, current: &str) -> Option<Update> {
    let version = release.tag_name.trim_start_matches('v').to_string();
    if release.draft || release.prerelease || !is_newer(&version, current) {
        return None;
    }
    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name.to_lowercase().ends_with(asset_suffix()))?;
    Some(Update {
        version,
        asset_name: asset.name,
        asset_url: asset.browser_download_url,
    })
}

/// `Some` when a newer, non-skipped release with a matching installer exists. Every failure is a
/// quiet `None` (logged): an update check must never bother the user.
pub async fn check(current: &str) -> Option<Update> {
    let result: Result<Option<Update>, reqwest::Error> = async {
        let resp = reqwest::Client::new()
            .get(format!(
                "https://api.github.com/repos/{}/releases/latest",
                repo()
            ))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", USER_AGENT)
            .timeout(Duration::from_secs(10))
            .send()
            .await?;
        if !resp.status().is_success() {
            tracing::info!("update check: GitHub answered {}", resp.status());
            return Ok(None);
        }
        Ok(pick(resp.json::<RawRelease>().await?, current))
    }
    .await;
    match result {
        Ok(Some(u)) if skipped().as_deref() == Some(u.version.as_str()) => None,
        Ok(update) => update,
        Err(e) => {
            tracing::info!("update check failed: {e}");
            None
        }
    }
}

fn skip_file() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("BetterRack")
            .join("skipped-update.txt"),
    )
}

fn skipped() -> Option<String> {
    std::fs::read_to_string(skip_file()?)
        .ok()
        .map(|s| s.trim().to_string())
}

/// "Skip this version".
pub fn skip(version: &str) {
    let Some(p) = skip_file() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&p, version) {
        tracing::warn!("could not remember the skipped update: {e}");
    }
}

/// Download the installer into the user's Downloads folder and return its path.
pub async fn download(update: &Update) -> Result<PathBuf, String> {
    // The asset name comes from the network: keep only its file name.
    let name = std::path::Path::new(&update.asset_name)
        .file_name()
        .ok_or("the release asset has no file name")?
        .to_owned();
    let dir = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
    let path = dir.join(name);

    let resp = reqwest::Client::new()
        .get(&update.asset_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("download failed ({})", resp.status()));
    }
    let mut out = tokio::fs::File::create(&path)
        .await
        .map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        out.write_all(&chunk.map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string())?;
    }
    out.flush().await.map_err(|e| e.to_string())?;
    tracing::info!("update {} downloaded to {}", update.version, path.display());
    Ok(path)
}

/// Run the installer on Windows, reveal the file elsewhere.
pub fn open_download(path: &std::path::Path) {
    let target = if cfg!(windows) {
        Some(path)
    } else {
        path.parent()
    };
    if let Some(t) = target {
        if let Err(e) = open::that(t) {
            tracing::warn!("could not open {}: {e}", t.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "0.10.0"));
        assert!(is_newer("1.2.1-beta", "1.2.0"));
    }

    #[test]
    fn ignores_drafts_prereleases_and_missing_assets() {
        let release = |draft, prerelease, assets: Vec<Asset>| RawRelease {
            tag_name: "v2.0.0".into(),
            draft,
            prerelease,
            assets,
        };
        let asset = || Asset {
            name: format!("BetterRack-Setup-2.0.0{}", asset_suffix()),
            browser_download_url: "https://example.test/a".into(),
        };
        assert!(pick(release(true, false, vec![asset()]), "1.0.0").is_none());
        assert!(pick(release(false, true, vec![asset()]), "1.0.0").is_none());
        assert!(pick(release(false, false, vec![]), "1.0.0").is_none());
        assert!(pick(release(false, false, vec![asset()]), "2.0.0").is_none());
        assert_eq!(
            pick(release(false, false, vec![asset()]), "1.0.0")
                .unwrap()
                .version,
            "2.0.0"
        );
    }
}
