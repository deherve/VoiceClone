//! Detection d'une nouvelle version : derniere release publique du depot GitHub.
//!
//! Regle de numerotation des versions : on avance de 0.0.1, et apres le chiffre 9 on passe a la dizaine
//! suivante (0.1.8 → 0.1.9 → 0.2.0 → … → 0.9.9 → 1.0.0). La comparaison est numerique, champ par champ.
use crate::R;
use serde_json::{json, Value};
use std::time::Duration;

pub const REPO: &str = "deherve/VoiceClone";

fn releases_prefix() -> String {
    format!("https://github.com/{REPO}/releases/")
}

/// Version courante : celle du binaire, ou `VOICECLONE_CURRENT_VERSION` (essais de la detection, jamais en usage normal).
pub fn current_version() -> String {
    std::env::var("VOICECLONE_CURRENT_VERSION")
        .ok()
        .filter(|v| parse_version(v).is_some())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// « v0.1.2 » ou « 0.1.2 » → (0, 1, 2). Refuse tout le reste : suffixes, champ manquant ou en trop, signe, espace.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut parts = s.strip_prefix('v').unwrap_or(s).split('.');
    let mut next = || {
        let p = parts.next()?;
        if p.is_empty() || p.len() > 9 || !p.bytes().all(|b| b.is_ascii_digit()) { None } else { p.parse::<u32>().ok() }
    };
    let v = (next()?, next()?, next()?);
    if parts.next().is_some() { None } else { Some(v) }
}

/// `latest` est strictement plus recente que `current` (faux si l'une des deux est illisible).
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// Version suivante selon la regle ci-dessus (utile aux scripts et aux tests ; le script PowerShell applique la meme).
pub fn next_version(v: &str) -> Option<String> {
    let (mut major, mut minor, mut patch) = parse_version(v)?;
    patch += 1;
    if patch > 9 {
        patch = 0;
        minor += 1;
    }
    if minor > 9 {
        minor = 0;
        major += 1;
    }
    Some(format!("{major}.{minor}.{patch}"))
}

/// Adresse qu'on accepte d'ouvrir dans le navigateur : une page du depot, rien d'autre.
pub fn is_release_url(url: &str) -> bool {
    let rest = url.strip_prefix(releases_prefix().as_str());
    rest.is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._/".contains(&b)) && !r.contains(".."))
}

/// Valide la reponse de l'API GitHub (donnee externe, jamais crue sur parole) et en tire ce que l'interface affiche.
pub fn parse_release(body: &Value, current: &str) -> R<Value> {
    let bad = || tr!("réponse GitHub inattendue", "unexpected GitHub response");
    let tag = body["tag_name"].as_str().ok_or_else(bad)?;
    parse_version(tag).ok_or_else(bad)?;
    let url = body["html_url"].as_str().filter(|u| is_release_url(u)).ok_or_else(bad)?;
    // Texte seulement, borne : l'interface l'affiche avec textContent.
    let notes: String = body["body"].as_str().unwrap_or("").chars().take(3000).collect();
    let asset = body["assets"].as_array().and_then(|a| {
        a.iter().find_map(|x| {
            let name = x["name"].as_str()?;
            let url = x["browser_download_url"].as_str().filter(|u| u.starts_with(&format!("{}download/", releases_prefix())))?;
            (name.ends_with(".zip")).then(|| json!({"name": name, "size": x["size"].as_u64().unwrap_or(0), "url": url}))
        })
    });
    Ok(json!({
        "current": current,
        "latest": tag.trim_start_matches('v'),
        "newer": is_newer(tag, current),
        "url": url,
        "notes": notes,
        "published": body["published_at"].as_str().unwrap_or(""),
        "asset": asset,
    }))
}

