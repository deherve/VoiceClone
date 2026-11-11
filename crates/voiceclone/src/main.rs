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
  voiceclone-server export-voice <nom> <dossier>   exporte une voix (échantillons + transcriptions)
  voiceclone-server import-voicebox <dossier sh.voicebox.app> [nom]",
"VoiceClone (server version)

  voiceclone-server                        starts the MCP connector (http://127.0.0.1:17494/mcp)
  voiceclone-server speak \"text\" [...]     generates audio from the command line (speak --help)
  voiceclone-server export-voice <name> <folder>   exports a voice (samples + transcripts)
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
            let (name, folder) = (args.get(2), args.get(3));
            let (Some(name), Some(folder)) = (name, folder) else {
                return Err(tr!("usage : voiceclone-server export-voice <nom> <dossier>", "usage: voiceclone-server export-voice <name> <folder>"));
            };
            let n = App::open(dir)?.export_voice(name, Path::new(folder))?;
            println!("{}", tr!("voix « {} » exportée : {} échantillon(s) dans {}", "voice \"{}\" exported: {} sample(s) to {}", name, n, folder));
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
