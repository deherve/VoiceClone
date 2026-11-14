//! Moteur Zonos 3 (BANC D'ESSAI, hors version publiee) : un `zonos3-server` local, lance a la demande, qui sert l'HTTP
//! sur 127.0.0.1. Chaque morceau est genere en plusieurs candidats (graines differentes) ; Whisper relit chacun et on
//! garde celui qui a le moins d'erreurs de mots (puis le premier a zero erreur arrete la recherche).
use crate::{e, R};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub const PORT: u16 = 17996;

/// Reglages lus dans la base (Parametres > Zonos 3).
#[derive(Clone, Debug)]
pub struct Config {
    pub passes: u32,      // candidats maximum par morceau (1 = pas de verification)
    pub reference: bool,  // clonage par audio de reference (plus lourd) au lieu de l'empreinte de voix
    pub expressive: bool, // hesitations + ton conversationnel
}

impl Default for Config {
    fn default() -> Self {
        Config { passes: 4, reference: false, expressive: true }
    }
}

/// Dossier du moteur : variable VOICECLONE_ZONOS, sinon `zonos` a cote du programme.
pub fn home() -> Option<PathBuf> {
    let d = std::env::var("VOICECLONE_ZONOS").map(PathBuf::from).ok().or_else(|| std::env::current_exe().ok()?.parent().map(|p| p.join("zonos")))?;
    d.join("zonos3-server.exe").is_file().then_some(d)
}

pub struct Server {
    child: Child,
    ids: HashMap<String, String>, // signature de voix -> identifiant d'empreinte du serveur
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn url(path: &str) -> String {
    format!("http://127.0.0.1:{PORT}{path}")
}

fn post(path: &str, body: &Value, secs: u64) -> R<Vec<u8>> {
    let r = ureq::AgentBuilder::new().timeout(Duration::from_secs(secs)).build().post(&url(path)).set("X-TTS-Session-ID", "voiceclone").set("Content-Type", "application/json").send_string(&body.to_string());
    match r {
        Ok(resp) => {
            let mut v = vec![];
            std::io::Read::read_to_end(&mut resp.into_reader(), &mut v).map_err(e)?;
            Ok(v)
        }
        Err(ureq::Error::Status(code, resp)) => Err(format!("Zonos HTTP {code}: {}", resp.into_string().unwrap_or_default().chars().take(300).collect::<String>())),
        Err(x) => Err(format!("Zonos : {x}")),
    }
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len() * 4 / 3 + 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// Reechantillonne par sinc fenetre (Hann), `from` Hz -> `to` Hz.
pub fn resample(pcm: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return pcm.to_vec();
    }
    let ratio = from as f64 / to as f64; // pas d'entree par echantillon de sortie
    let fc = (0.5 / ratio.max(1.0)) * 0.95; // coupure normalisee a l'entree
    let half = (16.0 * ratio.max(1.0)).ceil() as isize;
    let n_out = (pcm.len() as f64 / ratio) as usize;
    (0..n_out)
        .map(|n| {
            let t = n as f64 * ratio;
            let c = t.floor() as isize;
            let mut acc = 0.0f64;
            for k in (c - half + 1)..=(c + half) {
                if k < 0 || k as usize >= pcm.len() {
                    continue;
                }
                let x = k as f64 - t;
                let sinc = if x.abs() < 1e-9 { 2.0 * fc } else { (2.0 * std::f64::consts::PI * fc * x).sin() / (std::f64::consts::PI * x) };
                let w = 0.5 + 0.5 * (std::f64::consts::PI * x / half as f64).cos();
                acc += pcm[k as usize] as f64 * sinc * w;
            }
            acc as f32
        })
        .collect()
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase().replace('’', "'").chars().map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' }).collect::<String>().split_whitespace().map(String::from).collect()
}

/// Taux d'erreur de mots (distance d'edition / mots attendus).
pub fn wer(expected: &str, heard: &str) -> f64 {
    let (a, b) = (words(expected), words(heard));
    if a.is_empty() {
        return 0.0;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, wa) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, wb) in b.iter().enumerate() {
            cur.push((prev[j] + usize::from(wa != wb)).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()] as f64 / a.len() as f64
}

fn language(code: &str) -> &'static str {
    match code {
        "fr" => "fr_fr", "de" => "de_de", "es" => "es_es", "it" => "it_it", "pt" => "pt_pt", _ => "en_us",
    }
}

