//! Preparation du texte avant la synthese : dictionnaire de prononciation de l'utilisateur, puis nombres ecrits en
//! toutes lettres (francais et anglais ; les autres langues restent telles quelles). Le texte affiche (historique,
//! sous-titres) n'est jamais modifie : seul ce qui est envoye au modele l'est.

// ---------------------------------------------------------------- dictionnaire
/// Remplace chaque mot ou groupe de `dict` (sans tenir compte de la casse, aux frontieres de mots). Le plus long d'abord.
pub fn apply_dictionary(text: &str, dict: &[(String, String)]) -> String {
    replace_words(text, dict, false)
}

/// `cased` : si le mot d'origine commence par une majuscule, la lecture aussi (« Rust » → « Reuste »).
fn replace_words(text: &str, dict: &[(String, String)], cased: bool) -> String {
    let mut entries: Vec<(Vec<char>, &str)> = dict.iter().filter(|(f, _)| !f.trim().is_empty()).map(|(f, t)| (f.trim().chars().collect(), t.as_str())).collect();
    entries.sort_by_key(|(f, _)| std::cmp::Reverse(f.len()));
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'scan: while i < chars.len() {
        let word_start = i == 0 || !chars[i - 1].is_alphanumeric();
        if word_start {
            for (from, to) in &entries {
                let end = i + from.len();
                if end <= chars.len()
                    && chars[i..end].iter().zip(from).all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
                    && (end == chars.len() || !chars[end].is_alphanumeric() || !from.last().is_some_and(|c| c.is_alphanumeric()))
                {
                    match to.chars().next() {
                        Some(c) if cased && chars[i].is_uppercase() && chars.get(i + 1).map_or(true, |n| !n.is_uppercase()) && c.is_lowercase() => {
                            out.extend(c.to_uppercase());
                            out.push_str(&to[c.len_utf8()..]);
                        }
                        _ => out.push_str(to),
                    }
                    i = end;
                    continue 'scan;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

// ---------------------------------------------------------------- version phonetique (francais)
/// Mots d'origine anglaise (ou techniques) que le modele lit « a l'anglaise » : ecrits comme un francophone les dit.
/// Volontairement sans les mots qui sont aussi du francais (code, design, test, mail...) : ils sont deja bien lus.
const LEXICON_FR: &[(&str, &str)] = &[
    ("cloud", "claoude"), ("rust", "reuste"), ("software", "softouère"), ("hardware", "hardouère"), ("streaming", "strimingue"), ("streamer", "strimeur"),
    ("marketing", "marketingue"), ("meeting", "mitingue"), ("feedback", "fidbaque"), ("startup", "starteupe"), ("podcast", "podcaste"),
    ("bug", "beugue"), ("debug", "débeugue"), ("debugger", "débeugueur"), ("framework", "frèmeweurk"), ("backend", "baquènde"), ("frontend", "fronteunde"),
    ("open source", "opène source"), ("github", "guite-heub"), ("linux", "linuxe"), ("windows", "ouindoze"), ("google", "gougueul"), ("youtube", "youtioube"),
    ("python", "païtone"), ("javascript", "djavascripte"), ("typescript", "taïpscripte"), ("tauri", "tori"), ("vulkan", "vulcane"), ("nvidia", "ènevidia"),
    ("radeon", "radéone"), ("whisper", "ouisper"), ("token", "tokène"), ("prompt", "prompte"), ("email", "imèle"), ("e-mail", "imèle"),
    ("week-end", "ouiquènde"), ("weekend", "ouiquènde"), ("manager", "manadjeur"), ("shopping", "chopingue"), ("smartphone", "smarteufône"),
    ("laptop", "laptoppe"), ("hashtag", "hachetague"), ("selfie", "sèlefi"), ("follower", "followeur"), ("online", "onelaïne"), ("offline", "offlaïne"),
    ("download", "daounelode"), ("upload", "eupelode"), ("setup", "sèteupe"), ("update", "eupdète"), ("login", "loguine"), ("password", "passeweurde"),
    ("wifi", "ouifi"), ("bluetooth", "bloutouthe"), ("gaming", "guèmingue"), ("gamer", "guèmeur"), ("business", "bizenesse"), ("coaching", "cotchingue"),
    ("freelance", "frilance"), ("newsletter", "niouzeletteur"), ("cookie", "couki"), ("cookies", "coukiz"), ("spam", "spame"), ("thread", "thrède"),
    ("commit", "comite"), ("repository", "répozitori"), ("pull request", "poule rekouèste"), ("merge", "meurdje"), ("docker", "doqueur"),
    ("kubernetes", "koubernétèsse"), ("api", "a pé i"), ("cpu", "cé pé u"), ("gpu", "gé pé u"), ("ram", "rame"), ("buzz", "beuze"), ("fake", "féïque"),
    ("chatgpt", "tchatte gé pé té"), ("openai", "opène é aï"), ("remotion", "rimocheune"), ("hyperframes", "aïpeurfrèmes"), ("ggml", "gé gé èm èl"),
];

/// Sigles qui se lisent comme des mots : on ne les epelle pas.
const ACRONYM_WORDS: &[&str] = &["OK", "STOP", "NASA", "OTAN", "ONU", "SIDA", "UNESCO", "LASER", "RADAR", "WIFI", "ASAP", "COVID", "OVNI", "PACS"];

const LETTERS_FR: [&str; 26] = [
    "a", "bé", "cé", "dé", "e", "èf", "gé", "ache", "i", "ji", "ka", "èl", "èm", "èn", "o", "pé", "ku", "èr", "ès", "té", "u", "vé", "double vé", "iks", "i grec", "zèd",
];

/// « GPU » → « gé pé u » : mots de 2 a 6 majuscules (hors sigles qui se lisent comme des mots, hors chiffres romains de 3 lettres ou plus).
fn spell_acronyms(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        let start_ok = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
        if start_ok && chars[i].is_ascii_uppercase() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_uppercase() {
                j += 1;
            }
            let word: String = chars[i..j].iter().collect();
            let glued = chars.get(j).is_some_and(|c| c.is_alphanumeric() || *c == '_');
            let roman = word.len() >= 3 && word.chars().all(|c| "IVXLCDM".contains(c));
            if !glued && (2..=6).contains(&word.len()) && !roman && !ACRONYM_WORDS.contains(&word.as_str()) {
                out.push_str(&word.chars().map(|c| LETTERS_FR[(c as u8 - b'A') as usize]).collect::<Vec<_>>().join(" "));
                i = j;
                continue;
            }
            out.extend(&chars[i..j]);
            i = j;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Applique `f` au texte hors balises : « [Marco] », « [pause 2s] » restent tels quels.
fn outside_tags(text: &str, f: impl Fn(&str) -> String) -> String {
    let (mut out, mut rest) = (String::with_capacity(text.len() + 16), text);
    while let Some(open) = rest.find('[') {
        match rest[open..].find(']') {
            Some(len) if len <= 61 && !rest[open + 1..open + len].contains(['\n', '[']) => {
                out.push_str(&f(&rest[..open]));
                out.push_str(&rest[open..=open + len]);
                rest = &rest[open + len + 1..];
            }
            _ => {
                out.push_str(&f(&rest[..=open]));
                rest = &rest[open + 1..];
            }
        }
    }
    out.push_str(&f(rest));
    out
}

/// Version « phonetique » d'un texte francais : le dictionnaire de l'utilisateur d'abord, puis mots anglais respeles,
/// sigles epeles et nombres en lettres. Les balises (voix, pauses) ne sont pas touchees.
pub fn phonetic_fr(text: &str, dict: &[(String, String)]) -> String {
    let lexicon: Vec<(String, String)> = LEXICON_FR.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    outside_tags(text, |seg| numbers_to_words(&spell_acronyms(&replace_words(&apply_dictionary(seg, dict), &lexicon, true)), "fr"))
}

// ---------------------------------------------------------------- nombres en toutes lettres
const FR_UNITS: [&str; 20] = ["zéro", "un", "deux", "trois", "quatre", "cinq", "six", "sept", "huit", "neuf", "dix", "onze", "douze", "treize", "quatorze", "quinze", "seize", "dix-sept", "dix-huit", "dix-neuf"];
const FR_TENS: [&str; 7] = ["", "", "vingt", "trente", "quarante", "cinquante", "soixante"];

/// 0..=99. `last` : rien ne suit le nombre (« quatre-vingts » prend son s seulement a la fin).
fn fr_99(n: u64, last: bool) -> String {
    match n {
        0..=19 => FR_UNITS[n as usize].to_string(),
        20..=69 => {
            let (t, u) = (FR_TENS[(n / 10) as usize], n % 10);
            match u {
                0 => t.to_string(),
                1 => format!("{t} et un"),
                _ => format!("{t}-{}", FR_UNITS[u as usize]),
            }
        }
        70..=79 => if n == 71 { "soixante et onze".to_string() } else { format!("soixante-{}", FR_UNITS[(n - 60) as usize]) },
        80 => if last { "quatre-vingts".to_string() } else { "quatre-vingt".to_string() },
        _ => format!("quatre-vingt-{}", FR_UNITS[(n - 80) as usize]),
    }
}

/// 0..=999.
fn fr_999(n: u64, last: bool) -> String {
    let (h, r) = (n / 100, n % 100);
    if h == 0 {
        return fr_99(r, last);
    }
    let cent = match (h, r) {
        (1, _) => "cent".to_string(),
        (_, 0) if last => format!("{} cents", FR_UNITS[h as usize]),
        _ => format!("{} cent", FR_UNITS[h as usize]),
    };
    if r == 0 { cent } else { format!("{cent} {}", fr_99(r, last)) }
}

fn fr_number(n: u64) -> Option<String> {
    if n == 0 {
        return Some("zéro".into());
    }
    if n >= 1_000_000_000_000 {
        return None;
    }
    let mut parts: Vec<String> = vec![];
    for (div, one, many) in [(1_000_000_000u64, "un milliard", "milliards"), (1_000_000, "un million", "millions")] {
        let g = n / div % 1000;
        if g == 1 {
            parts.push(one.into());
        } else if g > 1 {
            parts.push(format!("{} {many}", fr_999(g, false)));
        }
    }
    let (thousands, rest) = (n / 1000 % 1000, n % 1000);
    match thousands {
        0 => {}
        1 => parts.push("mille".into()),
        g => parts.push(format!("{} mille", fr_999(g, false))),
    }
    if rest > 0 {
        parts.push(fr_999(rest, true));
    }
    Some(parts.join(" "))
}

const EN_UNITS: [&str; 20] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"];
const EN_TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

fn en_999(n: u64) -> String {
    let (h, r) = (n / 100, n % 100);
    let tail = match r {
        0 => String::new(),
        1..=19 => EN_UNITS[r as usize].to_string(),
        _ if r % 10 == 0 => EN_TENS[(r / 10) as usize].to_string(),
        _ => format!("{}-{}", EN_TENS[(r / 10) as usize], EN_UNITS[(r % 10) as usize]),
    };
    match (h, tail.is_empty()) {
        (0, _) => tail,
        (_, true) => format!("{} hundred", EN_UNITS[h as usize]),
        _ => format!("{} hundred {tail}", EN_UNITS[h as usize]),
    }
}

fn en_number(n: u64) -> Option<String> {
    if n == 0 {
        return Some("zero".into());
    }
    if n >= 1_000_000_000_000 {
        return None;
    }
    let mut parts = vec![];
    for (div, name) in [(1_000_000_000u64, "billion"), (1_000_000, "million"), (1000, "thousand")] {
        let g = n / div % 1000;
        if g > 0 {
            parts.push(format!("{} {name}", en_999(g)));
        }
    }
    if n % 1000 > 0 {
        parts.push(en_999(n % 1000));
    }
    Some(parts.join(" "))
}

fn words(n: u64, fr: bool) -> Option<String> {
    if fr { fr_number(n) } else { en_number(n) }
}

fn digits_one_by_one(s: &str, fr: bool) -> String {
    s.chars().filter_map(|c| c.to_digit(10)).map(|d| if fr { FR_UNITS[d as usize] } else { EN_UNITS[d as usize] }).collect::<Vec<_>>().join(" ")
}

/// Ecrit en toutes lettres les nombres isoles (« 12 », « 3,5 », « 1 000 », « 50 % », « 5 € »). Laisse tel quel ce qui est
/// colle a des lettres (« MP3 », « 1er »), les dates et heures (« 12/05 », « 10:30 »), les numeros de version (« 1.9.5 »).
/// `lang` : code court (« fr », « en ») ; toute autre langue : texte inchange.
pub fn numbers_to_words(text: &str, lang: &str) -> String {
    let fr = match lang {
        "fr" => true,
        "en" => false,
        _ => return text.to_string(),
    };
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        if i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            // Chiffres colles a des lettres (« v1.9.5 », « MP3 ») : on garde toute la suite telle quelle.
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || (matches!(chars[j], '.' | ',') && chars.get(j + 1).is_some_and(char::is_ascii_digit))) {
                j += 1;
            }
            out.extend(&chars[i..j]);
            i = j;
            continue;
        }
        // Un nombre : chiffres, groupes de 3 separes par une espace (« 1 000 »), puis partie decimale (« ,5 » ou « .5 »).
        let start = i;
        let mut j = i;
        let mut int_part = String::new();
        while j < chars.len() && chars[j].is_ascii_digit() {
            int_part.push(chars[j]);
            j += 1;
        }
        if int_part.len() <= 3 {
            while j + 4 <= chars.len()
                && matches!(chars[j], ' ' | '\u{a0}' | '\u{202f}')
                && chars[j + 1..j + 4].iter().all(char::is_ascii_digit)
                && chars.get(j + 4).map_or(true, |c| !c.is_ascii_digit())
            {
                int_part.extend(&chars[j + 1..j + 4]);
                j += 4;
            }
        }
        let mut frac = String::new();
        let mut k = j;
        if k + 1 < chars.len() && matches!(chars[k], ',' | '.') && chars[k + 1].is_ascii_digit() {
            k += 1;
            while k < chars.len() && chars[k].is_ascii_digit() {
                frac.push(chars[k]);
                k += 1;
            }
            j = k;
        }
        let next = chars.get(j).copied();
        let prev = if start > 0 { Some(chars[start - 1]) } else { None };
        let glued = next.is_some_and(|c| c.is_alphanumeric() || c == '_');
        let datetime = matches!(next, Some('/' | ':')) || matches!(prev, Some('/' | ':'))
            || (matches!(next, Some('-')) && chars.get(j + 1).is_some_and(char::is_ascii_digit))
            || (matches!(prev, Some('-')) && start >= 2 && chars[start - 2].is_ascii_digit());
        let version = matches!(next, Some('.' | ',')) && chars.get(j + 1).is_some_and(char::is_ascii_digit); // « 1.9.5 »
        if glued || datetime || version {
            // « 1.9.5 », « 2020-2021 » : on garde toute la suite de chiffres et de separateurs.
            while j < chars.len() && (chars[j].is_ascii_digit() || (matches!(chars[j], '.' | ',') && chars.get(j + 1).is_some_and(char::is_ascii_digit))) {
                j += 1;
            }
            out.extend(&chars[start..j]);
            i = j;
            continue;
        }
        let int_value: Option<u64> = int_part.parse().ok();
        let leading_zero = int_part.len() > 1 && int_part.starts_with('0') && frac.is_empty();
        let spoken = if leading_zero {
            Some(digits_one_by_one(&int_part, fr))
        } else {
            int_value.and_then(|n| words(n, fr)).map(|mut w| {
                if !frac.is_empty() {
                    let tail = match (frac.starts_with('0'), frac.len() <= 2, frac.parse::<u64>().ok().and_then(|v| words(v, fr))) {
                        (false, true, Some(v)) => v,
                        _ => digits_one_by_one(&frac, fr),
                    };
                    w = format!("{w} {} {tail}", if fr { "virgule" } else { "point" });
                }
                w
            })
        };
        let Some(mut spoken) = spoken else {
            out.extend(&chars[start..j]);
            i = j;
            continue;
        };
        // Symbole colle (ou a une espace) : %, €, $.
        let mut after = j;
        if chars.get(after) == Some(&' ') && matches!(chars.get(after + 1), Some('%' | '€' | '$')) {
            after += 1;
        }
        let one = int_value == Some(1) && frac.is_empty();
        match chars.get(after) {
            Some('%') => {
                spoken.push_str(if fr { " pour cent" } else { " percent" });
                j = after + 1;
            }
            Some('€') => {
                spoken.push_str(if one { " euro" } else { " euros" });
                j = after + 1;
            }
            Some('$') => {
                spoken.push_str(if one { " dollar" } else { " dollars" });
                j = after + 1;
            }
            _ => {}
        }
        out.push_str(&spoken);
        i = j;
    }
    out
}

/// Dictionnaire d'abord (il garde la priorite : « RTX 3060 » peut y etre lu a sa facon), puis les nombres.
pub fn normalize(text: &str, dict: &[(String, String)], numbers: bool, lang: &str) -> String {
    let t = apply_dictionary(text, dict);
    if numbers { numbers_to_words(&t, lang) } else { t }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fr(n: &str) -> String {
        numbers_to_words(n, "fr")
    }

    #[test]
    fn french_numbers() {
        for (n, w) in [
            ("0", "zéro"), ("1", "un"), ("16", "seize"), ("21", "vingt et un"), ("22", "vingt-deux"), ("70", "soixante-dix"), ("71", "soixante et onze"),
            ("77", "soixante-dix-sept"), ("80", "quatre-vingts"), ("81", "quatre-vingt-un"), ("91", "quatre-vingt-onze"), ("99", "quatre-vingt-dix-neuf"),
            ("100", "cent"), ("101", "cent un"), ("200", "deux cents"), ("280", "deux cent quatre-vingts"), ("999", "neuf cent quatre-vingt-dix-neuf"),
            ("1000", "mille"), ("1001", "mille un"), ("2000", "deux mille"), ("2026", "deux mille vingt-six"), ("1980", "mille neuf cent quatre-vingts"),
            ("80000", "quatre-vingt mille"), ("200000", "deux cent mille"), ("1000000", "un million"), ("2000000", "deux millions"),
            ("3500000", "trois millions cinq cent mille"), ("1000000000", "un milliard"), ("12345678", "douze millions trois cent quarante-cinq mille six cent soixante-dix-huit"),
        ] {
            assert_eq!(fr(n), w, "{n}");
        }
    }

    #[test]
    fn english_numbers() {
        for (n, w) in [("0", "zero"), ("13", "thirteen"), ("21", "twenty-one"), ("100", "one hundred"), ("342", "three hundred forty-two"), ("2026", "two thousand twenty-six"), ("1000000", "one million")] {
            assert_eq!(numbers_to_words(n, "en"), w, "{n}");
        }
        assert_eq!(numbers_to_words("3.5", "en"), "three point five");
    }

    #[test]
    fn numbers_in_sentences() {
        assert_eq!(fr("Il y a 3 chats et 12 chiens."), "Il y a trois chats et douze chiens.");
        assert_eq!(fr("Environ 1 000 personnes, soit 3,5 %."), "Environ mille personnes, soit trois virgule cinq pour cent.");
        assert_eq!(fr("Pour 5 € ou 1 €, puis 20$."), "Pour cinq euros ou un euro, puis vingt dollars.");
        assert_eq!(fr("Le 0,05 et le 2,25."), "Le zéro virgule zéro cinq et le deux virgule vingt-cinq.");
        assert_eq!(fr("En 2026."), "En deux mille vingt-six.");
        assert_eq!(fr("Appelez le 0612 puis 007."), "Appelez le zéro six un deux puis zéro zéro sept.");
        assert_eq!(fr("1 000 000 de fois"), "un million de fois");
        assert_eq!(fr("1 0000"), "un zéro zéro zéro zéro"); // pas un groupe de 3 : « 1 » puis « 0000 »
    }

    #[test]
    fn things_that_must_stay_as_they_are() {
        for keep in ["MP3 et H264", "le 1er", "v1.9.5", "12/05/2026", "10:30", "2020-2021", "RTX3060", "x_1", "99999999999999999999"] {
            assert_eq!(fr(keep), keep, "{keep}");
        }
        assert_eq!(numbers_to_words("12 chats", "de"), "12 chats"); // langue non geree
    }

    #[test]
    fn phonetic_version_of_french_text() {
        assert_eq!(phonetic_fr("Le cloud et la GPU de Rust.", &[]), "Le claoude et la gé pé u de Reuste."); // nom propre : majuscule gardee
        assert_eq!(phonetic_fr("Rust, rust et RUST", &[]), "Reuste, reuste et reuste"); // majuscule initiale gardee ; « RUST » = mot du lexique
        assert_eq!(phonetic_fr("Mon PC a 16 Go de RAM.", &[]), "Mon pé cé a seize Go de rame."); // « Go » n'est pas un sigle : minuscule
        assert_eq!(phonetic_fr("Louis XIV et l'OTAN, OK ? STOP.", &[]), "Louis XIV et l'OTAN, OK ? STOP."); // chiffre romain et sigles-mots gardes
        assert_eq!(phonetic_fr("USB3, v2 et RTX3060", &[]), "USB3, v2 et RTX3060"); // colles a des chiffres : intacts
        // Le dictionnaire de l'utilisateur gagne sur le lexique.
        let dict = vec![("rust".to_string(), "rosteu".to_string())];
        assert_eq!(phonetic_fr("Rust", &dict), "rosteu");
        // Les balises ne sont pas touchees, meme si le nom de voix ressemble a un mot du lexique.
        assert_eq!(phonetic_fr("[Cloud] Salut 3 fois. [pause 2s] [GPU] ok", &[]), "[Cloud] Salut trois fois. [pause 2s] [GPU] ok");
        assert_eq!(phonetic_fr("a [b", &[]), "a [b");
        // Deja phonetique : stable.
        let once = phonetic_fr("Le cloud de Google, 12 GPU.", &[]);
        assert_eq!(phonetic_fr(&once, &[]), once);
    }

    #[test]
    fn dictionary_replaces_whole_words_longest_first() {
        let dict = vec![("RTX".to_string(), "R T X".to_string()), ("RTX 3060".to_string(), "R T X trente soixante".to_string()), ("gpu".to_string(), "G P U".to_string())];
        assert_eq!(apply_dictionary("Ma RTX 3060 et ma rtx, pas RTXS ni un gpu.", &dict), "Ma R T X trente soixante et ma R T X, pas RTXS ni un G P U.");
        assert_eq!(apply_dictionary("rien", &dict), "rien");
        assert_eq!(apply_dictionary("x", &[("".to_string(), "y".to_string())]), "x"); // entree vide ignoree
        // Le dictionnaire passe avant les nombres.
        assert_eq!(normalize("RTX 3060 : 12 Go", &dict, true, "fr"), "R T X trente soixante : douze Go");
        assert_eq!(normalize("12", &[], false, "fr"), "12");
    }
}
