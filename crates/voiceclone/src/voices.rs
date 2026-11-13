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

/// Sous-dossiers de `src`, tries (un par voix).
fn voice_folders(src: &Path) -> R<Vec<PathBuf>> {
    let mut folders: Vec<PathBuf> = fs::read_dir(src).map_err(e)?.flatten().map(|d| d.path()).filter(|p| p.is_dir()).collect();
    folders.sort();
    Ok(folders)
}

/// Nom de dossier sur disque pour une voix : caracteres interdits par Windows remplaces par « _ », pas de point ni
/// d'espace a la fin, pas de nom reserve (CON, NUL...). Le vrai nom de la voix reste dans `voice.json`.
pub fn folder_name(name: &str) -> String {
    let mut s: String = name.trim().chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    s = s.trim_end_matches(['.', ' ']).to_string();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&s.to_uppercase().as_str())
        || (s.len() == 4 && (s.to_uppercase().starts_with("COM") || s.to_uppercase().starts_with("LPT")) && s.as_bytes()[3].is_ascii_digit());
    if s.is_empty() || reserved {
        s.insert(0, '_');
    }
    s
}

/// Nom de la voix exportee dans ce dossier (`voice.json`), s'il y en a une.
fn folder_voice_name(dir: &Path) -> Option<String> {
    let v: Value = serde_json::from_str(&fs::read_to_string(dir.join("voice.json")).ok()?).ok()?;
    v["name"].as_str().map(String::from)
}

/// `sample-12.wav` → 12 ; tout autre nom (chemin, extension, chiffres en trop) est refuse.
fn sample_number(file_name: &str) -> Option<u32> {
    let n = file_name.strip_prefix("sample-")?.strip_suffix(".wav")?;
    if n.is_empty() || n.len() > 3 || !n.bytes().all(|b| b.is_ascii_digit()) { None } else { n.parse().ok() }
}

impl App {
    /// Dossier `Voix` du programme (a cote de l'executable ; `VOICECLONE_VOICES` pour l'essayer ailleurs). S'il n'est pas
    /// modifiable (programme installe dans un dossier protege), on prend `Voix` dans le dossier de donnees.
    pub fn voices_dir(&self) -> PathBuf {
        self.voices_cache.get_or_init(|| self.probe_voices_dir()).clone()
    }

    fn probe_voices_dir(&self) -> PathBuf {
        let main = bundled_dir().unwrap_or_else(|| self.dir.join("Voix"));
        let usable = fs::create_dir_all(&main).is_ok() && {
            let probe = main.join(".ecriture");
            let ok = fs::write(&probe, b"").is_ok();
            let _ = fs::remove_file(&probe);
            ok
        };
        if usable {
            return main;
        }
        let fallback = self.dir.join("Voix");
        let _ = fs::create_dir_all(&fallback);
        fallback
    }

    /// Exporte une voix dans `Voix\<Nom>\` ; retourne ce dossier.
    pub fn export_voice_to_folder(&self, name: &str) -> R<PathBuf> {
        self.export_into(&self.voices_dir(), name)
    }

    fn export_into(&self, base_dir: &Path, name: &str) -> R<PathBuf> {
        let real = self.store.lock().unwrap().find_profile(Some(name))?.name;
        // « a:b » et « a_b » donnent le meme nom de dossier : la seconde voix prend un suffixe au lieu d'ecraser la premiere.
        let (base, mut n) = (folder_name(&real), 2);
        let mut dir = base_dir.join(&base);
        while folder_voice_name(&dir).is_some_and(|other| other != real) {
            dir = base_dir.join(format!("{base}-{n}"));
            n += 1;
        }
        self.export_voice(&real, &dir)?;
        Ok(dir)
    }

