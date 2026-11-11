//! Langue des messages du coeur. Fichier de test a part : la langue est globale au processus, un test qui la change
//! ne doit pas partager son processus avec ceux qui comparent des messages francais.
use voiceclone::{i18n, mcp, App};

#[test]
fn messages_follow_the_saved_language() {
    let dir = std::env::temp_dir().join(format!("voiceclone-i18n-{}", std::process::id()));
    let app = App::open(dir.clone()).unwrap();
    assert_eq!(i18n::lang(), "fr");
    assert_eq!(app.saved_lang(), None, "pas de langue choisie : l'interface propose celle du systeme");
    assert_eq!(app.create_profile("  ", "french").unwrap_err(), "nom de voix vide");

    app.set_lang("en").unwrap();
    assert_eq!(app.create_profile("  ", "french").unwrap_err(), "empty voice name");
    assert_eq!(app.add_sample(1, &[0.0; 10]).unwrap_err(), "sample too short (under one second)");
    assert!(app.set_lang("de").is_err() && i18n::lang() == "en", "langue inconnue refusee, rien ne change");
    let label = app.models()["models"][0]["label"].as_str().unwrap().to_string();
    assert!(label.contains("recommended"), "{label}");
    // Erreur renvoyee par un outil MCP : meme langue.
    let r = mcp::handle(&app, &serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "speak", "arguments": {}}})).unwrap();
    assert_eq!(r["result"]["content"][0]["text"], "parameter \"text\" is required");

    // La langue est conservee : la version serveur (autre processus) la retrouve a l'ouverture.
    i18n::set_lang("fr");
    drop(app);
    let again = App::open(dir.clone()).unwrap();
    assert_eq!((i18n::lang(), again.saved_lang().as_deref()), ("en", Some("en")));
    again.set_lang("fr").unwrap();
    assert_eq!(again.models()["models"][0]["label"].as_str().unwrap(), "Qwen3-TTS 1.7B · Q8 (recommandé)");
    drop(again);
    let _ = std::fs::remove_dir_all(dir);
}
