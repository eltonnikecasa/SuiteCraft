use crate::models::{AssetRule, CatalogApp, ReleaseAsset, ReleaseInfo};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use url::Url;

pub struct ReleaseCache(pub Mutex<HashMap<String, (Instant, ReleaseInfo)>>);
impl Default for ReleaseCache {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}
#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}
#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    published_at: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

fn repo_slug(value: &str) -> Result<String, String> {
    let url = Url::parse(value).map_err(|_| "URL de repositório inválida")?;
    if url.scheme() != "https" || url.host_str() != Some("github.com") {
        return Err("Somente repositórios github.com via HTTPS são aceitos".into());
    }
    let parts: Vec<_> = url
        .path_segments()
        .ok_or("URL sem caminho")?
        .filter(|x| !x.is_empty())
        .collect();
    if parts.len() != 2 {
        return Err("A URL deve apontar para um repositório GitHub".into());
    }
    Ok(format!(
        "{}/{}",
        parts[0],
        parts[1].trim_end_matches(".git")
    ))
}
pub fn select_asset(assets: &[(String, String, u64)], rule: &AssetRule) -> Option<ReleaseAsset> {
    assets
        .iter()
        .filter_map(|(name, url, size)| {
            let lower = name.to_lowercase();
            let extension = rule
                .extensions
                .iter()
                .any(|x| lower.ends_with(&x.to_lowercase()));
            let included = rule
                .include
                .iter()
                .any(|x| lower.contains(&x.to_lowercase()));
            let excluded = rule
                .exclude
                .iter()
                .any(|x| lower.contains(&x.to_lowercase()));
            (extension && included && !excluded && *size > 0).then(|| ReleaseAsset {
                name: name.clone(),
                url: url.clone(),
                size: *size,
                checksum: None,
            })
        })
        .min_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then(left.name.cmp(&right.name))
        })
}

