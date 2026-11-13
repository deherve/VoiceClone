//! WAV, filtre anti-gresillement et decoupage du texte.
use crate::{e, R};
use std::path::Path;

pub const SR: u32 = 24000;

pub fn read_wav_24k(path: &Path) -> R<Vec<f32>> {
    let mut r = hound::WavReader::open(path).map_err(|x| format!("{} : {x}", path.display()))?;
    let s = r.spec();
    // shortcut: 24 kHz mono exige, conversion (reechantillonnage, stereo) a ajouter avec l'import/micro (etape 3).
    if s.sample_rate != SR || s.channels != 1 {
        return Err(tr!("{} : 24 kHz mono requis (reçu {} Hz, {} canaux)", "{}: 24 kHz mono required (got {} Hz, {} channels)", path.display(), s.sample_rate, s.channels));
    }
    match s.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>().map_err(e),
        hound::SampleFormat::Int => {
            let k = (1i64 << (s.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|x| x.map(|v| v as f32 / k)).collect::<Result<_, _>>().map_err(e)
        }
    }
}

pub fn write_wav(path: &Path, pcm: &[f32]) -> R<()> {
    let spec = hound::WavSpec { channels: 1, sample_rate: SR, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).map_err(|x| format!("{} : {x}", path.display()))?;
    for &x in pcm {
        w.write_sample((x.clamp(-1.0, 1.0) * 32767.0) as i16).map_err(e)?;
    }
    w.finalize().map_err(e)
}

/// Passe-haut 60 Hz + passe-bas 8 kHz : supprime le souffle aigu du moteur (regle valide a l'ecoute).
pub fn clean(pcm: &mut [f32]) {
    biquad(pcm, 60.0, true);
    biquad(pcm, 8000.0, false);
}

// Filtre biquad (formules RBJ) : coefficients [b0, b1, b2, a0, a1, a2] non normalises.
fn run_biquad(pcm: &mut [f32], [b0, b1, b2, a0, a1, a2]: [f64; 6]) {
    let (b0, b1, b2, a1, a2) = (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    for s in pcm {
        let x = *s as f64;
        let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        (x2, x1, y2, y1) = (x1, x, y1, y);
        *s = y as f32;
    }
}

// Butterworth 2 poles, meme forme que le lowpass/highpass de ffmpeg.
fn biquad(pcm: &mut [f32], freq: f64, highpass: bool) {
    let w0 = 2.0 * std::f64::consts::PI * freq / SR as f64;
    let (cos, alpha) = (w0.cos(), w0.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2));
    let (b1, b0) = if highpass { (-(1.0 + cos), (1.0 + cos) / 2.0) } else { (1.0 - cos, (1.0 - cos) / 2.0) };
    run_biquad(pcm, [b0, b1, b0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha]);
}

// Cloche (peaking EQ) : `db` de gain autour de `freq`.
fn peaking(pcm: &mut [f32], freq: f64, q: f64, db: f64) {
    let (a, w0) = (10f64.powf(db / 40.0), 2.0 * std::f64::consts::PI * freq / SR as f64);
    let (cos, alpha) = (w0.cos(), w0.sin() / (2.0 * q));
    run_biquad(pcm, [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a]);
}

const TARGET_DBFS: f64 = -18.0; // niveau moyen de la parole visé après normalisation
const BLOCK: usize = SR as usize * 4 / 10; // blocs de 400 ms pour la mesure

/// Amélioration de la voix : retire le « creux » boîteux à 250 Hz, ajoute de la présence à 3 kHz,
/// ramène le niveau moyen à -18 dBFS et limite les crêtes sous 0 dBFS.
// shortcut: niveau mesuré en RMS par blocs avec porte (pas un vrai LUFS pondéré K) ; passer à
// un mesureur EBU R128 (crate ebur128) si le niveau doit s'aligner sur une norme de diffusion.
pub fn enhance(pcm: &mut [f32]) {
    peaking(pcm, 250.0, 1.0, -2.0);
    peaking(pcm, 3000.0, 0.9, 2.5);
    let blocks: Vec<&[f32]> = if pcm.len() < BLOCK { vec![&pcm[..]] } else { pcm.chunks_exact(BLOCK).collect() };
    let power = |c: &[f32]| c.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / c.len().max(1) as f64;
    // Porte : on ignore le silence (< -50 dBFS) puis ce qui est 10 dB sous la moyenne (respirations, pauses).
    let loud: Vec<f64> = blocks.iter().map(|c| power(c)).filter(|&p| p > 1e-5).collect();
    if loud.is_empty() {
        return;
    }
    let mean = loud.iter().sum::<f64>() / loud.len() as f64;
    let kept: Vec<f64> = loud.into_iter().filter(|&p| p > mean * 0.1).collect();
    let rms = (kept.iter().sum::<f64>() / kept.len() as f64).sqrt();
    let gain = (10f64.powf(TARGET_DBFS / 20.0) / rms).min(15.85) as f32; // jamais plus de +24 dB
    for s in pcm {
        *s = soft_limit(*s * gain);
    }
}

