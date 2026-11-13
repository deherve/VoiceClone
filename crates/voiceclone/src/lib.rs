//! Coeur de VoiceClone : utilise par le serveur MCP sans fenetre et par l'application Tauri.
#[macro_use]
pub mod i18n;
pub mod audio;
pub mod cli;
pub mod components;
pub mod engine;
pub mod export;
pub mod mcp;
pub mod models;
pub mod normalize;
pub mod quality;
pub mod script;
mod store;
pub mod transcribe;
pub mod writer;
pub mod update;
pub mod voices;

use engine::{Engine, Voice};
use serde_json::{json, Value};
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, time::{Instant, SystemTime, UNIX_EPOCH}};
use script::Item;
use store::Store;

pub type R<T> = Result<T, String>;
pub(crate) fn e<E: ToString>(x: E) -> String {
    x.to_string()
}

const MAX_CHUNK_CHARS: usize = 300;
/// Duree maximale d'un morceau synthetise : au-dela, il est recoupe en deux et regenere (meilleure qualite).
const MAX_CHUNK_SEC: usize = 30;
const DEFAULT_SEED: i64 = 7; // meme graine que les generations Voicebox existantes : resultat reproductible

fn sample_secs(path: &str) -> f64 {
    hound::WavReader::open(path).map(|r| r.duration() as f64 / audio::SR as f64).unwrap_or(0.0)
}

/// Synthetise `text` ; si le resultat depasse `MAX_CHUNK_SEC`, le coupe en deux et regenere chaque moitie.
fn synth_capped(engine: &Engine, text: &str, lang: &str, voice: &Voice, seed: i64, out: &mut Vec<f32>) -> R<()> {
    let pcm = engine.synth(text, lang, voice, seed)?;
    if pcm.len() > MAX_CHUNK_SEC * audio::SR as usize {
        let parts = audio::halve(text);
        if parts.len() > 1 {
            for (i, part) in parts.iter().enumerate() {
                if i > 0 {
                    out.resize(out.len() + audio::SR as usize * 3 / 10, 0.0);
                }
                synth_capped(engine, part, lang, voice, seed, out)?;
            }
            return Ok(());
        }
    }
    let mut pcm = pcm;
    audio::trim_silence(&mut pcm, 50);
    out.extend(pcm);
    Ok(())
}

/// Dossier de donnees : `VOICECLONE_DATA`, sinon `data\` a cote de l'executable si un fichier `portable` s'y trouve
/// (version autonome : tout reste dans le dossier, rien dans le profil Windows), sinon `%APPDATA%\VoiceClone`.
pub fn data_dir() -> R<PathBuf> {
    if let Ok(d) = std::env::var("VOICECLONE_DATA") {
        return Ok(d.into());
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|x| x.parent().map(Path::to_path_buf)).filter(|d| d.join("portable").is_file()) {
        return Ok(dir.join("data"));
    }
    Ok(PathBuf::from(std::env::var("APPDATA").map_err(|_| tr!("APPDATA non défini", "APPDATA is not set"))?).join("VoiceClone"))
}

fn now_ms() -> R<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH).map_err(e)?.as_millis())
}

struct Runtime {
    // Moteur charge, avec le nom du modele qu'il utilise.
    engine: Option<(String, Engine)>,
    // Voix par profil, avec la signature des echantillons qui l'ont produite : un echantillon
    // ajoute, supprime ou retranscrit change la signature et la voix est recalculee au speak suivant.
    // shortcut: voix gardees en memoire seulement (quelques secondes d'extraction par profil a chaque
    // lancement) ; les ecrire sur disque si ce delai gene.
    voices: HashMap<i64, (String, Voice)>,
    // Peripherique sur lequel le moteur tourne, et ce qui s'est passe s'il a fallu en changer.
    device: Option<String>,
    note: Option<String>,
}

type Hook = Box<dyn Fn() + Send + Sync>;

pub struct App {
    dir: PathBuf,
    store: Mutex<Store>,
    rt: Mutex<Runtime>,
    on_change: Mutex<Option<Hook>>,
    // Telechargements en cours : nom du fichier -> drapeau d'annulation.
    downloads: Mutex<HashMap<String, Arc<AtomicBool>>>,
    // Installation d'un composant (Vulkan, CUDA) : une seule a la fois, annulable.
    component_busy: AtomicBool,
    component_cancel: AtomicBool,
    // Dossier des audios imposé (VOICECLONE_AUDIOS, ou essais) ; sinon `Audios\` a cote du programme.
    audios_override: Option<PathBuf>,
}

fn busy() -> String {
    tr!("une génération est en cours, réessayez dans un instant", "a generation is in progress, try again in a moment")
}

impl App {
    pub fn open(dir: PathBuf) -> R<Self> {
        for d in ["profiles", "models"] {
            fs::create_dir_all(dir.join(d)).map_err(e)?;
        }
        let store = Store::open(&dir.join("voiceclone.db"))?;
        // La langue choisie dans Parametres vaut aussi pour la version serveur (messages d'erreur MCP).
        if let Some(l) = store.setting("lang") {
            i18n::set_lang(&l);
        }
        let app = App {
            dir,
            store: Mutex::new(store),
            rt: Mutex::new(Runtime { engine: None, voices: HashMap::new(), device: None, note: None }),
            on_change: Mutex::new(None),
            downloads: Mutex::new(HashMap::new()),
            component_busy: AtomicBool::new(false),
            component_cancel: AtomicBool::new(false),
            audios_override: std::env::var_os("VOICECLONE_AUDIOS").map(PathBuf::from),
        };
        app.seed_bundled_voices(); // la voix de Zen livree avec l'archive, au premier lancement
        Ok(app)
    }

    pub fn data_dir(&self) -> PathBuf {
        self.dir.clone()
    }

    /// Dossier ou sont enregistres tous les audios generes (application, MCP sans `output_path`, ligne de commande
    /// sans `--out`) : `Audios\` a cote du programme, visible avec lui. Si ce dossier n'est pas modifiable (programme dans
    /// Program Files...), repli sur `<donnees>\Audios`. Le dossier est cree s'il n'existe pas.
    pub fn audios_dir(&self) -> PathBuf {
        let writable = |d: &Path| {
            let probe = d.join(".ecriture");
            let ok = fs::create_dir_all(d).is_ok() && fs::write(&probe, b"").is_ok();
            let _ = fs::remove_file(&probe);
            ok
        };
        let preferred = self.audios_override.clone().or_else(|| std::env::current_exe().ok()?.parent().map(|p| p.join("Audios")));
        match preferred {
            Some(d) if writable(&d) => d,
            _ => {
                let d = self.dir.join("Audios");
                let _ = fs::create_dir_all(&d);
                d
            }
        }
    }

