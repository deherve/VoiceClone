//! VoiceClone sans fenetre : serveur MCP local et import des profils Voicebox.
//!   voiceclone                                  sert le MCP sur http://127.0.0.1:17494/mcp
//!   voiceclone import-voicebox <dossier> [nom]  copie les profils clones de Voicebox
use std::{env, path::Path, sync::Arc};
use voiceclone::{data_dir, mcp, App, R};

fn run() -> R<()> {
    let args: Vec<String> = env::args().collect();
    let dir = data_dir()?;
    let app = Arc::new(App::open(dir.clone())?);
    match args.get(1).map(String::as_str) {
        Some("import-voicebox") => {
            let src = args.get(2).ok_or("usage : voiceclone import-voicebox <dossier sh.voicebox.app> [nom]")?;
            let names = app.import_voicebox(Path::new(src), args.get(3).map(String::as_str))?;
            println!("profils importés : {}", names.join(", "));
            Ok(())
        }
        None | Some("serve") => {
            let port = mcp::port();
            let server = mcp::bind(port)?;
            println!("VoiceClone MCP : http://127.0.0.1:{port}/mcp  (données : {})", dir.display());
            mcp::serve(server, app, port);
            Ok(())
        }
        Some(x) => Err(format!("commande inconnue : {x}")),
    }
}

fn main() {
    if let Err(x) = run() {
        eprintln!("erreur : {x}");
        std::process::exit(1);
    }
}
