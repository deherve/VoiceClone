//! Assistant de qualite des voix : mesures simples sur chaque echantillon (duree, niveau, saturation, bruit, hauteur
//! de voix) et problemes qui en decoulent. Rien ici ne modifie un echantillon : on mesure, on explique, on conseille.
use crate::audio::{transcript_plausible, SR};

#[derive(Debug, Clone)]
pub struct Metrics {
    pub secs: f64,
    /// Crete maximale (0..1).
    pub peak: f32,
    /// Part des echantillons satures (|x| >= 0,99), en %.
    pub clipped_pct: f32,
    /// Ecart de niveau entre les passages forts et les passages faibles (dB) : petit = bruit de fond continu.
    pub snr_db: f32,
    /// Hauteur mediane de la voix en Hz (`None` si aucune voyelle claire n'a ete trouvee).
    pub f0: Option<f32>,
}

pub fn measure(pcm: &[f32]) -> Metrics {
    let sr = SR as f32;
    let peak = pcm.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let clipped = pcm.iter().filter(|x| x.abs() >= 0.99).count() as f32;
    // Niveau par fenetres de 20 ms : le 10e centile est le fond, le 90e la parole.
    let hop = (sr * 0.02) as usize;
    let mut levels: Vec<f32> = pcm.chunks_exact(hop).map(|w| (w.iter().map(|x| x * x).sum::<f32>() / hop as f32).sqrt()).collect();
    levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |p: f32| levels.get(((levels.len() as f32 - 1.0) * p) as usize).copied().unwrap_or(0.0);
    let snr_db = 20.0 * (pct(0.9).max(1e-6) / pct(0.1).max(1e-6)).log10();
    Metrics { secs: pcm.len() as f64 / SR as f64, peak, clipped_pct: 100.0 * clipped / pcm.len().max(1) as f32, snr_db, f0: median_f0(pcm) }
}

/// Hauteur par autocorrelation sur des trames de 40 ms (pas de 50 ms), de 70 a 400 Hz ; mediane des trames voisees.
fn median_f0(pcm: &[f32]) -> Option<f32> {
    let sr = SR as usize;
    let (frame, step) = (sr * 40 / 1000, sr * 50 / 1000);
    let (lo, hi) = (sr / 400, sr / 70);
    let mut found = vec![];
    let mut i = 0;
    while i + frame <= pcm.len() {
        let x = &pcm[i..i + frame];
        i += step;
        let mean = x.iter().sum::<f32>() / frame as f32;
        let x: Vec<f32> = x.iter().map(|v| v - mean).collect();
        let energy: f32 = x.iter().map(|v| v * v).sum();
        if (energy / frame as f32).sqrt() < 0.02 {
            continue; // silence
        }
        let best = (lo..hi.min(frame - 1)).map(|k| (k, x[..frame - k].iter().zip(&x[k..]).map(|(a, b)| a * b).sum::<f32>())).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        if let Some((k, ac)) = best {
            if ac > 0.3 * energy {
                found.push(sr as f32 / k as f32);
            }
        }
    }
    if found.is_empty() {
        return None;
    }
    found.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(found[found.len() / 2])
}

/// Problemes d'un echantillon (codes stables, traduits par l'interface). `text` : sa transcription.
pub fn sample_issues(m: &Metrics, text: &str) -> Vec<&'static str> {
    let mut v = vec![];
    if m.secs < 3.0 {
        v.push("short");
    }
    if m.clipped_pct > 0.05 {
        v.push("clipping");
    }
    if m.peak < 0.1 {
        v.push("quiet");
    }
    if m.snr_db < 20.0 {
        v.push("noisy");
    }
    if text.trim().is_empty() {
        v.push("no_text");
    } else if !transcript_plausible(text, m.secs) {
        v.push("mismatch");
    }
    v
}

/// Problemes de la voix entiere : des echantillons de hauteurs tres differentes (plus de 5 demi-tons)
/// viennent probablement de personnes differentes, et le clonage obtenu est un melange.
pub fn voice_issues(all: &[Metrics]) -> Vec<&'static str> {
    let pitches: Vec<f32> = all.iter().filter_map(|m| m.f0).collect();
    let (lo, hi) = pitches.iter().fold((f32::MAX, 0.0f32), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    if pitches.len() > 1 && hi / lo > 1.34 { vec!["pitch_mix"] } else { vec![] }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, secs: f32, amp: f32) -> Vec<f32> {
        (0..(SR as f32 * secs) as usize).map(|i| amp * (2.0 * std::f32::consts::PI * hz * i as f32 / SR as f32).sin()).collect()
    }

    #[test]
    fn pitch_is_found() {
        let f0 = measure(&tone(120.0, 2.0, 0.3)).f0.unwrap();
        assert!((f0 - 120.0).abs() < 4.0, "{f0}");
        assert!(measure(&vec![0.0; 48000]).f0.is_none());
    }

    #[test]
    fn clipping_quiet_and_short_are_reported() {
        let loud: Vec<f32> = tone(150.0, 5.0, 3.0).iter().map(|x| x.clamp(-1.0, 1.0)).collect();
        assert!(sample_issues(&measure(&loud), "x".repeat(80).as_str()).contains(&"clipping"));
        assert!(sample_issues(&measure(&tone(150.0, 5.0, 0.02)), "").contains(&"quiet"));
        assert!(sample_issues(&measure(&tone(150.0, 1.0, 0.3)), "ok").contains(&"short"));
    }

    #[test]
    fn continuous_noise_is_reported_but_clean_speech_like_audio_is_not() {
        // Parole simulee : un ton qui s'arrete (vrai silence) → ecart de niveau enorme.
        let mut clean = tone(130.0, 1.0, 0.3);
        clean.extend(vec![0.0; SR as usize / 2]);
        clean.extend(tone(130.0, 1.0, 0.3));
        clean.extend(vec![0.0; SR as usize / 2]);
        let text = "a".repeat(60);
        assert!(sample_issues(&measure(&clean), &text).is_empty(), "{:?}", sample_issues(&measure(&clean), &text));
        // Meme voix noyee dans un bruit de fond constant.
        let noisy: Vec<f32> = clean.iter().enumerate().map(|(i, x)| x + 0.2 * (((i * 7919) % 2000) as f32 / 1000.0 - 1.0)).collect();
        assert!(sample_issues(&measure(&noisy), &text).contains(&"noisy"));
    }

    #[test]
    fn mixed_pitches_are_flagged() {
        let low = measure(&tone(90.0, 2.0, 0.3));
        let high = measure(&tone(200.0, 2.0, 0.3));
        let alike = measure(&tone(95.0, 2.0, 0.3));
        assert_eq!(voice_issues(&[low.clone(), high]), ["pitch_mix"]);
        assert!(voice_issues(&[low, alike]).is_empty());
        assert!(voice_issues(&[]).is_empty());
    }
}
