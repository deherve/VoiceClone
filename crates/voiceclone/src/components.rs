//! Composants externes dont VoiceClone peut avoir besoin (Vulkan, CUDA, MP3) et leur installation depuis les sources
//! officielles. Seules les adresses ci-dessous sont utilisees : elles sont fixees dans le code, jamais recues de l'interface.
//!
//! - MP3 : LAME est compilee dans l'application, rien a installer.
//! - Vulkan : le « loader » (vulkan-1.dll) vient en general du pilote graphique. S'il manque, le programme d'installation
//!   LunarG (editeur officiel du loader) est telecharge, verifie (empreinte SHA-256 fixee + signature Authenticode « LunarG, Inc. »)
//!   puis lance avec l'elevation Windows : l'utilisateur confirme dans la fenetre habituelle de Windows.
//! - CUDA : les bibliotheques d'execution (cudart, cublas) sont telechargees chez NVIDIA, verifiees par l'empreinte SHA-256 de
//!   leur manifeste officiel, et seules les DLL attendues sont extraites dans `<donnees>\cuda`. Le pilote NVIDIA, lui, s'installe
//!   depuis le site de NVIDIA (la page officielle s'ouvre dans le navigateur).
use crate::{e, models, R};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, OnceLock},
};

// ---- Vulkan (LunarG). Version et empreinte fixees : verifiees le 2026-11-11 sur le programme d'installation officiel.
const VULKAN_URL: &str = "https://sdk.lunarg.com/sdk/download/1.3.296.0/windows/VulkanRT-1.3.296.0-Installer.exe";
const VULKAN_SHA256: &str = "93a14d17f70bde4e5e2a4daf0af0e72b0fcb9693d84757ffd08255e5828d28ca";
const VULKAN_SIGNER: &str = "CN=\"LunarG, Inc.\"";
const VULKAN_MB: u64 = 21;

// ---- CUDA (NVIDIA) : manifeste officiel des redistribuables, version majeure 12 (les DLL se nomment cudart64_12.dll...).
const CUDA_REDIST: &str = "12.9.1";
const CUDA_BASE: &str = "https://developer.download.nvidia.com/compute/cuda/redist/";
const CUDA_PACKAGES: [&str; 2] = ["cuda_cudart", "libcublas"];
const CUDA_MB: u64 = 528;

/// Pages officielles qu'on accepte d'ouvrir dans le navigateur.
pub fn official_page(key: &str) -> Option<&'static str> {
    Some(match key {
        "nvidia_drivers" => "https://www.nvidia.com/Download/index.aspx",
        "amd_drivers" => "https://www.amd.com/en/support/download/drivers.html",
        "intel_drivers" => "https://www.intel.com/content/www/us/en/download-center/home.html",
        "lunarg_runtime" => "https://vulkan.lunarg.com/sdk/home",
        _ => return None,
    })
}

fn system32(file: &str) -> PathBuf {
    PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32").join(file)
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

pub fn cuda_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("cuda")
}

/// Noms des cartes graphiques (une seule interrogation de Windows par lancement).
/// PowerShell sans fenetre de console (l'application n'a pas de console : elle apparaitrait un instant).
fn powershell() -> Command {
    let mut c = Command::new("powershell");
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000);
    c
}

fn gpu_names() -> &'static Vec<String> {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let out = powershell()
            .args(["-NoProfile", "-NonInteractive", "-Command", "(Get-CimInstance Win32_VideoController).Name"])
            .output();
        match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(),
            Err(_) => vec![],
        }
    })
}

fn has_cuda_runtime(data_dir: &Path) -> bool {
    [Some(cuda_dir(data_dir)), exe_dir()].into_iter().flatten().any(|d| d.join("cudart64_12.dll").exists() && d.join("cublas64_12.dll").exists())
}

/// Etat de chaque composant, pour la page Parametres.
pub fn status(data_dir: &Path) -> Value {
    let vulkan = system32("vulkan-1.dll").exists();
    let names = gpu_names();
    let nvidia = names.iter().any(|n| n.to_ascii_uppercase().contains("NVIDIA"));
    let driver = system32("nvcuda.dll").exists();
    let module = exe_dir().is_some_and(|d| d.join("ggml-cuda.dll").exists());
    let runtime = has_cuda_runtime(data_dir);
    // Une seule raison a la fois, dans l'ordre ou on peut la lever.
    let cuda_state = match () {
        _ if !nvidia => "not_applicable",
        _ if !driver => "driver_missing",
        _ if !module => "module_missing",
        _ if !runtime => "runtime_missing",
        _ => "installed",
    };
    json!([
        {"id": "mp3", "state": "builtin"},
        {"id": "vulkan", "state": if vulkan { "installed" } else { "missing" }, "can_install": !vulkan, "size_mb": VULKAN_MB,
         "source": "sdk.lunarg.com", "gpus": names},
        {"id": "cuda", "state": cuda_state, "can_install": cuda_state == "runtime_missing", "size_mb": CUDA_MB,
         "source": "developer.download.nvidia.com", "nvidia": nvidia, "driver": driver, "module": module, "runtime": runtime},
    ])
}

