//! Assistant d'ecriture : une IA locale legere (Qwen3 4B Instruct, licence Apache 2.0, francais et anglais) ecrit un recit
//! ou un dialogue a plusieurs voix, directement avec les balises de VoiceClone. Elle tourne avec `llama-server` (llama.cpp,
//! version Vulkan) installe dans `<donnees>\writer` depuis ses sources officielles, chaque fichier verifie par un SHA-256
//! fixe. Le serveur ne vit que le temps d'une demande, sur 127.0.0.1 : rien ne sort de l'ordinateur.
use crate::{e, script, transcribe, update, R};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const LLAMA_URL: &str = "https://github.com/ggml-org/llama.cpp/releases/download/b12360/llama-b12360-bin-win-vulkan-x64.zip";
const LLAMA_SHA256: &str = "42a6f9df608b99c1c83fcaf77284a516e3f167b32a4690f4e4e364c377547055";
const LLAMA_MB: u64 = 37;
pub const MODEL_FILE: &str = "Qwen3-4B-Instruct-2507-Q4_K_M.gguf";
const MODEL_URL: &str = "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen3-4B-Instruct-2507-Q4_K_M.gguf";
const MODEL_SHA256: &str = "3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597";
const MODEL_MB: u64 = 2382;

fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("writer")
}

pub fn status(data_dir: &Path) -> Value {
    let d = dir(data_dir);
    let (bin, model) = (transcribe::find_file(&d.join("bin"), "llama-server.exe").is_some(), d.join(MODEL_FILE).is_file());
    json!({"installed": bin && model, "bin": bin, "model": model, "size_mb": LLAMA_MB + MODEL_MB, "model_label": "Qwen3 4B Instruct (Q4_K_M)",
           "source": "github.com/ggml-org/llama.cpp · huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF"})
}

/// Installe le moteur puis le modele. `progress(etape, fait, total)` : etape « bin » puis « model ».
pub fn install(data_dir: &Path, cancel: &AtomicBool, progress: &dyn Fn(&str, u64, u64)) -> R<()> {
    let d = dir(data_dir);
    fs::create_dir_all(&d).map_err(e)?;
    if transcribe::find_file(&d.join("bin"), "llama-server.exe").is_none() {
        let zip = d.join("llama-bin.zip");
        transcribe::fetch(LLAMA_URL, LLAMA_SHA256, &zip, cancel, &|a, b| progress("bin", a, b))?;
        // Decompression dans un dossier temporaire puis renommage : une extraction interrompue ne laisse jamais un moteur a moitie installe.
        let (bin, staging) = (d.join("bin"), d.join("bin.tmp"));
        let _ = fs::remove_dir_all(&staging);
        update::unpack(&zip, &staging)?;
        let _ = fs::remove_file(&zip);
        let _ = fs::remove_dir_all(&bin);
        fs::rename(&staging, &bin).map_err(e)?;
        if transcribe::find_file(&bin, "llama-server.exe").is_none() {
            return Err(tr!("archive llama.cpp incomplète : llama-server.exe manque", "incomplete llama.cpp archive: llama-server.exe is missing"));
        }
    }
    if !d.join(MODEL_FILE).is_file() {
        transcribe::fetch(MODEL_URL, MODEL_SHA256, &d.join(MODEL_FILE), cancel, &|a, b| progress("model", a, b))?;
    }
    Ok(())
}

