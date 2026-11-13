//! Application VoiceClone : fenetre Tauri + serveur MCP local dans le meme processus.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{ipc::{InvokeBody, Request, Response}, Emitter, Manager, State};
use voiceclone::{components, data_dir, mcp, tr, update, App, R};

type Core<'a> = State<'a, Arc<App>>;

struct McpInfo(Value);

#[tauri::command]
fn mcp_info(info: State<'_, McpInfo>) -> Value {
    info.0.clone()
}

#[tauri::command]
fn profiles(app: Core<'_>) -> R<Value> {
    app.list_profiles()
}

#[tauri::command]
fn create_profile(app: Core<'_>, name: String, language: String) -> R<i64> {
    app.create_profile(&name, &language)
}

#[tauri::command]
fn delete_profile(app: Core<'_>, id: i64) -> R<()> {
    app.delete_profile(id)
}

#[tauri::command]
fn samples(app: Core<'_>, profile_id: i64) -> R<Value> {
    app.list_samples(profile_id)
}

/// Corps brut = PCM float32 little-endian, mono 24 kHz (converti par la webview) ; profil dans l'en-tete.
#[tauri::command]
fn add_sample(app: Core<'_>, request: Request<'_>) -> R<i64> {
    let profile_id: i64 = request
        .headers()
        .get("x-profile")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| tr!("en-tête x-profile manquant", "missing x-profile header"))?;
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(tr!("corps audio attendu", "audio body expected"));
    };
    let pcm: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    app.add_sample(profile_id, &pcm)
}

#[tauri::command]
fn set_sample_text(app: Core<'_>, id: i64, text: String) -> R<()> {
    app.set_sample_text(id, &text)
}

#[tauri::command]
fn delete_sample(app: Core<'_>, id: i64) -> R<()> {
    app.delete_sample(id)
}

#[tauri::command]
fn generations(app: Core<'_>) -> R<Value> {
    app.list_generations(200)
}

static TRANSCRIBE_CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static WRITER_CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false); // installation
static WRITE_CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false); // generation

#[tauri::command]
fn writer_status(app: Core<'_>) -> Value {
    voiceclone::writer::status(&app.data_dir())
}

/// Installe l'assistant d'ecriture (moteur + modele) ; progression : evenement "writer-progress" {stage, done, total}.
#[tauri::command]
async fn writer_install(app: Core<'_>, handle: tauri::AppHandle) -> R<()> {
    let data = app.data_dir();
    WRITER_CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        voiceclone::writer::install(&data, &WRITER_CANCEL, &|stage, done, total| {
            let _ = handle.emit("writer-progress", json!({"stage": stage, "done": done, "total": total}));
        })
    })
    .await
    .map_err(|x| x.to_string())?
}

#[tauri::command]
fn writer_cancel_install() {
    WRITER_CANCEL.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
fn writer_remove(app: Core<'_>) -> R<()> {
    voiceclone::writer::remove(&app.data_dir())
}

/// Ecrit un recit ou un dialogue : `mode` « dialogue » ou « story », `language` « fr » ou « en », `seconds` : duree visee a voix haute,
/// `tone` : natural, warm, cheerful, dramatic, funny ou calm, `voices` : noms des voix. Progression : evenement "write-progress".
#[tauri::command]
async fn write_text(app: Core<'_>, handle: tauri::AppHandle, prompt: String, mode: String, language: String, seconds: u32, tone: String, voices: Vec<String>) -> R<String> {
    let data = app.data_dir();
    WRITE_CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        let req = voiceclone::writer::Request {
            prompt,
            mode: if mode == "dialogue" { voiceclone::writer::Mode::Dialogue } else { voiceclone::writer::Mode::Story },
            language,
            seconds: seconds.clamp(15, voiceclone::writer::MAX_SECONDS),
            tone: voiceclone::writer::Tone::parse(&tone),
            voices,
        };
        voiceclone::writer::generate(&data, &req, &WRITE_CANCEL, &|part, parts, text| {
            let _ = handle.emit("write-progress", json!({"part": part, "parts": parts, "text": text}));
        })
    })
    .await
    .map_err(|x| x.to_string())?
}

/// Nombre de mots ecrits pour une duree et un ton (affiche dans la fenetre d'ecriture).
#[tauri::command]
fn write_estimate(seconds: u32, tone: String) -> Value {
    let seconds = seconds.clamp(15, voiceclone::writer::MAX_SECONDS);
    json!({"seconds": seconds, "words": voiceclone::writer::words_for(seconds, voiceclone::writer::Tone::parse(&tone))})
}