pub(crate) fn sha256_file(path: &Path) -> R<String> {
    let mut f = fs::File::open(path).map_err(e)?;
    let (mut h, mut buf) = (Sha256::new(), vec![0u8; 1 << 16]);
    loop {
        let n = f.read(&mut buf).map_err(e)?;
        if n == 0 {
            return Ok(format!("{:x}", h.finalize()));
        }
        h.update(&buf[..n]);
    }
}

/// Signature Authenticode valide ET editeur attendu. Windows verifie la chaine de certificats et l'integrite du fichier.
fn verify_signature(path: &Path, expected_signer: &str) -> R<()> {
    let script = "$s = Get-AuthenticodeSignature -LiteralPath $env:VC_VERIFY_PATH; \
                  [pscustomobject]@{status=[string]$s.Status; subject=[string]$s.SignerCertificate.Subject} | ConvertTo-Json -Compress";
    let out = powershell()
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("VC_VERIFY_PATH", path) // le chemin passe par l'environnement : aucun probleme de guillemets
        .output()
        .map_err(e)?;
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|_| tr!("signature illisible", "unreadable signature"))?;
    let (status, subject) = (v["status"].as_str().unwrap_or(""), v["subject"].as_str().unwrap_or(""));
    if status != "Valid" || !subject.contains(expected_signer) {
        return Err(tr!("signature invalide ({} / {})", "invalid signature ({} / {})", status, subject));
    }
    Ok(())
}

fn downloads_dir(data_dir: &Path) -> R<PathBuf> {
    let d = data_dir.join("downloads");
    fs::create_dir_all(&d).map_err(e)?;
    Ok(d)
}

/// Telecharge le programme d'installation LunarG et le verifie ; retourne son chemin (rien n'est execute ici).
pub fn fetch_vulkan_installer(data_dir: &Path, cancel: &AtomicBool, progress: &dyn Fn(&str, u64, u64)) -> R<PathBuf> {
    let dest = downloads_dir(data_dir)?.join("VulkanRT-1.3.296.0-Installer.exe");
    models::download(VULKAN_URL, &dest, cancel, &|d, t| progress("download", d, t))?;
    progress("verify", 0, 0);
    let ok = sha256_file(&dest)? == VULKAN_SHA256;
    if !ok || verify_signature(&dest, VULKAN_SIGNER).is_err() {
        let _ = fs::remove_file(&dest); // fichier douteux : on ne le garde pas
        return Err(tr!("le programme d'installation téléchargé ne correspond pas à la version officielle : abandon", "the downloaded installer does not match the official release: aborted"));
    }
    Ok(dest)
}

/// Lance un programme avec l'elevation Windows (fenetre UAC). Retourne apres le lancement, sans attendre la fin.
fn run_elevated(path: &Path) -> R<()> {
    use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt};
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(hwnd: *mut u8, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
    }
    let wide = |s: &OsStr| s.encode_wide().chain(once(0)).collect::<Vec<u16>>();
    let (op, file) = (wide(OsStr::new("runas")), wide(path.as_os_str()));
    // SAFETY : chaines UTF-16 terminees par zero, vivantes pendant l'appel ; > 32 = succes selon la documentation de ShellExecuteW.
    let r = unsafe { ShellExecuteW(std::ptr::null_mut(), op.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), 1) };
    if r > 32 { Ok(()) } else { Err(tr!("l'installation n'a pas été lancée (autorisation refusée ?)", "the installer was not started (permission denied?)")) }
}

fn is_cuda_dll(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["cudart64_", "cublas64_", "cublaslt64_"].iter().any(|p| n.starts_with(p)) && n.ends_with(".dll") && !n.contains(['/', '\\'])
}

