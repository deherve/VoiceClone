//! Detection d'une nouvelle version : derniere release publique du depot GitHub.
//!
//! Regle de numerotation des versions : on avance de 0.0.1, et apres le chiffre 9 on passe a la dizaine
//! suivante (0.1.8 → 0.1.9 → 0.2.0 → … → 0.9.9 → 1.0.0). La comparaison est numerique, champ par champ.
use crate::{components, e, models, R};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, sync::atomic::AtomicBool, time::Duration};

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
            // Empreinte SHA-256 publiee par GitHub pour ce fichier (« sha256:<64 hex> ») : sans elle, pas d'installation automatique.
            let sha = x["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()));
            (name.ends_with(".zip")).then(|| json!({"name": name, "size": x["size"].as_u64().unwrap_or(0), "url": url, "sha256": sha}))
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

const MAX_ARCHIVE: u64 = 300 << 20; // taille maximale acceptee pour l'archive de mise a jour
const MAX_UNPACKED: u64 = 800 << 20;

/// Nom d'archive attendu : `VoiceClone-<version>-windows-x64.zip`, rien d'autre.
fn is_archive_name(name: &str) -> bool {
    name.strip_prefix("VoiceClone-").and_then(|r| r.strip_suffix("-windows-x64.zip")).is_some_and(|v| parse_version(v).is_some())
}

/// Telecharge l'archive de la release, verifie son empreinte SHA-256 (celle de GitHub), la decompresse dans
/// `<donnees>\updates\new` et retourne ce dossier. Rien n'est installe ici. `asset` vient de `check` (jamais de l'interface).
pub fn stage(asset: &Value, data_dir: &Path, cancel: &AtomicBool, progress: &dyn Fn(u64, u64)) -> R<PathBuf> {
    let (name, url, size) = (asset["name"].as_str().unwrap_or(""), asset["url"].as_str().unwrap_or(""), asset["size"].as_u64().unwrap_or(0));
    let sha = asset["sha256"].as_str().ok_or_else(|| tr!("GitHub ne donne pas d'empreinte SHA-256 pour cette archive : installation automatique refusée", "GitHub gives no SHA-256 for this archive: automatic install refused"))?;
    if !is_archive_name(name) || !url.starts_with(&format!("{}download/", releases_prefix())) || size == 0 || size > MAX_ARCHIVE {
        return Err(tr!("archive de mise à jour refusée", "update archive refused"));
    }
    let dir = data_dir.join("updates");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(e)?;
    let zip = dir.join(name);
    models::download(url, &zip, cancel, progress)?;
    if !components::sha256_file(&zip)?.eq_ignore_ascii_case(sha) {
        let _ = fs::remove_file(&zip);
        return Err(tr!("l'empreinte SHA-256 de l'archive ne correspond pas : téléchargement supprimé", "the archive's SHA-256 does not match: download deleted"));
    }
    let out = dir.join("new");
    unpack(&zip, &out)?;
    for need in ["VoiceClone.exe", "voiceclone-server.exe"] {
        if !out.join(need).is_file() {
            return Err(tr!("archive incomplète : {} manque", "incomplete archive: {} is missing", need));
        }
    }
    let _ = fs::remove_file(&zip);
    Ok(out)
}

/// Decompresse sans jamais sortir de `out` (chemins verifies par `enclosed_name`), avec une limite de taille.
fn unpack(zip_path: &Path, out: &Path) -> R<()> {
    let mut z = zip::ZipArchive::new(fs::File::open(zip_path).map_err(e)?).map_err(e)?;
    if z.len() > 500 {
        return Err(tr!("archive de mise à jour refusée", "update archive refused"));
    }
    fs::create_dir_all(out).map_err(e)?;
    let mut total = 0u64;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(e)?;
        let Some(rel) = f.enclosed_name() else {
            return Err(tr!("archive de mise à jour refusée (chemin suspect)", "update archive refused (suspicious path)"));
        };
        let dest = out.join(rel);
        if f.is_dir() {
            fs::create_dir_all(&dest).map_err(e)?;
            continue;
        }
        total += f.size();
        if total > MAX_UNPACKED {
            return Err(tr!("archive de mise à jour refusée (trop volumineuse)", "update archive refused (too large)"));
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(e)?;
        }
        std::io::copy(&mut f, &mut fs::File::create(&dest).map_err(e)?).map_err(e)?;
    }
    Ok(())
}