    /// Exporte toutes les voix qui ont au moins un echantillon. Retourne les noms exportes.
    pub fn export_all_voices(&self) -> R<Vec<String>> {
        let names: Vec<String> = self.list_profiles()?.as_array().into_iter().flatten().filter(|p| p["samples"].as_i64().unwrap_or(0) > 0)
            .filter_map(|p| p["name"].as_str().map(String::from)).collect();
        for n in &names {
            self.export_voice_to_folder(n)?;
        }
        Ok(names)
    }

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
        // Un export plus ancien de la meme voix (plus d'echantillons) ne doit pas laisser de fichiers en trop.
        for f in fs::read_dir(dir).map_err(e)?.flatten() {
            let n = f.file_name().to_string_lossy().to_string();
            if n.starts_with("sample-") && (n.ends_with(".wav") || n.ends_with(".txt")) {
                let _ = fs::remove_file(f.path());
            }
        }
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
        let mut done = vec![];
        for folder in voice_folders(src)? {
            if let Some(name) = self.import_one(&folder)? {
                done.push(name);
            }
        }
        Ok(done)
    }

    /// Comme `import_voices_from`, mais une voix illisible n'arrete pas les autres : retourne (importees, deja presentes, erreurs).
    pub fn import_voices_report(&self, src: &Path) -> R<(Vec<String>, usize, Vec<String>)> {
        let (mut done, mut existing, mut errors) = (vec![], 0, vec![]);
        for folder in voice_folders(src)? {
            match self.import_one(&folder) {
                Ok(Some(name)) => done.push(name),
                Ok(None) => existing += 1,
                Err(x) => errors.push(x),
            }
        }
        Ok((done, existing, errors))
    }

    /// `Ok(None)` : une voix de ce nom existe deja (laissee telle quelle).
    fn import_one(&self, folder: &Path) -> R<Option<String>> {
        {
            let raw = fs::read_to_string(folder.join("voice.json")).map_err(|x| format!("{} : {x}", folder.display()))?;
            let info: Value = serde_json::from_str(&raw).map_err(|_| tr!("{} : voice.json illisible", "{}: unreadable voice.json", folder.display()))?;
            let name = info["name"].as_str().map(str::trim).filter(|n| !n.is_empty() && n.chars().count() <= 60 && !n.chars().any(char::is_control));
            let name = name.ok_or_else(|| tr!("{} : nom de voix invalide", "{}: invalid voice name", folder.display()))?;
            if self.store.lock().unwrap().find_profile(Some(name)).is_ok() {
                return Ok(None);
            }
            let language = cli::language(info["language"].as_str().unwrap_or("french"))?;
            let mut files: Vec<(u32, PathBuf)> = fs::read_dir(folder).map_err(e)?.flatten()
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
            Ok(Some(name.to_string()))
        }
    }

    /// Premier lancement : importe les voix livrees avec le programme (dossier `Voix`), une seule fois.
    /// Sans dossier `Voix` (version compilee depuis les sources, par exemple), rien ne se passe et rien n'est note.
    pub fn seed_bundled_voices(&self) -> Vec<String> {
        if self.store.lock().unwrap().setting("voices_seeded").is_some() {
            return vec![];
        }
        let Some(dir) = bundled_dir().filter(|d| d.is_dir()) else { return vec![] };
        // Une voix illisible n'empeche plus les suivantes (Zen comprise) ; « deja fait » n'est note que si tout s'est bien passe.
        let Ok((names, _, errors)) = self.import_voices_report(&dir) else { return vec![] };
        if errors.is_empty() {
            let _ = self.store.lock().unwrap().set_setting("voices_seeded", "1");
        }
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
    fn seeding_continues_after_an_unreadable_voice_and_exports_do_not_collide() {
        let base = std::env::temp_dir().join(format!("voiceclone-seed-{}", std::process::id()));
        let tone: Vec<f32> = (0..48000).map(|i| 0.1 * (i as f32 * 0.05).sin()).collect();
        let a = App::open(base.join("a")).unwrap();
        for name in ["a:b", "a_b"] {
            let id = a.create_profile(name, "french").unwrap();
            a.add_sample(id, &tone).unwrap();
        }
        // « a:b » et « a_b » donnent le meme nom de dossier : la seconde voix ne doit pas ecraser la premiere.
        let v = base.join("v");
        let (one, two) = (a.export_into(&v, "a:b").unwrap(), a.export_into(&v, "a_b").unwrap());
        assert_ne!(one, two);
        assert_eq!((folder_voice_name(&one).as_deref(), folder_voice_name(&two).as_deref()), (Some("a:b"), Some("a_b")));
        assert_eq!(a.export_into(&v, "a:b").unwrap(), one, "reexporter la meme voix reutilise son dossier");
        // Un dossier illisible en premier (ordre alphabetique) n'empeche pas l'import des suivants.
        fs::create_dir_all(base.join("v").join("0-casse")).unwrap();
        let b = App::open(base.join("b")).unwrap();
        let (done, _, errors) = b.import_voices_report(&base.join("v")).unwrap();
        assert_eq!((done, errors.len()), (vec!["a:b".to_string(), "a_b".to_string()], 1));
        drop((a, b));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn folder_names_are_safe_on_windows() {
        assert_eq!(folder_name("Zen"), "Zen");
        assert_eq!(folder_name("Zen 2"), "Zen 2");
        assert_eq!(folder_name("a/b\\c:d*e?"), "a_b_c_d_e_");
        assert_eq!(folder_name("fin. "), "fin");
        assert_eq!(folder_name("con"), "_con");
        assert_eq!(folder_name("COM1"), "_COM1");
        assert_eq!(folder_name("..."), "_");
        assert_eq!(folder_name("Corné"), "Corné");
    }

    #[test]
    fn report_import_skips_broken_voices_and_existing_ones() {
        let base = std::env::temp_dir().join(format!("voiceclone-report-{}", std::process::id()));
        let tone: Vec<f32> = (0..48000).map(|i| 0.1 * (i as f32 * 0.05).sin()).collect();
        let (a, b) = (App::open(base.join("a")).unwrap(), App::open(base.join("b")).unwrap());
        let id = a.create_profile("Zen", "french").unwrap();
        a.add_sample(id, &tone).unwrap();
        let src = base.join("voix");
        a.export_voice("Zen", &src.join(folder_name("Zen"))).unwrap();
        fs::create_dir_all(src.join("Casse")).unwrap(); // dossier sans voice.json
        let (done, existing, errors) = b.import_voices_report(&src).unwrap();
        assert_eq!((done, existing, errors.len()), (vec!["Zen".to_string()], 0, 1));
        let (done, existing, errors) = b.import_voices_report(&src).unwrap();
        assert_eq!((done.len(), existing, errors.len()), (0, 1, 1));
        // Reexport avec moins d'echantillons : pas de fichier en trop.
        a.add_sample(id, &tone).unwrap();
        a.export_voice("Zen", &src.join("Zen")).unwrap();
        assert_eq!(fs::read_dir(src.join("Zen")).unwrap().flatten().filter(|f| f.file_name().to_string_lossy().ends_with(".wav")).count(), 2);
        let s = a.list_samples(id).unwrap();
        a.delete_sample(s[1]["id"].as_i64().unwrap()).unwrap();
        a.export_voice("Zen", &src.join("Zen")).unwrap();
        assert_eq!(fs::read_dir(src.join("Zen")).unwrap().flatten().filter(|f| f.file_name().to_string_lossy().ends_with(".wav")).count(), 1);
        drop((a, b));
        let _ = fs::remove_dir_all(base);
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