// Au-dessus de 0,7 la courbe s'arrondit vers 0,99 : pas de saturation dure.
fn soft_limit(x: f32) -> f32 {
    const T: f32 = 0.7;
    const MAX: f32 = 0.99;
    let a = x.abs();
    if a <= T { x } else { x.signum() * (T + (MAX - T) * ((a - T) / (MAX - T)).tanh()) }
}

/// MP3 mono 24 kHz, 128 kb/s.
pub fn encode_mp3(pcm: &[f32]) -> R<Vec<u8>> {
    use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, MonoPcm, Quality};
    let mut b = Builder::new().ok_or_else(|| tr!("encodeur MP3 indisponible", "MP3 encoder unavailable"))?;
    b.set_num_channels(1).map_err(e)?;
    b.set_sample_rate(SR).map_err(e)?;
    b.set_brate(Bitrate::Kbps128).map_err(e)?;
    // NearBest (q2, reglage recommande par LAME) : 2,8 fois plus rapide que Best (q0) pour un resultat perceptivement identique a 128 kb/s.
    b.set_quality(Quality::NearBest).map_err(e)?;
    let mut enc = b.build().map_err(e)?;
    let samples: Vec<i16> = pcm.iter().map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
    let mut out = Vec::with_capacity(mp3lame_encoder::max_required_buffer_size(samples.len()));
    let n = enc.encode(MonoPcm(&samples), out.spare_capacity_mut()).map_err(e)?;
    // SAFETY : l'encodeur vient d'initialiser exactement `n` octets de la capacite reservee.
    unsafe { out.set_len(n) };
    let mut tail = Vec::with_capacity(7200);
    let n = enc.flush::<FlushNoGap>(tail.spare_capacity_mut()).map_err(e)?;
    unsafe { tail.set_len(n) };
    out.extend(tail);
    Ok(out)
}

/// Decoupe aux fins de phrase en morceaux d'au plus `max` caracteres (une phrase plus longue reste entiere).
pub fn split_text(text: &str, max: usize) -> Vec<String> {
    let (mut chunks, mut cur, mut sentence) = (vec![], String::new(), String::new());
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        sentence.push(c);
        // "3.5" ou "v1.0" ne coupent pas : il faut un espace (ou la fin) apres la ponctuation.
        // Chinois / japonais : « 。！？ » terminent la phrase sans espace derriere.
        let end = c == '\n' || "。！？".contains(c) || (".!?…".contains(c) && it.peek().is_none_or(|n| n.is_whitespace()));
        if end || it.peek().is_none() {
            if !cur.trim().is_empty() && cur.chars().count() + sentence.chars().count() > max {
                chunks.push(std::mem::take(&mut cur));
            }
            cur.push_str(&sentence);
            sentence.clear();
        }
    }
    chunks.push(cur);
    chunks.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// La transcription d'un echantillon est-elle compatible avec la duree de son audio ? Une parole normale fait environ
/// 10 a 25 caracteres par seconde ; hors de 7..=30, le texte ne correspond pas a ce qui est dit (enregistrement coupe,
/// texte propose laisse en place...) et le clonage avec transcription se met a deriver (voix qui lit n'importe quoi).
/// Chinois, japonais et coreen (peu de caracteres par seconde) ne sont pas controles.
pub fn transcript_plausible(text: &str, secs: f64) -> bool {
    let text = text.trim();
    let cjk = |c: char| ('\u{3000}'..='\u{9fff}').contains(&c) || ('\u{ac00}'..='\u{d7af}').contains(&c) || ('\u{f900}'..='\u{faff}').contains(&c);
    if text.is_empty() || text.chars().any(cjk) || secs <= 0.0 {
        return true;
    }
    (7.0..=30.0).contains(&(text.chars().count() as f64 / secs))
}

