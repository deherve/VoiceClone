use std::{env, fs, path::{Path, PathBuf}};

// Lie qwen.dll (compile par build-qwen.cmd) et copie ses DLL a cote de l'executable :
// qwen.dll, les ggml*.dll (CPU, Vulkan, et CUDA si la variante CUDA est choisie) et les DLL CUDA livrees avec.
// Variante : VOICECLONE_QWEN_BUILD=build (defaut, Vulkan) ou build-cuda (voir build-qwen.cmd).
fn main() {
    println!("cargo:rerun-if-env-changed=VOICECLONE_QWEN_BUILD");
    let variant = env::var("VOICECLONE_QWEN_BUILD").unwrap_or_else(|_| "build".into());
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let build = root.join("vendor/qwentts").join(&variant);
    assert!(build.join("qwen.lib").exists(), "qwen.lib absent dans {} : lancer build-qwen.cmd d'abord", build.display());
    println!("cargo:rustc-link-search=native={}", build.display());
    println!("cargo:rustc-link-search=native={}", build.join("ggml/src").display());
    println!("cargo:rustc-link-lib=dylib=qwen");
    // Registre des peripheriques (liste des GPU pour la page Parametres).
    println!("cargo:rustc-link-lib=dylib=ggml");
    println!("cargo:rustc-link-lib=dylib=ggml-base");
    println!("cargo:rerun-if-changed={}", build.display());

    // OUT_DIR = target/<profil>/build/qwen-sys-xxx/out -> on remonte a target/<profil>
    let exe_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("../../..");
    let ours = |name: &str| ["qwen", "ggml", "cudart", "cublas"].iter().any(|p| name.starts_with(p)) && name.ends_with(".dll");
    for dir in [exe_dir.clone(), exe_dir.join("deps")] {
        remove_dlls(&dir, &ours); // une variante precedente (CUDA) ne doit pas rester a cote de la nouvelle
    }
    for entry in fs::read_dir(&build).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if ours(&name) {
            // "deps" aussi : c'est la que tournent les executables de test.
            for dir in [exe_dir.clone(), exe_dir.join("deps")] {
                fs::copy(entry.path(), dir.join(entry.file_name())).unwrap_or_else(|e| panic!("copie {name}: {e}"));
            }
        }
    }
}

fn remove_dlls(dir: &Path, ours: &dyn Fn(&str) -> bool) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            if ours(&e.file_name().to_string_lossy().to_lowercase()) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}
