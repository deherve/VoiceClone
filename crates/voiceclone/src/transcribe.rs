//! Transcription automatique des echantillons : whisper.cpp (`whisper-cli.exe`) et un modele ggml, installes dans
//! `<donnees>\whisper` depuis leurs sources officielles (GitHub de whisper.cpp, Hugging Face), chacun verifie par un
//! SHA-256 fixe. Aucune donnee n'est envoyee : la transcription tourne en local.
use crate::{audio, components, e, models, update, R};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const CLI_URL: &str = "https://github.com/ggml-org/whisper.cpp/releases/download/v1.9.2/whisper-bin-x64.zip";
const CLI_SHA256: &str = "49dcc16de826f20bd53d44f947a1ae49dfa81f86cad67a64d80820cb192d674a";
const CLI_MB: u64 = 8;
pub const MODEL_FILE: &str = "ggml-small-q5_1.bin";
const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin";
const MODEL_SHA256: &str = "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb";
const MODEL_MB: u64 = 181;
const TIMEOUT: Duration = Duration::from_secs(300);

fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("whisper")
}

/// Fichier `name` ou qu'il soit sous `bin` (une archive peut le ranger dans un sous-dossier).
pub(crate) fn find_file(bin: &Path, name: &str) -> Option<PathBuf> {
    for f in fs::read_dir(bin).ok()?.flatten() {
        let p = f.path();
        if p.is_dir() {
            if let Some(found) = find_file(&p, name) {
                return Some(found);
            }
        } else if f.file_name().to_string_lossy().eq_ignore_ascii_case(name) {
            return Some(p);
        }
    }
    None
}

fn find_cli(bin: &Path) -> Option<PathBuf> {
    find_file(bin, "whisper-cli.exe")
}

pub fn status(data_dir: &Path) -> Value {
    let d = dir(data_dir);
    let (cli, model) = (find_cli(&d.join("bin")).is_some(), d.join(MODEL_FILE).is_file());
    json!({"installed": cli && model, "cli": cli, "model": model, "size_mb": CLI_MB + MODEL_MB, "model_label": "Whisper small (q5_1)",
           "source": "github.com/ggml-org/whisper.cpp · huggingface.co/ggerganov/whisper.cpp"})
}

/// Telecharge puis verifie le SHA-256 (fichier supprime s'il ne correspond pas).
pub(crate) fn fetch(url: &str, sha: &str, dest: &Path, cancel: &AtomicBool, progress: &dyn Fn(u64, u64)) -> R<()> {
    models::download(url, dest, cancel, progress)?;
    if !components::sha256_file(dest)?.eq_ignore_ascii_case(sha) {
        let _ = fs::remove_file(dest);
        return Err(tr!("l'empreinte SHA-256 du fichier téléchargé ne correspond pas : fichier supprimé", "the SHA-256 of the downloaded file does not match: file deleted"));
    }
    Ok(())
}

/// Installe le programme puis le modele. `progress(etape, fait, total)` : etape « cli » puis « model ».
pub fn install(data_dir: &Path, cancel: &AtomicBool, progress: &dyn Fn(&str, u64, u64)) -> R<()> {
    let d = dir(data_dir);
    fs::create_dir_all(&d).map_err(e)?;
    if find_cli(&d.join("bin")).is_none() {
        let zip = d.join("whisper-bin-x64.zip");
        fetch(CLI_URL, CLI_SHA256, &zip, cancel, &|a, b| progress("cli", a, b))?;
        // Decompression dans un dossier temporaire puis renommage : une extraction interrompue ne laisse jamais un programme a moitie installe.
        let (bin, staging) = (d.join("bin"), d.join("bin.tmp"));
        let _ = fs::remove_dir_all(&staging);
        update::unpack(&zip, &staging)?;
        let _ = fs::remove_file(&zip);
        let _ = fs::remove_dir_all(&bin);
        fs::rename(&staging, &bin).map_err(e)?;
        if find_cli(&bin).is_none() {
            return Err(tr!("archive whisper.cpp incomplète : whisper-cli.exe manque", "incomplete whisper.cpp archive: whisper-cli.exe is missing"));
        }
    }
    if !d.join(MODEL_FILE).is_file() {
        fetch(MODEL_URL, MODEL_SHA256, &d.join(MODEL_FILE), cancel, &|a, b| progress("model", a, b))?;
    }
    Ok(())
}

pub fn remove(data_dir: &Path) -> R<()> {
    let d = dir(data_dir);
    if d.exists() {
        fs::remove_dir_all(&d).map_err(e)?;
    }
    Ok(())
}

/// Nom de langue Qwen3-TTS → code Whisper (« auto » si inconnue).
pub fn lang_code(name: &str) -> &'static str {
    match name {
        "french" => "fr", "english" => "en", "german" => "de", "spanish" => "es", "italian" => "it",
        "portuguese" => "pt", "japanese" => "ja", "korean" => "ko", "chinese" => "zh", "russian" => "ru",
        _ => "auto",
    }
}