/// Retire le silence au debut et a la fin d'un morceau (il en garde `keep_ms` de chaque cote) : les pauses demandees
/// par `[pause]` sont alors exactes, sans s'ajouter au silence de fin que le modele laisse apres chaque phrase.
pub fn trim_silence(pcm: &mut Vec<f32>, keep_ms: usize) {
    let hop = SR as usize / 100; // fenetres de 10 ms
    let loud = |w: &[f32]| (w.iter().map(|x| x * x).sum::<f32>() / w.len() as f32).sqrt() > 0.01;
    let Some(first) = pcm.chunks(hop).position(loud) else { return };
    let last = pcm.chunks(hop).rposition(loud).unwrap_or(first);
    let keep = SR as usize * keep_ms / 1000;
    let (start, end) = ((first * hop).saturating_sub(keep), ((last + 1) * hop + keep).min(pcm.len()));
    pcm.truncate(end);
    pcm.drain(..start);
}

/// Coupe un texte en deux moities : aux fins de phrase si possible, sinon a l'espace le plus proche du milieu.
/// Un seul morceau en retour = indivisible (trop court ou sans espace).
pub fn halve(text: &str) -> Vec<String> {
    let n = text.chars().count();
    if n < 40 {
        return vec![text.to_string()];
    }
    let by_sentence = split_text(text, n / 2 + 1);
    if by_sentence.len() > 1 {
        return by_sentence;
    }
    let chars: Vec<char> = text.chars().collect();
    // Sans espace : ecritures chinoise/japonaise/coreenne (coupe a la ponctuation ou au milieu) ; un mot latin tres long reste entier.
    let cut = (0..n).filter(|&i| chars[i] == ' ').min_by_key(|&i| i.abs_diff(n / 2)).or_else(|| (1..n).filter(|&i| "，、,；;".contains(chars[i - 1])).min_by_key(|&i| i.abs_diff(n / 2)))
        .or_else(|| chars.iter().any(|&c| ('\u{3000}'..='\u{9fff}').contains(&c) || ('\u{ac00}'..='\u{d7af}').contains(&c)).then_some(n / 2));
    match cut {
        Some(i) => vec![chars[..i].iter().collect::<String>().trim().to_string(), chars[i..].iter().collect::<String>().trim().to_string()],
        None => vec![text.to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }
    fn sine(freq: f32) -> Vec<f32> {
        (0..SR).map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / SR as f32).sin()).collect()
    }

    #[test]
    fn clean_keeps_voice_cuts_hiss() {
        let (mut voice, mut hiss, mut rumble) = (sine(1000.0), sine(11500.0), sine(15.0));
        clean(&mut voice);
        clean(&mut hiss);
        clean(&mut rumble);
        assert!((rms(&voice) - 0.707).abs() < 0.02, "1 kHz doit passer : {}", rms(&voice));
        assert!(rms(&hiss) < 0.2, "11,5 kHz doit être atténué : {}", rms(&hiss));
        assert!(rms(&rumble) < 0.1, "15 Hz doit être atténué : {}", rms(&rumble));
    }

    fn db(x: f32) -> f32 {
        20.0 * x.log10()
    }

    #[test]
    fn enhance_levels() {
        // Voix trop faible : ramenée au niveau cible, sans écrêtage.
        let mut quiet: Vec<f32> = sine(500.0).iter().map(|v| v * 0.05).collect();
        enhance(&mut quiet);
        assert!((db(rms(&quiet)) - TARGET_DBFS as f32).abs() < 0.5, "niveau {}", db(rms(&quiet)));
        // Voix trop forte : réduite.
        let mut loud: Vec<f32> = sine(500.0).iter().map(|v| v * 0.99).collect();
        enhance(&mut loud);
        assert!((db(rms(&loud)) - TARGET_DBFS as f32).abs() < 0.5);
        // Crêtes : jamais d'écrêtage dur, même avec une pointe très au-dessus de la moyenne.
        let mut spiky: Vec<f32> = sine(500.0).iter().map(|v| v * 0.02).collect();
        for i in (0..spiky.len()).step_by(4000) {
            spiky[i] = 0.9;
        }
        enhance(&mut spiky);
        assert!(spiky.iter().all(|v| v.is_finite() && v.abs() <= 0.99));
        // Silence et clip très court : rien ne explose.
        let mut silence = vec![0.0f32; 48000];
        enhance(&mut silence);
        assert!(silence.iter().all(|&v| v == 0.0));
        let mut tiny = vec![0.01f32; 100];
        enhance(&mut tiny);
        assert!(tiny.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn soft_limit_is_monotonic_and_bounded() {
        let mut last = 0.0;
        for i in 0..400 {
            let y = soft_limit(i as f32 * 0.01);
            assert!(y >= last && y <= 0.99 && (i < 70 || y > 0.6));
            last = y;
        }
        assert_eq!(soft_limit(-5.0), -soft_limit(5.0));
    }

    #[test]
    fn mp3_header() {
        let mp3 = encode_mp3(&sine(440.0)).unwrap();
        // Trame MPEG : 0xFF 0xE? (synchro) ; 1 s de parole en 128 kb/s pèse environ 16 Ko.
        assert!(mp3.len() > 10_000 && mp3.len() < 20_000, "taille {}", mp3.len());
        assert!(mp3[0] == 0xFF && mp3[1] & 0xE0 == 0xE0 || &mp3[..3] == b"ID3");
    }

    #[test]
    fn split() {
        assert_eq!(split_text("  Bonjour. Ça va ?  ", 300), ["Bonjour. Ça va ?"]);
        assert_eq!(split_text("Un. Deux ! Trois…\nQuatre", 8), ["Un.", "Deux !", "Trois…", "Quatre"]);
        assert_eq!(split_text("Version 3.5 sortie. Fin.", 12), ["Version 3.5 sortie.", "Fin."]);
        assert!(split_text(" \n ", 10).is_empty());
    }

    #[test]
    fn transcript_must_fit_the_audio() {
        let sample = "Ce matin, le soleil se lève doucement sur les toits de Lyon. ".repeat(6); // 366 caracteres
        assert!(transcript_plausible(&sample, 22.6)); // 16 car/s : normal
        assert!(!transcript_plausible(&sample, 10.6)); // 35 car/s : audio coupe
        assert!(!transcript_plausible("Bonjour", 30.0)); // 0,2 car/s : texte trop court pour l'audio
        assert!(transcript_plausible("", 10.0) && transcript_plausible("你好，世界你好世界", 30.0));
    }

    #[test]
    fn silence_is_trimmed_at_both_ends() {
        let sr = SR as usize;
        let mut pcm = vec![0.0; sr]; // 1 s de silence
        pcm.extend((0..sr).map(|i| 0.3 * (i as f32 * 0.1).sin())); // 1 s de son
        pcm.extend(vec![0.0; sr * 2]); // 2 s de silence
        trim_silence(&mut pcm, 50);
        assert!((pcm.len() as i64 - (sr as i64 + sr as i64 / 10)).abs() < sr as i64 / 50, "{}", pcm.len()); // 1 s + 2 x 50 ms
        let mut silent = vec![0.0; sr];
        trim_silence(&mut silent, 50);
        assert_eq!(silent.len(), sr, "un morceau muet reste tel quel");
    }

    #[test]
    fn chinese_text_is_split_and_halved_without_spaces() {
        let zh = "今天天气很好。我们去公园散步吧！你觉得怎么样？".repeat(40);
        let chunks = split_text(&zh, 300);
        assert!(chunks.len() > 1 && chunks.iter().all(|c| c.chars().count() <= 330), "{}", chunks.len());
        assert_eq!(chunks.concat(), zh);
        let long = "字".repeat(100);
        let halves = halve(&long);
        assert_eq!(halves.len(), 2);
        assert_eq!(halves.concat(), long);
    }

    #[test]
    fn halving_long_text() {
        assert_eq!(halve("court"), ["court"]);
        let two = halve("Première phrase assez longue pour couper. Seconde phrase tout aussi longue que la première.");
        assert_eq!(two.len(), 2);
        let one_sentence = halve("un deux trois quatre cinq six sept huit neuf dix onze douze treize quatorze quinze");
        assert_eq!(one_sentence.len(), 2);
        assert_eq!(one_sentence.join(" "), "un deux trois quatre cinq six sept huit neuf dix onze douze treize quatorze quinze");
        assert_eq!(halve(&"a".repeat(100)).len(), 1);
    }
}