/// Extrait seulement les DLL CUDA attendues, a plat : aucun chemin de l'archive n'est utilise pour ecrire.
fn extract_cuda_dlls(zip_path: &Path, dest: &Path) -> R<Vec<String>> {
    let mut z = zip::ZipArchive::new(fs::File::open(zip_path).map_err(e)?).map_err(e)?;
    fs::create_dir_all(dest).map_err(e)?;
    let mut done = vec![];
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(e)?;
        let Some(name) = Path::new(f.name()).file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
        if !f.is_file() || !is_cuda_dll(&name) {
            continue;
        }
        let tmp = dest.join(format!("{name}.part"));
        std::io::copy(&mut f, &mut fs::File::create(&tmp).map_err(e)?).map_err(e)?;
        fs::rename(&tmp, dest.join(&name)).map_err(e)?;
        done.push(name);
    }
    Ok(done)
}

/// Adresse d'un paquet NVIDIA : seulement des chemins relatifs sobres, relatifs au depot officiel.
fn nvidia_package(manifest: &Value, pkg: &str) -> R<(String, String)> {
    let p = &manifest[pkg]["windows-x86_64"];
    let (rel, sha) = (p["relative_path"].as_str().unwrap_or(""), p["sha256"].as_str().unwrap_or(""));
    let sober = !rel.is_empty() && rel.starts_with(pkg) && rel.ends_with(".zip") && !rel.contains("..") && rel.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._/".contains(&b));
    if !sober || sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(tr!("manifeste NVIDIA inattendu pour {}", "unexpected NVIDIA manifest for {}", pkg));
    }
    Ok((format!("{CUDA_BASE}{rel}"), sha.to_ascii_lowercase()))
}

/// Telecharge les paquets `packages` du manifeste NVIDIA, verifie leur SHA-256 et extrait les DLL dans `dest`.
pub fn install_cuda_packages(data_dir: &Path, dest: &Path, packages: &[&str], cancel: &AtomicBool, progress: &dyn Fn(&str, u64, u64)) -> R<Vec<String>> {
    progress("manifest", 0, 0);
    let resp = ureq::get(&format!("{CUDA_BASE}redistrib_{CUDA_REDIST}.json"))
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|x| tr!("manifeste NVIDIA injoignable : {}", "cannot reach the NVIDIA manifest: {}", x))?;
    let mut raw = String::new();
    resp.into_reader().take(1 << 21).read_to_string(&mut raw).map_err(e)?;
    let manifest: Value = serde_json::from_str(&raw).map_err(|_| tr!("manifeste NVIDIA illisible", "unreadable NVIDIA manifest"))?;
    let mut all = vec![];
    for pkg in packages {
        let (url, sha) = nvidia_package(&manifest, pkg)?;
        let zip = downloads_dir(data_dir)?.join(format!("{pkg}-{CUDA_REDIST}.zip"));
        models::download(&url, &zip, cancel, &|d, t| progress("download", d, t))?;
        progress("verify", 0, 0);
        if sha256_file(&zip)? != sha {
            let _ = fs::remove_file(&zip);
            return Err(tr!("{} : empreinte SHA-256 différente du manifeste NVIDIA, fichier supprimé", "{}: SHA-256 differs from the NVIDIA manifest, file deleted", pkg));
        }
        progress("extract", 0, 0);
        all.extend(extract_cuda_dlls(&zip, dest)?);
        let _ = fs::remove_file(&zip);
    }
    Ok(all)
}

