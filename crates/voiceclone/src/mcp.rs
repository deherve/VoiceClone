//! Serveur MCP minimal (Streamable HTTP, reponses JSON) sur 127.0.0.1.
// shortcut: JSON-RPC ecrit a la main (initialize, ping, tools/list, tools/call), sans flux SSE ni session ;
// passer au SDK rmcp si on a besoin de notifications de progression ou de ressources.
use crate::{e, App, R};
use serde_json::{json, Value};
use std::{io::Read, path::Path, sync::Arc, thread};
use tiny_http::{Header, Method, Request, Response, Server};

fn tools() -> Value {
    json!([
        {"name": "speak",
         "description": "Génère un fichier WAV (24 kHz mono) du texte avec une voix clonée. Bloquant : retourne le chemin du fichier, sa durée et le temps de calcul. Environ 1 s par phrase une fois le modèle chargé.",
         "inputSchema": {"type": "object", "required": ["text"], "properties": {
            "text": {"type": "string", "description": "Texte à dire. Les textes longs sont découpés par phrases."},
            "profile": {"type": "string", "description": "Nom du profil de voix (voir list_profiles). Par défaut : le premier profil."},
            "output_path": {"type": "string", "description": "Chemin absolu .wav où copier le résultat (ex. dossier public d'un projet Remotion)."},
            "language": {"type": "string", "description": "Nom de langue Qwen3-TTS (french, english...). Par défaut : celle du profil."},
            "enhanced": {"type": "boolean", "description": "true : version améliorée (égaliseur de voix + volume normalisé à -18 dBFS). Le fichier écrit dans output_path est alors cette version ; l'original n'est pas conservé hors de l'application. Par défaut false."},
            "seed": {"type": "integer", "description": "Graine d'échantillonnage. Par défaut 7 (résultat reproductible)."}}}},
        {"name": "list_profiles", "description": "Liste les profils de voix clonées.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "list_generations", "description": "Liste les dernières générations (texte, chemin du WAV, durée), la plus récente en premier.",
         "inputSchema": {"type": "object", "properties": {"limit": {"type": "integer", "description": "Nombre maximum (défaut 20)."}}}}
    ])
}

fn call(app: &App, p: &Value) -> Value {
    let a = &p["arguments"];
    let r = match p["name"].as_str().unwrap_or("") {
        "speak" => match a["text"].as_str() {
            Some(t) => app.speak(t, a["profile"].as_str(), a["language"].as_str(), a["seed"].as_i64(), a["output_path"].as_str().map(Path::new), a["enhanced"].as_bool().unwrap_or(false)),
            None => Err("paramètre « text » requis".into()),
        },
        "list_profiles" => app.list_profiles(),
        "list_generations" => app.list_generations(a["limit"].as_i64().unwrap_or(20)),
        n => Err(format!("outil inconnu : {n}")),
    };
    let (text, is_error) = match r {
        Ok(v) => (v.to_string(), false),
        Err(x) => (x, true),
    };
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

/// Traite un message JSON-RPC. `None` pour une notification (pas de reponse).
pub fn handle(app: &App, req: &Value) -> Option<Value> {
    let id = req.get("id")?.clone();
    let result = match req["method"].as_str().unwrap_or("") {
        "initialize" => Ok(json!({
            "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2025-03-26"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "voiceclone", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "VoiceClone génère de l'audio avec une voix clonée, en local. Utiliser speak (avec output_path pour écrire dans un projet) et list_profiles."})),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => Ok(call(app, &req["params"])),
        _ => Err(json!({"code": -32601, "message": "Method not found"})),
    };
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err(x) => json!({"jsonrpc": "2.0", "id": id, "error": x}),
    })
}

fn respond(app: &App, mut rq: Request, port: u16) {
    let header = |n: &str| rq.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(n)).map(|h| h.value.as_str().to_string());
    // Un navigateur envoie toujours Origin sur un POST inter-sites, et Host trahit un DNS rebinding :
    // on refuse les deux pour qu'une page web ne puisse pas piloter le serveur local.
    let host_ok = header("Host").is_some_and(|h| h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}"));
    let from_browser = header("Origin").is_some();
    let (status, body) = if from_browser || !host_ok {
        (403, String::new())
    } else if rq.url().trim_end_matches('/') != "/mcp" {
        (404, String::new())
    } else if *rq.method() != Method::Post {
        (405, String::new())
    } else {
        let mut raw = String::new();
        let parsed = rq.as_reader().take(1 << 20).read_to_string(&mut raw).ok().and_then(|_| serde_json::from_str::<Value>(&raw).ok());
        match parsed {
            Some(v) => handle(app, &v).map_or((202, String::new()), |r| (200, r.to_string())),
            None => (400, json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}}).to_string()),
        }
    };
    let json_type = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    let _ = rq.respond(Response::from_string(body).with_status_code(status).with_header(json_type));
}