pub fn remove(data_dir: &Path) -> R<()> {
    let d = dir(data_dir);
    if d.exists() {
        fs::remove_dir_all(&d).map_err(e)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- consigne
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Dialogue,
    Story,
}

/// Mots par minute a la lecture (silences compris) selon le ton, mesures avec la voix de synthese sur des textes ecrits
/// par l'assistant : dialogue vif ~188, recit calme ~158. Une conversation humaine normale en fait 150 a 170 ; la voix
/// de synthese parle un peu plus vite quand le texte est enjoue.
fn words_per_minute(tone: Tone) -> f64 {
    match tone {
        Tone::Natural | Tone::Funny => 185.0,
        Tone::Cheerful => 190.0,
        Tone::Warm => 172.0,
        Tone::Dramatic => 165.0,
        Tone::Calm => 158.0,
    }
}
/// Duree maximale : 2 heures (un podcast d'une heure ecrit en une quarantaine de parties, chacune avec son plan).
pub const MAX_SECONDS: u32 = 7200;
const PART_WORDS: u32 = 280; // au-dela, le texte est ecrit en plusieurs parties (le petit modele garde mieux le fil)

/// Nombre de mots a ecrire pour durer `seconds` secondes a voix haute avec ce ton.
pub fn words_for(seconds: u32, tone: Tone) -> u32 {
    (seconds.clamp(15, MAX_SECONDS) as f64 / 60.0 * words_per_minute(tone)).round() as u32
}

/// Mots dits d'un texte : les balises `[...]` ne comptent pas.
fn count_words(text: &str) -> u32 {
    let mut depth = 0;
    let spoken: String = text.chars().filter(|&c| match c {
        '[' => { depth += 1; false }
        ']' if depth > 0 => { depth -= 1; false }
        _ => depth == 0,
    }).collect();
    spoken.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count() as u32
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tone {
    Natural,
    Warm,
    Cheerful,
    Dramatic,
    Funny,
    Calm,
}

impl Tone {
    pub fn parse(s: &str) -> Tone {
        match s {
            "warm" => Tone::Warm,
            "cheerful" => Tone::Cheerful,
            "dramatic" => Tone::Dramatic,
            "funny" => Tone::Funny,
            "calm" => Tone::Calm,
            _ => Tone::Natural,
        }
    }

    /// Consigne de style : l'intonation de la voix suit la ponctuation, les interjections et le rythme du texte.
    fn style(self) -> &'static str {
        match self {
            Tone::Natural => "Everyday spontaneous speech: contractions, interjections (ah, oh, hmm, bah, hein), small hesitations written with ellipses, questions and exclamations, speakers reacting to each other, sentences of very different lengths.",
            Tone::Warm => "Warm and kind: gentle reactions, reassuring words, soft questions, a few tender exclamations, sentences that flow slowly.",
            Tone::Cheerful => "Cheerful and energetic: enthusiasm, many exclamations, quick reactions, laughter in the words, short punchy sentences mixed with longer excited ones.",
            Tone::Dramatic => "Dramatic and tense: very short sentences, rhetorical questions, suspense with ellipses, sudden exclamations, silences before important words.",
            Tone::Funny => "Funny and playful: teasing, wordplay, exaggeration, comic timing with a silence just before each punchline, surprised reactions.",
            Tone::Calm => "Calm and thoughtful: slow, gentle, well-balanced sentences, few exclamations, soft ellipses, a reassuring rhythm.",
        }
    }
}

pub struct Request {
    pub prompt: String,
    pub mode: Mode,
    /// « fr » ou « en ».
    pub language: String,
    /// Duree visee a voix haute, en secondes (15 a `MAX_SECONDS`).
    pub seconds: u32,
    pub tone: Tone,
    /// Voix autorisees (dialogue) : leurs noms exacts.
    pub voices: Vec<String>,
}

/// `part` : (numero, total) quand le texte est ecrit en plusieurs parties.
fn system_prompt(r: &Request) -> String {
    let lang = if r.language == "en" { "English" } else { "French" };
    let mut s = format!(
        "You write texts that a text-to-speech voice will read aloud. Write in {lang}. Reply with ONLY the text to read: no title, no introduction, no explanation, \
         no markdown, no emoji, no stage directions, no sound effects, nothing between parentheses. Write numbers, dates and abbreviations out in words. \
         Make it sound alive and human when spoken: {} \
         Punctuation drives the voice's intonation, so use it on purpose: \"?\" for questions, \"!\" for strong feelings, \"...\" for hesitation or trailing off, commas for breath.\n\
         Add silences with pause tags to bring the text to life: [pause 0.5s] for a short beat (a hesitation, a breath, just before a punchline), [pause 1s] between two exchanges or paragraphs, \
         [pause 2s] only for a change of scene. Write a pause tag inside a line or alone on its own line, and use about one pause every two or three lines. Never use any other tag than the ones allowed here.\n",
        r.tone.style()
    );
    match r.mode {
        Mode::Dialogue => {
            let tags = r.voices.iter().map(|v| format!("[{v}]")).collect::<Vec<_>>().join(" ");
            s.push_str(&format!(
                "The text is a dialogue between these speakers: {tags}. Every line of speech starts with the speaker's tag in square brackets exactly as written above, followed by what that speaker says, \
                 for example \"[{}] Hello!\". Use only these speaker tags and let every speaker talk.                  EVERY line of speech, without exception, starts with a speaker tag. Keep lines short (one to three sentences) and let the speakers alternate often and answer each other directly:                  never let one speaker talk for more than three lines in a row, and never write a monologue.",
                r.voices.first().map_or("Name", String::as_str)
            ));
        }
        Mode::Story => s.push_str("The text is a story told by a single narrator: flowing prose in short paragraphs, with no speaker tag and no speaker names."),
    }
    s
}

/// Consigne pour la partie `part` sur `parts`. Chaque partie est une demande independante (le contexte du petit modele
/// reste borne, meme pour une heure de texte) : elle recoit le plan complet et la fin de la partie precedente.
fn part_prompt(r: &Request, part: u32, parts: u32, words: u32, outline: &[String], tail: &str) -> String {
    if parts == 1 {
        return format!("{}\n\nLength: about {words} words (about {} seconds when read aloud).", r.prompt.trim(), r.seconds);
    }
    let mut s = format!("{}\n\nThe whole text lasts about {} minutes when read aloud and is written in {parts} consecutive parts.", r.prompt.trim(), (r.seconds as f64 / 60.0).round().max(1.0));
    if outline.len() == parts as usize {
        s.push_str("\nOutline of the whole text:\n");
        for (i, o) in outline.iter().enumerate() {
            s.push_str(&format!("{}. {o}\n", i + 1));
        }
    }
    s.push_str(&format!("\nNow write part {part} of {parts}"));
    if let Some(o) = outline.get(part as usize - 1).filter(|_| outline.len() == parts as usize) {
        s.push_str(&format!(" ({o})"));
    }
    s.push_str(&format!(", about {words} words."));
    if part > 1 {
        s.push_str(&format!(" The previous part ended with:\n\"{tail}\"\nContinue naturally from there, without repeating or summarising."));
    } else {
        s.push_str(" Start the text.");
    }
    s.push_str(if part < parts { " Do not conclude the text yet." } else { " This is the last part: bring the whole text to a natural conclusion." });
    s
}

/// Demande d'un plan : une ligne par partie.
fn outline_prompt(r: &Request, parts: u32, words: u32) -> String {
    format!(
        "{}\n\nPlan this text as {parts} consecutive parts of about {words} words each, from the beginning to a natural conclusion, with variety and progression (no repetition between parts). \
         Reply with exactly {parts} lines, one per part, formatted \"N. short description (at most 20 words)\". Nothing else.",
        r.prompt.trim()
    )
}

/// Lignes « N. description » d'une reponse de plan (au plus `max`).
fn parse_outline(reply: &str, max: usize) -> Vec<String> {
    reply
        .lines()
        .filter_map(|l| {
            let l = l.trim().trim_start_matches(['-', '*', ' ']);
            let digits = l.chars().take_while(char::is_ascii_digit).count();
            let rest = l[digits..].trim_start_matches(['.', ')', ':', ' ']).trim();
            (digits > 0 && !rest.is_empty()).then(|| rest.chars().take(160).collect::<String>())
        })
        .take(max)
        .collect()
}

/// Part des lignes de parole qui commencent par une balise de voix (les lignes de pause seules ne comptent pas).
fn tagged_share(text: &str, voices: &[String]) -> f64 {
    let speech: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !(l.starts_with('[') && l.ends_with(']') && script::is_pause_tag(&l[1..l.len() - 1]))).collect();
    if speech.is_empty() {
        return 1.0;
    }
    let tagged = speech.iter().filter(|l| l.starts_with('[') && l.find(']').is_some_and(|i| voices.iter().any(|v| v.eq_ignore_ascii_case(l[1..i].trim())))).count();
    tagged as f64 / speech.len() as f64
}

