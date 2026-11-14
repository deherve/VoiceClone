//! Catalogue des modeles GGUF et telechargement avec reprise.
use crate::{e, R};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, sync::atomic::{AtomicBool, Ordering}};

pub const CODEC: &str = "qwen-tokenizer-12hz-Q8_0.gguf";
pub const DEFAULT_TALKER: &str = "qwen-talker-1.7b-base-Q8_0.gguf";
pub const REPO: &str = "https://huggingface.co/Serveurperso/Qwen3-TTS-GGUF/resolve/main/";

/// (fichier, libelle francais, libelle anglais, taille en Mo). Seuls les modeles « base » savent cloner une voix.
pub const CATALOG: &[(&str, &str, &str, u32)] = &[
    (DEFAULT_TALKER, "Qwen3-TTS 1.7B · Q8 (recommandé)", "Qwen3-TTS 1.7B · Q8 (recommended)", 1983),
    ("qwen-talker-1.7b-base-BF16.gguf", "Qwen3-TTS 1.7B · BF16 (moins compressé, plus lourd)", "Qwen3-TTS 1.7B · BF16 (less compressed, heavier)", 3697),
    ("qwen-talker-1.7b-base-Q4_K_M.gguf", "Qwen3-TTS 1.7B · Q4 (plus léger)", "Qwen3-TTS 1.7B · Q4 (lighter)", 1163),
    (CODEC, "Codec audio 12 Hz (obligatoire)", "12 Hz audio codec (required)", 278),
];

/// Modeles Zonos 3-Mini (Zyphra, Apache-2.0), dans `<modeles>\zonos\`. Le serveur retrouve lui-meme les fichiers voisins du
/// modele principal. (identifiant `zonos/<fichier>`, libelle fr, libelle en, taille en Mo, indispensable)
pub const ZONOS_REPO: &str = "https://huggingface.co/Zyphra/ZONOS3-Mini/resolve/main/gguf/";
pub const ZONOS: &[(&str, &str, &str, u32, bool)] = &[
    ("zonos/backbone-ptq.gguf", "Zonos 3-Mini · modèle de voix", "Zonos 3-Mini · voice model", 663, true),
    ("zonos/zac.gguf", "Zonos 3-Mini · décodeur audio (qualité)", "Zonos 3-Mini · audio decoder (quality)", 2200, true),
    ("zonos/zac-keep2.gguf", "Zonos 3-Mini · décodeur audio (rapide)", "Zonos 3-Mini · audio decoder (fast)", 452, true),
    ("zonos/spk-encoder.gguf", "Zonos 3-Mini · empreinte de voix", "Zonos 3-Mini · voice embedding", 45, true),
    ("zonos/steer.gguf", "Zonos 3-Mini · expressivité", "Zonos 3-Mini · expressiveness", 1, true),
    ("zonos/zac-encoder.gguf", "Zonos 3-Mini · clonage de référence (1/2, facultatif)", "Zonos 3-Mini · reference cloning (1/2, optional)", 4035, false),
    ("zonos/ssl-teacher.gguf", "Zonos 3-Mini · clonage de référence (2/2, facultatif)", "Zonos 3-Mini · reference cloning (2/2, optional)", 1549, false),
];

/// Seuls les noms du catalogue sont acceptes : pas de chemin libre venant de l'interface.
pub fn is_known(file: &str) -> bool {
    CATALOG.iter().any(|m| m.0 == file) || ZONOS.iter().any(|m| m.0 == file)
}

/// Adresse de telechargement d'un modele du catalogue.
pub fn url(file: &str) -> Option<String> {
    match file.strip_prefix("zonos/") {
        Some(f) if is_known(file) => Some(format!("{ZONOS_REPO}{f}")),
        None if is_known(file) => Some(format!("{REPO}{file}")),
        _ => None,
    }
}

