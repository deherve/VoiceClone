//! VoiceClone sans fenetre : serveur MCP local, generation rapide en ligne de commande, import des profils Voicebox.
//!   voiceclone-server                           sert le MCP sur http://127.0.0.1:17494/mcp
//!   voiceclone-server speak "texte" [options]   texte -> audio (voir `speak --help`)
//!   voiceclone-server import-voicebox <dossier> [nom]  copie les profils clones de Voicebox
use serde_json::{json, Value};
use std::{env, io::Read, path::Path, sync::Arc, time::{Duration, Instant}};
use voiceclone::{cli, data_dir, i18n, mcp, tr, App, R};

fn usage() -> String {
    tr!(
"VoiceClone (version serveur)

  voiceclone-server                        lance le connecteur MCP (http://127.0.0.1:17494/mcp)
  voiceclone-server speak \"texte\" [...]    génère un audio depuis la ligne de commande (speak --help)
  voiceclone-server export-voice <nom> [dossier]   exporte une voix (défaut : dossier Voix du programme)
  voiceclone-server import-voices [dossier]        importe les voix d'un dossier (défaut : Voix)
  voiceclone-server voice-check <nom>              analyse la qualité des échantillons d'une voix
  voiceclone-server transcribe <fichier.wav> [lg]  transcrit un enregistrement (Whisper installé)
  voiceclone-server write <dialogue|recit> \"demande\" [fr|en] [secondes] [ton] [voix]   écrit un texte avec l'IA locale
  voiceclone-server import-voicebox <dossier sh.voicebox.app> [nom]",
"VoiceClone (server version)

  voiceclone-server                        starts the MCP connector (http://127.0.0.1:17494/mcp)
  voiceclone-server speak \"text\" [...]     generates audio from the command line (speak --help)
  voiceclone-server export-voice <name> [folder]   exports a voice (default: the program's Voix folder)
  voiceclone-server import-voices [folder]         imports the voices of a folder (default: Voix)
  voiceclone-server voice-check <name>             analyses the quality of a voice's samples
  voiceclone-server transcribe <file.wav> [lang]   transcribes a recording (Whisper installed)
  voiceclone-server write <dialogue|story> \"request\" [fr|en] [seconds] [tone] [voices]   writes a text with the local AI
  voiceclone-server import-voicebox <sh.voicebox.app folder> [name]")
}

/// Genere par le serveur deja lance (rapide : le modele est en memoire). `Ok(None)` : aucun serveur n'ecoute.
fn via_server(arguments: &Value) -> R<Option<Value>> {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "speak", "arguments": arguments}});
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(2))
        .timeout(Duration::from_secs(900)) // la premiere phrase charge le modele
        .build();
    let resp = match agent.post(&format!("http://127.0.0.1:{}/mcp", mcp::port())).set("Content-Type", "application/json").send_string(&body.to_string()) {
        Ok(r) => r,
        Err(ureq::Error::Transport(t)) if t.kind() == ureq::ErrorKind::ConnectionFailed => return Ok(None),
        Err(x) => return Err(tr!("le serveur VoiceClone a répondu par une erreur : {}", "the VoiceClone server answered with an error: {}", x)),
    };
    let mut raw = String::new();
    resp.into_reader().take(1 << 20).read_to_string(&mut raw).map_err(|x| x.to_string())?;
    let reply: Value = serde_json::from_str(&raw).map_err(|_| tr!("réponse inattendue du port MCP", "unexpected reply from the MCP port"))?;
    let text = reply["result"]["content"][0]["text"].as_str().unwrap_or("").to_string();
    if reply["result"]["isError"].as_bool().unwrap_or(true) {
        return Err(if text.is_empty() { tr!("réponse inattendue du port MCP", "unexpected reply from the MCP port") } else { text });
    }
    serde_json::from_str(&text).map(Some).map_err(|_| tr!("réponse inattendue du port MCP", "unexpected reply from the MCP port"))
}

