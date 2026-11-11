//! Voix livrees avec les archives compilees (dossier `Voix\<nom>\` a cote du programme) et export d'une voix.
//!
//! Une voix = un dossier : `voice.json` (`{"name": "Zen", "language": "french"}`), puis `sample-1.wav` (+ `sample-1.txt`,
//! sa transcription exacte), `sample-2.wav`... Les archives de VoiceClone ne contiennent que la voix de Zen ; le depot
//! GitHub n'en contient aucune (le dossier d'export `release-assets\` est ignore par git).
use crate::{audio, cli, e, App, R};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}};

/// Dossier des voix fournies : `VOICECLONE_VOICES`, sinon `Voix\` a cote de l'executable.
pub fn bundled_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("VOICECLONE_VOICES") {
        return Some(d.into());
    }
    Some(std::env::current_exe().ok()?.parent()?.join("Voix"))
}

/// `sample-12.wav` → 12 ; tout autre nom (chemin, extension, chiffres en trop) est refuse.
fn sample_number(file_name: &str) -> Option<u32> {
    let n = file_name.strip_prefix("sample-")?.strip_suffix(".wav")?;
    if n.is_empty() || n.len() > 3 || !n.bytes().all(|b| b.is_ascii_digit()) { None } else { n.parse().ok() }
}

impl App {
    /// Ecrit la voix `name` (profil + echantillons + transcriptions) dans `dir`. Retourne le nombre d'echantillons.
    pub fn export_voice(&self, name: &str, dir: &Path) -> R<usize> {
        let (profile, samples) = {
            let store = self.store.lock().unwrap();
            let p = store.find_profile(Some(name))?;
            let s = store.samples(p.id)?;
            (p, s)
        };
        if samples.is_empty() {
            return Err(tr!("la voix « {} » n'a aucun échantillon", "voice \"{}\" has no sample", profile.name));
        }
        fs::create_dir_all(dir).map_err(e)?;
        let info: Value = json!({"name": profile.name, "language": profile.language});
        fs::write(dir.join("voice.json"), serde_json::to_string_pretty(&info).map_err(e)?).map_err(e)?;
        for (i, s) in samples.iter().enumerate() {
            let n = i + 1;
            fs::copy(&s.audio, dir.join(format!("sample-{n}.wav"))).map_err(|x| format!("{} : {x}", s.audio))?;
            fs::write(dir.join(format!("sample-{n}.txt")), s.ref_text.trim()).map_err(e)?;
        }
        Ok(samples.len())
    }

    /// Importe les voix de `src` (un sous-dossier par voix). Une voix qui existe deja est laissee telle quelle.
    /// Retourne les noms importes ; une voix illisible est refusee en entier (rien n'est cree pour elle).
    pub fn import_voices_from(&self, src: &Path) -> R<Vec<String>> {
        let mut folders: Vec<PathBuf> = fs::read_dir(src).map_err(e)?.flatten().map(|d| d.path()).filter(|p| p.is_dir()).collect();
        folders.sort();
        let mut done = vec![];
        for folder in folders {
            let raw = fs::read_to_string(folder.join("voice.json")).map_err(|x| format!("{} : {x}", folder.display()))?;
            let info: Value = serde_json::from_str(&raw).map_err(|_| tr!("{} : voice.json illisible", "{}: unreadable voice.json", folder.display()))?;
            let name = info["name"].as_str().map(str::trim).filter(|n| !n.is_empty() && n.chars().count() <= 60 && !n.chars().any(char::is_control));
            let name = name.ok_or_else(|| tr!("{} : nom de voix invalide", "{}: invalid voice name", folder.display()))?;
            if self.store.lock().unwrap().find_profile(Some(name)).is_ok() {
                continue;
            }
            let language = cli::language(info["language"].as_str().unwrap_or("french"))?;
            let mut files: Vec<(u32, PathBuf)> = fs::read_dir(&folder).map_err(e)?.flatten()
                .filter_map(|f| Some((sample_number(&f.file_name().to_string_lossy())?, f.path()))).collect();
            files.sort();
            if files.is_empty() {
                return Err(tr!("{} : aucun échantillon", "{}: no sample", folder.display()));
            }
            let id = self.create_profile(name, &language)?;
            let fill = || -> R<()> {
                for (_, wav) in &files {
                    let sample = self.add_sample(id, &audio::read_wav_24k(wav)?)?;
                    if let Ok(text) = fs::read_to_string(wav.with_extension("txt")) {
                        self.set_sample_text(sample, &text)?;
                    }
                }
                Ok(())
            };
            if let Err(x) = fill() {
                let _ = self.delete_profile(id); // pas de voix a moitie importee
                return Err(x);
            }
            done.push(name.to_string());
        }
        Ok(done)
    }

