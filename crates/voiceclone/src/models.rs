//! Catalogue des modeles GGUF et telechargement avec reprise.
use crate::{e, R};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, sync::atomic::{AtomicBool, Ordering}};

pub const CODEC: &str = "qwen-tokenizer-12hz-Q8_0.gguf";
pub const DEFAULT_TALKER: &str = "qwen-talker-1.7b-base-Q8_0.gguf";
pub const REPO: &str = "https://huggingface.co/Serveurperso/Qwen3-TTS-GGUF/resolve/main/";

/// (fichier, libelle, taille en Mo). Seuls les modeles « base » savent cloner une voix.
pub const CATALOG: &[(&str, &str, u32)] = &[
    (DEFAULT_TALKER, "Qwen3-TTS 1.7B · Q8 (recommandé)", 1983),
    ("qwen-talker-1.7b-base-BF16.gguf", "Qwen3-TTS 1.7B · BF16 (moins compressé, plus lourd)", 3697),
    ("qwen-talker-1.7b-base-Q4_K_M.gguf", "Qwen3-TTS 1.7B · Q4 (plus léger)", 1163),
    ("qwen-talker-0.6b-base-Q8_0.gguf", "Qwen3-TTS 0.6B · Q8 (plus rapide)", 947),
    ("qwen-talker-0.6b-base-Q4_K_M.gguf", "Qwen3-TTS 0.6B · Q4 (le plus léger)", 600),
    (CODEC, "Codec audio 12 Hz (obligatoire)", 278),
];

/// Seuls les noms du catalogue sont acceptes : pas de chemin libre venant de l'interface.
pub fn is_known(file: &str) -> bool {
    CATALOG.iter().any(|m| m.0 == file)
}

/// Telecharge `url` vers `dest` en passant par `dest.part` ; reprend un `.part` existant.
// shortcut: integrite verifiee par la taille seulement (HTTPS) ; comparer au sha256 publie par
// Hugging Face si des fichiers corrompus apparaissent.
pub fn download(url: &str, dest: &Path, cancel: &AtomicBool, progress: &dyn Fn(u64, u64)) -> R<()> {
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let mut rq = ureq::get(url);
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
        Err(x) => return Err(format!("téléchargement impossible : {x}")),
    };
    let resumed = resp.status() == 206;
    let mut done = if resumed { have } else { 0 };
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).map_or(0, |n| n + done);
    let mut out = fs::OpenOptions::new().create(true).write(true).append(resumed).truncate(!resumed).open(&part).map_err(e)?;
    let (mut reader, mut buf, mut reported) = (resp.into_reader(), vec![0u8; 1 << 16], done);
    progress(done, total);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("téléchargement annulé (il reprendra où il s'est arrêté)".into());
        }
        let n = reader.read(&mut buf).map_err(|x| format!("téléchargement interrompu : {x}"))?;
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
        return Err(format!("téléchargement incomplet ({done} octets sur {total}) : relancez pour reprendre"));
    }
    fs::rename(&part, dest).map_err(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicUsize, Arc};

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
        fs::create_dir_all(&dir).unwrap();
        let (dest, go) = (dir.join("m.gguf"), AtomicBool::new(false));
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