/// Telecharge `url` vers `dest` en passant par `dest.part` ; reprend un `.part` existant.
// shortcut: integrite verifiee par la taille seulement (HTTPS) ; comparer au sha256 publie par
// Hugging Face si des fichiers corrompus apparaissent.
pub fn download(url: &str, dest: &Path, cancel: &AtomicBool, progress: &dyn Fn(u64, u64)) -> R<()> {
    let part = PathBuf::from(format!("{}.part", dest.display()));
    if let Some(d) = dest.parent() {
        fs::create_dir_all(d).map_err(e)?;
    }
    let have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    // Delais : sans eux, une connexion figee bloque le telechargement (et son annulation) indefiniment.
    let agent = ureq::AgentBuilder::new().timeout_connect(std::time::Duration::from_secs(20)).timeout_read(std::time::Duration::from_secs(60)).build();
    let mut rq = agent.get(url);
    if have > 0 {
        rq = rq.set("Range", &format!("bytes={have}-"));
    }
    let resp = match rq.call() {
        Ok(r) => r,
        // .part deja complet ou incoherent : on repart de zero.
        Err(ureq::Error::Status(416, _)) => {
            fs::remove_file(&part).map_err(e)?;
            return download(url, dest, cancel, progress);
        }
        Err(x) => return Err(tr!("téléchargement impossible : {}", "download failed: {}", x)),
    };
    let resumed = resp.status() == 206;
    let mut done = if resumed { have } else { 0 };
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).map_or(0, |n| n + done);
    let mut out = fs::OpenOptions::new().create(true).write(true).append(resumed).truncate(!resumed).open(&part).map_err(e)?;
    let (mut reader, mut buf, mut reported) = (resp.into_reader(), vec![0u8; 1 << 16], done);
    progress(done, total);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(tr!("téléchargement annulé (il reprendra où il s'est arrêté)", "download cancelled (it will resume where it stopped)"));
        }
        let n = reader.read(&mut buf).map_err(|x| tr!("téléchargement interrompu : {}", "download interrupted: {}", x))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(e)?;
        done += n as u64;
        if done - reported >= 1 << 20 {
            reported = done;
            progress(done, total);
        }
    }
    drop(out);
    if total > 0 && done != total {
        return Err(tr!("téléchargement incomplet ({} octets sur {}) : relancez pour reprendre", "incomplete download ({} of {} bytes): retry to resume", done, total));
    }
    fs::rename(&part, dest).map_err(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicUsize, Arc};

    #[test]
    fn urls_follow_the_catalog() {
        assert_eq!(url("zonos/steer.gguf").unwrap(), "https://huggingface.co/Zyphra/ZONOS3-Mini/resolve/main/gguf/steer.gguf");
        assert!(url(DEFAULT_TALKER).unwrap().starts_with(REPO));
        assert!(url("zonos/../x.gguf").is_none() && url("autre.gguf").is_none());
    }

    #[test]
    fn download_full_resume_cancel() {
        // Petit serveur local qui gere Range, comme le CDN de Hugging Face.
        let data: Arc<Vec<u8>> = Arc::new((0..3_000_000u32).map(|i| (i % 251) as u8).collect());
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/m.gguf", server.server_addr().to_ip().unwrap().port());
        let ranged = Arc::new(AtomicUsize::new(0));
        let (d, r) = (data.clone(), ranged.clone());
        std::thread::spawn(move || {
            for rq in server.incoming_requests() {
                let from = rq.headers().iter().find(|h| h.field.equiv("Range"))
                    .and_then(|h| h.value.as_str().strip_prefix("bytes=")?.strip_suffix('-')?.parse::<usize>().ok());
                let _ = match from {
                    Some(n) => {
                        r.fetch_add(1, Ordering::Relaxed);
                        rq.respond(tiny_http::Response::from_data(d[n..].to_vec()).with_status_code(206).with_chunked_threshold(usize::MAX))
                    }
                    // Sans decoupage "chunked", pour envoyer Content-Length comme le CDN.
                    None => rq.respond(tiny_http::Response::from_data(d.to_vec()).with_chunked_threshold(usize::MAX)),
                };
            }
        });
        let dir = std::env::temp_dir().join(format!("voiceclone-dl-{}", std::process::id()));
        // Sous-dossier volontairement absent : le telechargement le cree (modeles Zonos dans `zonos\`).
        let (dest, go) = (dir.join("zonos").join("m.gguf"), AtomicBool::new(false));
        let part = PathBuf::from(format!("{}.part", dest.display()));

        let last = std::sync::Mutex::new((0, 0));
        download(&url, &dest, &go, &|a, b| *last.lock().unwrap() = (a, b)).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), *data);
        assert_eq!(last.lock().unwrap().1, 3_000_000);
        assert!(!part.exists());

        // Reprise : un .part de 1000 octets, le serveur ne renvoie que la suite.
        fs::remove_file(&dest).unwrap();
        fs::write(&part, &data[..1000]).unwrap();
        download(&url, &dest, &go, &|_, _| {}).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), *data);
        assert_eq!(ranged.load(Ordering::Relaxed), 1);

        // Annulation : erreur, pas de fichier final.
        fs::remove_file(&dest).unwrap();
        assert!(download(&url, &dest, &AtomicBool::new(true), &|_, _| {}).is_err());
        assert!(!dest.exists());
        let _ = fs::remove_dir_all(dir);
    }
}
