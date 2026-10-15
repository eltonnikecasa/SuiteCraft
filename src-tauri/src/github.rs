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
#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}
#[derive(Deserialize)]
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
        .next()
}
pub async fn check(app: &CatalogApp, client: &reqwest::Client) -> Result<ReleaseInfo, String> {
    let repo = repo_slug(
        app.repository_url
            .as_deref()
            .ok_or("Fonte do app não configurada")?,
    )?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .send()
        .await
        .map_err(|e| format!("Falha de rede: {e}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("Nenhum release estável publicado".into());
    }
    if !response.status().is_success() {
        return Err(format!("GitHub respondeu HTTP {}", response.status()));
    }
    let release: GithubRelease = response
        .json()
        .await
        .map_err(|e| format!("Resposta inválida do GitHub: {e}"))?;
    if release.draft || release.prerelease {
        return Err("O release retornado não é estável".into());
    }
    let candidates = release
        .assets
        .into_iter()
        .map(|x| (x.name, x.browser_download_url, x.size))
        .collect::<Vec<_>>();
    let asset = select_asset(&candidates, &app.asset_rule);
    Ok(ReleaseInfo {
        version: release.tag_name,
        release_url: release.html_url,
        published_at: release.published_at,
        reason: if asset.is_none() {
            Some(format!(
                "Release sem AppImage {} compatível",
                app.asset_rule.architecture
            ))
        } else {
            None
        },
        asset,
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
                result.insert(app.id, info)
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
}