#[tauri::command]
fn write_cancel() {
    WRITE_CANCEL.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
fn transcribe_status(app: Core<'_>) -> Value {
    voiceclone::transcribe::status(&app.data_dir())
}

/// Installe Whisper (programme + modele) ; progression par l'evenement "transcribe-progress" {stage, done, total}.
#[tauri::command]
async fn transcribe_install(app: Core<'_>, handle: tauri::AppHandle) -> R<()> {
    let data = app.data_dir();
    TRANSCRIBE_CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        voiceclone::transcribe::install(&data, &TRANSCRIBE_CANCEL, &|stage, done, total| {
            let _ = handle.emit("transcribe-progress", json!({"stage": stage, "done": done, "total": total}));
        })
    })
    .await
    .map_err(|x| x.to_string())?
}

#[tauri::command]
fn transcribe_cancel() {
    TRANSCRIBE_CANCEL.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
fn transcribe_remove(app: Core<'_>) -> R<()> {
    voiceclone::transcribe::remove(&app.data_dir())
}

/// Transcrit un echantillon et enregistre le texte ; retourne ce texte.
#[tauri::command]
async fn transcribe_sample(app: Core<'_>, id: i64) -> R<String> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.transcribe_sample(id)).await.map_err(|x| x.to_string())?
}

/// Analyse de qualite des echantillons d'une voix (assistant de qualite).
#[tauri::command]
fn voice_quality(app: Core<'_>, profile_id: i64) -> R<Value> {
    app.voice_quality(profile_id)
}

/// Dossier Voix du programme et ce qu'il contient (une voix par sous-dossier).
#[tauri::command]
fn voices_info(app: Core<'_>) -> Value {
    let dir = app.voices_dir();
    let found = std::fs::read_dir(&dir).map(|d| d.flatten().filter(|f| f.path().join("voice.json").is_file()).count()).unwrap_or(0);
    json!({"dir": dir.display().to_string(), "found": found})
}

#[tauri::command]
fn export_voice(app: Core<'_>, name: String) -> R<String> {
    Ok(app.export_voice_to_folder(&name)?.display().to_string())
}

#[tauri::command]
fn export_all_voices(app: Core<'_>) -> R<Vec<String>> {
    app.export_all_voices()
}

/// Importe les voix du dossier Voix : {imported, existing, errors}.
#[tauri::command]
fn import_voices(app: Core<'_>) -> R<Value> {
    let (imported, existing, errors) = app.import_voices_report(&app.voices_dir())?;
    Ok(json!({"imported": imported, "existing": existing, "errors": errors}))
}

#[tauri::command]
fn open_voices_dir(app: Core<'_>) -> R<()> {
    std::process::Command::new("explorer").arg(app.voices_dir()).spawn().map(|_| ()).map_err(|x| x.to_string())
}

#[tauri::command]
fn mark_heard(app: Core<'_>, id: i64) -> R<()> {
    app.mark_heard(id)
}

#[tauri::command]
fn delete_all_generations(app: Core<'_>) -> R<usize> {
    app.delete_all_generations()
}

#[tauri::command]
fn delete_generation(app: Core<'_>, id: i64) -> R<()> {
    app.delete_generation(id)
}

/// La synthese bloque quelques secondes : hors du thread de l'interface.
#[tauri::command]
async fn speak(app: Core<'_>, text: String, profile: String) -> R<Value> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.speak(&text, Some(&profile), None, None, None, false))
        .await
        .map_err(|x| x.to_string())?
}

/// Octets WAV d'une generation ou d'un echantillon, pour le lecteur de la webview.
#[tauri::command]
fn audio(app: Core<'_>, kind: String, id: i64) -> R<Response> {
    let path = app.audio_path(&kind, id)?;
    Ok(Response::new(std::fs::read(&path).map_err(|x| format!("{} : {x}", path.display()))?))
}

/// Ce qu'il faut pour connecter une IA ou un script : adresse du connecteur MCP et chemins reels des programmes
/// (la version serveur est dans le dossier de l'application ; les chemins suivent l'endroit ou elle est installee).
#[tauri::command]
fn connection_info(app: Core<'_>) -> Value {
    let exe = std::env::current_exe().ok();
    let dir = exe.as_ref().and_then(|e| e.parent()).map(std::path::Path::to_path_buf);
    let server = dir.as_ref().map(|d| d.join("voiceclone-server.exe")).filter(|p| p.is_file());
    let text = |p: Option<std::path::PathBuf>| p.map(|p| p.display().to_string());
    json!({
        "app": text(exe), "server": text(server), "port": mcp::port(), "url": format!("http://127.0.0.1:{}/mcp", mcp::port()),
        "audios": app.audios_dir().display().to_string(), "data": app.data_dir().display().to_string(),
    })
}

// ---------------------------------------------------------------- langue, version, mises a jour, composants
/// Langue enregistree (None au premier lancement : l'interface propose celle du systeme).
#[tauri::command]
fn get_lang(app: Core<'_>) -> Option<String> {
    app.saved_lang()
}

#[tauri::command]
fn set_lang(app: Core<'_>, lang: String) -> R<()> {
    app.set_lang(&lang)
}

#[tauri::command]
fn app_version() -> String {
    update::current_version()
}

/// Interroge GitHub (une requete, aucune donnee envoyee) : nouvelle version disponible ou non.
#[tauri::command]
async fn check_update() -> R<Value> {
    tauri::async_runtime::spawn_blocking(|| update::check(&update::current_version())).await.map_err(|x| x.to_string())?
}

fn open_in_browser(url: &str) -> R<()> {
    std::process::Command::new("explorer").arg(url).spawn().map(|_| ()).map_err(|x| x.to_string())
}

/// Ouvre la page d'une release du depot, et seulement elle.
/// Installe la derniere version : telecharge l'archive de la release, verifie son SHA-256, la decompresse puis
/// laisse un script remplacer les fichiers apres la fermeture de l'application, qui se relance toute seule.
/// L'archive est decidee ici (nouvelle interrogation de GitHub), jamais par l'interface.
#[tauri::command]
async fn install_update(app: Core<'_>, handle: tauri::AppHandle) -> R<()> {
    let data = app.data_dir();
    tauri::async_runtime::spawn_blocking(move || -> R<()> {
        let info = update::check(&update::current_version())?;
        if !info["newer"].as_bool().unwrap_or(false) {
            return Err(tr!("vous avez déjà la dernière version", "you already have the latest version"));
        }
        let exe = std::env::current_exe().map_err(|x| x.to_string())?;
        let install = exe.parent().ok_or_else(|| tr!("dossier du programme introuvable", "program folder not found"))?.to_path_buf();
        if !update::can_write(&install) {
            return Err(tr!("le dossier du programme n'est pas modifiable : téléchargez l'archive et remplacez-le à la main", "the program folder is not writable: download the archive and replace it by hand"));
        }
        let progress = handle.clone();
        let staged = update::stage(&info["asset"], &data, &std::sync::atomic::AtomicBool::new(false), &move |done, total| {
            let _ = progress.emit("update-progress", json!({"done": done, "total": total}));
        })?;
        update::apply(&staged, &install, &exe, std::process::id(), &data)?;
        handle.exit(0);
        Ok(())
    })
    .await
    .map_err(|x| x.to_string())?
}

/// Resultat de la derniere installation (« ok » ou « error: ... »), lu une seule fois.
#[tauri::command]
fn update_log(app: Core<'_>) -> Option<String> {
    update::take_update_log(&app.data_dir())
}

#[tauri::command]
fn open_release(url: String) -> R<()> {
    if !update::is_release_url(&url) {
        return Err(tr!("adresse refusée", "address refused"));
    }
    open_in_browser(&url)
}

/// Ouvre une page officielle de la liste fermee (pilotes NVIDIA/AMD/Intel, LunarG).
#[tauri::command]
fn open_official(key: String) -> R<()> {
    open_in_browser(components::official_page(&key).ok_or_else(|| tr!("page inconnue", "unknown page"))?)
}

#[tauri::command]
async fn components(app: Core<'_>) -> R<Value> {
    let dir = app.data_dir();
    tauri::async_runtime::spawn_blocking(move || components::status(&dir)).await.map_err(|x| x.to_string())
}

/// Installe un composant manquant depuis sa source officielle ; progression par l'evenement "component-progress".
#[tauri::command]
async fn install_component(app: Core<'_>, handle: tauri::AppHandle, id: String) -> R<String> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let name = id.clone();
        app.install_component(&id, &move |stage, done, total| {
            let _ = handle.emit("component-progress", json!({"id": name, "stage": stage, "done": done, "total": total}));
        })
        .map(str::to_string)
    })
    .await
    .map_err(|x| x.to_string())?
}

