//! Liaison FFI vers vendor/qwentts/src/qwen.h (ABI 6).
// shortcut: structs recopiees a la main depuis qwen.h, a regenerer avec bindgen
// si QT_ABI_VERSION change (le test `abi` ci-dessous echoue dans ce cas).
#![allow(non_camel_case_types)]
use std::os::raw::{c_char, c_int, c_void};

pub const QT_ABI_VERSION: c_int = 6;
pub const QT_STATUS_OK: c_int = 0;

#[repr(C)]
pub struct qt_context {
    _p: [u8; 0],
}

#[repr(C)]
pub struct qt_audio {
    pub samples: *mut f32,
    pub n_samples: c_int,
    pub sample_rate: c_int,
    pub channels: c_int,
}

#[repr(C)]
pub struct qt_init_params {
    pub abi_version: c_int,
    pub talker_path: *const c_char,
    pub codec_path: *const c_char,
    pub use_fa: bool,
    pub clamp_fp16: bool,
    pub max_batch: c_int,
    pub codec_chunk_sec: f32,
    pub device: *const c_char,
}

#[repr(C)]
pub struct qt_voice_ref {
    pub ref_spk_emb: *mut f32,
    pub ref_spk_dim: c_int,
    pub ref_codes: *mut i32,
    pub ref_t: c_int,
    pub num_codebooks: c_int,
}

pub type qt_cancel_cb = Option<unsafe extern "C" fn(*mut c_void) -> bool>;
pub type qt_audio_chunk_cb = Option<unsafe extern "C" fn(*const f32, c_int, *mut c_void) -> bool>;

#[repr(C)]
pub struct qt_tts_params {
    pub abi_version: c_int,
    pub text: *const c_char,
    pub lang: *const c_char,
    pub instruct: *const c_char,
    pub speaker: *const c_char,
    pub ref_audio_24k: *const f32,
    pub ref_n_samples: c_int,
    pub ref_text: *const c_char,
    pub seed: i64,
    pub max_new_tokens: c_int,
    pub temperature: f32,
    pub top_k: c_int,
    pub top_p: f32,
    pub repetition_penalty: f32,
    pub subtalker_temperature: f32,
    pub subtalker_top_k: c_int,
    pub subtalker_top_p: f32,
    pub dump_dir: *const c_char,
    pub cancel: qt_cancel_cb,
    pub cancel_user_data: *mut c_void,
    pub on_chunk: qt_audio_chunk_cb,
    pub on_chunk_user_data: *mut c_void,
    pub ref_spk_emb: *const f32,
    pub ref_spk_dim: c_int,
    pub ref_codes: *const i32,
    pub ref_t: c_int,
}

// Registre des peripheriques de calcul de ggml (CPU, Vulkan0, CUDA0...), exporte par ggml.dll / ggml-base.dll.
pub type ggml_backend_dev_t = *mut c_void;
pub const GGML_DEV_CPU: c_int = 0;
pub const GGML_DEV_GPU: c_int = 1;
pub const GGML_DEV_IGPU: c_int = 2;

extern "C" {
    pub fn ggml_backend_load_all();
    pub fn ggml_backend_dev_count() -> usize;
    pub fn ggml_backend_dev_get(i: usize) -> ggml_backend_dev_t;
    pub fn ggml_backend_dev_name(d: ggml_backend_dev_t) -> *const c_char;
    pub fn ggml_backend_dev_description(d: ggml_backend_dev_t) -> *const c_char;
    pub fn ggml_backend_dev_memory(d: ggml_backend_dev_t, free: *mut usize, total: *mut usize);
    pub fn ggml_backend_dev_type(d: ggml_backend_dev_t) -> c_int;

    pub fn qt_version() -> *const c_char;
    pub fn qt_last_error() -> *const c_char;
    pub fn qt_audio_free(a: *mut qt_audio);
    pub fn qt_init_default_params(p: *mut qt_init_params);
    pub fn qt_init(p: *const qt_init_params) -> *mut qt_context;
    pub fn qt_free(q: *mut qt_context);
    pub fn qt_extract_voice_ref(q: *mut qt_context, audio: *const f32, n: c_int, out: *mut qt_voice_ref) -> c_int;
    pub fn qt_voice_ref_free(r: *mut qt_voice_ref);
    pub fn qt_tts_default_params(p: *mut qt_tts_params);
    pub fn qt_synthesize(q: *mut qt_context, p: *const qt_tts_params, out: *mut qt_audio) -> c_int;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi() {
        // La lib ecrit sa version d'ABI et ses defauts : si nos structs sont decalees, ca ne colle plus.
        unsafe {
            let mut i: qt_init_params = std::mem::zeroed();
            qt_init_default_params(&mut i);
            assert_eq!(i.abi_version, QT_ABI_VERSION);
            assert!(i.use_fa && !i.clamp_fp16 && i.max_batch == 1 && i.codec_chunk_sec == 24.0);
            let mut t: qt_tts_params = std::mem::zeroed();
            qt_tts_default_params(&mut t);
            assert_eq!(t.abi_version, QT_ABI_VERSION);
            assert!(t.top_k == 50 && t.subtalker_top_k == 50 && (t.repetition_penalty - 1.05).abs() < 1e-6);
            assert!(t.on_chunk.is_none() && t.ref_codes.is_null());
        }
    }
}
