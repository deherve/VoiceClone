//! Texte a balises : `[pause]`, `[pause 2.5s]` et `[NomDeVoix]` (change de voix jusqu'a la balise suivante).
//!
//! Le modele de base (clonage) ne comprend aucune balise : VoiceClone les retire du texte et en tire le plan de
//! generation (voix, morceaux de texte, silences). Un crochet qui n'est ni une pause ni une voix connue est une erreur
//! franche (jamais lu a voix haute par surprise).
use crate::R;

pub const DEFAULT_PAUSE: f32 = 1.0;
pub const MAX_PAUSE: f32 = 30.0;

#[derive(Debug, PartialEq, Clone)]
pub enum Item {
    /// Silence en secondes.
    Pause(f32),
    /// Nom de voix a utiliser pour la suite (a resoudre par l'appelant).
    Voice(String),
    Text(String),
}

/// `pause`, `pause 2`, `pause 2.5s`, `pause 2,5 s` → secondes ; `None` si ce n'est pas une balise de pause.
fn pause_secs(tag: &str) -> Option<R<f32>> {
    let lower = tag.to_lowercase();
    let rest = lower.strip_prefix("pause")?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None; // « [pauses] » est un nom de voix
    }
    let num = rest.trim().trim_end_matches(['s', 'S']).trim().replace(',', ".");
    if num.is_empty() {
        return Some(Ok(DEFAULT_PAUSE));
    }
    Some(match num.parse::<f32>() {
        Ok(s) if s.is_finite() && s > 0.0 && s <= MAX_PAUSE => Ok(s),
        _ => Err(tr!("pause invalide : [{}] (de 0,1 à 30 secondes)", "invalid pause: [{}] (0.1 to 30 seconds)", tag)),
    })
}

/// `pause`, `pause 2s`, `Pause 0,5 s`... : balise de pause valide ?
pub fn is_pause_tag(tag: &str) -> bool {
    matches!(pause_secs(tag.trim()), Some(Ok(_)))
}

pub fn parse(text: &str) -> R<Vec<Item>> {
    let mut items = vec![];
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(len) = rest[open..].find(']') else { break };
        let tag = rest[open + 1..open + len].trim();
        let before = rest[..open].trim();
        if !before.is_empty() {
            items.push(Item::Text(before.to_string()));
        }
        if tag.is_empty() || tag.chars().count() > 60 || tag.contains(['\n', '[']) {
            return Err(tr!("balise invalide : [{}]", "invalid tag: [{}]", tag));
        }
        items.push(match pause_secs(tag) {
            Some(p) => Item::Pause(p?),
            None => Item::Voice(tag.to_string()),
        });
        rest = &rest[open + len + 1..];
    }
    if !rest.trim().is_empty() {
        items.push(Item::Text(rest.trim().to_string()));
    }
    Ok(items)
}

/// Le texte sans ses balises (pour le nom du fichier et l'historique lisible).
pub fn strip(text: &str) -> String {
    parse(text).map_or_else(
        |_| text.to_string(),
        |items| items.iter().filter_map(|i| if let Item::Text(t) = i { Some(t.as_str()) } else { None }).collect::<Vec<_>>().join(" "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_parsed_in_order() {
        let items = parse("Bonjour. [pause] [Zen 2] Salut ! [pause 2,5 s]Fin [PAUSE 0.5]").unwrap();
        assert_eq!(
            items,
            [Item::Text("Bonjour.".into()), Item::Pause(1.0), Item::Voice("Zen 2".into()), Item::Text("Salut !".into()),
             Item::Pause(2.5), Item::Text("Fin".into()), Item::Pause(0.5)]
        );
        // La casse ne compte pas pour « pause ».
        assert_eq!(parse("[Pause 3s]").unwrap(), [Item::Pause(3.0)]);
        assert_eq!(parse("sans balise").unwrap(), [Item::Text("sans balise".into())]);
        assert_eq!(parse("[pauses]").unwrap(), [Item::Voice("pauses".into())]);
        assert!(parse("  ").unwrap().is_empty());
    }

    #[test]
    fn bad_tags_are_refused() {
        for bad in ["[pause 0]", "[pause 31]", "[pause abc]", "[pause -2]", "[]", "[ ]", "[a\nb]", "[pause NaN]"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        // Crochet jamais ferme : traite comme du texte.
        assert_eq!(parse("a [b").unwrap(), [Item::Text("a [b".into())]);
    }

    #[test]
    fn strip_keeps_only_the_spoken_text() {
        assert_eq!(strip("Salut [pause] [Zen 2] ça va ?"), "Salut ça va ?");
        assert_eq!(strip("[pause 99] x"), "[pause 99] x");
    }
}