const APPLY_SCRIPT: &str = r#"param([int]$ProcId, [string]$Src, [string]$Dst, [string]$Exe, [string]$Log)
$ErrorActionPreference = 'Stop'
try {
    if ($ProcId) { try { Wait-Process -Id $ProcId -Timeout 60 } catch { } }
    # Le serveur sans fenetre du meme dossier verrouille son fichier : on l'arrete.
    Get-Process voiceclone-server -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($Dst, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force
    Start-Sleep -Milliseconds 700
    # Les donnees de l'utilisateur ne sont jamais touchees : data, Audios, Voix, fichier portable.
    robocopy $Src $Dst /E /XD Voix data Audios /XF portable /R:10 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "robocopy $LASTEXITCODE" }
    Remove-Item $Src -Recurse -Force -ErrorAction SilentlyContinue
    'ok' | Out-File -LiteralPath $Log -Encoding utf8
} catch {
    ('error: ' + $_) | Out-File -LiteralPath $Log -Encoding utf8
}
Start-Process -FilePath $Exe
"#;

/// Lance l'installation : un script PowerShell attend la fermeture de l'application (`pid`), copie les fichiers
/// de `staged` sur `install_dir` puis relance `exe`. L'appelant doit quitter aussitot apres.
/// Le dossier du programme est-il modifiable ? (faux dans un dossier protege : installation manuelle.)
pub fn can_write(dir: &Path) -> bool {
    let probe = dir.join(".ecriture-maj");
    let ok = fs::write(&probe, b"").is_ok();
    let _ = fs::remove_file(&probe);
    ok
}

pub fn apply(staged: &Path, install_dir: &Path, exe: &Path, pid: u32, data_dir: &Path) -> R<()> {
    // Le dossier du programme doit etre modifiable (sinon : installation manuelle).
    if !can_write(install_dir) {
        return Err(tr!("le dossier du programme n'est pas modifiable : téléchargez l'archive et remplacez-le à la main", "the program folder is not writable: download the archive and replace it by hand"));
    }
    let dir = data_dir.join("updates");
    fs::create_dir_all(&dir).map_err(e)?;
    let script = dir.join("update.ps1");
    // BOM : PowerShell 5.1 lit sinon le script en ANSI.
    fs::write(&script, [&[0xEF, 0xBB, 0xBF][..], APPLY_SCRIPT.as_bytes()].concat()).map_err(e)?;
    let log = dir.join("update.log");
    let _ = fs::remove_file(&log);
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
        .arg(&script)
        .arg("-ProcId").arg(pid.to_string())
        .arg("-Src").arg(staged)
        .arg("-Dst").arg(install_dir)
        .arg("-Exe").arg(exe)
        .arg("-Log").arg(&log)
        .spawn()
        .map(|_| ())
        .map_err(e)
}

