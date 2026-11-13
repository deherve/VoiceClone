//! Enveloppe sure autour de qwen.dll : modele resident, voix extraite une fois.
use crate::R;
use qwen_sys::*;
use std::{ffi::{CStr, CString}, path::Path};

fn last_err() -> String {
    unsafe { CStr::from_ptr(qt_last_error()).to_string_lossy().into_owned() }
}

fn cstr(s: &str) -> R<CString> {
    CString::new(s).map_err(|_| tr!("caractère nul interdit dans le texte", "null character not allowed in the text"))
}

pub struct Engine(*mut qt_context, usize);
// qwen.h : qt_synthesize est bloquant et thread safe ; l'App le garde en plus derriere un Mutex.
unsafe impl Send for Engine {}
// Plusieurs threads peuvent appeler `synth` en meme temps : le moteur regroupe alors leurs demandes en un seul calcul
// (voir `max_batch`).
unsafe impl Sync for Engine {}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, desc: &str, gpu: bool, integrated: bool) -> Device {
        Device { name: name.into(), description: desc.into(), gpu, integrated, total_mb: 0, free_mb: 0 }
    }

    #[test]
    fn auto_order_prefers_cuda_then_discrete_vulkan() {
        // Machine NVIDIA avec les deux backends : CUDA d'abord, Vulkan en repli, CPU a la fin.
        let nvidia = [dev("CPU", "Ryzen", false, false), dev("Vulkan0", "NVIDIA GeForce RTX 4070", true, false), dev("CUDA0", "NVIDIA GeForce RTX 4070", true, false)];
        assert_eq!(auto_order(&nvidia), ["CUDA0", "Vulkan0", "CPU"]);
        // Portable avec GPU integre AMD et GPU dedie NVIDIA : le dedie passe avant l'integre.
        let laptop = [dev("Vulkan0", "AMD Radeon Graphics", true, true), dev("Vulkan1", "NVIDIA GeForce GTX 1650", true, false), dev("CPU", "i7", false, false)];
        assert_eq!(auto_order(&laptop), ["Vulkan1", "Vulkan0", "CPU"]);
        // Notre machine : un GPU AMD en Vulkan.
        assert_eq!(auto_order(&[dev("Vulkan0", "AMD Radeon RX 6700 XT", true, false), dev("CPU", "Ryzen 7", false, false)]), ["Vulkan0", "CPU"]);
        // Aucun GPU, ou registre vide : le CPU reste toujours possible.
        assert_eq!(auto_order(&[dev("CPU", "x", false, false)]), ["CPU"]);
        assert_eq!(auto_order(&[]), ["CPU"]);
    }

    #[test]
    fn batch_follows_free_video_memory() {
        let gpu = |free: u64| Device { free_mb: free, ..dev("Vulkan0", "AMD Radeon RX 6700 XT", true, false) };
        assert_eq!(choose_batch(Some(&gpu(11500))), 4); // 12 Go libres : maximum
        assert_eq!(choose_batch(Some(&gpu(8800))), 3);
        assert_eq!(choose_batch(Some(&gpu(7600))), 2);
        assert_eq!(choose_batch(Some(&gpu(7000))), 1);
        assert_eq!(choose_batch(Some(&gpu(4000))), 1); // petite carte : une a la fois
        assert_eq!(choose_batch(Some(&gpu(0))), 2); // inconnu
        assert_eq!(choose_batch(Some(&dev("CPU", "Ryzen", false, false))), 1);
        assert_eq!(choose_batch(None), 1);
    }

    #[test]
    fn clamp_only_for_pre_ampere_or_unknown() {
        for old in ["NVIDIA GeForce GTX 1060", "NVIDIA GeForce RTX 2070 SUPER", "Quadro P2000", "Tesla T4", "TITAN V", "carte inconnue"] {
            assert!(needs_clamp(old), "{old}");
        }
        for recent in ["NVIDIA GeForce RTX 3060", "NVIDIA GeForce RTX 4090", "NVIDIA GeForce RTX 5080", "NVIDIA RTX A4000", "NVIDIA A100-SXM4-80GB", "NVIDIA H100", "NVIDIA L40S"] {
            assert!(!needs_clamp(recent), "{recent}");
        }
    }

    #[test]
    fn devices_are_unique_and_include_cpu() {
        let d = devices();
        assert!(d.iter().any(|x| x.name == "CPU" && !x.gpu));
        let mut names: Vec<_> = d.iter().map(|x| x.name.clone()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), d.len(), "un peripherique ne doit apparaitre qu'une fois");
    }
}

/// Conditionnement de voix reutilisable (copie des latents, independante du moteur).
pub struct Voice {
    spk: Vec<f32>,
    codes: Vec<i32>,
    t: i32,
    // None : pas de transcription, clonage par timbre seul (x-vector), moins fidele.
    ref_text: Option<CString>,
}

