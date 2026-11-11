# Publie une version : compile, assemble l'archive autonome et cree la release GitHub.
#   .\scripts\release.ps1                  version = celle de crates\app\Cargo.toml ; notes = sa section de CHANGELOG.md
#   .\scripts\release.ps1 -SansPublier     assemble et controle l'archive, sans rien publier
# Une seule archive (aucune installation, runtimes Visual C++ inclus) :
#   VoiceClone-<v>-windows-x64.zip          VoiceClone.exe + voiceclone-server.exe dans le meme dossier : la page
#                                           Parametres de l'application en tire les chemins reels (MCP, ligne de commande)
# Prerequis : `gh` connecte, version commitee et poussee sur main, moteur compile (build-qwen.cmd).
param([switch]$SansPublier)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
Set-Location $root

$Version = (Select-String -Path "$root\crates\app\Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$tauri = (Select-String -Path "$root\crates\app\tauri.conf.json" -Pattern '"version":\s*"([^"]+)"').Matches[0].Groups[1].Value
$core = (Select-String -Path "$root\crates\voiceclone\Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
if ($tauri -ne $Version -or $core -ne $Version) { throw "Versions différentes (app $Version, tauri.conf.json $tauri, coeur $core) : utiliser scripts\bump-version.ps1." }
if (git status --porcelain) { throw "Des changements ne sont pas commités : commit et push d'abord." }
if ((git rev-parse HEAD) -ne (git rev-parse origin/main)) { throw "main local et origin/main diffèrent : push d'abord (la release part de main)." }

# Notes de la version : sa section du CHANGELOG (anglais puis francais).
$log = Get-Content "$root\CHANGELOG.md" -Raw -Encoding UTF8
$m = [regex]::Match($log, "(?ms)^## $([regex]::Escape($Version))\b[^\r\n]*\r?\n(.*?)(?=^## |\z)")
if (-not $m.Success) { throw "CHANGELOG.md n'a pas de section « ## $Version »." }
$notes = $m.Groups[1].Value.Trim()

node "$root\scripts\check-i18n.mjs"
if ($LASTEXITCODE) { throw 'traductions incohérentes' }
# Cargo ecrit sa progression sur stderr : avec ErrorActionPreference=Stop, PowerShell 5.1 y verrait une erreur.
$ErrorActionPreference = 'Continue'
cargo test --release -p voiceclone 2>&1 | Out-Null
$tests = $LASTEXITCODE
cargo build --release 2>&1 | Out-Null
$build = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
if ($tests) { throw 'les tests échouent' }
if ($build) { throw 'cargo build a échoué' }

$rel = "$root\target\release"
$redistRoot = Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Redist\MSVC\*" -Directory | Where-Object { Test-Path "$($_.FullName)\x64\Microsoft.VC143.CRT" } | Sort-Object Name | Select-Object -Last 1
if (-not $redistRoot) { throw 'Runtimes Visual C++ introuvables (Build Tools, dossier Redist).' }
# Runtimes necessaires au moteur (verifie par check-deps.ps1) : redistribuables au sens de la licence Visual Studio.
$runtimes = @("$($redistRoot.FullName)\x64\Microsoft.VC143.CRT\vcruntime140.dll", "$($redistRoot.FullName)\x64\Microsoft.VC143.CRT\vcruntime140_1.dll",
              "$($redistRoot.FullName)\x64\Microsoft.VC143.CRT\msvcp140.dll", "$($redistRoot.FullName)\x64\Microsoft.VC143.OpenMP\vcomp140.dll")

$readme = @"
VoiceClone $Version
===================

ENGLISH
Local voice cloning and audio generation, driven from a window or from Claude Code (MCP).
- No installation: unzip anywhere and run. The Visual C++ runtimes are included.
- First launch: Settings > Models: download the synthesis model (about 2 GB) and the audio codec (about 0.3 GB).
- Needs Windows 10/11 64-bit and a graphics driver with Vulkan (AMD, NVIDIA, Intel); without one the processor is used, about 4 times slower.
- Portable mode: create an empty file named "portable" next to the program; your data then stay in the "data" folder instead of %APPDATA%\VoiceClone.
- MCP connector: http://127.0.0.1:17494/mcp  (tools: speak, list_profiles, list_generations)
- Connect an AI or a script: Settings > "Connect an AI or a script" gives the MCP declaration, the command line and complete instructions to copy, with the real paths of this installation. voiceclone-server.exe (same folder) is the version without a window.
- Quick command line: voiceclone-server.exe speak "text" --lang en --quality enhanced --voice Name   (default destination: the Audios folder next to the program)
- Windows may warn about an unsigned program (SmartScreen): choose "More info" then "Run anyway".
- Generated audios are saved in the "Audios" folder next to the program. The archive includes one ready-made voice, "Zen" (folder "Voix", imported at first launch).
- Source code and updates: https://github.com/deherve/VoiceClone  -  Support: https://www.paypal.com/donate/?hosted_button_id=CASVBK5K77DHC

FRANCAIS
Clonage de voix et generation d'audio en local, depuis une fenetre ou depuis Claude Code (MCP).
- Aucune installation : decompressez n'importe ou et lancez. Les runtimes Visual C++ sont inclus.
- Premier lancement : Parametres > Modeles : telechargez le modele de synthese (environ 2 Go) et le codec audio (environ 0,3 Go).
- Necessite Windows 10/11 64 bits et un pilote graphique avec Vulkan (AMD, NVIDIA, Intel) ; sans lui, le processeur est utilise, environ 4 fois plus lent.
- Mode portable : creez un fichier vide nomme "portable" a cote du programme ; vos donnees restent alors dans le dossier "data" au lieu de %APPDATA%\VoiceClone.
- Connecteur MCP : http://127.0.0.1:17494/mcp  (outils : speak, list_profiles, list_generations)
- Connecter une IA ou un script : Parametres > « Connecter une IA ou un script » donne la declaration MCP, la ligne de commande et des instructions completes a copier, avec les chemins reels de cette installation. voiceclone-server.exe (meme dossier) est la version sans fenetre.
- Ligne de commande rapide : voiceclone-server.exe speak "texte" --lang fr --quality ameliore --voice Nom   (destination par defaut : le dossier Audios a cote du programme)
- Windows peut avertir qu'un programme n'est pas signe (SmartScreen) : « Informations complementaires » puis « Executer quand meme ».
- Les audios generes sont enregistres dans le dossier "Audios" a cote du programme. L'archive contient une voix prete a l'emploi, « Zen » (dossier "Voix", importee au premier lancement).
- Code source et mises a jour : https://github.com/deherve/VoiceClone  -  Soutenir : https://www.paypal.com/donate/?hosted_button_id=CASVBK5K77DHC
"@

# Seule voix livree : Zen (jamais dans le depot git). Exportee depuis les donnees locales si release-assets\Voix\Zen manque.
$voices = "$root\release-assets\Voix"
if (-not (Test-Path "$voices\Zen\voice.json")) {
    & "$rel\voiceclone-server.exe" export-voice Zen "$voices\Zen" | Out-Host
    if ($LASTEXITCODE -or -not (Test-Path "$voices\Zen\voice.json")) { throw 'Export de la voix Zen impossible (profil Zen absent ?).' }
}
$extra = Get-ChildItem $voices -Directory | Where-Object Name -ne 'Zen'
if ($extra) { throw "release-assets\Voix ne doit contenir que Zen : $($extra.Name -join ', ')" }

Remove-Item "$root\target\dist" -Recurse -Force -ErrorAction SilentlyContinue
function Assemble([string]$name, [string[]]$exes) {
    $dir = "$root\target\dist\$name"
    New-Item -ItemType Directory "$dir\licences" | Out-Null
    foreach ($e in $exes) { Copy-Item "$rel\$e" $dir }
    # DLL du moteur : memes familles que qwen-sys/build.rs (qwen, ggml, et cudart/cublas pour la variante CUDA).
    Get-ChildItem $rel -Filter *.dll | Where-Object { $_.Name -match '^(qwen|ggml|cudart|cublas)' } | Copy-Item -Destination $dir
    Copy-Item $runtimes $dir
    Copy-Item "$root\vendor\qwentts\LICENSE" "$dir\licences\qwentts.cpp-LICENSE.txt"
    Copy-Item "$root\vendor\qwentts\ggml\LICENSE" "$dir\licences\ggml-LICENSE.txt"
    Copy-Item $voices "$dir\Voix" -Recurse
    Set-Content "$dir\README.txt" $readme -Encoding UTF8
    & "$root\scripts\check-deps.ps1" -Dir $dir | Out-Host
    if ($LASTEXITCODE) { throw "$name : des DLL manquent, l'archive ne serait pas autonome." }
    # Garde-fou : les seuls fichiers audio de l'archive sont les echantillons de Zen.
    $audio = Get-ChildItem $dir -Recurse -File | Where-Object { $_.Extension -match '^\.(wav|mp3|flac|ogg|m4a)$' -and $_.FullName -notmatch '\\Voix\\Zen\\sample-\d+\.wav$' }
    if ($audio) { throw "Fichiers audio inattendus dans l'archive : $($audio.Name -join ', ')" }
    $zip = "$root\target\dist\$name.zip"
    Compress-Archive -Path "$dir\*" -DestinationPath $zip
    Write-Host ("{0} : {1:N1} Mo, {2} fichiers" -f $zip, ((Get-Item $zip).Length / 1MB), (Get-ChildItem $dir -Recurse -File).Count)
    $zip
}
$app = Assemble "VoiceClone-$Version-windows-x64" @('VoiceClone.exe', 'voiceclone-server.exe') | Select-Object -Last 1
if ($SansPublier) { return }

# Notes par fichier : passees en argument, PowerShell 5.1 mange les guillemets des exemples de code.
$notesFile = Join-Path $env:TEMP "voiceclone-notes-$Version.md"
[IO.File]::WriteAllText($notesFile, $notes, (New-Object Text.UTF8Encoding $false))
gh release create "v$Version" $app --target main --title "VoiceClone $Version" --notes-file $notesFile
