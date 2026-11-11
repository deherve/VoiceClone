//! VoiceClone sans fenetre : serveur MCP local et import des profils Voicebox.
//!   voiceclone                                  sert le MCP sur http://127.0.0.1:17494/mcp
//!   voiceclone import-voicebox <dossier> [nom]  copie les profils clones de Voicebox
use std::{env, path::Path, sync::Arc};
use voiceclone::{data_dir, mcp, tr, App, R};

fn run() -> R<()> {
    let args: Vec<String> = env::args().collect();
    let dir = data_dir()?;
    let app = Arc::new(App::open(dir.clone())?);
    match args.get(1).map(String::as_str) {
        Some("import-voicebox") => {
            let src = args.get(2).ok_or_else(|| tr!("usage : voiceclone import-voicebox <dossier sh.voicebox.app> [nom]", "usage: voiceclone import-voicebox <sh.voicebox.app folder> [name]"))?;
            let names = app.import_voicebox(Path::new(src), args.get(3).map(String::as_str))?;
            println!("{}", tr!("profils importés : {}", "voices imported: {}", names.join(", ")));
            Ok(())
        }
        None | Some("serve") => {
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