impl Server {
    /// Lance le serveur (ou echoue avec un message clair) et attend qu'il reponde.
    pub fn start(home: &Path, gguf: &Path, log: &Path) -> R<Server> {
        let model = gguf.join("backbone-ptq.gguf");
        if !model.is_file() {
            return Err(format!("modèle Zonos introuvable : {}", model.display()));
        }
        let logf = std::fs::File::create(log).map_err(e)?;
        let mut cmd = Command::new(home.join("zonos3-server.exe"));
        cmd.current_dir(home).arg(&model).args(["--gpu", "--port", &PORT.to_string()]).stdout(Stdio::from(logf.try_clone().map_err(e)?)).stderr(Stdio::from(logf)).stdin(Stdio::null());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
        let child = cmd.spawn().map_err(|x| format!("impossible de lancer zonos3-server : {x}"))?;
        crate::jobs::adopt(&child);
        let mut s = Server { child, ids: HashMap::new() };
        let t0 = Instant::now();
        loop {
            if ureq::get(&url("/v2/capabilities")).timeout(Duration::from_secs(2)).call().is_ok() {
                return Ok(s);
            }
            if let Some(st) = s.child.try_wait().map_err(e)? {
                return Err(format!("zonos3-server s'est arrêté ({st}) ; voir {}", log.display()));
            }
            if t0.elapsed() > Duration::from_secs(180) {
                return Err("zonos3-server ne répond pas (180 s)".into());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// Enregistre (une fois par signature) l'audio de reference d'une voix et renvoie son identifiant.
    pub fn speaker(&mut self, signature: &str, name: &str, wav_bytes: &[u8]) -> R<String> {
        if let Some(id) = self.ids.get(signature) {
            return Ok(id.clone());
        }
        let raw = post("/v2/speakers", &json!({"label": name, "speaker_audio_base64": b64(wav_bytes), "speaker_audio_name": format!("{name}.wav")}), 300)?;
        let j: Value = serde_json::from_slice(&raw).map_err(e)?;
        let id = j["id"].as_str().or_else(|| j["speaker"]["id"].as_str()).ok_or("réponse Zonos sans identifiant de voix")?.to_string();
        self.ids.insert(signature.into(), id.clone());
        Ok(id)
    }

    fn once(&self, cfg: &Config, text: &str, lang: &str, speaker: &str, seed: i64) -> R<Vec<f32>> {
        let mut body = json!({"text": text, "language": language(lang), "stream": false, "format": "wav", "seed": seed,
            "speaker_embedding_id": speaker, "speaker_mode": if cfg.reference { "reference" } else { "embedding" }, "decoder": "full_mr"});
        if cfg.expressive {
            body["controls"] = json!({"disfluency.disfluent": 2, "delivery.conversational": 2});
        }
        let raw = post("/v2/generate", &body, 900)?;
        let r = hound::WavReader::new(std::io::Cursor::new(raw)).map_err(e)?;
        let spec = r.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => r.into_samples::<f32>().collect::<Result<_, _>>().map_err(e)?,
            hound::SampleFormat::Int => {
                let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
                r.into_samples::<i32>().map(|s| s.map(|v| v as f32 / scale)).collect::<Result<_, _>>().map_err(e)?
            }
        };
        let mono: Vec<f32> = samples.chunks(spec.channels as usize).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
        Ok(resample(&mono, spec.sample_rate, crate::audio::SR))
    }

    /// Genere `text`, jusqu'a `cfg.passes` candidats ; `check` renvoie ce que Whisper entend (None : verification impossible).
    /// Retourne l'audio 24 kHz retenu et le nombre d'essais.
    pub fn synth(&self, cfg: &Config, text: &str, lang: &str, speaker: &str, seed: i64, check: &dyn Fn(&[f32]) -> Option<String>) -> R<(Vec<f32>, u32, f64)> {
        let mut best: Option<(f64, Vec<f32>)> = None;
        let mut tries = 0;
        for i in 0..cfg.passes.max(1) {
            let pcm = self.once(cfg, text, lang, speaker, seed + i as i64)?;
            tries += 1;
            let score = if cfg.passes > 1 && pcm.len() > crate::audio::SR as usize / 4 { check(&pcm).map(|h| wer(text, &h)) } else { Some(0.0) };
            let Some(score) = score else { return Ok((pcm, tries, f64::NAN)) }; // pas de Whisper : on garde le premier
            if best.as_ref().map_or(true, |(b, _)| score < *b) {
                best = Some((score, pcm));
            }
            if score <= 0.0 {
                break;
            }
        }
        let (score, pcm) = best.ok_or("Zonos n'a produit aucun audio")?;
        Ok((pcm, tries, score))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_reference() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn wer_counts_word_edits() {
        assert_eq!(wer("Bonjour à tous.", "bonjour a tous"), 1.0 / 3.0);
        assert_eq!(wer("un deux trois", "un deux trois"), 0.0);
        assert_eq!(wer("", "x"), 0.0);
    }

    #[test]
    fn resample_keeps_duration() {
        let x = vec![0.5f32; 44100];
        let y = resample(&x, 44100, 24000);
        assert!((y.len() as i64 - 24000).abs() <= 1);
        assert!((y[12000] - 0.5).abs() < 0.02);
    }
}