#[tauri::command]
fn cancel_component(app: Core<'_>) {
    app.cancel_component()
}

/// Forme d'onde (40 barres de 0 a 100) d'une generation ou d'un echantillon, calculee hors du thread de l'interface.
#[tauri::command]
async fn waveform(app: Core<'_>, kind: String, id: i64) -> R<Vec<u8>> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.waveform(&kind, id, 40)).await.map_err(|x| x.to_string())?
}

#[tauri::command]
fn set_enhanced(app: Core<'_>, id: i64, on: bool) -> R<()> {
    app.set_enhanced(id, on)
}

/// Demande ou enregistrer (boite « Enregistrer sous », nom propose modifiable) puis ecrit la version
/// en vigueur (amelioree ou non). `None` si l'utilisateur annule.
// shortcut: repli sur le dossier Telechargements suppose sous %USERPROFILE% au premier usage (faux s'il a
// ete deplace dans Windows) ; ensuite la boite se rouvre dans le dernier dossier choisi.
#[tauri::command]
async fn download(app: Core<'_>, id: i64, format: String) -> R<Option<String>> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let name = app.export_name(id, &format)?;
        let start = app.export_dir().or_else(|| std::env::var_os("USERPROFILE").map(|h| std::path::Path::new(&h).join("Downloads")));
        let mut dialog = rfd::FileDialog::new()
            .set_title(if format == "mp3" { tr!("Enregistrer l'audio en MP3", "Save the audio as MP3") } else { tr!("Enregistrer l'audio en WAV", "Save the audio as WAV") })
            .set_file_name(name)
            .add_filter(format.to_uppercase(), &[format.as_str()]);
        if let Some(dir) = start {
            dialog = dialog.set_directory(dir);
        }
        let Some(chosen) = dialog.save_file() else { return Ok(None) };
        let path = app.export_to(id, &format, &chosen)?;
        if let Some(dir) = path.parent() {
            app.remember_export_dir(dir)?;
        }
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|x| x.to_string())?
}