/// Fin de la partie precedente : environ les 400 derniers caracteres, a partir d'un debut de ligne ou de mot.
fn tail_of(text: &str) -> String {
    let n = text.chars().count();
    if n <= 400 {
        return text.to_string();
    }
    let cut: String = text.chars().skip(n - 400).collect();
    match cut.find('\n') {
        Some(i) if i < 200 => cut[i + 1..].to_string(),
        _ => cut.split_once(' ').map_or(cut.clone(), |(_, r)| r.to_string()),
    }
}

// ---------------------------------------------------------------- nettoyage de la reponse
/// Remet la reponse du modele au format attendu : pas de markdown ni d'indications entre parentheses ; « Nom : texte »
/// devient « [Nom] texte » ; les balises inconnues disparaissent (le texte reste) ; seules les balises des voix et `[pause]` restent.
pub fn clean_reply(raw: &str, voices: &[String], dialogue: bool) -> String {
    let mut text = raw.to_string();
    // Reflexion eventuelle du modele.
    while let (Some(a), Some(b)) = (text.find("<think>"), text.find("</think>")) {
        if b < a {
            break;
        }
        text.replace_range(a..b + "</think>".len(), "");
    }
    let mut lines = vec![];
    for line in text.lines() {
        let mut l = line.trim().replace(['*', '`', '_'], "");
        l = l.trim_start_matches(['#', '>', '-', '•', ' ']).to_string();
        if dialogue {
            l = strip_parentheses(&l);
            l = speaker_prefix(&l, voices);
        }
        l = drop_unknown_tags(&l, voices, dialogue);
        let l = l.split_whitespace().collect::<Vec<_>>().join(" ");
        lines.push(l);
    }
    let mut out = lines.join("\n");
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim().to_string()
}