/// 24 kHz → 16 kHz (Whisper) par sinc fenetre : coupure a 7,6 kHz, 16 points de chaque cote.
pub fn resample_16k(pcm: &[f32]) -> Vec<f32> {
    let (fc, half) = (7600.0 / 24000.0, 16isize);
    let n_out = pcm.len() * 2 / 3;
    (0..n_out)
        .map(|n| {
            let t = n as f64 * 1.5;
            let c = t.floor() as isize;
            let mut acc = 0.0f64;
            for k in (c - half + 1)..=(c + half) {
                if k < 0 || k as usize >= pcm.len() {
                    continue;
                }
                let x = k as f64 - t;
                let sinc = if x.abs() < 1e-9 { 2.0 * fc } else { (2.0 * std::f64::consts::PI * fc * x).sin() / (std::f64::consts::PI * x) };
                let w = 0.5 + 0.5 * (std::f64::consts::PI * x / half as f64).cos(); // fenetre de Hann
                acc += pcm[k as usize] as f64 * sinc * w;
            }
            acc as f32
        })
        .collect()
}

/// Nettoie la sortie de whisper-cli : lignes jointes, balises entre crochets ou parentheses retirees ([BLANK_AUDIO], (musique)...).
fn clean_output(raw: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in raw.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(if c == '\n' || c == '\r' { ' ' } else { c }),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Transcrit un echantillon (PCM mono 24 kHz) dans la langue `lang` (code Whisper ou « auto »).
pub fn transcribe(data_dir: &Path, pcm: &[f32], lang: &str) -> R<String> {
    let d = dir(data_dir);
    let cli = find_cli(&d.join("bin")).ok_or_else(|| tr!("la transcription automatique n'est pas installée (Paramètres > Modèles)", "automatic transcription is not installed (Settings > Models)"))?;
    let model = d.join(MODEL_FILE);
    if !model.is_file() {
        return Err(tr!("le modèle de transcription manque (Paramètres > Modèles)", "the transcription model is missing (Settings > Models)"));
    }
    // Nom unique : deux transcriptions en meme temps ne doivent jamais s'ecraser le fichier lu par Whisper.
    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let wav = d.join(format!("tmp-{}-{}.wav", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    {
        let spec = hound::WavSpec { channels: 1, sample_rate: 16000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(&wav, spec).map_err(e)?;
        for s in resample_16k(pcm) {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).map_err(e)?;
        }
        w.finalize().map_err(e)?;
    }
    let result = run(&cli, &model, &wav, lang);
    let _ = fs::remove_file(&wav);
    let text = clean_output(&result?);
    if text.is_empty() {
        return Err(tr!("aucune parole reconnue dans cet échantillon", "no speech recognised in this sample"));
    }
    Ok(text)
}

fn run(cli: &Path, model: &Path, wav: &Path, lang: &str) -> R<String> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let mut cmd = Command::new(cli);
    cmd.arg("-m").arg(model).arg("-f").arg(wav).args(["-l", lang, "-nt", "-np", "-t"]).arg(threads.to_string());
    cmd.stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // pas de fenetre de console
    let mut child = cmd.spawn().map_err(|x| tr!("impossible de lancer whisper-cli : {}", "cannot start whisper-cli: {}", x))?;
    crate::jobs::adopt(&child);
    let mut stdout = child.stdout.take().ok_or_else(|| "stdout".to_string())?;
    let reader = std::thread::spawn(move || {
        let mut bytes = vec![];
        let _ = stdout.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let started = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(e)? {
            break s;
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            return Err(tr!("la transcription a pris trop de temps (5 min) : interrompue", "transcription took too long (5 min): stopped"));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let text = reader.join().unwrap_or_default();
    if !status.success() {
        return Err(tr!("whisper-cli a échoué (code {})", "whisper-cli failed (code {})", status.code().unwrap_or(-1)));
    }
    Ok(text)
}

/// Lit un fichier WAV quelconque supporte par `audio::read_wav_24k` et le transcrit (ligne de commande).
pub fn transcribe_file(data_dir: &Path, wav: &Path, lang: &str) -> R<String> {
    transcribe(data_dir, &audio::read_wav_24k(wav)?, lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_is_cleaned() {
        assert_eq!(clean_output(" Bonjour à tous.\n[BLANK_AUDIO]\n (musique) Salut  "), "Bonjour à tous. Salut");
        assert_eq!(clean_output("[BLANK_AUDIO]"), "");
        assert_eq!(clean_output("a ] b ) c"), "a ] b ) c"); // fermeture sans ouverture : conservee
    }

    #[test]
    fn language_codes() {
        assert_eq!((lang_code("french"), lang_code("chinese"), lang_code("???")), ("fr", "zh", "auto"));
    }

    #[test]
    fn resampling_keeps_pitch_and_level() {
        let sr = 24000.0f32;
        let tone: Vec<f32> = (0..24000).map(|i| 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin()).collect();
        let out = resample_16k(&tone);
        assert_eq!(out.len(), 16000);
        // Amplitude (zone centrale, loin des bords) et nombre de passages par zero : 1000 Hz sur 1 s.
        let mid = &out[2000..14000];
        let peak = mid.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((peak - 0.5).abs() < 0.03, "{peak}");
        let crossings = mid.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        assert!((crossings as i32 - 750).abs() <= 3, "{crossings}"); // 12000 echantillons = 0,75 s
    }

    #[test]
    fn nothing_installed_by_default_and_transcribe_says_so() {
        let dir = std::env::temp_dir().join(format!("voiceclone-whisper-{}", std::process::id()));
        assert_eq!(status(&dir)["installed"], false);
        assert!(transcribe(&dir, &[0.0; 24000], "fr").is_err());
    }
}