/// Nombre de syntheses calculees ensemble (`max_batch` du moteur), selon la memoire video libre.
/// Mesure sur RX 6700 XT (12 Go), texte de 12 minutes : 1 → 154 s et 5,2 Go de memoire video ; 2 → 107 s, 6,1 Go ; 3 → 92 s,
/// 7,0 Go ; 4 → 84 s, 7,9 Go ; 6 → 113 s, 9,0 Go (plus lent : la carte est saturee). Chaque place coute environ 0,9 Go ;
/// on garde 1,5 Go de marge. Le processeur (pas de memoire video) calcule une synthese a la fois.
/// `VOICECLONE_BATCH` impose une valeur (essais).
pub fn choose_batch(dev: Option<&Device>) -> i32 {
    if let Some(v) = std::env::var("VOICECLONE_BATCH").ok().and_then(|v| v.parse::<i32>().ok()) {
        return v.clamp(1, 8);
    }
    let Some(d) = dev.filter(|d| d.gpu) else { return 1 };
    if d.free_mb == 0 {
        return 2; // memoire libre inconnue : prudence
    }
    let spare = d.free_mb as i64 - 5200 - 1500;
    (1 + spare / 900).clamp(1, 4) as i32
}

/// Peripherique de calcul vu par ggml : "CPU", "Vulkan0", "CUDA0"...
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub name: String,
    pub description: String,
    /// GPU dedie ou integre (faux pour le processeur).
    pub gpu: bool,
    /// GPU integre (partage la memoire du systeme).
    pub integrated: bool,
    pub total_mb: u64,
    pub free_mb: u64,
}

unsafe extern "C" fn mute(_level: std::os::raw::c_int, _msg: *const std::os::raw::c_char, _user: *mut std::os::raw::c_void) {}

/// Coupe les journaux du moteur et de ggml (la ligne de commande n'affiche que le chemin du fichier et un resume ;
/// les erreurs restent renvoyees par les fonctions, pas par le journal). A appeler avant le premier chargement.
pub fn silence_logs() {
    // SAFETY : `mute` est une fonction `extern "C"` sans etat, valable pour toute la duree du processus.
    unsafe {
        ggml_log_set(Some(mute), std::ptr::null_mut());
        qt_log_set(Some(mute), std::ptr::null_mut());
    }
}

/// Liste les peripheriques disponibles (une seule entree par nom). Charge les backends ggml au premier appel :
/// les DLL ggml-vulkan.dll / ggml-cuda.dll presentes a cote de l'executable.
pub fn devices() -> Vec<Device> {
    static LOAD: std::sync::Once = std::sync::Once::new();
    LOAD.call_once(|| {
        // Bibliotheques CUDA installees depuis Parametres (<donnees>\cuda) : ggml-cuda.dll les y trouve a son chargement.
        if let Some(dir) = crate::data_dir().ok().map(|d| crate::components::cuda_dir(&d)).filter(|d| d.is_dir()) {
            use std::{iter::once, os::windows::ffi::OsStrExt};
            extern "system" {
                fn SetDllDirectoryW(path: *const u16) -> i32;
            }
            let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(once(0)).collect();
            // SAFETY : chaine UTF-16 terminee par zero, vivante pendant l'appel.
            unsafe { SetDllDirectoryW(wide.as_ptr()) };
        }
        unsafe { ggml_backend_load_all() }
    });
    let text = |p: *const std::os::raw::c_char| unsafe { if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() } };
    let mut out: Vec<Device> = vec![];
    unsafe {
        for i in 0..ggml_backend_dev_count() {
            let d = ggml_backend_dev_get(i);
            let name = text(ggml_backend_dev_name(d));
            // Le moteur recharge les backends a son tour : le registre peut alors lister un peripherique deux fois.
            if name.is_empty() || out.iter().any(|x| x.name == name) {
                continue;
            }
            let (mut free, mut total) = (0usize, 0usize);
            ggml_backend_dev_memory(d, &mut free, &mut total);
            let kind = ggml_backend_dev_type(d);
            out.push(Device {
                name,
                description: text(ggml_backend_dev_description(d)),
                gpu: kind == GGML_DEV_GPU || kind == GGML_DEV_IGPU,
                integrated: kind == GGML_DEV_IGPU,
                total_mb: (total >> 20) as u64,
                free_mb: (free >> 20) as u64,
            });
        }
    }
    out
}

/// Ordre d'essai en mode automatique : CUDA (NVIDIA, le plus rapide), puis les autres accelerateurs
/// (ROCm/HIP), puis Vulkan (GPU dedie avant GPU integre), puis le processeur en dernier recours.
pub fn auto_order(devs: &[Device]) -> Vec<String> {
    let rank = |d: &Device| -> u8 {
        let n = d.name.to_ascii_lowercase();
        match () {
            _ if !d.gpu => 9,
            _ if n.starts_with("cuda") => 0,
            _ if n.starts_with("rocm") || n.starts_with("hip") => 1,
            _ if n.starts_with("vulkan") && !d.integrated => 2,
            _ if n.starts_with("vulkan") => 4,
            _ if d.integrated => 4,
            _ => 3,
        }
    };
    let mut v: Vec<&Device> = devs.iter().collect();
    v.sort_by_key(|d| rank(d)); // tri stable : l'ordre de ggml departage les egalites
    let mut names: Vec<String> = v.iter().map(|d| d.name.clone()).collect();
    if !names.iter().any(|n| n == "CPU") {
        names.push("CPU".into()); // le CPU est toujours disponible, meme si ggml ne le liste pas
    }
    names
}

