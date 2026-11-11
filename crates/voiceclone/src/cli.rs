//! Ligne de commande « speak » : analyse des arguments, langue, qualite et fichier de sortie.
//! L'execution (appel du serveur deja lance, ou generation sur place) est dans `main.rs`.
use crate::R;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub struct Speak {
    pub text: String,
    /// Nom de langue Qwen3-TTS (« french »...) ; None = langue de la voix.
    pub lang: Option<String>,
    pub enhanced: bool,
    pub voice: Option<String>,
    /// Fichier .wav, ou dossier ; None = dossier Telechargements.
    pub out: Option<String>,
    pub seed: Option<i64>,
}

pub fn usage() -> String {
    tr!(
"VoiceClone : génération rapide de texte en audio

Usage : voiceclone-server speak \"texte\" [options]

  -t, --text <texte>      texte à dire (ou en argument seul ; « - » lit l'entrée standard)
  -l, --lang <langue>     fr, en, de, es, it, pt, ja, ko, zh, ru (ou french, english...) ; défaut : langue de la voix
  -q, --quality <niveau>  standard (défaut) ou ameliore / enhanced : égaliseur de voix + volume normalisé
  -v, --voice <nom>       voix à utiliser (voir list_profiles) ; défaut : la première voix
  -o, --out <chemin>      fichier .wav, ou dossier (sans \\ final) ; défaut : le dossier Téléchargements
      --seed <n>          graine d'échantillonnage (défaut 7 : résultat reproductible)
  -h, --help              cette aide

Affiche le chemin du fichier créé. Utilise le serveur VoiceClone s'il tourne déjà (rapide), sinon charge le modèle
pour cette seule génération (une dizaine de secondes de plus).

Exemples :
  voiceclone-server speak \"Bonjour à tous\" -l fr -q ameliore -v Zen
  voiceclone-server speak -t \"Hello everyone\" -l en -o C:\\projet\\public\\voix\\01.wav",
"VoiceClone: quick text-to-audio generation

Usage: voiceclone-server speak \"text\" [options]

  -t, --text <text>       text to say (or as the only argument; \"-\" reads standard input)
  -l, --lang <language>   fr, en, de, es, it, pt, ja, ko, zh, ru (or french, english...); default: the voice's language
  -q, --quality <level>   standard (default) or enhanced: voice equalizer + normalized volume
  -v, --voice <name>      voice to use (see list_profiles); default: the first voice
  -o, --out <path>        .wav file, or folder (without a final \\); default: the Downloads folder
      --seed <n>          sampling seed (default 7: reproducible result)
  -h, --help              this help

Prints the path of the file created. Uses the VoiceClone server if it is already running (fast), otherwise loads the
model for this single generation (about ten seconds more).

Examples:
  voiceclone-server speak \"Hello everyone\" -l en -q enhanced -v Zen
  voiceclone-server speak -t \"Bonjour à tous\" -l fr -o C:\\project\\public\\voice\\01.wav")
}

/// Code ou nom de langue → nom attendu par Qwen3-TTS.
pub fn language(s: &str) -> R<String> {
    let l = s.trim().to_lowercase();
    let name = match l.as_str() {
        "fr" | "french" | "francais" | "français" => "french",
        "en" | "english" | "anglais" => "english",
        "de" | "german" | "allemand" => "german",
        "es" | "spanish" | "espagnol" => "spanish",
        "it" | "italian" | "italien" => "italian",
        "pt" | "portuguese" | "portugais" => "portuguese",
        "ja" | "japanese" | "japonais" => "japanese",
        "ko" | "korean" | "coreen" | "coréen" => "korean",
        "zh" | "chinese" | "chinois" => "chinese",
        "ru" | "russian" | "russe" => "russian",
        _ => return Err(tr!("langue inconnue : {} (fr, en, de, es, it, pt, ja, ko, zh, ru)", "unknown language: {} (fr, en, de, es, it, pt, ja, ko, zh, ru)", s)),
    };
    Ok(name.into())
}

/// Qualite → faut-il la version amelioree ?
pub fn quality(s: &str) -> R<bool> {
    match s.trim().to_lowercase().as_str() {
        "standard" | "normal" | "brut" | "raw" => Ok(false),
        "enhanced" | "ameliore" | "amélioré" | "amelioree" | "améliorée" => Ok(true),
        _ => Err(tr!("qualité inconnue : {} (standard, ameliore)", "unknown quality: {} (standard, enhanced)", s)),
    }
}

/// Analyse les arguments qui suivent « speak ». `Ok(None)` = aide demandee. `read_stdin` fournit le texte quand on passe « - ».
pub fn parse(args: &[String], read_stdin: &dyn Fn() -> R<String>) -> R<Option<Speak>> {
    let (mut text, mut lang, mut enhanced, mut voice, mut out, mut seed) = (None::<String>, None, false, None, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        // « --opt=valeur » ou « --opt valeur ».
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (a.as_str(), None),
        };
        let mut value = |name: &str| -> R<String> {
            inline.clone().or_else(|| it.next().cloned()).ok_or_else(|| tr!("valeur manquante pour {}", "missing value for {}", name))
        };
        match flag {
            "-h" | "--help" => return Ok(None),
            "-t" | "--text" => text = Some(value(flag)?),
            "-l" | "--lang" | "--language" => lang = Some(language(&value(flag)?)?),
            "-q" | "--quality" => enhanced = quality(&value(flag)?)?,
            "-v" | "--voice" => voice = Some(value(flag)?),
            "-o" | "--out" => out = Some(value(flag)?),
            "--seed" => {
                let v = value(flag)?;
                seed = Some(v.parse::<i64>().ok().filter(|n| *n >= 0).ok_or_else(|| tr!("graine invalide : {}", "invalid seed: {}", v))?);
            }
            "-" => text = Some("-".into()),
            f if f.starts_with('-') => return Err(tr!("option inconnue : {}", "unknown option: {}", f)),
            positional => {
                if text.is_some() {
                    return Err(tr!("un seul texte attendu (mettez-le entre guillemets) : « {} » en trop", "only one text expected (put it in quotes): extra \"{}\"", positional));
                }
                text = Some(positional.to_string());
            }
        }
    }
    let mut text = text.ok_or_else(|| tr!("texte manquant (voir --help)", "missing text (see --help)"))?;
    if text == "-" {
        text = read_stdin()?;
    }
    if text.trim().is_empty() {
        return Err(tr!("texte vide", "empty text"));
    }
    Ok(Some(Speak { text, lang, enhanced, voice, out, seed }))
}

/// Nom de fichier tire du debut du texte : « Bonjour-je-suis-Zen.wav ».
fn file_stem(text: &str, enhanced: bool) -> String {
    let slug: String = text.chars().take(40).map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let slug = slug.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    let base = if slug.is_empty() { if crate::i18n::is_en() { "voice".to_string() } else { "voix".to_string() } } else { slug };
    if enhanced { format!("{base}-ameliore") } else { base }
}

/// Premier nom libre dans `dir` : `nom.wav`, puis `nom-2.wav`, `nom-3.wav`... (jamais d'ecrasement silencieux).
fn free_path(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.wav"));
    if !first.exists() {
        return first;
    }
    (2u32..).map(|n| dir.join(format!("{stem}-{n}.wav"))).find(|p| !p.exists()).expect("une infinite de noms")
}

/// Fichier de sortie : `out` = fichier .wav (ecrase s'il existe : c'est le choix explicite de l'utilisateur) ou dossier ;
/// sans `out`, le dossier Telechargements. Retourne toujours un chemin absolu.
pub fn output_path(out: Option<&str>, downloads: &Path, cwd: &Path, text: &str, enhanced: bool) -> R<PathBuf> {
    let stem = file_stem(text, enhanced);
    let Some(out) = out else { return Ok(free_path(downloads, &stem)) };
    // Piege de Windows : "C:\mon dossier\" est lu comme un guillemet echappe, l'argument arrive avec un « " » en fin.
    // Ce guillemet ne peut venir que de la, et l'antislash final disait qu'il s'agissait d'un dossier.
    let (out, quoted_dir) = (out.trim_end_matches('"'), out.ends_with('"'));
    let p = if Path::new(out).is_absolute() { PathBuf::from(out) } else { cwd.join(out) };
    let is_dir = quoted_dir || p.is_dir() || out.ends_with(['\\', '/']);
    if is_dir {
        std::fs::create_dir_all(&p).map_err(|x| format!("{} : {x}", p.display()))?;
        return Ok(free_path(&p, &stem));
    }
    if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav")) {
        return Err(tr!("--out doit être un fichier .wav ou un dossier (reçu : {})", "--out must be a .wav file or a folder (got: {})", out));
    }
    Ok(p)
}