    /// Installe un composant manquant depuis sa source officielle (voir `components`). Une seule installation a la fois.
    pub fn install_component(&self, id: &str, progress: &dyn Fn(&str, u64, u64)) -> R<&'static str> {
        if self.component_busy.swap(true, Ordering::SeqCst) {
            return Err(tr!("une installation est déjà en cours", "an installation is already in progress"));
        }
        self.component_cancel.store(false, Ordering::SeqCst);
        let r = components::install(id, &self.dir, &self.component_cancel, progress);
        self.component_busy.store(false, Ordering::SeqCst);
        r
    }

    pub fn cancel_component(&self) {
        self.component_cancel.store(true, Ordering::SeqCst);
    }

    /// Langue enregistree (None tant que l'utilisateur n'en a pas choisi une : l'interface propose alors celle du systeme).
    pub fn saved_lang(&self) -> Option<String> {
        self.store.lock().unwrap().setting("lang").filter(|l| i18n::LANGS.contains(&l.as_str()))
    }

    pub fn set_lang(&self, lang: &str) -> R<()> {
        if !i18n::set_lang(lang) {
            return Err(tr!("langue inconnue : {}", "unknown language: {}", lang));
        }
        self.store.lock().unwrap().set_setting("lang", lang)
    }

    /// Dossier des modeles : `VOICECLONE_MODELS` s'il est defini (modeles partages entre installations), sinon `<donnees>\models`.
    pub fn models_dir(&self) -> PathBuf {
        std::env::var_os("VOICECLONE_MODELS").map(PathBuf::from).unwrap_or_else(|| self.dir.join("models"))
    }

    /// Modele de synthese choisi dans Parametres (par defaut le 1.7B Q8).
    fn talker(&self) -> String {
        self.store.lock().unwrap().setting("talker").filter(|f| models::is_known(f)).unwrap_or_else(|| models::DEFAULT_TALKER.into())
    }

    /// Etat du catalogue pour la page Parametres.
    pub fn models(&self) -> Value {
        let (active, dir) = (self.talker(), self.models_dir());
        // Verrou pris = generation en cours, donc le modele actif est en memoire.
        let loaded = match self.rt.try_lock() {
            Ok(rt) => rt.engine.as_ref().map(|(f, _)| f.clone()),
            Err(_) => Some(active.clone()),
        };
        let downloading = self.downloads.lock().unwrap();
        let list: Vec<Value> = models::CATALOG
            .iter()
            .map(|&(file, label_fr, label_en, mb)| {
                let label = if i18n::is_en() { label_en } else { label_fr };
                let codec = file == models::CODEC;
                json!({"file": file, "label": label, "size_mb": mb, "codec": codec, "downloaded": dir.join(file).exists(),
                    "downloading": downloading.contains_key(file), "active": file == active,
                    "loaded": loaded.as_deref() == Some(file) || (codec && loaded.is_some())})
            })
            .collect();
        json!({"models": list, "dir": dir, "loaded": loaded.is_some(), "source": models::REPO})
    }

    /// Le prochain `speak` chargera ce modele a la place de l'ancien.
    pub fn set_talker(&self, file: &str) -> R<()> {
        if !models::is_known(file) || file == models::CODEC {
            return Err(tr!("modèle inconnu : {}", "unknown model: {}", file));
        }
        if !self.models_dir().join(file).exists() {
            return Err(tr!("ce modèle n'est pas encore téléchargé", "this model is not downloaded yet"));
        }
        self.store.lock().unwrap().set_setting("talker", file)
    }

    /// Telecharge un modele du catalogue (bloquant). `progress(octets recus, total)`.
    pub fn download_model(&self, file: &str, progress: &dyn Fn(u64, u64)) -> R<()> {
        if !models::is_known(file) {
            return Err(tr!("modèle inconnu : {}", "unknown model: {}", file));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut d = self.downloads.lock().unwrap();
            if d.contains_key(file) {
                return Err(tr!("téléchargement déjà en cours", "download already in progress"));
            }
            d.insert(file.into(), cancel.clone());
        }
        let r = models::download(&format!("{}{file}", models::REPO), &self.models_dir().join(file), &cancel, progress);
        self.downloads.lock().unwrap().remove(file);
        r
    }

    pub fn cancel_download(&self, file: &str) {
        if let Some(c) = self.downloads.lock().unwrap().get(file) {
            c.store(true, Ordering::Relaxed);
        }
    }

    /// Libere la RAM et la VRAM du modele ; il sera recharge au prochain `speak`.
    pub fn unload(&self) -> R<()> {
        let mut rt = self.rt.try_lock().map_err(|_| busy())?;
        rt.voices.clear();
        rt.engine = None;
        rt.device = None;
        rt.note = None;
        Ok(())
    }

    // ---------------------------------------------------------------- carte graphique

    fn gpu_setting(&self, key: &str) -> String {
        let value = self.store.lock().unwrap().setting(key).unwrap_or_default();
        // Valeurs connues seulement ; le reste (ancien reglage, base modifiee a la main) vaut « auto ».
        if key == "device" || matches!(value.as_str(), "on" | "off") { value } else { "auto".into() }
    }

    /// Charge le moteur : le peripherique choisi d'abord (ou le meilleur en mode automatique), puis les
    /// suivants si le chargement echoue (pilote absent, memoire insuffisante...), le processeur en dernier.
    /// Retourne le moteur, le peripherique retenu et, s'il a fallu en changer, ce qui s'est passe.
    fn load_engine(&self, talker: &str) -> R<(Engine, String, Option<String>)> {
        let devs = engine::devices();
        let choice = std::env::var("VOICECLONE_DEVICE").unwrap_or_else(|_| self.gpu_setting("device"));
        let (fa, clamp) = (self.gpu_setting("fa"), self.gpu_setting("clamp"));
        let mut order = engine::auto_order(&devs);
        let mut notes: Vec<String> = vec![];
        if choice != "auto" && !choice.is_empty() {
            if order.contains(&choice) {
                order.retain(|n| *n != choice);
                order.insert(0, choice.clone());
            } else {
                notes.push(tr!("{} est introuvable, choix automatique", "{} not found, automatic choice", choice));
            }
        }
        let dir = self.models_dir();
        for f in [talker, models::CODEC] {
            if !dir.join(f).exists() {
                return Err(tr!("modèle absent ({}) : téléchargez-le dans Paramètres", "model missing ({}): download it in Settings", f));
            }
        }
        for name in &order {
            let desc = devs.iter().find(|d| d.name == *name).map_or("", |d| d.description.as_str());
            let cuda = name.to_ascii_lowercase().starts_with("cuda");
            // Reglages par defaut : CUDA = attention flash (valeur du moteur) et clamp FP16 selon la carte ;
            // Vulkan et le reste = les reglages valides sur la RX 6700 XT (pas de flash, pas de clamp).
            let use_fa = match fa.as_str() { "on" => true, "off" => false, _ => cuda };
            let clamp_fp16 = match clamp.as_str() { "on" => true, "off" => false, _ => cuda && engine::needs_clamp(desc) };
            match Engine::load(&dir.join(talker), &dir.join(models::CODEC), name, use_fa, clamp_fp16) {
                Ok(loaded) => return Ok((loaded, name.clone(), (!notes.is_empty()).then(|| notes.join(" · ")))),
                Err(x) => notes.push(tr!("{} indisponible ({})", "{} unavailable ({})", name, x)),
            }
        }
        Err(tr!("aucun périphérique n'a pu charger le modèle : {}", "no device could load the model: {}", notes.join(" · ")))
    }

    /// Etat de la page Parametres : peripheriques detectes, meilleur choix, choix actuel, moteur en cours.
    pub fn gpu_status(&self) -> Value {
        let devs = engine::devices();
        let best = engine::auto_order(&devs).into_iter().next();
        let (active, note) = match self.rt.try_lock() {
            Ok(rt) => (rt.device.clone(), rt.note.clone()),
            Err(_) => (None, None), // generation en cours : l'etat reste valable, on ne bloque pas l'interface
        };
        let has_cuda = devs.iter().any(|d| d.name.to_ascii_lowercase().starts_with("cuda"));
        let nvidia = devs.iter().any(|d| d.gpu && d.description.to_ascii_uppercase().contains("NVIDIA"));
        json!({
            "devices": devs.iter().map(|d| json!({"name": d.name, "description": d.description, "gpu": d.gpu,
                "integrated": d.integrated, "total_mb": d.total_mb, "free_mb": d.free_mb,
                "recommended": best.as_deref() == Some(d.name.as_str())})).collect::<Vec<_>>(),
            "best": best, "choice": self.gpu_setting("device"), "fa": self.gpu_setting("fa"), "clamp": self.gpu_setting("clamp"),
            "active": active, "note": note,
            // Carte NVIDIA vue uniquement par Vulkan : elle marche, mais CUDA serait plus rapide.
            "nvidia_without_cuda": nvidia && !has_cuda,
        })
    }

    /// Enregistre le choix du peripherique et les options ; le modele est decharge pour qu'ils s'appliquent
    /// des la prochaine generation.
    pub fn set_gpu(&self, device: &str, fa: &str, clamp: &str) -> R<()> {
        if device != "auto" && !engine::devices().iter().any(|d| d.name == device) {
            return Err(tr!("périphérique inconnu : {}", "unknown device: {}", device));
        }
        for v in [fa, clamp] {
            if !matches!(v, "auto" | "on" | "off") {
                return Err(tr!("valeur inconnue : {}", "unknown value: {}", v));
            }
        }
        self.unload()?;
        let store = self.store.lock().unwrap();
        store.set_setting("device", device)?;
        store.set_setting("fa", fa)?;
        store.set_setting("clamp", clamp)
    }

    pub fn delete_model(&self, file: &str) -> R<()> {
        if !models::is_known(file) {
            return Err(tr!("modèle inconnu : {}", "unknown model: {}", file));
        }
        if self.downloads.lock().unwrap().contains_key(file) {
            return Err(tr!("téléchargement en cours : annulez-le d'abord", "download in progress: cancel it first"));
        }
        // Le moteur garde le fichier ouvert : on le decharge s'il s'en sert.
        let mut rt = self.rt.try_lock().map_err(|_| busy())?;
        if rt.engine.as_ref().is_some_and(|(f, _)| f == file || file == models::CODEC) {
            rt.voices.clear();
            rt.engine = None;
            rt.device = None;
            rt.note = None;
        }
        let _ = fs::remove_file(self.models_dir().join(format!("{file}.part")));
        match fs::remove_file(self.models_dir().join(file)) {
            Err(x) if x.kind() != std::io::ErrorKind::NotFound => Err(format!("{file} : {x}")),
            _ => Ok(()),
        }
    }

    /// Appele apres chaque generation (y compris celles demandees par MCP) pour rafraichir l'interface.
    pub fn set_on_change(&self, hook: Hook) {
        *self.on_change.lock().unwrap() = Some(hook);
    }

    pub fn list_profiles(&self) -> R<Value> {
        self.store.lock().unwrap().list_profiles()
    }

    pub fn list_generations(&self, limit: i64) -> R<Value> {
        self.store.lock().unwrap().list_generations(limit)
    }

    pub fn create_profile(&self, name: &str, language: &str) -> R<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr!("nom de voix vide", "empty voice name"));
        }
        self.store.lock().unwrap().add_profile(name, language)
    }

    pub fn delete_profile(&self, id: i64) -> R<()> {
        self.store.lock().unwrap().delete_profile(id)?;
        let _ = fs::remove_dir_all(self.dir.join("profiles").join(id.to_string()));
        Ok(())
    }

    /// Ajoute un echantillon (PCM mono 24 kHz, duree libre) au profil. La transcription se regle ensuite.
    pub fn add_sample(&self, profile_id: i64, pcm: &[f32]) -> R<i64> {
        if pcm.len() < audio::SR as usize {
            return Err(tr!("échantillon trop court (moins d'une seconde)", "sample too short (under one second)"));
        }
        let dir = self.dir.join("profiles").join(profile_id.to_string());
        fs::create_dir_all(&dir).map_err(e)?;
        let path = dir.join(format!("{}.wav", now_ms()?));
        audio::write_wav(&path, pcm)?;
        let r = self.store.lock().unwrap().add_sample(profile_id, &path.to_string_lossy(), "");
        if r.is_err() {
            let _ = fs::remove_file(&path);
        }
        r
    }

    pub fn set_sample_text(&self, id: i64, text: &str) -> R<()> {
        self.store.lock().unwrap().set_sample_text(id, text.trim())
    }

    pub fn delete_sample(&self, id: i64) -> R<()> {
        let store = self.store.lock().unwrap();
        let path = store.sample_path(id)?;
        store.delete_sample(id)?;
        let _ = fs::remove_file(path);
        Ok(())
    }

    /// Analyse la qualite des echantillons d'une voix : mesures, problemes par echantillon et problemes de la voix.
    pub fn voice_quality(&self, profile_id: i64) -> R<Value> {
        let samples = self.store.lock().unwrap().samples(profile_id)?;
        let mut metrics = vec![];
        let mut rows = vec![];
        for s in &samples {
            let m = quality::measure(&audio::read_wav_24k(Path::new(&s.audio))?);
            rows.push(json!({"id": s.id, "secs": (m.secs * 10.0).round() / 10.0, "peak": (m.peak as f64 * 100.0).round() / 100.0,
                "clipped_pct": (m.clipped_pct as f64 * 1000.0).round() / 1000.0, "snr_db": m.snr_db.round() as i64, "f0": m.f0.map(|f| f.round() as i64),
                "issues": quality::sample_issues(&m, &s.ref_text)}));
            metrics.push(m);
        }
        Ok(json!({"samples": rows, "voice_issues": quality::voice_issues(&metrics)}))
    }

    /// Transcrit l'echantillon `sample_id` avec Whisper (installe a part) et enregistre le texte comme sa transcription.
    pub fn transcribe_sample(&self, sample_id: i64) -> R<String> {
        let (path, language) = {
            let store = self.store.lock().unwrap();
            let profiles = store.list_profiles()?;
            let mut found = None;
            for p in profiles.as_array().into_iter().flatten() {
                let pid = p["id"].as_i64().unwrap_or(0);
                if let Some(s) = store.samples(pid)?.into_iter().find(|s| s.id == sample_id) {
                    found = Some((s.audio, p["language"].as_str().unwrap_or("auto").to_string()));
                    break;
                }
            }
            found.ok_or_else(|| tr!("échantillon introuvable", "sample not found"))?
        };
        let text = transcribe::transcribe(&self.dir, &audio::read_wav_24k(Path::new(&path))?, transcribe::lang_code(&language))?;
        self.set_sample_text(sample_id, &text)?;
        Ok(text)
    }

    pub fn list_samples(&self, profile_id: i64) -> R<Value> {
        let samples = self.store.lock().unwrap().samples(profile_id)?;
        Ok(Value::Array(
            samples
                .iter()
                .map(|s| {
                    let dur = sample_secs(&s.audio);
                    json!({"id": s.id, "ref_text": s.ref_text, "duration_sec": (dur * 10.0).round() / 10.0,
                        "suspect": !audio::transcript_plausible(&s.ref_text, dur)})
                })
                .collect(),
        ))
    }

    /// La generation a ete ecoutee : elle n'est plus mise en evidence.
    pub fn mark_heard(&self, id: i64) -> R<()> {
        self.store.lock().unwrap().set_heard(id)
    }

    pub fn delete_generation(&self, id: i64) -> R<()> {
        let store = self.store.lock().unwrap();
        let g = store.generation(id)?;
        store.delete_generation(id)?;
        let _ = fs::remove_file(&g.original);
        if let Some(p) = g.enhanced {
            let _ = fs::remove_file(p);
        }
        Ok(())
    }

    /// Supprime toutes les generations (historique et fichiers audio). Retourne leur nombre.
    pub fn delete_all_generations(&self) -> R<usize> {
        let mut n = 0;
        loop {
            let batch = self.list_generations(500)?;
            let ids: Vec<i64> = batch.as_array().into_iter().flatten().filter_map(|g| g["id"].as_i64()).collect();
            if ids.is_empty() {
                return Ok(n);
            }
            for id in ids {
                self.delete_generation(id)?;
                n += 1;
            }
        }
    }

    /// Version phonetique du texte (francais seulement) : `None` si la langue n'est pas geree.
    pub fn phoneticize(&self, text: &str, language: &str) -> Option<String> {
        let code = transcribe::lang_code(&cli::language(language).unwrap_or_default());
        (code == "fr").then(|| normalize::phonetic_fr(text, &self.pronunciations()))
    }

    // ------------------------------------------------------------ prononciation
    /// Dictionnaire de prononciation de l'utilisateur : (ecrit, dit).
    pub fn pronunciations(&self) -> Vec<(String, String)> {
        let raw = self.store.lock().unwrap().setting("pronunciations").unwrap_or_default();
        serde_json::from_str::<Value>(&raw)
            .ok()
            .and_then(|v| v.as_array().map(|a| a.iter().filter_map(|e| Some((e["from"].as_str()?.to_string(), e["to"].as_str()?.to_string()))).collect()))
            .unwrap_or_default()
    }

    pub fn set_pronunciations(&self, entries: &[(String, String)]) -> R<()> {
        if entries.len() > 300 {
            return Err(tr!("300 entrées au plus", "300 entries at most"));
        }
        let mut clean = vec![];
        for (from, to) in entries {
            let (from, to) = (from.trim(), to.trim());
            if from.is_empty() {
                continue;
            }
            if from.chars().count() > 60 || to.chars().count() > 200 || from.chars().chain(to.chars()).any(char::is_control) {
                return Err(tr!("entrée invalide : « {} »", "invalid entry: \"{}\"", from));
            }
            clean.push(json!({"from": from, "to": to}));
        }
        self.store.lock().unwrap().set_setting("pronunciations", &Value::Array(clean).to_string())
    }

    /// Nombres ecrits en toutes lettres avant la synthese (oui par defaut).
    pub fn numbers_words(&self) -> bool {
        self.store.lock().unwrap().setting("numbers_words").as_deref() != Some("0")
    }

    pub fn set_numbers_words(&self, on: bool) -> R<()> {
        self.store.lock().unwrap().set_setting("numbers_words", if on { "1" } else { "0" })
    }

    // ------------------------------------------------------------ export pour le montage
    fn timeline_of(&self, id: i64) -> R<Vec<export::Cue>> {
        let json = self.store.lock().unwrap().timeline(id)?.ok_or_else(|| tr!("cette génération n'a pas de chronologie (créée avant cette fonction)", "this generation has no timeline (made before this feature)"))?;
        export::from_json(&json)
    }

    /// Ecrit les sous-titres `.srt` de la generation vers `dest` (extension completee). Retourne le chemin ecrit.
    pub fn export_srt_to(&self, id: i64, dest: &Path) -> R<PathBuf> {
        let cues = self.timeline_of(id)?;
        let mut dest = dest.to_path_buf();
        if !dest.extension().is_some_and(|x| x.eq_ignore_ascii_case("srt")) {
            let mut name = dest.file_name().ok_or_else(|| tr!("nom de fichier vide", "empty file name"))?.to_os_string();
            name.push(".srt");
            dest.set_file_name(name);
        }
        // BOM : certains logiciels de montage lisent sinon le fichier en ANSI.
        fs::write(&dest, format!("\u{feff}{}", export::srt(&cues))).map_err(|x| format!("{} : {x}", dest.display()))?;
        Ok(dest)
    }

    /// Ecrit dans `dir` une piste WAV par voix (meme duree que l'audio, silence quand elle ne parle pas) et les
    /// sous-titres. Retourne les fichiers crees.
    pub fn export_stems_to(&self, id: i64, dir: &Path) -> R<Vec<PathBuf>> {
        let cues = self.timeline_of(id)?;
        let pcm = audio::read_wav_24k(&self.audio_path("generation", id)?)?;
        let base = self.export_name(id, "wav")?.trim_end_matches(".wav").to_string();
        fs::create_dir_all(dir).map_err(e)?;
        let mut made = vec![];
        for voice in export::voices(&cues) {
            let path = dir.join(format!("{base}-{}.wav", voices::folder_name(&voice)));
            audio::write_wav(&path, &export::stem(&pcm, &cues, &voice))?;
            made.push(path);
        }
        made.push(self.export_srt_to(id, &dir.join(&base))?);
        Ok(made)
    }

    /// Active ou desactive l'amelioration (egaliseur + normalisation) d'une generation.
    /// L'original n'est jamais modifie : la version amelioree est un fichier a part, supprime a la desactivation.
    pub fn set_enhanced(&self, id: i64, on: bool) -> R<()> {
        let g = self.store.lock().unwrap().generation(id)?;
        if on == g.enhanced.is_some() {
            return Ok(());
        }
        if on {
            let mut pcm = audio::read_wav_24k(Path::new(&g.original))?;
            audio::enhance(&mut pcm);
            let path = Path::new(&g.original).with_extension("enh.wav");
            audio::write_wav(&path, &pcm)?;
            self.store.lock().unwrap().set_enhanced_path(id, Some(&path.to_string_lossy()))
        } else {
            self.store.lock().unwrap().set_enhanced_path(id, None)?;
            let _ = fs::remove_file(g.enhanced.unwrap());
            Ok(())
        }
    }

    /// Nom de fichier propose pour l'export : debut du texte, + « -ameliore » si l'amelioration est active.
    pub fn export_name(&self, id: i64, format: &str) -> R<String> {
        let g = self.store.lock().unwrap().generation(id)?;
        let slug: String = g.text.chars().take(40).map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
        let slug = slug.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
        let suffix = if g.enhanced.is_some() { "-ameliore" } else { "" };
        let fallback = if i18n::is_en() { "voice" } else { "voix" };
        Ok(format!("{}{suffix}.{format}", if slug.is_empty() { fallback } else { &slug }))
    }

    /// Ecrit la version en vigueur de la generation (amelioree ou non) vers `dest`, en "wav" ou "mp3".
    /// L'extension est completee si le nom choisi n'a pas celle du format. Retourne le chemin ecrit.
    pub fn export_to(&self, id: i64, format: &str, dest: &Path) -> R<PathBuf> {
        if !matches!(format, "wav" | "mp3") {
            return Err(tr!("format inconnu : {}", "unknown format: {}", format));
        }
        let g = self.store.lock().unwrap().generation(id)?;
        let mut dest = dest.to_path_buf();
        if !dest.extension().is_some_and(|x| x.eq_ignore_ascii_case(format)) {
            let mut name = dest.file_name().ok_or_else(|| tr!("nom de fichier vide", "empty file name"))?.to_os_string();
            name.push(format!(".{format}"));
            dest.set_file_name(name);
        }
        let err = |x: std::io::Error| format!("{} : {x}", dest.display());
        if format == "wav" {
            fs::copy(g.active(), &dest).map(|_| ()).map_err(err)?;
        } else {
            fs::write(&dest, audio::encode_mp3(&audio::read_wav_24k(Path::new(g.active()))?)?).map_err(err)?;
        }
        Ok(dest)
    }

    /// Forme d'onde pour l'affichage : `bars` crêtes de 0 à 100, relatives au pic du fichier
    /// (échelle racine carrée pour que les passages faibles restent visibles).
    pub fn waveform(&self, kind: &str, id: i64, bars: usize) -> R<Vec<u8>> {
        let pcm = audio::read_wav_24k(&self.audio_path(kind, id)?)?;
        let bars = bars.clamp(1, 200);
        let size = pcm.len().div_ceil(bars).max(1);
        let peaks: Vec<f32> = pcm.chunks(size).map(|c| c.iter().fold(0.0f32, |m, x| m.max(x.abs()))).collect();
        let top = peaks.iter().cloned().fold(0.0f32, f32::max);
        if top <= 0.0 {
            return Ok(vec![0; peaks.len()]);
        }
        Ok(peaks.iter().map(|p| ((p / top).sqrt() * 100.0).round() as u8).collect())
    }

    /// Dernier dossier choisi pour un export (None au premier usage).
    pub fn export_dir(&self) -> Option<PathBuf> {
        self.store.lock().unwrap().setting("export_dir").map(PathBuf::from).filter(|p| p.is_dir())
    }

    pub fn remember_export_dir(&self, dir: &Path) -> R<()> {
        self.store.lock().unwrap().set_setting("export_dir", &dir.to_string_lossy())
    }

    /// Chemin du WAV d'une generation ("generation", version en vigueur) ou d'un echantillon ("sample").
    pub fn audio_path(&self, kind: &str, id: i64) -> R<PathBuf> {
        let store = self.store.lock().unwrap();
        Ok(match kind {
            "generation" => store.generation(id)?.active().to_string(),
            "sample" => store.sample_path(id)?,
            k => return Err(tr!("type audio inconnu : {}", "unknown audio type: {}", k)),
        }
        .into())
    }

    /// Genere `text` avec la voix du profil. Le modele est charge au premier appel puis reste en memoire.
    /// `enhanced` : egaliseur + normalisation (comme le bouton « Amelioration ») ; `output` recoit alors cette version.
    pub fn speak(&self, text: &str, profile: Option<&str>, lang: Option<&str>, seed: Option<i64>, output: Option<&Path>, enhanced: bool) -> R<Value> {
        let items = script::parse(text)?;
        if !items.iter().any(|i| matches!(i, Item::Text(_))) {
            return Err(tr!("texte vide", "empty text"));
        }
        if let Some(o) = output {
            let wav = o.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav"));
            if !o.is_absolute() || !wav {
                return Err(tr!("output_path doit être un chemin absolu en .wav", "output_path must be an absolute path ending in .wav"));
            }
        }
        // Distribution : la voix principale, puis chaque voix appelee par une balise [Nom] (sans tenir compte de la casse).
        enum Step {
            Silence(usize),
            Say(usize, String, String), // voix, texte dit (normalise), texte affiche (d'origine)
        }
        type Cast = Vec<(store::ProfileRef, Vec<store::Sample>)>;
        let (dict, numbers) = (self.pronunciations(), self.numbers_words());
        let lang_code = |name: &str| transcribe::lang_code(&cli::language(name).unwrap_or_default());
        let (cast, steps) = {
            let store = self.store.lock().unwrap();
            let main = store.find_profile(profile)?;
            let names: Vec<String> = store.list_profiles()?.as_array().into_iter().flatten().filter_map(|v| v["name"].as_str().map(String::from)).collect();
            let join = |cast: &mut Cast, p: store::ProfileRef| -> R<usize> {
                if let Some(i) = cast.iter().position(|(c, _)| c.id == p.id) {
                    return Ok(i);
                }
                let samples = store.samples(p.id)?;
                if samples.is_empty() {
                    return Err(tr!("la voix « {} » n'a aucun échantillon", "voice \"{}\" has no sample", p.name));
                }
                cast.push((p, samples));
                Ok(cast.len() - 1)
            };
            let (mut cast, mut steps): (Cast, Vec<Step>) = (vec![], vec![]);
            let mut current = join(&mut cast, main)?;
            for item in items {
                match item {
                    Item::Pause(s) => steps.push(Step::Silence((s * audio::SR as f32) as usize)),
                    Item::Voice(n) => {
                        let real = names.iter().find(|x| x.to_lowercase() == n.to_lowercase()).ok_or_else(|| {
                            tr!("voix inconnue : [{}] (voix disponibles : {})", "unknown voice: [{}] (available voices: {})", n, names.join(", "))
                        })?;
                        current = join(&mut cast, store.find_profile(Some(real))?)?;
                    }
                    Item::Text(t) => {
                        let code = lang_code(lang.unwrap_or(&cast[current].0.language));
                        steps.extend(audio::split_text(&t, MAX_CHUNK_CHARS).into_iter().map(|c| Step::Say(current, normalize::normalize(&c, &dict, numbers, code), c)));
                    }
                }
            }
            (cast, steps)
        };
        // Voix qui parlent vraiment (dans l'ordre) : la generation est rangee sous la premiere ; plusieurs = « multiple ».
        let used: Vec<usize> = steps.iter().fold(vec![], |mut u, s| {
            if let Step::Say(i, _, _) = s {
                if !u.contains(i) {
                    u.push(*i);
                }
            }
            u
        });
        let p = cast[used.first().copied().unwrap_or(0)].0.clone();
        let (lang, seed) = (lang.map(String::from), seed.unwrap_or(DEFAULT_SEED));
        let t0 = Instant::now();

        let mut pcm = Vec::new();
        let mut warnings: Vec<String> = vec![];
        let mut timeline: Vec<export::Cue> = vec![];
        {
            let mut rt = self.rt.lock().unwrap();
            let Runtime { engine, voices, device, note } = &mut *rt;
            let talker = self.talker();
            if engine.as_ref().map_or(true, |(f, _)| *f != talker) {
                // Les voix extraites dependent du modele : on libere tout avant de charger le nouveau.
                voices.clear();
                *engine = None;
                *device = None;
                let (loaded, name, why) = self.load_engine(&talker)?;
                *engine = Some((talker, loaded));
                *device = Some(name);
                *note = why;
            }
            let engine = &engine.as_ref().unwrap().1;
            for (vp, samples) in &cast {
                // Une transcription qui ne correspond pas a l'audio (enregistrement coupe, texte propose laisse en place)
                // fait deriver le clonage : on l'ignore alors (timbre seul, moins fidele) et on le dit.
                let bad: Vec<usize> = samples.iter().enumerate()
                    .filter(|(_, s)| !audio::transcript_plausible(&s.ref_text, sample_secs(&s.audio))).map(|(i, _)| i + 1).collect();
                if !bad.is_empty() {
                    warnings.push(tr!(
                        "voix « {} » : la transcription de l'échantillon {} ne correspond pas à son audio ; elle est ignorée (clonage moins fidèle). Corrigez-la dans l'application.",
                        "voice \"{}\": the transcript of sample {} does not match its audio; it is ignored (less faithful cloning). Fix it in the app.",
                        vp.name, bad.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")));
                }
                let complete = bad.is_empty() && samples.iter().all(|s| !s.ref_text.trim().is_empty());
                let signature: String = samples.iter().map(|s| format!("{}:{}|", s.id, s.ref_text)).collect::<String>() + if complete { "icl" } else { "x" };
                if voices.get(&vp.id).map_or(true, |(sig, _)| *sig != signature) {
                    // Tous les echantillons du profil servent, bout a bout, avec 300 ms de silence entre eux.
                    let mut reference = Vec::new();
                    for s in samples {
                        reference.extend(audio::read_wav_24k(Path::new(&s.audio))?);
                        reference.resize(reference.len() + audio::SR as usize * 3 / 10, 0.0);
                    }
                    // Le clonage fidele exige la transcription (correcte) de tout l'audio ; sinon timbre seul (moins fidele).
                    let ref_text = complete.then(|| samples.iter().map(|s| s.ref_text.trim()).collect::<Vec<_>>().join(" "));
                    voices.insert(vp.id, (signature, engine.voice(&reference, ref_text.as_deref())?));
                }
            }
            let mut spoken = false; // le dernier pas a-t-il produit de la voix ? (silence de 300 ms entre deux morceaux parles)
            for step in &steps {
                match step {
                    Step::Silence(n) => {
                        pcm.resize(pcm.len() + n, 0.0);
                        spoken = false;
                    }
                    Step::Say(i, chunk, shown) => {
                        if spoken {
                            pcm.resize(pcm.len() + audio::SR as usize * 3 / 10, 0.0);
                        }
                        let (vp, _) = &cast[*i];
                        let start = pcm.len();
                        synth_capped(engine, chunk, lang.as_deref().unwrap_or(&vp.language), &voices[&vp.id].1, seed, &mut pcm)?;
                        timeline.push(export::Cue { voice: vp.name.clone(), start: start as f64 / audio::SR as f64, end: pcm.len() as f64 / audio::SR as f64, text: shown.clone() });
                        spoken = true;
                    }
                }
            }
        }
        audio::clean(&mut pcm);

        // Nom lisible tire du texte (« Bonjour-je-suis-Zen.wav », puis -2, -3...) dans le dossier Audios du programme.
        let stored = cli::free_path(&self.audios_dir(), &cli::file_stem(&script::strip(text), false));
        audio::write_wav(&stored, &pcm)?;
        let duration = pcm.len() as f64 / audio::SR as f64;
        let id = self.store.lock().unwrap().add_generation(p.id, text.trim(), &stored.to_string_lossy(), duration, seed)?;
        if used.len() > 1 {
            self.store.lock().unwrap().set_multi(id)?;
        }
        self.store.lock().unwrap().set_timeline(id, &export::to_json(&timeline))?;
        // La generation existe des maintenant dans l'historique ; si la suite echoue on la retire pour ne pas laisser d'orphelin.
        let finish = || -> R<PathBuf> {
            if enhanced {
                self.set_enhanced(id, true)?;
            }
            let active = self.audio_path("generation", id)?; // version amelioree si activee, sinon originale
            let Some(o) = output else { return Ok(active) };
            if let Some(parent) = o.parent() {
                fs::create_dir_all(parent).map_err(e)?;
            }
            fs::copy(&active, o).map_err(|x| format!("{} : {x}", o.display()))?;
            Ok(o.to_path_buf())
        };
        let path = match finish() {
            Ok(p) => p,
            Err(x) => {
                let _ = self.delete_generation(id);
                return Err(x);
            }
        };
        if let Some(hook) = &*self.on_change.lock().unwrap() {
            hook();
        }
        Ok(json!({"id": id, "path": path, "duration_sec": (duration * 100.0).round() / 100.0, "profile": p.name,
            "seed": seed, "enhanced": enhanced, "elapsed_sec": (t0.elapsed().as_secs_f64() * 10.0).round() / 10.0, "warnings": warnings}))
    }

    /// Copie les profils clones de Voicebox (dossier de donnees `sh.voicebox.app`), sans y toucher.
    /// `rename` renomme le profil s'il n'y en a qu'un.
    pub fn import_voicebox(&self, vb_dir: &Path, rename: Option<&str>) -> R<Vec<String>> {
        let vb = rusqlite::Connection::open_with_flags(vb_dir.join("voicebox.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(e)?;
        let mut st = vb.prepare("SELECT id,name,language FROM profiles WHERE voice_type='cloned'").map_err(e)?;
        let profiles: Vec<(String, String, String)> =
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
        let store = self.store.lock().unwrap();
        let mut done = vec![];
        for (vb_id, vb_name, lang) in &profiles {
            let name = rename.filter(|_| profiles.len() == 1).unwrap_or(vb_name);
            let id = store.add_profile(name, lang_name(lang))?;
            let dest = self.dir.join("profiles").join(id.to_string());
            fs::create_dir_all(&dest).map_err(e)?;
            let mut st = vb.prepare("SELECT audio_path,reference_text FROM profile_samples WHERE profile_id=?1").map_err(e)?;
            let samples: Vec<(String, String)> =
                st.query_map([vb_id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
            for (rel, text) in samples {
                let src = vb_dir.join(&rel);
                let to = dest.join(src.file_name().ok_or_else(|| tr!("chemin d'échantillon invalide", "invalid sample path"))?);
                fs::copy(&src, &to).map_err(|x| format!("{} : {x}", src.display()))?;
                store.add_sample(id, &to.to_string_lossy(), &text)?;
            }
            done.push(name.to_string());
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Avec les vrais modeles : un peripherique enregistre mais absent (carte retiree, ou CUDA sur une machine AMD)
    /// ne doit pas empecher de generer, le moteur se rabat sur le meilleur peripherique et explique pourquoi.
    /// `cargo test -p voiceclone -- --ignored`
    #[test]
    #[ignore = "demande les modeles installes et un GPU"]
    fn missing_device_falls_back() {
        let models = PathBuf::from(std::env::var("APPDATA").unwrap()).join("VoiceClone").join("models");
        if !models.join(models::DEFAULT_TALKER).exists() {
            return;
        }
        std::env::set_var("VOICECLONE_MODELS", &models);
        let dir = std::env::temp_dir().join(format!("voiceclone-gpu-{}", std::process::id()));
        let app = App::open(dir.clone()).unwrap();
        let best = engine::auto_order(&engine::devices())[0].clone();

        app.store.lock().unwrap().set_setting("device", "CUDA0").unwrap();
        let status = app.gpu_status();
        assert_eq!(status["choice"], "CUDA0");
        assert_eq!(status["best"], best.as_str());
        let (_engine, name, note) = app.load_engine(models::DEFAULT_TALKER).unwrap();
        assert_eq!(name, best, "repli sur le meilleur peripherique");
        assert!(note.unwrap().contains("CUDA0"), "la note nomme le peripherique manquant");

        // Choix explicite d'un peripherique present : il est utilise sans note.
        app.set_gpu(&best, "auto", "auto").unwrap();
        let (_engine, name, note) = app.load_engine(models::DEFAULT_TALKER).unwrap();
        assert_eq!((name, note), (best, None));
        drop(app);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn gpu_settings_are_validated() {
        let dir = std::env::temp_dir().join(format!("voiceclone-gpuset-{}", std::process::id()));
        let app = App::open(dir.clone()).unwrap();
        assert!(app.set_gpu("CUDA9", "auto", "auto").is_err(), "peripherique inconnu refuse");
        assert!(app.set_gpu("auto", "peut-etre", "auto").is_err() && app.set_gpu("auto", "auto", "x").is_err());
        app.set_gpu("CPU", "on", "off").unwrap();
        let s = app.gpu_status();
        assert_eq!((s["choice"].as_str(), s["fa"].as_str(), s["clamp"].as_str()), (Some("CPU"), Some("on"), Some("off")));
        assert!(s["devices"].as_array().unwrap().iter().filter(|d| d["recommended"] == true).count() <= 1);
        // Un reglage abime dans la base retombe sur « auto » au lieu de passer tel quel au moteur.
        app.store.lock().unwrap().set_setting("fa", "n'importe quoi").unwrap();
        assert_eq!(app.gpu_status()["fa"], "auto");
        drop(app);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn srt_and_stems_are_exported_from_the_timeline() {
        let dir = std::env::temp_dir().join(format!("voiceclone-montage-{}", std::process::id()));
        let app = App::open(dir.clone()).unwrap();
        let pid = app.create_profile("Zen", "french").unwrap();
        let wav = dir.join("g.wav");
        let sr = audio::SR as usize;
        audio::write_wav(&wav, &vec![0.4f32; sr * 4]).unwrap();
        let cues = vec![
            export::Cue { voice: "Zen".into(), start: 0.0, end: 1.5, text: "Bonjour. Ça va ?".into() },
            export::Cue { voice: "Dore".into(), start: 2.5, end: 4.0, text: "Très bien 3 fois.".into() },
        ];
        let id = app.store.lock().unwrap().add_generation(pid, "x", &wav.to_string_lossy(), 4.0, 7).unwrap();
        assert!(app.export_srt_to(id, &dir.join("a")).is_err(), "pas de chronologie : refus clair");
        assert_eq!(app.list_generations(5).unwrap()[0]["has_timeline"], false);
        app.store.lock().unwrap().set_timeline(id, &export::to_json(&cues)).unwrap();
        assert_eq!(app.list_generations(5).unwrap()[0]["has_timeline"], true);

        let srt = app.export_srt_to(id, &dir.join("sous-titres")).unwrap();
        assert_eq!(srt.file_name().unwrap(), "sous-titres.srt");
        let text = fs::read_to_string(&srt).unwrap();
        assert!(text.starts_with('\u{feff}') && text.contains("Très bien 3 fois.") && text.contains("00:00:02,500 --> 00:00:04,000"), "{text}");

        let made = app.export_stems_to(id, &dir.join("pistes")).unwrap();
        assert_eq!(made.len(), 3, "{made:?}"); // une piste par voix + les sous-titres
        let zen = audio::read_wav_24k(made.iter().find(|p| p.to_string_lossy().contains("-Zen.wav")).unwrap()).unwrap();
        assert_eq!(zen.len(), sr * 4);
        assert!(zen[sr * 2..].iter().all(|&x| x.abs() < 0.001) && zen[..sr].iter().all(|&x| x.abs() > 0.3));
        drop(app);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pronunciation_settings_round_trip_and_are_validated() {
        let dir = std::env::temp_dir().join(format!("voiceclone-pron-{}", std::process::id()));
        let app = App::open(dir.clone()).unwrap();
        assert!(app.pronunciations().is_empty() && app.numbers_words());
        app.set_pronunciations(&[("RTX".into(), "R T X".into()), ("  ".into(), "ignoré".into())]).unwrap();
        assert_eq!(app.pronunciations(), vec![("RTX".to_string(), "R T X".to_string())]);
        assert!(app.set_pronunciations(&[("a".repeat(61), "x".into())]).is_err());
        assert!(app.set_pronunciations(&[("a\nb".into(), "x".into())]).is_err());
        app.set_numbers_words(false).unwrap();
        assert!(!app.numbers_words());
        drop(app);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn all_generations_can_be_cleared() {
        let dir = std::env::temp_dir().join(format!("voiceclone-clear-{}", std::process::id()));
        let app = App::open(dir.clone()).unwrap();
        let pid = app.create_profile("Zen", "french").unwrap();
        let mut files = vec![];
        for i in 0..3 {
            let f = dir.join(format!("g{i}.wav"));
            fs::write(&f, b"x").unwrap();
            app.store.lock().unwrap().add_generation(pid, "x", &f.to_string_lossy(), 1.0, 7).unwrap();
            files.push(f);
        }
        assert_eq!(app.delete_all_generations().unwrap(), 3);
        assert!(app.list_generations(10).unwrap().as_array().unwrap().is_empty());
        assert!(files.iter().all(|f| !f.exists()));
        assert_eq!(app.delete_all_generations().unwrap(), 0);
        drop(app);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn new_generations_are_unheard_until_marked() {
        let dir = std::env::temp_dir().join(format!("voiceclone-heard-{}", std::process::id()));
        let app = App::open(dir.clone()).unwrap();
        let pid = app.create_profile("Zen", "french").unwrap();
        let id = app.store.lock().unwrap().add_generation(pid, "x", "x.wav", 1.0, 7).unwrap();
        assert_eq!(app.list_generations(10).unwrap()[0]["heard"], false);
        app.mark_heard(id).unwrap();
        assert_eq!(app.list_generations(10).unwrap()[0]["heard"], true);
        drop(app);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn audios_folder_is_created_and_falls_back_when_not_writable() {
        let dir = std::env::temp_dir().join(format!("voiceclone-aud-{}", std::process::id()));
        let mut app = App::open(dir.clone()).unwrap();
        // Dossier impose : cree s'il manque, rien n'est ecrit dans le dossier de donnees.
        app.audios_override = Some(dir.join("mes audios").join("Audios"));
        let d = app.audios_dir();
        assert!(d.is_dir() && d.ends_with("Audios") && !dir.join("Audios").exists());
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0, "le test d'ecriture ne laisse rien derriere lui");
        // Dossier impossible a creer (un fichier porte deja ce nom) : repli sur <donnees>\Audios.
        let blocker = dir.join("bloque");
        fs::write(&blocker, b"x").unwrap();
        app.audios_override = Some(blocker.join("Audios"));
        assert_eq!(app.audios_dir(), dir.join("Audios"));
        assert!(dir.join("Audios").is_dir());
        drop(app);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn enhance_and_export_keep_original() {
        let dir = std::env::temp_dir().join(format!("voiceclone-enh-{}", std::process::id()));
        let mut app = App::open(dir.clone()).unwrap();
        app.audios_override = Some(dir.join("Audios"));
        let audios = app.audios_dir();
        assert_eq!(audios, dir.join("Audios"));
        let pid = app.create_profile("Test", "french").unwrap();
        // Une « generation » faible (-34 dBFS) : sinus 500 Hz, 2 s.
        let pcm: Vec<f32> = (0..48000).map(|i| 0.02 * (2.0 * std::f32::consts::PI * 500.0 * i as f32 / 24000.0).sin()).collect();
        let original = audios.join("1.wav");
        audio::write_wav(&original, &pcm).unwrap();
        let id = app.store.lock().unwrap().add_generation(pid, "Bonjour, ça va ? Très bien !", &original.to_string_lossy(), 2.0, 7).unwrap();
        let out = dir.join("export");
        fs::create_dir_all(&out).unwrap();
        let list = |app: &App| app.list_generations(10).unwrap()[0].clone();

        // Original : nom propose lisible sans suffixe ; le nom choisi par l'utilisateur est respecte
        // (extension completee si absente ou differente), et le WAV exporte est identique.
        assert_eq!(app.export_name(id, "wav").unwrap(), "Bonjour-ça-va-Très-bien.wav");
        let wav = app.export_to(id, "wav", &out.join("Ma voix")).unwrap();
        assert_eq!(wav.file_name().unwrap().to_string_lossy(), "Ma voix.wav");
        assert_eq!(app.export_to(id, "wav", &out.join("v.MP3")).unwrap().file_name().unwrap().to_string_lossy(), "v.MP3.wav");
        assert_eq!(app.export_to(id, "wav", &out.join("B.WAV")).unwrap().file_name().unwrap().to_string_lossy(), "B.WAV");
        assert_eq!(fs::read(&wav).unwrap(), fs::read(&original).unwrap());
        assert_eq!(list(&app)["enhanced"], false);
        assert!(app.export_dir().is_none());
        app.remember_export_dir(&out).unwrap();
        assert_eq!(app.export_dir().unwrap(), out);
        app.remember_export_dir(&out.join("disparu")).unwrap();
        assert!(app.export_dir().is_none(), "un dossier disparu n'est pas reproposé");

        // Amelioration activee : fichier a part, plus fort, l'original ne bouge pas, les exports suivent.
        let before = fs::read(&original).unwrap();
        app.set_enhanced(id, true).unwrap();
        app.set_enhanced(id, true).unwrap(); // idempotent
        let enh = app.audio_path("generation", id).unwrap();
        assert_ne!(enh, original);
        assert_eq!(fs::read(&original).unwrap(), before);
        let level = |p: &Path| { let x = audio::read_wav_24k(p).unwrap(); (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt() };
        assert!(level(&enh) > 3.0 * level(&original), "{} vs {}", level(&enh), level(&original));
        assert_eq!(app.export_name(id, "mp3").unwrap(), "Bonjour-ça-va-Très-bien-ameliore.mp3");
        let (wav2, mp3) = (app.export_to(id, "wav", &out.join("e")).unwrap(), app.export_to(id, "mp3", &out.join("e")).unwrap());
        assert_eq!(fs::read(&wav2).unwrap(), fs::read(&enh).unwrap());
        assert!(fs::metadata(&mp3).unwrap().len() > 5_000 && mp3.extension().unwrap() == "mp3");
        assert_eq!(list(&app)["enhanced"], true);
        assert!(app.export_to(id, "ogg", &out.join("e")).is_err());
        assert!(app.export_to(999, "wav", &out.join("x")).is_err());
        assert!(app.export_to(id, "wav", &out.join("absent").join("x")).is_err(), "dossier absent : erreur claire, pas de panique");

        // Desactivation : retour a l'original, version amelioree supprimee.
        app.set_enhanced(id, false).unwrap();
        assert_eq!(app.audio_path("generation", id).unwrap(), original);
        assert!(!enh.exists());
        assert_eq!(list(&app)["enhanced"], false);

        // Forme d'onde : une valeur par barre, le pic vaut 100, le silence reste a 0, pas de panique si peu d'echantillons.
        let w = app.waveform("generation", id, 40).unwrap();
        assert_eq!(w.len(), 40);
        assert_eq!(w.iter().max(), Some(&100));
        let quiet = audios.join("quiet.wav");
        audio::write_wav(&quiet, &vec![0.0; 24000]).unwrap();
        let qid = app.store.lock().unwrap().add_generation(pid, "silence", &quiet.to_string_lossy(), 1.0, 7).unwrap();
        assert!(app.waveform("generation", qid, 40).unwrap().iter().all(|&v| v == 0));
        let short = audios.join("short.wav");
        audio::write_wav(&short, &[0.5; 10]).unwrap();
        let sid = app.store.lock().unwrap().add_generation(pid, "court", &short.to_string_lossy(), 0.0, 7).unwrap();
        assert_eq!(app.waveform("generation", sid, 40).unwrap().len(), 10);

        // Suppression : tous les fichiers de la generation partent.
        app.set_enhanced(id, true).unwrap();
        let enh = app.audio_path("generation", id).unwrap();
        app.delete_generation(id).unwrap();
        assert!(!original.exists() && !enh.exists());
        assert!(app.set_enhanced(id, true).is_err());
        drop(app);
        let _ = fs::remove_dir_all(dir);
    }
}

/// Code de langue Voicebox -> nom de langue attendu par Qwen3-TTS.
fn lang_name(code: &str) -> &'static str {
    match code {
        "fr" => "french", "en" => "english", "de" => "german", "es" => "spanish", "it" => "italian",
        "pt" => "portuguese", "ja" => "japanese", "ko" => "korean", "zh" => "chinese", "ru" => "russian",
        _ => "auto",
    }
}