/// Sous-titres .srt de la generation : boite « Enregistrer sous » ; `None` si annulee.
#[tauri::command]
async fn download_srt(app: Core<'_>, id: i64) -> R<Option<String>> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let name = app.export_name(id, "srt")?;
        let start = app.export_dir().or_else(|| std::env::var_os("USERPROFILE").map(|h| std::path::Path::new(&h).join("Downloads")));
        let mut dialog = rfd::FileDialog::new().set_title(tr!("Enregistrer les sous-titres", "Save the subtitles")).set_file_name(name).add_filter("SRT", &["srt"]);
        if let Some(dir) = start {
            dialog = dialog.set_directory(dir);
        }
        let Some(chosen) = dialog.save_file() else { return Ok(None) };
        let path = app.export_srt_to(id, &chosen)?;
        if let Some(dir) = path.parent() {
            app.remember_export_dir(dir)?;
        }
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|x| x.to_string())?
}

/// Une piste WAV par voix et les sous-titres, dans un dossier choisi ; retourne ce dossier (`None` si annule).
#[tauri::command]
async fn export_stems(app: Core<'_>, id: i64) -> R<Option<String>> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let start = app.export_dir().or_else(|| std::env::var_os("USERPROFILE").map(|h| std::path::Path::new(&h).join("Downloads")));
        let mut dialog = rfd::FileDialog::new().set_title(tr!("Choisir le dossier des pistes", "Choose the folder for the tracks"));
        if let Some(dir) = start {
            dialog = dialog.set_directory(dir);
        }
        let Some(dir) = dialog.pick_folder() else { return Ok(None) };
        app.export_stems_to(id, &dir)?;
        app.remember_export_dir(&dir)?;
        Ok(Some(dir.display().to_string()))
    })
    .await
    .map_err(|x| x.to_string())?
}

/// Version phonetique d'un texte : {supported, text}. Seul le francais est gere.
#[tauri::command]
fn phoneticize(app: Core<'_>, text: String, language: String) -> Value {
    match app.phoneticize(&text, &language) {
        Some(t) => json!({"supported": true, "text": t}),
        None => json!({"supported": false, "text": text}),
    }
}

/// Dictionnaire de prononciation et option « nombres en toutes lettres ».
#[tauri::command]
fn pronunciations(app: Core<'_>) -> Value {
    json!({"entries": app.pronunciations(), "numbers": app.numbers_words()})
}