fn speak(args: &[String], dir: &Path) -> R<()> {
    let read_stdin = || {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).map_err(|x| x.to_string())?;
        Ok(s)
    };
    let Some(sp) = cli::parse(args, &read_stdin)? else {
        println!("{}", cli::usage());
        return Ok(());
    };
    let cwd = env::current_dir().map_err(|x| x.to_string())?;
    // Sans --out, pas de output_path : l'audio est enregistre dans le dossier Audios du programme et son chemin est affiche.
    let out = sp.out.as_deref().map(|o| cli::output_path(o, &cwd, &sp.text, sp.enhanced)).transpose()?;

    let mut arguments = json!({"text": sp.text, "enhanced": sp.enhanced});
    if let Some(o) = &out {
        arguments["output_path"] = json!(o);
    }
    for (key, value) in [("profile", &sp.voice), ("language", &sp.lang)] {
        if let Some(v) = value {
            arguments[key] = json!(v);
        }
    }
    if let Some(n) = sp.seed {
        arguments["seed"] = json!(n);
    }

    let started = Instant::now();
    let result = match via_server(&arguments)? {
        Some(r) => r,
        None => {
            // Pas de serveur : on charge le modele pour cette seule generation.
            eprintln!("{}", tr!("Serveur VoiceClone absent : chargement du modèle (une dizaine de secondes)…", "VoiceClone server not running: loading the model (about ten seconds)…"));
            voiceclone::engine::silence_logs();
            App::open(dir.to_path_buf())?.speak(&sp.text, sp.voice.as_deref(), sp.lang.as_deref(), sp.seed, out.as_deref(), sp.enhanced)?
        }
    };
    // Sortie standard : le chemin seul (facile a recuperer dans un script) ; le detail va sur la sortie d'erreur.
    println!("{}", result["path"].as_str().unwrap_or_default());
    for w in result["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        eprintln!("{}", tr!("attention : {}", "warning: {}", w));
    }
    eprintln!(
        "{}",
        tr!("{} s d'audio en {} s (voix {}, qualité {})", "{} s of audio in {} s (voice {}, quality {})",
            result["duration_sec"], started.elapsed().as_secs_f32().round(), result["profile"].as_str().unwrap_or("?"),
            if sp.enhanced { tr!("améliorée", "enhanced") } else { "standard".to_string() })
    );
    Ok(())
}