/// Interroge GitHub (une requete, 8 s au plus). Aucune donnee de l'utilisateur n'est envoyee.
pub fn check(current: &str) -> R<Value> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = ureq::get(&url)
        .set("User-Agent", "VoiceClone-update-check")
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(8))
        .call()
        .map_err(|x| match x {
            ureq::Error::Status(404, _) => tr!("aucune version publiée sur GitHub", "no release published on GitHub"),
            ureq::Error::Status(403 | 429, _) => tr!("GitHub limite les requêtes, réessayez plus tard", "GitHub is rate limiting requests, try again later"),
            other => tr!("GitHub injoignable : {}", "cannot reach GitHub: {}", other),
        })?;
    let mut raw = String::new();
    std::io::Read::read_to_string(&mut std::io::Read::take(resp.into_reader(), 1 << 20), &mut raw).map_err(|x| x.to_string())?;
    let body: Value = serde_json::from_str(&raw).map_err(|_| tr!("réponse GitHub inattendue", "unexpected GitHub response"))?;
    parse_release(&body, current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_compare() {
        assert_eq!(parse_version("0.1.9"), Some((0, 1, 9)));
        assert_eq!(parse_version("v1.12.3"), Some((1, 12, 3)));
        for bad in ["", "1", "1.2", "1.2.3.4", "1.2.x", "1.2.3-beta", "+1.2.3", " 1.2.3", "1..3", "v", "1.2.99999999999"] {
            assert_eq!(parse_version(bad), None, "{bad:?}");
        }
        assert!(is_newer("0.2.0", "0.1.9") && is_newer("v0.1.10", "0.1.9") && is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.9", "0.1.9") && !is_newer("0.1.8", "0.1.9") && !is_newer("n'importe quoi", "0.1.0"));
    }

    #[test]
    fn version_rollover_rule() {
        let steps: Vec<_> = ["0.0.1", "0.0.9", "0.1.0", "0.1.8", "0.1.9", "0.2.0", "0.8.9", "0.9.9", "1.0.0"].iter().map(|v| next_version(v).unwrap()).collect();
        assert_eq!(steps, ["0.0.2", "0.1.0", "0.1.1", "0.1.9", "0.2.0", "0.2.1", "0.9.0", "1.0.0", "1.0.1"]);
        assert_eq!(next_version("pas une version"), None);
    }

    #[test]
    fn release_urls_are_restricted_to_the_repo() {
        assert!(is_release_url("https://github.com/deherve/VoiceClone/releases/tag/v0.1.1"));
        for bad in ["https://github.com/deherve/VoiceClone/releases/", "https://github.com/deherve/VoiceClone/issues/1", "https://evil.example/deherve/VoiceClone/releases/tag/v1",
                    "https://github.com/deherve/VoiceClone/releases/tag/../../x", "https://github.com/deherve/VoiceClone/releases/tag/v1&calc", "http://github.com/deherve/VoiceClone/releases/tag/v1",
                    "https://github.com.evil.example/deherve/VoiceClone/releases/tag/v1"] {
            assert!(!is_release_url(bad), "{bad}");
        }
    }

    #[test]
    fn parses_a_github_release_and_rejects_forged_ones() {
        let good = json!({"tag_name": "v0.1.2", "html_url": "https://github.com/deherve/VoiceClone/releases/tag/v0.1.2", "body": "Notes",
            "published_at": "2026-11-11T10:00:00Z",
            "assets": [{"name": "notes.txt", "size": 1, "browser_download_url": "https://github.com/deherve/VoiceClone/releases/download/v0.1.2/notes.txt"},
                       {"name": "VoiceClone-0.1.2-windows-x64.zip", "size": 19000000, "browser_download_url": "https://github.com/deherve/VoiceClone/releases/download/v0.1.2/VoiceClone-0.1.2-windows-x64.zip"}]});
        let r = parse_release(&good, "0.1.1").unwrap();
        assert_eq!((r["latest"].as_str(), r["newer"].as_bool(), r["asset"]["size"].as_u64()), (Some("0.1.2"), Some(true), Some(19000000)));
        assert_eq!(parse_release(&good, "0.1.2").unwrap()["newer"], false);
        // Adresse hors depot, tag illisible, ou fichier venant d'ailleurs : refus ou fichier ignore.
        let mut forged = good.clone();
        forged["html_url"] = json!("https://evil.example/x");
        assert!(parse_release(&forged, "0.1.1").is_err());
        let mut tag = good.clone();
        tag["tag_name"] = json!("latest");
        assert!(parse_release(&tag, "0.1.1").is_err());
        let mut asset = good.clone();
        asset["assets"][1]["browser_download_url"] = json!("https://evil.example/VoiceClone.zip");
        assert!(parse_release(&asset, "0.1.1").unwrap()["asset"].is_null());
        assert!(parse_release(&json!({}), "0.1.1").is_err());
    }
}