#[tauri::command]
fn set_pronunciations(app: Core<'_>, entries: Vec<(String, String)>, numbers: bool) -> R<()> {
    app.set_pronunciations(&entries)?;
    app.set_numbers_words(numbers)
}

/// Montre le fichier d'une generation dans l'Explorateur.
#[tauri::command]
fn reveal(app: Core<'_>, id: i64) -> R<()> {
    let path = app.audio_path("generation", id)?;
    std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn().map(|_| ()).map_err(|x| x.to_string())
}

/// Ouvre le dossier Audios du programme dans l'Explorateur.
#[tauri::command]
fn open_audios_dir(app: Core<'_>) -> R<()> {
    std::process::Command::new("explorer").arg(app.audios_dir()).spawn().map(|_| ()).map_err(|x| x.to_string())
}

// ---------------------------------------------------------------- modeles (page Parametres)
#[tauri::command]
fn models(app: Core<'_>) -> Value {
    app.models()
}

#[tauri::command]
fn set_model(app: Core<'_>, file: String) -> R<()> {
    app.set_talker(&file)
}

/// Long : hors du thread de l'interface, progression envoyee par l'evenement "model-progress".
#[tauri::command]
async fn download_model(app: Core<'_>, handle: tauri::AppHandle, file: String) -> R<()> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let name = file.clone();
        app.download_model(&file, &move |done, total| {
            let _ = handle.emit("model-progress", json!({"file": name, "done": done, "total": total}));
        })
    })
    .await
    .map_err(|x| x.to_string())?
}

#[tauri::command]
fn cancel_download(app: Core<'_>, file: String) {
    app.cancel_download(&file)
}

#[tauri::command]
fn delete_model(app: Core<'_>, file: String) -> R<()> {
    app.delete_model(&file)
}

/// Detection des GPU : peripheriques, meilleur choix, choix actuel (la premiere detection charge les pilotes).
#[tauri::command]
async fn gpu(app: Core<'_>) -> R<Value> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.gpu_status()).await.map_err(|x| x.to_string())
}

#[tauri::command]
fn set_gpu(app: Core<'_>, device: String, fa: String, clamp: String) -> R<()> {
    app.set_gpu(&device, &fa, &clamp)
}

#[tauri::command]
fn unload_model(app: Core<'_>) -> R<()> {
    app.unload()
}

#[tauri::command]
fn open_models_dir(app: Core<'_>) -> R<()> {
    std::process::Command::new("explorer").arg(app.models_dir()).spawn().map(|_| ()).map_err(|x| x.to_string())
}

fn main() {
    let core = Arc::new(data_dir().and_then(App::open).expect("ouverture des données VoiceClone"));
    let port = mcp::port();
    let url = format!("http://127.0.0.1:{port}/mcp");
    // Port deja pris (autre VoiceClone ouvert) : la fenetre marche quand meme et l'affiche.
    let info = match mcp::bind(port) {
        Ok(server) => {
            let core = core.clone();
            std::thread::spawn(move || mcp::serve(server, core, port));
            json!({"url": url})
        }
        Err(error) => json!({"url": url, "error": error}),
    };
    let hook_core = core.clone();
    tauri::Builder::default()
        .manage(core)
        .manage(McpInfo(info))
        .setup(move |t| {
            // Titre de la fenetre : « VoiceClone 0.1.5 » (version du programme en cours).
            if let Some(w) = t.get_webview_window("main") {
                let _ = w.set_title(&format!("VoiceClone {}", update::current_version()));
            }
            let handle = t.handle().clone();
            hook_core.set_on_change(Box::new(move || {
                let _ = handle.emit("changed", ());
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            mcp_info, profiles, create_profile, delete_profile, samples, add_sample, set_sample_text,
            delete_sample, generations, delete_generation, speak, audio, reveal, waveform, set_enhanced, download,
            connection_info, open_audios_dir, mark_heard, voice_quality, write_estimate, writer_status, writer_install, writer_cancel_install, writer_remove, write_text, write_cancel, download_srt, export_stems, phoneticize, pronunciations, set_pronunciations, transcribe_status, transcribe_install, transcribe_cancel, transcribe_remove, transcribe_sample, voices_info, export_voice, export_all_voices, import_voices, open_voices_dir, delete_all_generations, get_lang, set_lang, app_version, check_update, install_update, update_log, open_release, open_official, components, install_component, cancel_component,
            models, gpu, set_gpu, set_model, download_model, cancel_download, delete_model, unload_model, open_models_dir
        ])
        .run(tauri::generate_context!())
        .expect("lancement de VoiceClone");
}