fn strip_parentheses(s: &str) -> String {
    let (mut out, mut depth) = (String::new(), 0);
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// « Marco : Bonjour » ou « [Marco] : Bonjour » → « [Marco] Bonjour » quand « Marco » est une voix connue.
fn speaker_prefix(line: &str, voices: &[String]) -> String {
    let (head, rest) = match line.find([':', '：']) {
        Some(i) if i <= 80 => (line[..i].trim().trim_matches(|c| c == '[' || c == ']').trim().to_string(), line[i + line[i..].chars().next().map_or(1, char::len_utf8)..].trim()),
        _ => return line.to_string(),
    };
    match voices.iter().find(|v| v.to_lowercase() == head.to_lowercase()) {
        Some(v) => format!("[{v}] {rest}"),
        None => line.to_string(),
    }
}

/// Retire les `[...]` qui ne sont ni une voix connue (dialogue) ni une pause valide.
fn drop_unknown_tags(line: &str, voices: &[String], dialogue: bool) -> String {
    let (mut out, mut rest) = (String::new(), line);
    while let Some(open) = rest.find('[') {
        let Some(len) = rest[open..].find(']') else { break };
        let tag = rest[open + 1..open + len].trim();
        let known = script::is_pause_tag(tag) || (dialogue && voices.iter().any(|v| v.to_lowercase() == tag.to_lowercase()));
        out.push_str(&rest[..open]);
        if known {
            out.push_str(&rest[open..=open + len]);
        }
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------- serveur
struct Server {
    child: Child,
    port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> R<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").map_err(e)?;
    Ok(l.local_addr().map_err(e)?.port())
}

fn start(exe: &Path, model: &Path, gpu: bool) -> R<Server> {
    let port = free_port()?;
    let mut cmd = Command::new(exe);
    cmd.current_dir(exe.parent().unwrap_or(Path::new(".")))
        .arg("-m").arg(model)
        .args(["--host", "127.0.0.1", "--port"]).arg(port.to_string())
        .args(["-c", "8192", "-np", "1", "-ngl", if gpu { "99" } else { "0" }, "--no-warmup"])
        .stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // pas de fenetre de console
    let child = cmd.spawn().map_err(|x| tr!("impossible de lancer llama-server : {}", "cannot start llama-server: {}", x))?;
    crate::jobs::adopt(&child); // meurt avec l'application, meme si elle est fermee pendant l'ecriture
    Ok(Server { child, port })
}

/// Attend que le modele soit charge (`/health` repond 200). Echec si le processus s'arrete ou si le delai passe.
fn wait_ready(s: &mut Server, timeout: Duration, cancel: &AtomicBool) -> R<()> {
    let started = Instant::now();
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(2)).build();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(tr!("génération annulée", "generation cancelled"));
        }
        if let Ok(Some(status)) = s.child.try_wait() {
            return Err(tr!("llama-server s'est arrêté au démarrage (code {})", "llama-server stopped at startup (code {})", status.code().unwrap_or(-1)));
        }
        if agent.get(&format!("http://127.0.0.1:{}/health", s.port)).call().is_ok() {
            return Ok(());
        }
        if started.elapsed() > timeout {
            return Err(tr!("le modèle met trop de temps à se charger", "the model takes too long to load"));
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn chat(port: u16, messages: &[Value], max_tokens: u32, cancel: &AtomicBool) -> R<String> {
    let body = json!({"model": "local", "stream": false, "temperature": 0.85, "top_p": 0.9, "max_tokens": max_tokens, "messages": messages});
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(900)).build();
        let r = agent.post(&format!("http://127.0.0.1:{port}/v1/chat/completions")).set("Content-Type", "application/json").send_string(&body.to_string()).map_err(|x| x.to_string()).and_then(|resp| {
            let mut raw = String::new();
            std::io::Read::read_to_string(&mut std::io::Read::take(resp.into_reader(), 4 << 20), &mut raw).map_err(|x| x.to_string())?;
            Ok(raw)
        });
        let _ = tx.send(r);
    });
    let raw = loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(r) => break r.map_err(|x| tr!("génération impossible : {}", "generation failed: {}", x))?,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) if cancel.load(Ordering::Relaxed) => return Err(tr!("génération annulée", "generation cancelled")),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return Err(tr!("génération interrompue", "generation interrupted")),
        }
    };
    let v: Value = serde_json::from_str(&raw).map_err(|_| tr!("réponse inattendue du modèle", "unexpected model reply"))?;
    v["choices"][0]["message"]["content"].as_str().map(str::to_string).ok_or_else(|| tr!("réponse vide du modèle", "empty model reply"))
}