/// Dossier Telechargements de l'utilisateur.
// shortcut: %USERPROFILE%\Downloads (faux si le dossier a ete deplace dans Windows) ; passer par SHGetKnownFolderPath si besoin.
pub fn downloads_dir() -> R<PathBuf> {
    let home = std::env::var_os("USERPROFILE").ok_or_else(|| tr!("USERPROFILE non défini", "USERPROFILE is not set"))?;
    Ok(PathBuf::from(home).join("Downloads"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> R<Option<Speak>> {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>(), &|| Ok("texte venu de stdin".into()))
    }

    #[test]
    fn parses_every_form() {
        let s = p(&["Bonjour à tous", "-l", "fr", "-q", "ameliore", "-v", "Zen", "-o", "C:\\x\\a.wav", "--seed", "3"]).unwrap().unwrap();
        assert_eq!(s, Speak { text: "Bonjour à tous".into(), lang: Some("french".into()), enhanced: true, voice: Some("Zen".into()), out: Some("C:\\x\\a.wav".into()), seed: Some(3) });
        // --opt=valeur, texte via -t, qualite standard par defaut, langues par code ou par nom.
        let s = p(&["--text=Hello", "--lang=EN", "--voice=Zen 2"]).unwrap().unwrap();
        assert_eq!((s.text.as_str(), s.lang.as_deref(), s.enhanced, s.voice.as_deref()), ("Hello", Some("english"), false, Some("Zen 2")));
        assert_eq!(p(&["x", "-l", "allemand"]).unwrap().unwrap().lang.as_deref(), Some("german"));
        assert!(p(&["x", "-q", "améliorée"]).unwrap().unwrap().enhanced && !p(&["x", "-q", "Standard"]).unwrap().unwrap().enhanced);
        // « - » lit l'entree standard ; aide ; options placees avant le texte.
        assert_eq!(p(&["-"]).unwrap().unwrap().text, "texte venu de stdin");
        assert_eq!(p(&["-t", "-"]).unwrap().unwrap().text, "texte venu de stdin");
        assert_eq!(p(&["--help"]).unwrap(), None);
        assert_eq!(p(&["-l", "fr", "Bonjour"]).unwrap().unwrap().text, "Bonjour");
    }

    #[test]
    fn rejects_bad_input_with_a_clear_message() {
        for (args, expect) in [
            (vec![], "texte manquant"), (vec!["  "], "texte vide"), (vec!["a", "b"], "un seul texte"), (vec!["x", "-l", "klingon"], "langue inconnue"),
            (vec!["x", "-q", "luxe"], "qualité inconnue"), (vec!["x", "--seed", "-3"], "graine invalide"), (vec!["x", "-l"], "valeur manquante"),
            (vec!["x", "--bizarre"], "option inconnue"),
        ] {
            let e = p(&args).unwrap_err();
            assert!(e.contains(expect), "{args:?} → {e}");
        }
    }

    #[test]
    fn output_goes_to_downloads_by_default_and_never_overwrites_silently() {
        let dir = std::env::temp_dir().join(format!("voiceclone-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = Path::new("C:\\travail");
        let first = output_path(None, &dir, cwd, "Bonjour, je suis Zen !", false).unwrap();
        assert_eq!(first, dir.join("Bonjour-je-suis-Zen.wav"));
        std::fs::write(&first, b"x").unwrap();
        assert_eq!(output_path(None, &dir, cwd, "Bonjour, je suis Zen !", false).unwrap(), dir.join("Bonjour-je-suis-Zen-2.wav"));
        assert_eq!(output_path(None, &dir, cwd, "Bonjour, je suis Zen !", true).unwrap(), dir.join("Bonjour-je-suis-Zen-ameliore.wav"));
        assert_eq!(output_path(None, &dir, cwd, "???", false).unwrap(), dir.join("voix.wav"));
        // Dossier existant ou termine par un separateur : nom automatique dedans (cree si besoin).
        let sub = dir.join("sortie");
        let made = output_path(Some(&format!("{}\\", sub.display())), &dir, cwd, "Salut", false).unwrap();
        assert_eq!(made, sub.join("Salut.wav"));
        assert!(sub.is_dir());
        // « "dossier\" » : l'antislash final echappe le guillemet de fermeture, l'argument se termine par un « " ».
        let trap = format!("{}\"", sub.join("avec espace").display());
        assert_eq!(output_path(Some(&trap), &dir, cwd, "Salut", false).unwrap(), sub.join("avec espace").join("Salut.wav"));
        // Fichier explicite : tel quel, relatif resolu depuis le dossier courant ; autre extension refusee.
        assert_eq!(output_path(Some("voix\\01.wav"), &dir, cwd, "x", false).unwrap(), cwd.join("voix\\01.wav"));
        assert_eq!(output_path(Some(first.to_str().unwrap()), &dir, cwd, "x", false).unwrap(), first, "fichier explicite : ecrase, c'est voulu");
        assert!(output_path(Some("a.mp3"), &dir, cwd, "x", false).is_err() && output_path(Some("a"), &dir, cwd, "x", false).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