/// Les cartes NVIDIA anterieures a Ampere (GTX, RTX 20, Quadro, Tesla, Titan) peuvent deborder en FP16 :
/// qwen.h conseille alors le « clamp ». Activer le clamp par erreur est sans danger (un peu plus lent),
/// l'oublier peut abimer le son : on l'active donc sauf pour les familles Ampere et plus recentes reconnues.
pub fn needs_clamp(description: &str) -> bool {
    let d = description.to_ascii_uppercase();
    let recent = ["RTX 30", "RTX 40", "RTX 50", "RTX A", "A100", "A10", "A30", "A40", "H100", "H200", "L4", "L40", "B100", "B200", "GB200"];
    !recent.iter().any(|r| d.contains(r))
}

impl Engine {
    /// Charge le modele sur `device` (nom ggml). `use_fa` : attention « flash » ; `clamp` : garde FP16.
    /// Nombre de syntheses que le moteur calcule ensemble (voir `choose_batch`).
    pub fn batch(&self) -> usize {
        self.1
    }

    /// `batch` : nombre de syntheses calculees ensemble sur la carte (1 = une a la fois).
    pub fn load(talker: &Path, codec: &Path, device: &str, use_fa: bool, clamp: bool, batch: i32) -> R<Self> {
        for p in [talker, codec] {
            if !p.exists() {
                return Err(tr!("modèle absent ({}) : téléchargez-le dans Paramètres", "model missing ({}): download it in Settings", p.file_name().unwrap_or_default().to_string_lossy()));
            }
        }
        let (t, c) = (cstr(&talker.to_string_lossy())?, cstr(&codec.to_string_lossy())?);
        let device = cstr(device)?;
        unsafe {
            let mut p: qt_init_params = std::mem::zeroed();
            qt_init_default_params(&mut p);
            if p.abi_version != QT_ABI_VERSION {
                return Err(tr!("ABI qwen {} différente de la liaison Rust {}", "qwen ABI {} differs from the Rust binding {}", p.abi_version, QT_ABI_VERSION));
            }
            p.talker_path = t.as_ptr();
            p.codec_path = c.as_ptr();
            p.device = device.as_ptr();
            p.use_fa = use_fa;
            p.clamp_fp16 = clamp;
            p.max_batch = batch.max(1);
            let q = qt_init(&p);
            if q.is_null() { Err(tr!("chargement du modèle : {}", "loading the model: {}", last_err())) } else { Ok(Engine(q, batch.max(1) as usize)) }
        }
    }

    /// `pcm` : mono 24 kHz. `ref_text` : transcription exacte de l'echantillon, si on l'a.
    pub fn voice(&self, pcm: &[f32], ref_text: Option<&str>) -> R<Voice> {
        unsafe {
            let mut v: qt_voice_ref = std::mem::zeroed();
            if qt_extract_voice_ref(self.0, pcm.as_ptr(), pcm.len() as i32, &mut v) != QT_STATUS_OK {
                return Err(tr!("extraction de la voix : {}", "extracting the voice: {}", last_err()));
            }
            let out = Voice {
                spk: std::slice::from_raw_parts(v.ref_spk_emb, v.ref_spk_dim as usize).to_vec(),
                codes: std::slice::from_raw_parts(v.ref_codes, (v.num_codebooks * v.ref_t) as usize).to_vec(),
                t: v.ref_t,
                ref_text: ref_text.map(cstr).transpose()?,
            };
            qt_voice_ref_free(&mut v);
            Ok(out)
        }
    }

    /// Retourne du PCM mono 24 kHz.
    pub fn synth(&self, text: &str, lang: &str, v: &Voice, seed: i64) -> R<Vec<f32>> {
        let (text, lang) = (cstr(text)?, cstr(lang)?);
        unsafe {
            let mut p: qt_tts_params = std::mem::zeroed();
            qt_tts_default_params(&mut p);
            p.text = text.as_ptr();
            p.lang = lang.as_ptr();
            p.ref_spk_emb = v.spk.as_ptr();
            p.ref_spk_dim = v.spk.len() as i32;
            if let Some(t) = &v.ref_text {
                // Mode ICL : transcription + codes de la reference.
                p.ref_text = t.as_ptr();
                p.ref_codes = v.codes.as_ptr();
                p.ref_t = v.t;
            }
            p.seed = seed;
            let mut out: qt_audio = std::mem::zeroed();
            if qt_synthesize(self.0, &p, &mut out) != QT_STATUS_OK {
                return Err(tr!("synthèse : {}", "synthesis: {}", last_err()));
            }
            // Le moteur peut repondre « OK » sans aucun son (texte sans rien a dire) : pointeur nul, longueur 0.
            let pcm = if out.samples.is_null() || out.n_samples <= 0 { vec![] } else { std::slice::from_raw_parts(out.samples, out.n_samples as usize).to_vec() };
            qt_audio_free(&mut out);
            Ok(pcm)
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe { qt_free(self.0) }
    }
}