fn select_release(
    releases: Vec<GithubRelease>,
    rule: &AssetRule,
) -> Option<(GithubRelease, ReleaseAsset)> {
    releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let candidates = release
                .assets
                .iter()
                .map(|asset| {
                    (
                        asset.name.clone(),
                        asset.browser_download_url.clone(),
                        asset.size,
                    )
                })
                .collect::<Vec<_>>();
            select_asset(&candidates, rule).map(|asset| (release, asset))
        })
        .max_by(|(left, _), (right, _)| {
            left.published_at
                .cmp(&right.published_at)
                .then(right.prerelease.cmp(&left.prerelease))
                .then(left.tag_name.cmp(&right.tag_name))
        })
}
pub async fn check(app: &CatalogApp, client: &reqwest::Client) -> Result<ReleaseInfo, String> {
    let repo = repo_slug(
        app.repository_url
            .as_deref()
            .ok_or("Fonte do app não configurada")?,
    )?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases?per_page=100"
        ))
        .send()
        .await
        .map_err(|e| format!("Falha de rede: {e}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("Repositório não encontrado ou indisponível".into());
    }
    if !response.status().is_success() {
        return Err(format!("GitHub respondeu HTTP {}", response.status()));
    }
    let releases: Vec<GithubRelease> = response
        .json()
        .await
        .map_err(|e| format!("Resposta inválida do GitHub: {e}"))?;
    let (release, asset) = select_release(releases, &app.asset_rule).ok_or_else(|| {
        format!(
            "Nenhum release oficial contém AppImage {} compatível",
            app.asset_rule.architecture
        )
    })?;
    Ok(ReleaseInfo {
        version: release.tag_name,
        release_url: release.html_url,
        published_at: release.published_at,
        reason: None,
        asset: Some(asset),
        cached: false,
    })
}
pub async fn all(
    apps: Vec<CatalogApp>,
    force: bool,
    cache: &ReleaseCache,
) -> Result<HashMap<String, ReleaseInfo>, String> {
    let client = reqwest::Client::builder()
        .user_agent("SuiteCraft/0.1 (GitHub Releases client)")
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let mut result = HashMap::new();
    for app in apps {
        if !force {
            if let Some((time, item)) = cache
                .0
                .lock()
                .map_err(|_| "Cache indisponível")?
                .get(&app.id)
            {
                if time.elapsed() < Duration::from_secs(900) {
                    let mut hit = item.clone();
                    hit.cached = true;
                    result.insert(app.id.clone(), hit);
                    continue;
                }
            }
        }
        match check(&app, &client).await {
            Ok(info) => {
                cache
                    .0
                    .lock()
                    .map_err(|_| "Cache indisponível")?
                    .insert(app.id.clone(), (Instant::now(), info.clone()));
                result.insert(app.id, info);
            }
            Err(reason) => {
                result.insert(
                    app.id,
                    ReleaseInfo {
                        version: String::new(),
                        release_url: String::new(),
                        published_at: None,
                        asset: None,
                        reason: Some(reason),
                        cached: false,
                    },
                );
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule() -> AssetRule {
        AssetRule {
            extensions: vec![".appimage".into()],
            include: vec!["x86_64".into(), "amd64".into()],
            exclude: vec![
                "arm".into(),
                "aarch64".into(),
                "zsync".into(),
                "sha256".into(),
            ],
            architecture: "x86_64".into(),
        }
    }
    #[test]
    fn accepts_only_matching_appimage() {
        let assets = vec![
            ("tool-aarch64.AppImage".into(), "a".into(), 1),
            ("tool-x86_64.AppImage.zsync".into(), "b".into(), 1),
            ("tool-x86_64.AppImage".into(), "c".into(), 10),
        ];
        assert_eq!(select_asset(&assets, &rule()).unwrap().url, "c")
    }
    #[test]
    fn rejects_ambiguous_asset() {
        let assets = vec![
            ("tool.AppImage".into(), "a".into(), 10),
            ("tool.tar.gz".into(), "b".into(), 10),
        ];
        assert!(select_asset(&assets, &rule()).is_none())
    }
    #[test]
    fn validates_repository_origin() {
        assert!(repo_slug("https://github.com/acme/app").is_ok());
        assert!(repo_slug("https://example.com/acme/app").is_err())
    }
    #[test]
    fn selects_latest_compatible_release_including_prereleases() {
        let releases = vec![
            GithubRelease {
                tag_name: "v1.0.0".into(),
                html_url: "stable".into(),
                published_at: Some("2026-01-01T00:00:00Z".into()),
                draft: false,
                prerelease: false,
                assets: vec![GithubAsset {
                    name: "tool-linux-x86_64.AppImage".into(),
                    browser_download_url: "stable-asset".into(),
                    size: 10,
                }],
            },
            GithubRelease {
                tag_name: "v1.1.0-alpha".into(),
                html_url: "alpha".into(),
                published_at: Some("2026-02-01T00:00:00Z".into()),
                draft: false,
                prerelease: true,
                assets: vec![GithubAsset {
                    name: "tool-linux-x86_64.AppImage".into(),
                    browser_download_url: "alpha-asset".into(),
                    size: 10,
                }],
            },
        ];
        let (release, asset) = select_release(releases, &rule()).unwrap();
        assert_eq!(release.tag_name, "v1.1.0-alpha");
        assert_eq!(asset.url, "alpha-asset");
    }
    #[test]
    fn ignores_newer_release_without_compatible_asset() {
        let releases = vec![
            GithubRelease {
                tag_name: "v1.0.0".into(),
                html_url: "compatible".into(),
                published_at: Some("2026-01-01T00:00:00Z".into()),
                draft: false,
                prerelease: false,
                assets: vec![GithubAsset {
                    name: "tool-linux-x86_64.AppImage".into(),
                    browser_download_url: "compatible-asset".into(),
                    size: 10,
                }],
            },
            GithubRelease {
                tag_name: "v2.0.0".into(),
                html_url: "arm".into(),
                published_at: Some("2026-02-01T00:00:00Z".into()),
                draft: false,
                prerelease: false,
                assets: vec![GithubAsset {
                    name: "tool-linux-aarch64.AppImage".into(),
                    browser_download_url: "arm-asset".into(),
                    size: 10,
                }],
            },
        ];
        assert_eq!(
            select_release(releases, &rule()).unwrap().0.tag_name,
            "v1.0.0"
        );
    }
}
