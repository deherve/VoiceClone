# Publie une version : compile, assemble le zip et cree la release GitHub avec le zip en piece jointe.
#   .\scripts\release.ps1 -Version 0.1.1 -Notes "Ce qui change"
# Prerequis : `gh` connecte, version deja commitee et poussee sur main, moteur compile (build-qwen.cmd).
param(
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [string]$Notes = ''
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
Set-Location $root

if (git status --porcelain) { throw "Des changements ne sont pas commités : commit et push d'abord." }
if ((git rev-parse HEAD) -ne (git rev-parse origin/main)) { throw "main local et origin/main diffèrent : push d'abord (la release part de main)." }
$cargo = (Select-String -Path "$root\crates\app\Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
if ($cargo -ne $Version) { throw "Cargo.toml indique la version $cargo : l'aligner sur $Version avant de publier." }

cargo build --release
if ($LASTEXITCODE) { throw 'cargo build a échoué' }

$name = "VoiceClone-$Version-windows-x64"
$dist = "$root\target\dist\$name"
Remove-Item "$root\target\dist" -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory "$dist\licences" | Out-Null
$rel = "$root\target\release"
Copy-Item "$rel\VoiceClone.exe", "$rel\voiceclone-server.exe" $dist
# DLL du moteur : mêmes familles que qwen-sys/build.rs (qwen, ggml, et cudart/cublas pour la variante CUDA).
Get-ChildItem $rel -Filter *.dll | Where-Object { $_.Name -match '^(qwen|ggml|cudart|cublas)' } | Copy-Item -Destination $dist
Copy-Item "$root\vendor\qwentts\LICENSE" "$dist\licences\qwentts.cpp-LICENSE.txt"
Copy-Item "$root\vendor\qwentts\ggml\LICENSE" "$dist\licences\ggml-LICENSE.txt"

@'
VoiceClone — génération d'audio avec une voix clonée, en local
===============================================================

Lancer : VoiceClone.exe (application avec fenêtre).
Version sans fenêtre, pour Claude Code et les scripts : voiceclone-server.exe (même connecteur MCP, même port).
Ne lancez pas les deux en même temps.

Prérequis : Windows 10/11 64 bits, WebView2 (déjà présent sous Windows 11) et un pilote graphique avec Vulkan
(AMD, NVIDIA, Intel). Sans GPU compatible, le calcul se fait sur le processeur, environ 4 fois plus lent.

Premier lancement
1. Paramètres > Modèles : télécharger le modèle de synthèse (environ 2 Go) et le codec audio (environ 0,3 Go).
2. Studio > Voix : créer une voix, enregistrer un échantillon au micro ou importer un fichier, saisir sa transcription exacte.
3. Saisir le texte, Générer.

Connecteur MCP (Claude Code) : http://127.0.0.1:17494/mcp
  .mcp.json :  { "mcpServers": { "voiceclone": { "type": "http", "url": "http://127.0.0.1:17494/mcp" } } }
  Outils : speak (text, profile, language, enhanced, output_path, seed), list_profiles, list_generations.

Windows peut afficher « Windows a protégé votre ordinateur » : l'exécutable n'est pas signé.
Cartes NVIDIA : le calcul passe par Vulkan ; la variante CUDA n'est pas fournie dans cette archive.
Données : %APPDATA%\VoiceClone. Licences des composants tiers : dossier licences.
Code source : https://github.com/deherve/VoiceClone
'@ | Set-Content "$dist\LISEZMOI.txt" -Encoding UTF8

$zip = "$root\target\dist\$name.zip"
Compress-Archive -Path "$dist\*" -DestinationPath $zip
"{0} : {1:N1} Mo, {2} fichiers" -f $zip, ((Get-Item $zip).Length / 1MB), (Get-ChildItem $dist -Recurse -File).Count

$args_ = @("v$Version", $zip, '--target', 'main', '--title', "VoiceClone $Version")
if ($Notes) { $args_ += @('--notes', $Notes) } else { $args_ += '--generate-notes' }
gh release create @args_