/// Resultat de la derniere installation (ecrit par le script), lu une seule fois au demarrage suivant.
pub fn take_update_log(data_dir: &Path) -> Option<String> {
    let path = data_dir.join("updates").join("update.log");
    let text = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    Some(text.trim_start_matches('\u{feff}').trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
        for (name, data) in entries {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn unpack_refuses_paths_that_escape() {
        let base = std::env::temp_dir().join(format!("voiceclone-unpack-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let (ok, bad) = (base.join("ok.zip"), base.join("bad.zip"));
        fs::write(&ok, zip_with(&[("VoiceClone.exe", b"x"), ("licences/a.txt", b"y"), ("./b.txt", b"z")])).unwrap();
        fs::write(&bad, zip_with(&[("../evil.txt", b"x")])).unwrap();
        unpack(&ok, &base.join("out")).unwrap();
        assert!(base.join("out/VoiceClone.exe").is_file() && base.join("out/licences/a.txt").is_file() && base.join("out/b.txt").is_file());
        assert!(unpack(&bad, &base.join("out2")).is_err());
        assert!(!base.join("evil.txt").exists());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn apply_script_replaces_the_program_but_never_user_data() {
        let base = std::env::temp_dir().join(format!("voiceclone-apply-{}", std::process::id()));
        let (install, staged, data) = (base.join("install"), base.join("staged"), base.join("appdata"));
        for d in [&install, &staged, &data, &install.join("data"), &install.join("Audios"), &install.join("Voix/Zen"), &staged.join("Voix/Zen"), &staged.join("data")] {
            fs::create_dir_all(d).unwrap();
        }
        for (p, c) in [("VoiceClone.exe", "old"), ("data/keep.txt", "mine"), ("Audios/a.wav", "audio"), ("Voix/Zen/x.txt", "ma voix"), ("portable", "")] {
            fs::write(install.join(p), c).unwrap();
        }
        for (p, c) in [("VoiceClone.exe", "new"), ("voiceclone-server.exe", "srv"), ("Voix/Zen/x.txt", "livree"), ("data/evil.txt", "x"), ("README.txt", "doc")] {
            fs::write(staged.join(p), c).unwrap();
        }
        let exe = Path::new(&std::env::var("SystemRoot").unwrap()).join("System32/whoami.exe"); // relance inoffensive
        apply(&staged, &install, &exe, 0, &data).unwrap();
        let log = data.join("updates/update.log");
        for _ in 0..60 {
            if log.is_file() && fs::read_to_string(&log).map_or(false, |l| !l.trim().is_empty()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        let result = take_update_log(&data).unwrap_or_default();
        assert_eq!(result, "ok", "{result}");
        let read = |p: &str| fs::read_to_string(install.join(p)).unwrap();
        assert_eq!((read("VoiceClone.exe").as_str(), read("voiceclone-server.exe").as_str(), read("README.txt").as_str()), ("new", "srv", "doc"));
        assert_eq!((read("data/keep.txt").as_str(), read("Voix/Zen/x.txt").as_str(), read("Audios/a.wav").as_str()), ("mine", "ma voix", "audio"));
        assert!(!install.join("data/evil.txt").exists() && install.join("portable").is_file() && !staged.exists());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn archive_names_and_missing_hash_are_refused() {
        assert!(is_archive_name("VoiceClone-0.1.9-windows-x64.zip"));
        for bad in ["VoiceClone-0.1.9.zip", "evil.zip", "VoiceClone-x-windows-x64.zip", "../VoiceClone-0.1.9-windows-x64.zip"] {
            assert!(!is_archive_name(bad), "{bad}");
        }
        let cancel = AtomicBool::new(false);
        let dir = std::env::temp_dir().join(format!("voiceclone-stage-{}", std::process::id()));
        let url = "https://github.com/deherve/VoiceClone/releases/download/v0.1.9/VoiceClone-0.1.9-windows-x64.zip";
        let no_hash = json!({"name": "VoiceClone-0.1.9-windows-x64.zip", "url": url, "size": 100, "sha256": null});
        assert!(stage(&no_hash, &dir, &cancel, &|_, _| {}).is_err());
        let evil_url = json!({"name": "VoiceClone-0.1.9-windows-x64.zip", "url": "https://evil.example/a.zip", "size": 100, "sha256": "0".repeat(64)});
        assert!(stage(&evil_url, &dir, &cancel, &|_, _| {}).is_err());
        let huge = json!({"name": "VoiceClone-0.1.9-windows-x64.zip", "url": url, "size": 1u64 << 40, "sha256": "0".repeat(64)});
        assert!(stage(&huge, &dir, &cancel, &|_, _| {}).is_err());
        assert!(!dir.join("updates").exists(), "refus avant tout telechargement");
    }

    #[test]
    #[ignore = "telecharge la derniere release (21 Mo)"]
    fn real_release_downloads_verifies_and_unpacks() {
        let info = check("0.0.1").unwrap();
        let asset = info["asset"].clone();
        assert!(asset["sha256"].is_string(), "{asset}");
        let dir = std::env::temp_dir().join(format!("voiceclone-realstage-{}", std::process::id()));
        let out = stage(&asset, &dir, &AtomicBool::new(false), &|_, _| {}).unwrap();
        assert!(out.join("VoiceClone.exe").is_file() && out.join("voiceclone-server.exe").is_file());
        let _ = fs::remove_dir_all(dir);
    }

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