    /// Premier lancement : importe les voix livrees avec le programme (dossier `Voix`), une seule fois.
    /// Sans dossier `Voix` (version compilee depuis les sources, par exemple), rien ne se passe et rien n'est note.
    pub fn seed_bundled_voices(&self) -> Vec<String> {
        if self.store.lock().unwrap().setting("voices_seeded").is_some() {
            return vec![];
        }
        let Some(dir) = bundled_dir().filter(|d| d.is_dir()) else { return vec![] };
        let names = self.import_voices_from(&dir).unwrap_or_default();
        let _ = self.store.lock().unwrap().set_setting("voices_seeded", "1");
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_names_are_strict() {
        assert_eq!(sample_number("sample-1.wav"), Some(1));
        assert_eq!(sample_number("sample-120.wav"), Some(120));
        for bad in ["sample-.wav", "sample-1.mp3", "sample-1234.wav", "Sample-1.wav", "sample-1.wav.exe", "../sample-1.wav", "sample-a.wav", "x.wav"] {
            assert_eq!(sample_number(bad), None, "{bad}");
        }
    }

    #[test]
    fn export_then_import_round_trip() {
        let base = std::env::temp_dir().join(format!("voiceclone-voices-{}", std::process::id()));
        let tone: Vec<f32> = (0..48000).map(|i| 0.1 * (i as f32 * 0.05).sin()).collect();
        let (a, b) = (App::open(base.join("a")).unwrap(), App::open(base.join("b")).unwrap());
        let id = a.create_profile("Zen", "french").unwrap();
        let s1 = a.add_sample(id, &tone).unwrap();
        a.set_sample_text(s1, "Bonjour tout le monde").unwrap();
        a.add_sample(id, &tone).unwrap(); // 2e echantillon sans transcription

        assert_eq!(a.export_voice("Zen", &base.join("voix").join("Zen")).unwrap(), 2);
        assert!(a.export_voice("Inconnue", &base.join("x")).is_err());
        // Dossiers parasites : un fichier a la racine, un sous-dossier sans voice.json valide plus bas dans l'ordre alphabetique.
        fs::write(base.join("voix").join("notes.txt"), "ignoré").unwrap();

        assert_eq!(b.import_voices_from(&base.join("voix")).unwrap(), ["Zen"]);
        let voices = b.list_profiles().unwrap();
        assert_eq!((voices[0]["name"].as_str(), voices[0]["language"].as_str(), voices[0]["samples"].as_i64()), (Some("Zen"), Some("french"), Some(2)));
        let samples = b.list_samples(voices[0]["id"].as_i64().unwrap()).unwrap();
        assert_eq!(samples[0]["ref_text"], "Bonjour tout le monde");
        assert_eq!(samples[1]["ref_text"], "");
        // Deja presente : jamais reimportee ni dupliquee.
        assert!(b.import_voices_from(&base.join("voix")).unwrap().is_empty());
        assert_eq!(b.list_profiles().unwrap().as_array().unwrap().len(), 1);

        // Une voix cassee (echantillon qui n'est pas du 24 kHz mono) est refusee en entier, sans profil a moitie cree.
        let bad = base.join("voix").join("Cassee");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join("voice.json"), r#"{"name":"Cassee","language":"french"}"#).unwrap();
        fs::write(bad.join("sample-1.wav"), b"pas un wav").unwrap();
        assert!(b.import_voices_from(&base.join("voix")).is_err());
        assert!(b.list_profiles().unwrap().as_array().unwrap().iter().all(|p| p["name"] != "Cassee"));
        drop((a, b));
        let _ = fs::remove_dir_all(base);
    }
}
