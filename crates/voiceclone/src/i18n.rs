//! Langue des messages produits par le coeur (erreurs, libelles, boites de dialogue) : « fr » (defaut) ou « en ».
//! L'interface a son propre dictionnaire (ui/i18n.js) ; ce module couvre ce qui vient de Rust.
use std::sync::atomic::{AtomicBool, Ordering};

static EN: AtomicBool = AtomicBool::new(false);

/// Langues proposees, dans l'ordre de la page Parametres.
pub const LANGS: [&str; 2] = ["fr", "en"];

pub fn is_en() -> bool {
    EN.load(Ordering::Relaxed)
}

pub fn lang() -> &'static str {
    if is_en() { "en" } else { "fr" }
}

/// Fixe la langue des messages ; `false` (et rien ne change) si la langue n'est pas prise en charge.
pub fn set_lang(lang: &str) -> bool {
    if !LANGS.contains(&lang) {
        return false;
    }
    EN.store(lang == "en", Ordering::Relaxed);
    true
}

/// Applique la langue enregistree dans `<donnees>\voiceclone.db` sans ouvrir la base en ecriture (la ligne de commande
/// ne doit rien modifier quand le serveur tourne deja). Sans base ou sans reglage, la langue par defaut reste.
pub fn load_saved(data_dir: &std::path::Path) {
    let Ok(db) = rusqlite::Connection::open_with_flags(data_dir.join("voiceclone.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else { return };
    if let Ok(lang) = db.query_row("SELECT value FROM settings WHERE key='lang'", [], |r| r.get::<_, String>(0)) {
        set_lang(&lang);
    }
}

/// Choisit le texte selon la langue : `tr!("fr {}", "en {}", valeur)`. Les deux textes sont des litteraux
/// (les arguments sont communs), ce qui garde le francais et l'anglais cote a cote dans le code.
#[macro_export]
macro_rules! tr {
    ($fr:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if $crate::i18n::is_en() { format!($en $(, $arg)*) } else { format!($fr $(, $arg)*) }
    };
}