/// Port du serveur : `VOICECLONE_PORT`, sinon 17494.
pub fn port() -> u16 {
    std::env::var("VOICECLONE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(17494)
}

/// Ouvre le port local (echoue tout de suite s'il est deja pris).
pub fn bind(port: u16) -> R<Server> {
    Server::http(("127.0.0.1", port)).map_err(|x| format!("port {port} indisponible : {}", e(x)))
}

/// Sert le MCP sur http://127.0.0.1:<port>/mcp jusqu'a l'arret du processus.
pub fn serve(server: Server, app: Arc<App>, port: u16) {
    let server = Arc::new(server);
    // Quelques threads pour que list_* reponde pendant une generation.
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let (server, app) = (server.clone(), app.clone());
            thread::spawn(move || {
                for rq in server.incoming_requests() {
                    respond(&app, rq, port);
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol() {
        let dir = std::env::temp_dir().join(format!("voiceclone-test-{}", std::process::id()));
        let app = App::open(dir.clone()).unwrap();
        let r = handle(&app, &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}})).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "voiceclone");
        assert!(handle(&app, &json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        let r = handle(&app, &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).unwrap();
        assert_eq!(r["result"]["tools"].as_array().unwrap().len(), 3);
        let r = handle(&app, &json!({"jsonrpc": "2.0", "id": 3, "method": "nope"})).unwrap();
        assert_eq!(r["error"]["code"], -32601);
        // Base vide : list_profiles repond [], speak echoue proprement sans charger le modele.
        let r = handle(&app, &json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "list_profiles", "arguments": {}}})).unwrap();
        assert_eq!(r["result"]["content"][0]["text"], "[]");
        let r = handle(&app, &json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "speak", "arguments": {"text": "Bonjour", "output_path": "relatif.mp3"}}})).unwrap();
        assert_eq!(r["result"]["isError"], true);
        // Cycle de vie d'une voix sans moteur : creation, echantillon, transcription, suppression.
        let id = app.create_profile(" Test ", "french").unwrap();
        assert!(app.create_profile("test", "french").is_err(), "nom en double refusé");
        assert!(app.add_sample(id, &[0.0; 100]).is_err(), "échantillon trop court refusé");
        let sid = app.add_sample(id, &vec![0.1; 36000]).unwrap();
        app.set_sample_text(sid, " Bonjour ").unwrap();
        let s = app.list_samples(id).unwrap();
        assert_eq!((s[0]["ref_text"].as_str(), s[0]["duration_sec"].as_f64()), (Some("Bonjour"), Some(1.5)));
        let wav = app.audio_path("sample", sid).unwrap();
        assert!(wav.exists());
        app.delete_sample(sid).unwrap();
        assert!(!wav.exists() && app.delete_sample(sid).is_err());
        assert!(app.speak("Bonjour", Some("test"), None, None, None, true).unwrap_err().contains("aucun échantillon"));
        app.delete_profile(id).unwrap();
        // Modeles : rien d'installe dans ce dossier de test, choix et noms hors catalogue refuses.
        let m = app.models();
        assert_eq!(m["models"].as_array().unwrap().len(), crate::models::CATALOG.len());
        assert!(m["models"][0]["active"] == true && m["models"][0]["downloaded"] == false && m["loaded"] == false);
        assert!(app.set_talker(crate::models::DEFAULT_TALKER).is_err(), "modèle non téléchargé refusé");
        assert!(app.set_talker("..\\x.gguf").is_err() && app.delete_model("..\\voiceclone.db").is_err());
        assert!(app.download_model("..\\x", &|_, _| {}).is_err());
        app.unload().unwrap();
        assert_eq!(app.list_profiles().unwrap(), json!([]));
        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }
}
