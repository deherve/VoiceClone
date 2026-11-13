//! Mesures de temps des fonctions de traitement (jamais lancees par `cargo test` : `-- --ignored --nocapture`).
//!   cargo test --release -p voiceclone bench -- --ignored --nocapture --test-threads=1
use crate::{audio, export, normalize, quality, script, App};
use std::time::Instant;

fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let r = f();
    println!("BENCH {label:<42} {:>9.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    r
}

/// Signal « parole » : harmoniques de 120 Hz modulees en amplitude (syllabes) avec des silences.
fn speech(secs: usize) -> Vec<f32> {
    (0..secs * audio::SR as usize)
        .map(|i| {
            let t = i as f32 / audio::SR as f32;
            let env = ((t * 4.0).sin().max(0.0)).powi(2) * if (t * 0.5) as usize % 5 == 4 { 0.0 } else { 1.0 };
            env * (0.3 * (2.0 * std::f32::consts::PI * 120.0 * t).sin() + 0.15 * (2.0 * std::f32::consts::PI * 240.0 * t).sin() + 0.08 * (2.0 * std::f32::consts::PI * 360.0 * t).sin())
        })
        .collect()
}

fn dialogue(words: usize) -> String {
    let mut s = String::new();
    let voices = ["Marco", "Annie", "Loic"];
    let mut n = 0;
    let mut i = 0;
    while n < words {
        s.push_str(&format!("[{}] Alors, le cloud de la GPU, c'est 12 fois plus rapide, non ? Je n'en reviens pas, vraiment... [pause 0.5s]\n", voices[i % 3]));
        n += 20;
        i += 1;
    }
    s
}

#[test]
#[ignore]
fn bench_audio() {
    let pcm = speech(600); // 10 minutes
    let mut a = pcm.clone();
    timed("audio::clean (10 min)", || audio::clean(&mut a));
    let mut a = pcm.clone();
    timed("audio::enhance (10 min)", || audio::enhance(&mut a));
    let dir = std::env::temp_dir().join(format!("vc-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("b.wav");
    timed("audio::write_wav (10 min)", || audio::write_wav(&wav, &pcm).unwrap());
    let back = timed("audio::read_wav_24k (10 min)", || audio::read_wav_24k(&wav).unwrap());
    assert_eq!(back.len(), pcm.len());
    timed("audio::encode_mp3 (1 min)", || audio::encode_mp3(&pcm[..audio::SR as usize * 60]).unwrap());
    timed("transcribe::resample_16k (25 s)", || crate::transcribe::resample_16k(&pcm[..audio::SR as usize * 25]));
    let sample = &pcm[..audio::SR as usize * 25];
    timed("quality::measure (25 s)", || quality::measure(sample));
    let mut t = pcm.clone();
    timed("audio::trim_silence (10 min)", || audio::trim_silence(&mut t, 50));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore]
fn bench_text() {
    let text = dialogue(11000); // une heure de dialogue
    println!("BENCH texte : {} caracteres", text.len());
    timed("script::parse (1 h)", || script::parse(&text).unwrap());
    timed("audio::split_text (1 h)", || audio::split_text(&text, 300));
    let dict: Vec<(String, String)> = (0..60).map(|i| (format!("mot{i}"), format!("lecture{i}"))).collect();
    timed("normalize::phonetic_fr (1 h, 60 mots)", || normalize::phonetic_fr(&text, &dict));
    timed("normalize::normalize (1 h)", || normalize::normalize(&text, &dict, true, "fr"));
    let cues: Vec<export::Cue> = (0..800).map(|i| export::Cue { voice: "Marco".into(), start: i as f64 * 4.0, end: i as f64 * 4.0 + 3.5, text: "Une phrase de test. Et une autre phrase un peu plus longue que la premiere.".into() }).collect();
    timed("export::srt (800 morceaux)", || export::srt(&cues));
    let json = export::to_json(&cues);
    timed("export::from_json (800 morceaux)", || export::from_json(&json).unwrap());
}

#[test]
#[ignore]
fn bench_app() {
    let dir = std::env::temp_dir().join(format!("vc-bench-app-{}", std::process::id()));
    let app = timed("App::open (base vide)", || App::open(dir.clone()).unwrap());
    let pid = app.create_profile("Zen", "french").unwrap();
    let pcm = speech(25);
    let sample = app.add_sample(pid, &pcm).unwrap();
    app.set_sample_text(sample, &"a".repeat(380)).unwrap();
    timed("App::voice_quality (1 echantillon de 25 s)", || app.voice_quality(pid).unwrap());
    timed("App::list_samples", || app.list_samples(pid).unwrap());
    // Beaucoup de generations : la liste est relue a chaque changement.
    let wav = dir.join("g.wav");
    audio::write_wav(&wav, &speech(5)).unwrap();
    for i in 0..500 {
        let id = app.store.lock().unwrap().add_generation(pid, &format!("texte {i} avec [Zen] une longue phrase pour l'historique"), &wav.to_string_lossy(), 5.0, 7).unwrap();
        app.store.lock().unwrap().set_timeline(id, &export::to_json(&[export::Cue { voice: "Zen".into(), start: 0.0, end: 5.0, text: "x".into() }])).unwrap();
    }
    timed("App::list_generations (500)", || app.list_generations(200).unwrap());
    // Forme d'onde d'une longue generation (une heure).
    let long = dir.join("long.wav");
    audio::write_wav(&long, &speech(3600)).unwrap();
    let id = app.store.lock().unwrap().add_generation(pid, "une heure", &long.to_string_lossy(), 3600.0, 7).unwrap();
    timed("App::waveform (1 h)", || app.waveform("generation", id, 64).unwrap());
    timed("App::set_enhanced on (1 h)", || app.set_enhanced(id, true).unwrap());
    drop(app);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore]
fn bench_mp3_quality() {
    // VC_BENCH_WAV : un vrai fichier de parole (24 kHz mono). Compare le temps et la taille selon le reglage de LAME.
    let Some(path) = std::env::var_os("VC_BENCH_WAV") else { return };
    let pcm = audio::read_wav_24k(std::path::Path::new(&path)).unwrap();
    let pcm = &pcm[..pcm.len().min(audio::SR as usize * 120)];
    use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, MonoPcm, Quality};
    for (name, q) in [("Best(0)", Quality::Best), ("NearBest(2)", Quality::NearBest), ("Good(5)", Quality::Good)] {
        let t = Instant::now();
        let mut b = Builder::new().unwrap();
        b.set_num_channels(1).unwrap();
        b.set_sample_rate(audio::SR).unwrap();
        b.set_brate(Bitrate::Kbps128).unwrap();
        b.set_quality(q).unwrap();
        let mut enc = b.build().unwrap();
        let samples: Vec<i16> = pcm.iter().map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
        let mut out = Vec::with_capacity(mp3lame_encoder::max_required_buffer_size(samples.len()));
        let n = enc.encode(MonoPcm(&samples), out.spare_capacity_mut()).unwrap();
        unsafe { out.set_len(n) };
        let mut tail = Vec::with_capacity(7200);
        let n = enc.flush::<FlushNoGap>(tail.spare_capacity_mut()).unwrap();
        unsafe { tail.set_len(n) };
        out.extend(tail);
        println!("BENCH mp3 {name:<12} 120 s : {:>8.1} ms, {} octets", t.elapsed().as_secs_f64() * 1000.0, out.len());
    }
}
