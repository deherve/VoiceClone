//! Application VoiceClone : fenetre Tauri + serveur MCP local dans le meme processus.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{ipc::{InvokeBody, Request, Response}, Emitter, State};
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
            let handle = t.handle().clone();
            hook_core.set_on_change(Box::new(move || {
                let _ = handle.emit("changed", ());
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            mcp_info, profiles, create_profile, delete_profile, samples, add_sample, set_sample_text,
            delete_sample, generations, delete_generation, speak, audio, reveal, waveform, set_enhanced, download,
            connection_info, open_audios_dir, get_lang, set_lang, app_version, check_update, open_release, open_official, components, install_component, cancel_component,
            models, gpu, set_gpu, set_model, download_model, cancel_download, delete_model, unload_model, open_models_dir
        ])
        .run(tauri::generate_context!())
        .expect("lancement de VoiceClone");
}
