//! Export pour le montage : sous-titres `.srt` et une piste audio par voix, calcules depuis la chronologie
//! (qui parle, de quand a quand, quel texte) enregistree avec chaque generation.
use crate::{audio, R};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub voice: String,
    pub start: f64,
    pub end: f64,
    pub text: String,
}

pub fn to_json(cues: &[Cue]) -> String {
    Value::Array(cues.iter().map(|c| json!({"v": c.voice, "s": (c.start * 1000.0).round() / 1000.0, "e": (c.end * 1000.0).round() / 1000.0, "t": c.text})).collect()).to_string()
}

pub fn from_json(s: &str) -> R<Vec<Cue>> {
    let v: Value = serde_json::from_str(s).map_err(|_| tr!("chronologie illisible", "unreadable timeline"))?;
    v.as_array()
        .ok_or_else(|| tr!("chronologie illisible", "unreadable timeline"))?
        .iter()
        .map(|c| {
            Ok(Cue { voice: c["v"].as_str().unwrap_or("").to_string(), start: c["s"].as_f64().unwrap_or(0.0), end: c["e"].as_f64().unwrap_or(0.0), text: c["t"].as_str().unwrap_or("").to_string() })
        })
        .collect()
}

/// `00:01:02,500`
fn stamp(secs: f64) -> String {
    let ms = (secs.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

/// Lignes de 42 caracteres au plus, coupees aux espaces.
fn wrap(text: &str, width: usize) -> String {
    let (mut lines, mut cur) = (vec![], String::new());
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    lines.push(cur);
    lines.join("\n")
}

/// Un sous-titre par phrase : le temps de chaque morceau est reparti entre ses phrases selon leur longueur.
pub fn srt(cues: &[Cue]) -> String {
    let mut out = String::new();
    let mut n = 0;
    for cue in cues {
        let sentences = audio::split_text(&cue.text, 1);
        let total: usize = sentences.iter().map(|s| s.chars().count()).sum::<usize>().max(1);
        let mut t = cue.start;
        for s in &sentences {
            let d = (cue.end - cue.start) * s.chars().count() as f64 / total as f64;
            n += 1;
            out.push_str(&format!("{n}\n{} --> {}\n{}\n\n", stamp(t), stamp(t + d), wrap(s, 42)));
            t += d;
        }
    }
    out
}

/// Meme duree que `pcm` : le son de `voice` seulement, silence partout ailleurs (les pistes se superposent au montage).
pub fn stem(pcm: &[f32], cues: &[Cue], voice: &str) -> Vec<f32> {
    let mut out = vec![0.0; pcm.len()];
    for c in cues.iter().filter(|c| c.voice == voice) {
        let (a, b) = (((c.start * audio::SR as f64) as usize).min(pcm.len()), ((c.end * audio::SR as f64).ceil() as usize).min(pcm.len()));
        let b = b.max(a); // chronologie incoherente (fin avant debut) : plage vide
        out[a..b].copy_from_slice(&pcm[a..b]);
    }
    out
}

/// Voix qui parlent, dans l'ordre de leur premiere apparition.
pub fn voices(cues: &[Cue]) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    for c in cues {
        if !v.contains(&c.voice) {
            v.push(c.voice.clone());
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(v: &str, s: f64, e: f64, t: &str) -> Cue {
        Cue { voice: v.into(), start: s, end: e, text: t.into() }
    }

    #[test]
    fn srt_splits_sentences_and_shares_the_time() {
        let out = srt(&[cue("Marco", 0.0, 4.0, "Bonjour à tous. Ça va ?"), cue("Annie", 65.5, 66.0, "Oui.")]);
        let blocks: Vec<&str> = out.trim_end().split("\n\n").collect();
        assert_eq!(blocks.len(), 3);
        assert!(blocks[0].starts_with("1\n00:00:00,000 --> 00:00:02,"), "{}", blocks[0]);
        assert!(blocks[0].ends_with("Bonjour à tous."));
        assert!(blocks[1].starts_with("2\n00:00:02,") && blocks[1].contains("--> 00:00:04,000\nÇa va ?"), "{}", blocks[1]);
        assert!(blocks[2].starts_with("3\n00:01:05,500 --> 00:01:06,000\nOui."), "{}", blocks[2]);
        assert_eq!(stamp(3661.0045), "01:01:01,005");
    }

    #[test]
    fn long_sentences_are_wrapped() {
        let t = "un deux trois quatre cinq six sept huit neuf dix onze douze treize quatorze quinze";
        let w = wrap(t, 42);
        assert!(w.lines().all(|l| l.chars().count() <= 42) && w.lines().count() == 2, "{w}");
        assert_eq!(w.replace('\n', " "), t);
    }

    #[test]
    fn stems_keep_only_their_voice_and_the_same_length() {
        let sr = audio::SR as usize;
        let pcm = vec![0.5f32; sr * 4];
        let cues = [cue("A", 0.0, 1.0, "x"), cue("B", 1.5, 2.5, "y"), cue("A", 3.0, 4.0, "z")];
        let a = stem(&pcm, &cues, "A");
        assert_eq!(a.len(), pcm.len());
        assert!(a[..sr].iter().all(|&x| x == 0.5) && a[sr..sr * 3].iter().all(|&x| x == 0.0) && a[sr * 3..].iter().all(|&x| x == 0.5));
        let b = stem(&pcm, &cues, "B");
        assert_eq!(b.iter().filter(|&&x| x != 0.0).count(), sr);
        assert_eq!(voices(&cues), ["A", "B"]);
    }

    #[test]
    fn inconsistent_cues_do_not_panic() {
        let pcm = vec![0.5f32; audio::SR as usize];
        assert_eq!(stem(&pcm, &[cue("A", 0.8, 0.2, "x"), cue("A", 5.0, 9.0, "y")], "A").len(), pcm.len());
    }

    #[test]
    fn json_round_trip() {
        let cues = vec![cue("Marco", 0.0, 1.2345, "Salut \"toi\""), cue("Zen 2", 2.0, 3.5, "Ça va ?")];
        assert_eq!(from_json(&to_json(&cues)).unwrap(), vec![cue("Marco", 0.0, 1.235, "Salut \"toi\""), cue("Zen 2", 2.0, 3.5, "Ça va ?")]);
        assert!(from_json("pas du json").is_err());
    }
}