fn run() -> R<()> {
    let args: Vec<String> = env::args().collect();
    let dir = data_dir()?;
    i18n::load_saved(&dir); // messages dans la langue choisie dans Parametres
    match args.get(1).map(String::as_str) {
        Some("speak") => speak(&args[2..], &dir),
        Some("-h" | "--help" | "help") => {
            println!("{}", usage());
            Ok(())
        }
        Some("export-voice") => {
            let Some(name) = args.get(2) else {
                return Err(tr!("usage : voiceclone-server export-voice <nom> [dossier]", "usage: voiceclone-server export-voice <name> [folder]"));
            };
            let app = App::open(dir)?;
            let target = match args.get(3) {
                Some(folder) => {
                    app.export_voice(name, Path::new(folder))?;
                    Path::new(folder).to_path_buf()
                }
                None => app.export_voice_to_folder(name)?, // dossier Voix du programme
            };
            println!("{}", tr!("voix « {} » exportée dans {}", "voice \"{}\" exported to {}", name, target.display()));
            Ok(())
        }
        Some("import-voices") => {
            let app = App::open(dir)?;
            let src = args.get(2).map(std::path::PathBuf::from).unwrap_or_else(|| app.voices_dir());
            let (done, existing, errors) = app.import_voices_report(&src)?;
            println!("{}", tr!("voix importées : {} ({} déjà présentes)", "voices imported: {} ({} already present)", if done.is_empty() { "-".to_string() } else { done.join(", ") }, existing));
            for x in errors {
                eprintln!("{}", tr!("ignorée : {}", "skipped: {}", x));
            }
            Ok(())
        }
        Some("write") => {
            // voiceclone-server write <dialogue|recit> "<demande>" [fr|en] [secondes] [ton] [voix1,voix2,...]
            let (mode, prompt) = (args.get(2).map(String::as_str), args.get(3));
            let (Some(mode), Some(prompt)) = (mode, prompt) else {
                return Err(tr!("usage : voiceclone-server write <dialogue|recit> \"demande\" [fr|en] [secondes] [ton] [voix1,voix2]", "usage: voiceclone-server write <dialogue|story> \"request\" [fr|en] [seconds] [tone] [voice1,voice2]"));
            };
            let app = App::open(dir.clone())?;
            let voices: Vec<String> = match args.get(7) {
                Some(list) => list.split(',').map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect(),
                None => app.list_profiles()?.as_array().into_iter().flatten().filter_map(|p| p["name"].as_str().map(String::from)).collect(),
            };
            let req = voiceclone::writer::Request {
                prompt: prompt.clone(),
                mode: if mode.starts_with('d') { voiceclone::writer::Mode::Dialogue } else { voiceclone::writer::Mode::Story },
                language: args.get(4).cloned().unwrap_or_else(|| "fr".into()),
                seconds: args.get(5).and_then(|w| w.parse().ok()).unwrap_or(60),
                tone: voiceclone::writer::Tone::parse(args.get(6).map(String::as_str).unwrap_or("natural")),
                voices,
            };
            println!("{}", voiceclone::writer::generate(&dir, &req, &std::sync::atomic::AtomicBool::new(false), &|part, parts, done| if parts > 1 && done.is_none() { eprintln!("{}", if part == 0 { tr!("plan du texte…", "planning the text…") } else { tr!("partie {} sur {}…", "part {} of {}…", part, parts) }) })?);
            Ok(())
        }
        Some("transcribe") => {
            let Some(file) = args.get(2) else {
                return Err(tr!("usage : voiceclone-server transcribe <fichier.wav> [langue]", "usage: voiceclone-server transcribe <file.wav> [language]"));
            };
            let lang = args.get(3).map(String::as_str).unwrap_or("auto");
            println!("{}", voiceclone::transcribe::transcribe_file(&dir, Path::new(file), lang)?);
            Ok(())
        }
        Some("voice-check") => {
            let Some(name) = args.get(2) else {
                return Err(tr!("usage : voiceclone-server voice-check <nom>", "usage: voiceclone-server voice-check <name>"));
            };
            let app = App::open(dir)?;
            let id = app.list_profiles()?.as_array().into_iter().flatten().find(|p| p["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name)))
                .and_then(|p| p["id"].as_i64()).ok_or_else(|| tr!("voix introuvable : {}", "voice not found: {}", name))?;
            println!("{}", serde_json::to_string_pretty(&app.voice_quality(id)?).map_err(|x| x.to_string())?);
            Ok(())
        }
        Some("import-voicebox") => {
            let app = App::open(dir)?;
            let src = args.get(2).ok_or_else(|| tr!("usage : voiceclone import-voicebox <dossier sh.voicebox.app> [nom]", "usage: voiceclone import-voicebox <sh.voicebox.app folder> [name]"))?;
            let names = app.import_voicebox(Path::new(src), args.get(3).map(String::as_str))?;
            println!("{}", tr!("profils importés : {}", "voices imported: {}", names.join(", ")));
            Ok(())
        }
        None | Some("serve") => {
            let app = Arc::new(App::open(dir.clone())?);
            let port = mcp::port();
            let server = mcp::bind(port)?;
            println!("{}", tr!("VoiceClone MCP : http://127.0.0.1:{}/mcp  (données : {})", "VoiceClone MCP: http://127.0.0.1:{}/mcp  (data: {})", port, dir.display()));
            mcp::serve(server, app, port);
            Ok(())
        }
        Some(x) => Err(tr!("commande inconnue : {}", "unknown command: {}", x)),
    }
}

fn main() {
    if let Err(x) = run() {
        eprintln!("{}", tr!("erreur : {}", "error: {}", x));
        std::process::exit(1);
    }
}