/// Installe un composant. Retourne ce qui s'est passe : « launched » (programme d'installation lance, l'utilisateur confirme dans
/// Windows) ou « installed » (fichiers en place ; un redemarrage de VoiceClone est necessaire).
pub fn install(id: &str, data_dir: &Path, cancel: &AtomicBool, progress: &dyn Fn(&str, u64, u64)) -> R<&'static str> {
    match id {
        "vulkan" => {
            if system32("vulkan-1.dll").exists() {
                return Err(tr!("Vulkan est déjà installé", "Vulkan is already installed"));
            }
            let exe = fetch_vulkan_installer(data_dir, cancel, progress)?;
            progress("launch", 0, 0);
            run_elevated(&exe)?;
            Ok("launched")
        }
        "cuda" => {
            install_cuda_packages(data_dir, &cuda_dir(data_dir), &CUDA_PACKAGES, cancel, progress)?;
            Ok("installed")
        }
        "mp3" => Err(tr!("LAME est intégré à l'application : rien à installer", "LAME is built into the application: nothing to install")),
        other => Err(tr!("composant inconnu : {}", "unknown component: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_expected_cuda_dlls_are_extracted_flat() {
        let dir = std::env::temp_dir().join(format!("voiceclone-zip-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("t.zip");
        {
            use std::io::Write;
            let mut w = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            for (name, data) in [
                ("pkg/bin/cudart64_12.dll", "dll"), ("pkg/bin/cublasLt64_12.dll", "lt"), ("pkg/include/cuda.h", "h"),
                ("pkg/bin/evil.dll", "x"), ("../../escape.dll", "x"), ("pkg/bin/cudart64_12.dll.exe", "x"),
            ] {
                w.start_file(name, o).unwrap();
                w.write_all(data.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        let out = dir.join("cuda");
        let mut got = extract_cuda_dlls(&zip_path, &out).unwrap();
        got.sort();
        assert_eq!(got, ["cublasLt64_12.dll", "cudart64_12.dll"]);
        assert_eq!(fs::read_dir(&out).unwrap().count(), 2, "rien d'autre n'est ecrit, rien hors du dossier");
        assert!(!dir.join("escape.dll").exists() && !dir.parent().unwrap().join("escape.dll").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn nvidia_manifest_entries_are_validated() {
        let sha = "a".repeat(64);
        let ok = json!({"cuda_cudart": {"windows-x86_64": {"relative_path": "cuda_cudart/windows-x86_64/x.zip", "sha256": sha}}});
        assert_eq!(nvidia_package(&ok, "cuda_cudart").unwrap().0, format!("{CUDA_BASE}cuda_cudart/windows-x86_64/x.zip"));
        for bad in [
            json!({"cuda_cudart": {"windows-x86_64": {"relative_path": "../x.zip", "sha256": sha}}}),
            json!({"cuda_cudart": {"windows-x86_64": {"relative_path": "autre/x.zip", "sha256": sha}}}),
            json!({"cuda_cudart": {"windows-x86_64": {"relative_path": "cuda_cudart/x.zip", "sha256": "court"}}}),
            json!({"cuda_cudart": {"windows-x86_64": {"relative_path": "cuda_cudart/x y.zip", "sha256": sha}}}),
            json!({}),
        ] {
            assert!(nvidia_package(&bad, "cuda_cudart").is_err(), "{bad}");
        }
    }

    #[test]
    fn official_pages_are_a_closed_list() {
        assert!(official_page("nvidia_drivers").unwrap().starts_with("https://www.nvidia.com/"));
        assert_eq!(official_page("https://evil.example"), None);
    }

    #[test]
    fn signature_check_rejects_unsigned_files_and_wrong_signer() {
        let f = std::env::temp_dir().join(format!("voiceclone-sig-{}.exe", std::process::id()));
        fs::write(&f, b"MZ pas un vrai programme").unwrap();
        assert!(verify_signature(&f, VULKAN_SIGNER).is_err());
        // Un binaire Windows signe par Microsoft passe la signature mais pas le controle de l'editeur.
        let signed = system32("notepad.exe");
        if signed.exists() {
            assert!(verify_signature(&signed, VULKAN_SIGNER).is_err(), "bon fichier signe, mauvais editeur");
            assert!(verify_signature(&signed, "Microsoft").is_ok());
        }
        let _ = fs::remove_file(f);
    }

    /// Reseau : telecharge le vrai programme d'installation LunarG (21 Mo), verifie son empreinte et sa signature. Ne l'execute pas.
    /// `cargo test -p voiceclone -- --ignored official_vulkan`
    #[test]
    #[ignore = "telecharge 21 Mo"]
    fn official_vulkan_installer_verifies() {
        let dir = std::env::temp_dir().join(format!("voiceclone-vk-{}", std::process::id()));
        let exe = fetch_vulkan_installer(&dir, &AtomicBool::new(false), &|_, _, _| {}).unwrap();
        assert!(exe.exists());
        let _ = fs::remove_dir_all(dir);
    }

    /// Reseau : telecharge le paquet cudart officiel de NVIDIA (3 Mo), verifie le SHA-256 du manifeste et extrait la DLL.
    #[test]
    #[ignore = "telecharge 3 Mo"]
    fn official_cudart_installs() {
        let dir = std::env::temp_dir().join(format!("voiceclone-cu-{}", std::process::id()));
        let dest = dir.join("cuda");
        let got = install_cuda_packages(&dir, &dest, &["cuda_cudart"], &AtomicBool::new(false), &|_, _, _| {}).unwrap();
        assert!(got.iter().any(|n| n.eq_ignore_ascii_case("cudart64_12.dll")), "{got:?}");
        assert!(dest.join("cudart64_12.dll").metadata().unwrap().len() > 100_000);
        let _ = fs::remove_dir_all(dir);
    }
}
