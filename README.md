# VoiceClone

**[English](#english) · [Français](#français)**

[![Donate with PayPal / Faire un don avec PayPal](https://img.shields.io/badge/PayPal-Donate%20%2F%20Faire%20un%20don-0070ba?logo=paypal&logoColor=white)](https://www.paypal.com/donate/?hosted_button_id=CASVBK5K77DHC)

---

## Soutenir le projet / Support the project

**Français** : VoiceClone est gratuit. Je le développe seul, avec l'aide d'un assistant IA, et mon abonnement actuel m'impose des quotas qui limitent le rythme des évolutions. Si VoiceClone vous est utile, vous pouvez m'aider à prendre un abonnement plus poussé, pour avancer sans être freiné par ces limites. Le don est entièrement libre : il ne donne aucun avantage ni accès particulier, tout le monde a le même programme.

**English**: VoiceClone is free. I develop it on my own, with the help of an AI assistant, and the usage quota of my current plan limits how fast I can move. If VoiceClone is useful to you, you can help me move to a higher plan so I can keep developing without hitting those limits. Donations are entirely voluntary and give no perks or special access: everyone gets the same program.

[**Faire un don via PayPal / Donate via PayPal**](https://www.paypal.com/donate/?hosted_button_id=CASVBK5K77DHC)

---

## English

**VoiceClone** clones a voice from a short recording and generates speech with it, **locally** on your computer. It runs the **Qwen3-TTS** model through [qwentts.cpp](https://github.com/ServeurpersoCom/qwentts.cpp) on your graphics card (Vulkan: AMD, NVIDIA, Intel) and can be driven from its window or from **Claude Code** through an MCP connector. Nothing is sent to a server: voices, models and audio stay on your machine.

### Features

- **Voices from samples**: record with the microphone or import an audio file, with no length limit, and give its exact transcript for a faithful clone.
- **Text to speech** in a three-zone window: voices and samples, text to say, list of generations (waveform, play, copy the text).
- **Enhancement**: voice equalizer and normalized volume, switchable per generation (the original is always kept). WAV and MP3 export with a "Save as" dialog.
- **Claude Code (MCP)**: tools `speak` (with `enhanced`), `list_profiles`, `list_generations`; also available as a headless server with no window.
- **Graphics cards**: automatic detection, best choice proposed in Settings, automatic fallback to the processor. **Components** are checked in Settings and the missing ones can be installed from their official sources.
- **English and French** interface, four themes, **update check** against the GitHub releases.
- **Standalone**: no installation, Visual C++ runtimes included, optional portable mode.

### Inspiration

VoiceClone is inspired by [**Voicebox**](https://github.com/jamiepine/voicebox) by Jamie Pine, "the open-source AI voice studio" (MIT licence): voice cloning, audio generation and MCP control. Voicebox runs on PyTorch; VoiceClone keeps the idea of a page with voices and samples, text and generations, in a lighter version rewritten in Rust, with a ggml/Vulkan engine that uses AMD graphics cards on Windows (which Voicebox 0.5.0 did not). No Voicebox code is used.

### Download and first run

1. Get the latest archive from the [Releases](https://github.com/deherve/VoiceClone/releases) page:
   - `VoiceClone-<version>-windows-x64.zip`: the application with its window, plus the headless server.
   - `VoiceClone-Server-<version>-windows-x64.zip`: the headless server only (MCP connector, no window, no WebView2 needed).
2. Unzip anywhere and run `VoiceClone.exe`. Nothing to install. Windows may warn about an unsigned program (SmartScreen): choose "More info", then "Run anyway".
3. **Settings > Models**: download the synthesis model (about 2 GB) and the audio codec (about 0.3 GB).
4. **Studio**: create a voice, add a sample and its transcript, type a text, **Generate**.

*Portable mode*: create an empty file named `portable` next to the program; your data then stay in a `data` folder instead of `%APPDATA%\VoiceClone`.
Requirements: Windows 10/11 64-bit, a graphics driver with Vulkan (otherwise the processor is used, about 4 times slower).

### Using it from Claude Code

Start `VoiceClone.exe` or `voiceclone-server.exe` (not both: they share the port), then declare the connector in your project's `.mcp.json`:

```json
{ "mcpServers": { "voiceclone": { "type": "http", "url": "http://127.0.0.1:17494/mcp" } } }
```

Then ask for example: *"say this sentence with the voice Zen and write it to `public/voice/01.wav`, enhanced"* → `speak(text, profile="Zen", language="french", enhanced=true, output_path="…\01.wav")`.

### Graphics cards and components

| Hardware | Status |
|---|---|
| AMD, Vulkan (RX 6700 XT) | **Tested**: faster than real time |
| NVIDIA, Vulkan | Same code as AMD, **not tested** on an NVIDIA card |
| NVIDIA, CUDA | **Not tested, not shipped** (no NVIDIA card or CUDA toolkit on the development machine) |
| Processor | Tested (about 4 times slower than the GPU) |

Settings > *Graphics card* lists the detected devices, proposes the best one (CUDA > Vulkan discrete > Vulkan integrated > processor) and falls back automatically if loading fails. Settings > *Components and tools* shows what is present and installs what is missing from official sources only: the Vulkan runtime from LunarG (SHA-256 and signature checked, Windows asks for permission) and the CUDA 12 libraries from NVIDIA (SHA-256 from NVIDIA's manifest). MP3 (LAME) is built in.

### Updates

VoiceClone checks the latest GitHub release (once a day at startup, or on demand; it can be turned off in Settings), shows a badge with the release notes and opens the release page. To update, download the new archive, close VoiceClone and replace its files: your data are kept. Versions go up by 0.0.1 and roll over after 9 (0.1.9 → 0.2.0); see [CHANGELOG.md](CHANGELOG.md).

### Build from source

Requirements: Rust, CMake, Visual Studio 2022 Build Tools, Vulkan SDK, Node.js (translation check), GitHub CLI (releases).

```bat
build-qwen.cmd                              :: engine with Vulkan → vendor\qwentts\build
cargo build --release                       :: the application; copies the engine DLLs next to VoiceClone.exe
cargo test --release -p voiceclone          :: tests
cargo test --release -p voiceclone -- --ignored   :: real model, GPU fallback and downloads from LunarG / NVIDIA
node scripts\check-i18n.mjs                 :: English and French dictionaries match
```

CUDA variant (untested): `build-qwen.cmd cuda` (needs the CUDA Toolkit 12.x), then `set VOICECLONE_QWEN_BUILD=build-cuda` and `cargo build --release`.
Publishing a version: `scripts\bump-version.ps1`, edit `CHANGELOG.md`, commit and push, then `scripts\release.ps1` (builds, checks that nothing is missing with `scripts\check-deps.ps1`, publishes both archives). Releases are built on this machine: there is no continuous integration.

| Environment variable | Role |
|---|---|
| `VOICECLONE_DATA` | data folder (default `%APPDATA%\VoiceClone`, or `data\` in portable mode) |
| `VOICECLONE_MODELS` | models folder (default `<data>\models`) |
| `VOICECLONE_PORT` | MCP connector port (default 17494) |
| `VOICECLONE_DEVICE` | force a device (`CUDA0`, `Vulkan0`, `CPU`), takes priority over Settings |
| `VOICECLONE_QWEN_BUILD` | engine variant linked at build time (`build` or `build-cuda`) |

### Third-party components and licence

- `vendor/qwentts`: a copy of commit `de2ad83` of [ServeurpersoCom/qwentts.cpp](https://github.com/ServeurpersoCom/qwentts.cpp) (MIT, see `vendor/qwentts/LICENSE` and `vendor/qwentts/ggml/LICENSE`), a C++ port of Qwen3-TTS (Alibaba, Qwen team).
- GGUF models downloaded from [Serveurperso/Qwen3-TTS-GGUF](https://huggingface.co/Serveurperso/Qwen3-TTS-GGUF) on Hugging Face (not included here; their licences apply).
- Interface: [Tauri](https://tauri.app). MP3: LAME (through `mp3lame-encoder`, LGPL).
- VoiceClone's own code has no licence file yet: until one is added, the default copyright applies.

Support the project: [Donate with PayPal](https://www.paypal.com/donate/?hosted_button_id=CASVBK5K77DHC). Thank you!

---

## Français

**VoiceClone** clone une voix à partir d'un court enregistrement et génère de la parole avec elle, **en local** sur votre ordinateur. Il exécute le modèle **Qwen3-TTS** via [qwentts.cpp](https://github.com/ServeurpersoCom/qwentts.cpp) sur votre carte graphique (Vulkan : AMD, NVIDIA, Intel) et se pilote depuis sa fenêtre ou depuis **Claude Code** grâce à un connecteur MCP. Rien n'est envoyé à un serveur : voix, modèles et audio restent sur votre machine.

### Fonctions

- **Voix à partir d'échantillons** : enregistrement au micro ou import d'un fichier audio, sans limite de durée, avec sa transcription exacte pour un clonage fidèle.
- **Synthèse vocale** dans une fenêtre à trois zones : voix et échantillons, texte à dire, liste des générations (forme d'onde, lecture, copie du texte).
- **Amélioration** : égaliseur de voix et volume normalisé, activable par génération (l'original est toujours conservé). Export WAV et MP3 avec boîte « Enregistrer sous ».
- **Claude Code (MCP)** : outils `speak` (avec `enhanced`), `list_profiles`, `list_generations` ; disponible aussi en version serveur sans fenêtre.
- **Cartes graphiques** : détection automatique, meilleur choix proposé dans Paramètres, repli automatique sur le processeur. Les **composants** sont contrôlés dans Paramètres et ceux qui manquent s'installent depuis leurs sources officielles.
- Interface en **anglais et français**, quatre thèmes, **détection des mises à jour** sur les releases GitHub.
- **Autonome** : aucune installation, runtimes Visual C++ inclus, mode portable en option.

### Inspiration

VoiceClone s'inspire de [**Voicebox**](https://github.com/jamiepine/voicebox) de Jamie Pine, « the open-source AI voice studio » (licence MIT) : clonage de voix, génération d'audio et pilotage par MCP. Voicebox tourne sur PyTorch ; VoiceClone reprend l'idée d'une page voix + échantillons, texte et générations, en version allégée réécrite en Rust, avec un moteur ggml/Vulkan qui utilise les cartes graphiques AMD sous Windows (que Voicebox 0.5.0 n'exploitait pas). Aucun code de Voicebox n'est repris.

### Téléchargement et premier lancement

1. Récupérez la dernière archive sur la page [Releases](https://github.com/deherve/VoiceClone/releases) :
   - `VoiceClone-<version>-windows-x64.zip` : l'application avec sa fenêtre, plus la version serveur.
   - `VoiceClone-Server-<version>-windows-x64.zip` : la version serveur seule (connecteur MCP, sans fenêtre, sans WebView2).
2. Décompressez n'importe où et lancez `VoiceClone.exe`. Rien à installer. Windows peut avertir qu'un programme n'est pas signé (SmartScreen) : « Informations complémentaires », puis « Exécuter quand même ».
3. **Paramètres > Modèles** : téléchargez le modèle de synthèse (environ 2 Go) et le codec audio (environ 0,3 Go).
4. **Studio** : créez une voix, ajoutez un échantillon et sa transcription, saisissez un texte, **Générer**.

*Mode portable* : créez un fichier vide nommé `portable` à côté du programme ; vos données restent alors dans un dossier `data` au lieu de `%APPDATA%\VoiceClone`.
Prérequis : Windows 10/11 64 bits, un pilote graphique avec Vulkan (sinon le processeur est utilisé, environ 4 fois plus lent).

### Utilisation depuis Claude Code

Lancez `VoiceClone.exe` ou `voiceclone-server.exe` (pas les deux : ils partagent le port), puis déclarez le connecteur dans le `.mcp.json` de votre projet :

```json
{ "mcpServers": { "voiceclone": { "type": "http", "url": "http://127.0.0.1:17494/mcp" } } }
```

Demandez ensuite par exemple : *« dis cette phrase avec la voix Zen et écris-la dans `public/voix/01.wav`, en version améliorée »* → `speak(text, profile="Zen", language="french", enhanced=true, output_path="…\01.wav")`.

### Cartes graphiques et composants

| Matériel | Statut |
|---|---|
| AMD, Vulkan (RX 6700 XT) | **Testé** : plus rapide que le temps réel |
| NVIDIA, Vulkan | Même code que l'AMD, **non testé** sur une carte NVIDIA |
| NVIDIA, CUDA | **Non testé, non fourni** (ni carte NVIDIA ni CUDA Toolkit sur la machine de développement) |
| Processeur | Testé (environ 4 fois plus lent que le GPU) |

Paramètres > *Carte graphique* liste les périphériques détectés, propose le meilleur (CUDA > Vulkan dédié > Vulkan intégré > processeur) et se replie automatiquement si le chargement échoue. Paramètres > *Composants et outils* montre ce qui est présent et installe ce qui manque depuis les seules sources officielles : le runtime Vulkan de LunarG (SHA-256 et signature vérifiés, Windows demande l'autorisation) et les bibliothèques CUDA 12 de NVIDIA (SHA-256 du manifeste NVIDIA). Le MP3 (LAME) est intégré.

### Mises à jour

VoiceClone consulte la dernière release GitHub (une fois par jour au démarrage, ou à la demande ; désactivable dans Paramètres), affiche un badge avec les notes de version et ouvre la page de la release. Pour mettre à jour, téléchargez la nouvelle archive, fermez VoiceClone et remplacez ses fichiers : vos données sont conservées. Les versions avancent de 0.0.1 et passent à la dizaine suivante après 9 (0.1.9 → 0.2.0) ; voir [CHANGELOG.md](CHANGELOG.md).

### Compiler depuis les sources

Prérequis : Rust, CMake, Visual Studio 2022 Build Tools, Vulkan SDK, Node.js (contrôle des traductions), GitHub CLI (releases).

```bat
build-qwen.cmd                              :: moteur en Vulkan → vendor\qwentts\build
cargo build --release                       :: l'application ; copie les DLL du moteur à côté de VoiceClone.exe
cargo test --release -p voiceclone          :: tests
cargo test --release -p voiceclone -- --ignored   :: vrai modèle, repli du GPU et téléchargements LunarG / NVIDIA
node scripts\check-i18n.mjs                 :: les dictionnaires anglais et français concordent
```

Variante CUDA (non testée) : `build-qwen.cmd cuda` (CUDA Toolkit 12.x requis), puis `set VOICECLONE_QWEN_BUILD=build-cuda` et `cargo build --release`.
Publier une version : `scripts\bump-version.ps1`, compléter `CHANGELOG.md`, commit et push, puis `scripts\release.ps1` (compile, vérifie que rien ne manque avec `scripts\check-deps.ps1`, publie les deux archives). Les releases sont construites sur cette machine : il n'y a pas d'intégration continue.

| Variable d'environnement | Rôle |
|---|---|
| `VOICECLONE_DATA` | dossier de données (défaut `%APPDATA%\VoiceClone`, ou `data\` en mode portable) |
| `VOICECLONE_MODELS` | dossier des modèles (défaut `<données>\models`) |
| `VOICECLONE_PORT` | port du connecteur MCP (défaut 17494) |
| `VOICECLONE_DEVICE` | force un périphérique (`CUDA0`, `Vulkan0`, `CPU`), prioritaire sur Paramètres |
| `VOICECLONE_QWEN_BUILD` | variante du moteur liée à la compilation (`build` ou `build-cuda`) |

### Composants tiers et licence

- `vendor/qwentts` : copie du commit `de2ad83` de [ServeurpersoCom/qwentts.cpp](https://github.com/ServeurpersoCom/qwentts.cpp) (MIT, voir `vendor/qwentts/LICENSE` et `vendor/qwentts/ggml/LICENSE`), portage C++ de Qwen3-TTS (Alibaba, Qwen team).
- Modèles GGUF téléchargés depuis [Serveurperso/Qwen3-TTS-GGUF](https://huggingface.co/Serveurperso/Qwen3-TTS-GGUF) sur Hugging Face (non inclus ici ; leurs licences s'appliquent).
- Interface : [Tauri](https://tauri.app). MP3 : LAME (via `mp3lame-encoder`, LGPL).
- Le code propre de VoiceClone n'a pas encore de fichier de licence : tant qu'il n'y en a pas, le droit d'auteur par défaut s'applique.

Soutenir le projet : [Faire un don avec PayPal](https://www.paypal.com/donate/?hosted_button_id=CASVBK5K77DHC). Merci !