/// Ecrit le texte demande et le remet au format VoiceClone. Bloquant (une dizaine de secondes, plus pour un long texte) ;
/// `cancel` l'interrompt. `progress(partie, parties, texte deja ecrit)` est appele avant chaque partie (`None`) puis apres
/// (`Some`) : l'appelant garde ainsi ce qui est ecrit meme si la suite echoue ou est annulee.
pub fn generate(data_dir: &Path, req: &Request, cancel: &AtomicBool, progress: &dyn Fn(u32, u32, Option<&str>)) -> R<String> {
    if req.prompt.trim().is_empty() {
        return Err(tr!("décrivez le texte à écrire", "describe the text to write"));
    }
    if req.mode == Mode::Dialogue && req.voices.len() < 2 {
        return Err(tr!("un dialogue demande au moins deux voix", "a dialogue needs at least two voices"));
    }
    let d = dir(data_dir);
    let exe = transcribe::find_file(&d.join("bin"), "llama-server.exe").ok_or_else(|| tr!("l'assistant d'écriture n'est pas installé (Paramètres > Modèles)", "the writing assistant is not installed (Settings > Models)"))?;
    let model = d.join(MODEL_FILE);
    if !model.is_file() {
        return Err(tr!("le modèle d'écriture manque (Paramètres > Modèles)", "the writing model is missing (Settings > Models)"));
    }
    // Carte graphique d'abord ; si le serveur ne demarre pas (memoire video insuffisante...), on retente sur le processeur.
    let mut server = start(&exe, &model, true)?;
    if let Err(first) = wait_ready(&mut server, Duration::from_secs(90), cancel) {
        if cancel.load(Ordering::Relaxed) {
            return Err(first);
        }
        drop(server);
        server = start(&exe, &model, false)?;
        wait_ready(&mut server, Duration::from_secs(240), cancel)?;
    }
    let (total, system) = (words_for(req.seconds, req.tone), system_prompt(req));
    let parts = total.div_ceil(PART_WORDS).max(1);
    let dialogue = req.mode == Mode::Dialogue;
    let ask = |user: String, max_tokens: u32| chat(server.port, &[json!({"role": "system", "content": system.clone()}), json!({"role": "user", "content": user})], max_tokens, cancel);

    // Plan de l'ensemble des qu'il y a plusieurs parties : il garde le fil et evite les redites sur une longue duree.
    let mut outline: Vec<String> = vec![];
    if parts > 1 {
        progress(0, parts, None);
        let reply = chat(server.port, &[json!({"role": "system", "content": "You are a careful planner. Answer exactly in the requested format."}), json!({"role": "user", "content": outline_prompt(req, parts, total.div_ceil(parts))})], (parts * 60 + 200).min(3000), cancel)?;
        outline = parse_outline(&reply, parts as usize);
    }

    let (mut pieces, mut written, mut tail): (Vec<String>, u32, String) = (vec![], 0, String::new());
    let sep = if dialogue { "\n" } else { "\n\n" };
    for part in 1..=parts {
        progress(part, parts, None);
        // Le petit modele depasse souvent la longueur demandee : on repartit ce qui reste sur les parties restantes.
        let words = (total.saturating_sub(written) / (parts - part + 1)).max(40);
        let prompt = part_prompt(req, part, parts, words, &outline, &tail);
        let mut text = clean_reply(&ask(prompt.clone(), (words * 3 + 150).min(1500))?, &req.voices, dialogue);
        // Dialogue : si trop de lignes n'ont pas de balise de voix (le modele derive vers un monologue), une seule nouvelle tentative.
        if dialogue && tagged_share(&text, &req.voices) < 0.8 {
            let retry = format!("{prompt}

IMPORTANT: every line of speech must start with a speaker tag from the list, and the speakers must alternate.");
            let again = clean_reply(&ask(retry, (words * 3 + 150).min(1500))?, &req.voices, dialogue);
            if tagged_share(&again, &req.voices) > tagged_share(&text, &req.voices) {
                text = again;
            }
        }
        if text.is_empty() {
            return Err(tr!("le modèle n'a rien écrit : reformulez la demande", "the model wrote nothing: rephrase the request"));
        }
        written += count_words(&text);
        tail = tail_of(&text);
        pieces.push(text);
        progress(part, parts, Some(&pieces.join(sep)));
    }
    Ok(pieces.join(sep))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn replies_are_put_back_in_voiceclone_format() {
        let voices = v(&["Marco", "Annie"]);
        let raw = "**Marco** : Bonjour Annie ! (il sourit)\nAnnie: Salut Marco, ça va ?\n[Marco]: Très bien.\n[Narrateur] Il rit.\n- Annie : Moi aussi.\n\n\n\n[Pause 2s]\nBoby : intrus";
        assert_eq!(
            clean_reply(raw, &voices, true),
            "[Marco] Bonjour Annie !\n[Annie] Salut Marco, ça va ?\n[Marco] Très bien.\nIl rit.\n[Annie] Moi aussi.\n\n[Pause 2s]\nBoby : intrus"
        );
    }

    #[test]
    fn story_has_no_tags_at_all() {
        let raw = "# Titre\nIl était **une fois** un chat [Marco]. Il dormait (beaucoup). [pause 1s]\n\n\n\n[pause 999s] Fin [pause 0,5s].";
        assert_eq!(clean_reply(raw, &v(&["Marco"]), false), "Titre\nIl était une fois un chat . Il dormait (beaucoup). [pause 1s]\n\nFin [pause 0,5s].");
        assert_eq!(clean_reply("<think>je réfléchis</think>Bonjour.", &[], false), "Bonjour.");
    }

    fn request(mode: Mode) -> Request {
        Request { prompt: "x".into(), mode, language: "fr".into(), seconds: 60, tone: Tone::Natural, voices: v(&["Marco", "Annie"]) }
    }

    #[test]
    fn duration_becomes_a_number_of_words() {
        assert_eq!(words_for(60, Tone::Natural), 185);
        assert_eq!(words_for(30, Tone::Natural), 93);
        assert_eq!(words_for(60, Tone::Calm), 158, "un recit calme se lit plus lentement");
        assert_eq!(words_for(600, Tone::Cheerful), 1900);
        assert_eq!(words_for(5, Tone::Natural), words_for(15, Tone::Natural), "minimum 15 s");
        assert_eq!(words_for(99999, Tone::Natural), words_for(MAX_SECONDS, Tone::Natural), "maximum 2 heures");
        assert_eq!(count_words("[Marco] Bonjour toi ! [pause 1s] Ça va ?"), 4);
    }

    #[test]
    fn system_prompt_lists_speakers_pauses_and_tone() {
        let s = system_prompt(&request(Mode::Dialogue));
        assert!(s.contains("French") && s.contains("[Marco] [Annie]") && s.contains("[pause 0.5s]") && s.contains("[pause 1s]") && s.contains("intonation"));
        assert!(s.contains("spontaneous") && !s.contains("Calm and thoughtful"));
        let story = system_prompt(&Request { tone: Tone::Calm, language: "en".into(), ..request(Mode::Story) });
        assert!(story.contains("English") && story.contains("single narrator") && story.contains("Calm and thoughtful") && !story.contains("[Marco]"));
        assert_eq!(Tone::parse("dramatic"), Tone::Dramatic);
        assert_eq!(Tone::parse("???"), Tone::Natural);
    }

    #[test]
    fn long_texts_are_written_in_parts_with_an_outline() {
        let one = part_prompt(&request(Mode::Story), 1, 1, 185, &[], "");
        assert!(one.contains("185 words") && !one.contains("part"));
        let r = Request { seconds: 3600, ..request(Mode::Dialogue) };
        let outline: Vec<String> = (1..=3).map(|i| format!("sujet {i}")).collect();
        let (first, mid, last) = (part_prompt(&r, 1, 3, 227, &outline, ""), part_prompt(&r, 2, 3, 227, &outline, "fin de la 1"), part_prompt(&r, 3, 3, 227, &outline, "fin de la 2"));
        assert!(first.contains("60 minutes") && first.contains("1. sujet 1") && first.contains("part 1 of 3 (sujet 1)") && first.contains("Start the text") && first.contains("Do not conclude"));
        assert!(mid.contains("part 2 of 3 (sujet 2)") && mid.contains("\"fin de la 1\"") && mid.contains("without repeating"));
        assert!(last.contains("part 3 of 3 (sujet 3)") && last.contains("natural conclusion") && !last.contains("Do not conclude"));
        // Plan incomplet : on n'en affiche rien plutot qu'un plan faux.
        assert!(!part_prompt(&r, 2, 3, 227, &outline[..2], "x").contains("Outline"));
        // Une heure = 11100 mots = 40 parties de 278 mots ; deux heures : 80 parties.
        let total = words_for(3600, Tone::Natural);
        assert_eq!((total, total.div_ceil(PART_WORDS), words_for(7200, Tone::Natural).div_ceil(PART_WORDS)), (11100, 40, 80));
    }

    #[test]
    fn untagged_dialogue_is_detected() {
        let voices = v(&["Marco", "Annie"]);
        assert_eq!(tagged_share("[Marco] Salut.
[Annie] Oui.
[pause 1s]
[Pause 2s]", &voices), 1.0);
        assert_eq!(tagged_share("[Marco] Salut.
Suite sans balise.
Encore.
[Inconnu] x", &voices), 0.25);
        assert_eq!(tagged_share("", &voices), 1.0);
    }

    #[test]
    fn outline_replies_are_parsed() {
        let reply = "Voici le plan :\n1. Ouverture et présentation\n2) Premier sujet : le cloud\n- 3: Conclusion\n4.\nTexte libre";
        assert_eq!(parse_outline(reply, 10), ["Ouverture et présentation", "Premier sujet : le cloud", "Conclusion"]);
        assert_eq!(parse_outline(reply, 2).len(), 2);
        assert!(parse_outline("rien de numéroté", 5).is_empty());
    }

    #[test]
    fn the_tail_keeps_the_end_of_the_previous_part() {
        assert_eq!(tail_of("court"), "court");
        let long = format!("{}\n[Marco] Dernière réplique.", "mot ".repeat(200));
        let t = tail_of(&long);
        assert!(t.ends_with("Dernière réplique.") && t.chars().count() <= 400, "{t}");
        assert!(t.starts_with("mot"), "coupe a une frontiere de mot : {t}");
        let lines = format!("{}
[Marco] a
[Annie] {}", "xx ".repeat(100), "mot ".repeat(60));
        assert!(tail_of(&lines).starts_with("[Annie]") || tail_of(&lines).starts_with("[Marco]"), "debut de ligne quand il y en a un tot : {}", tail_of(&lines));
    }

    #[test]
    fn requests_are_checked_before_anything_starts() {
        let dir = std::env::temp_dir().join(format!("voiceclone-writer-{}", std::process::id()));
        let cancel = AtomicBool::new(false);
        let none = |_: u32, _: u32, _: Option<&str>| {};
        let mut r = Request { prompt: "  ".into(), voices: vec![], ..request(Mode::Story) };
        assert!(generate(&dir, &r, &cancel, &none).is_err());
        r.prompt = "un récit".into();
        assert!(generate(&dir, &r, &cancel, &none).is_err(), "pas installe");
        r.mode = Mode::Dialogue;
        r.voices = v(&["Seul"]);
        assert!(generate(&dir, &r, &cancel, &none).is_err());
        assert_eq!(status(&dir)["installed"], false);
    }
}
