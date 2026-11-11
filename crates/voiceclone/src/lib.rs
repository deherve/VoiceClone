//! Coeur de VoiceClone : utilise par le serveur MCP sans fenetre et par l'application Tauri.
#[macro_use]
pub mod i18n;
pub mod audio;
pub mod cli;
pub mod components;
pub mod engine;
pub mod mcp;
pub mod models;
mod store;
pub mod update;
pub mod voices;

use engine::{Engine, Voice};
use serde_json::{json, Value};
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, time::{Instant, SystemTime, UNIX_EPOCH}};
use store::Store;

pub type R<T> = Result<T, String>;
pub(crate) fn e<E: ToString>(x: E) -> String {
    x.to_string()
}

const MAX_CHUNK_CHARS: usize = 300;
const DEFAULT_SEED: i64 = 7; // meme graine que les generations Voicebox existantes : resultat reproductible

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

    pub fn list_samples(&self, profile_id: i64) -> R<Value> {
        let samples = self.store.lock().unwrap().samples(profile_id)?;
        Ok(Value::Array(
            samples
                .iter()
                .map(|s| {
                    let dur = hound::WavReader::open(&s.audio).map(|r| r.duration() as f64 / audio::SR as f64).unwrap_or(0.0);
                    json!({"id": s.id, "ref_text": s.ref_text, "duration_sec": (dur * 10.0).round() / 10.0})
                })
                .collect(),
        ))
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
        let chunks = audio::split_text(text, MAX_CHUNK_CHARS);
        if chunks.is_empty() {
            return Err(tr!("texte vide", "empty text"));
        }
        if let Some(o) = output {
            let wav = o.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav"));
            if !o.is_absolute() || !wav {
                return Err(tr!("output_path doit être un chemin absolu en .wav", "output_path must be an absolute path ending in .wav"));
            }
        }
        let (p, samples) = {
            let store = self.store.lock().unwrap();
            let p = store.find_profile(profile)?;
            let samples = store.samples(p.id)?;
            (p, samples)
        };
        if samples.is_empty() {
            return Err(tr!("la voix « {} » n'a aucun échantillon", "voice \"{}\" has no sample", p.name));
        }
        let signature: String = samples.iter().map(|s| format!("{}:{}|", s.id, s.ref_text)).collect();
        let (lang, seed) = (lang.unwrap_or(&p.language), seed.unwrap_or(DEFAULT_SEED));
        let t0 = Instant::now();

        let mut pcm = Vec::new();
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
            if voices.get(&p.id).map_or(true, |(sig, _)| *sig != signature) {
                // Tous les echantillons du profil servent, bout a bout, avec 300 ms de silence entre eux.
                let mut reference = Vec::new();
                for s in &samples {
                    reference.extend(audio::read_wav_24k(Path::new(&s.audio))?);
                    reference.resize(reference.len() + audio::SR as usize * 3 / 10, 0.0);
                }
                // Le clonage fidele exige la transcription de tout l'audio ; s'il en manque une,
                // on retombe sur le timbre seul (moins fidele).
                let complete = samples.iter().all(|s| !s.ref_text.trim().is_empty());
                let ref_text = complete.then(|| samples.iter().map(|s| s.ref_text.trim()).collect::<Vec<_>>().join(" "));
                voices.insert(p.id, (signature, engine.voice(&reference, ref_text.as_deref())?));
            }
            for (i, chunk) in chunks.iter().enumerate() {
                if i > 0 {
                    pcm.resize(pcm.len() + audio::SR as usize * 15 / 100, 0.0); // 150 ms de silence entre morceaux
                }
                pcm.extend(engine.synth(chunk, lang, &voices[&p.id].1, seed)?);
            }
        }
        audio::clean(&mut pcm);

        // Nom lisible tire du texte (« Bonjour-je-suis-Zen.wav », puis -2, -3...) dans le dossier Audios du programme.
        let stored = cli::free_path(&self.audios_dir(), &cli::file_stem(text, false));
        audio::write_wav(&stored, &pcm)?;
        let duration = pcm.len() as f64 / audio::SR as f64;
        let id = self.store.lock().unwrap().add_generation(p.id, text.trim(), &stored.to_string_lossy(), duration, seed)?;
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
            "seed": seed, "enhanced": enhanced, "elapsed_sec": (t0.elapsed().as_secs_f64() * 10.0).round() / 10.0}))
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
