# Changelog

Versions go up by 0.0.1; after the digit 9 the next tens digit is used: 0.1.8 → 0.1.9 → 0.2.0 → … → 0.9.9 → 1.0.0.
Each release on GitHub carries the notes of its section below (English, then French).

Les versions avancent de 0.0.1 ; après le chiffre 9 on passe à la dizaine suivante : 0.1.8 → 0.1.9 → 0.2.0 → … → 0.9.9 → 1.0.0.
Chaque release GitHub reprend les notes de sa section ci-dessous (anglais, puis français).

## 0.1.5 - 2026-11-11

**English**
- **Unheard generations are highlighted** in the list (accent border and a "New" badge) until you play them. Generations that already existed count as heard.
- **Exact pauses**: the silence the model leaves at the end of each piece is trimmed, so `[pause 2s]` gives a 2 second gap instead of 2 seconds on top of the model's own silence (which could hide the pause). Pieces of the same voice are joined with 0.3 s.

**Français**
- **Les générations non écoutées sont mises en surbrillance** dans la liste (bordure de couleur et pastille « Nouveau ») jusqu'à leur première lecture. Les générations déjà présentes comptent comme écoutées.
- **Pauses exactes** : le silence que le modèle laisse à la fin de chaque morceau est retiré, donc `[pause 2s]` donne bien 2 secondes de silence et non 2 secondes en plus de celui du modèle (qui pouvait masquer la pause). Les morceaux de la même voix sont raccordés avec 0,3 s.

## 0.1.4 - 2026-11-11

**English**
- **Tags in the text**: buttons above the text box insert at the cursor `[pause]` (1 second; `[pause 2.5s]` sets the length) and one `[VoiceName]` tag per voice. The text after a voice tag is spoken with that voice and everything is merged in order: dialogues with several voices in a single long text. Unknown tags are an error (never read aloud by surprise).
- **Long texts**: generated in pieces of 30 seconds at most (a piece that comes out longer is cut in two and regenerated), then merged into one file.
- The cloning model understands no emotion tag, so none is offered: for another tone, record a voice in that tone and use its tag.
- Works the same through the MCP `speak` tool and the command line.

**Français**
- **Balises dans le texte** : des boutons au-dessus de la zone de texte insèrent au curseur `[pause]` (1 seconde ; `[pause 2.5s]` règle la durée) et une balise `[NomDeVoix]` par voix. Le texte qui suit une balise de voix est dit avec cette voix et tout est fusionné dans l'ordre : dialogues à plusieurs voix dans un seul long texte. Une balise inconnue est une erreur (jamais lue à voix haute par surprise).
- **Longs textes** : générés en morceaux de 30 secondes au plus (un morceau plus long est coupé en deux et régénéré), puis fusionnés en un seul fichier.
- Le modèle de clonage ne comprend aucune balise d'émotion, donc aucune n'est proposée : pour un autre ton, enregistrez une voix sur ce ton et utilisez sa balise.
- Fonctionne aussi avec l'outil MCP `speak` et la ligne de commande.

## 0.1.3 - 2026-11-11

**English**
- **Audios folder**: every generated audio (window, MCP, command line) is now saved in the `Audios` subfolder next to the program, with a readable name taken from the text (never replacing an existing file). Settings shows the path and a button to open it. The command line no longer writes to Downloads by default.
- **Zen voice included in the compiled archives**: `Voix\Zen` is imported at first launch (once). The source repository and its source archives contain no voice. New command `voiceclone-server.exe export-voice <name> <folder>`.

**Français**
- **Dossier Audios** : tous les audios générés (fenêtre, MCP, ligne de commande) sont enregistrés dans le sous-dossier `Audios` à côté du programme, avec un nom lisible tiré du texte (sans jamais remplacer un fichier existant). Les Paramètres montrent le chemin et un bouton pour l'ouvrir. La ligne de commande n'écrit plus dans Téléchargements par défaut.
- **Voix de Zen dans les archives compilées** : `Voix\Zen` est importée au premier lancement (une seule fois). Le dépôt source et ses archives ne contiennent aucune voix. Nouvelle commande `voiceclone-server.exe export-voice <nom> <dossier>`.

## 0.1.2 - 2026-11-11

**English**
- **Quick command line**: `voiceclone-server.exe speak "text" --lang en --quality enhanced --voice Zen` generates an audio file and prints its path. Options: language (`fr`, `en`, `de`, `es`, `it`, `pt`, `ja`, `ko`, `zh`, `ru`), quality (`standard` or `enhanced`), voice, text (or standard input), destination (default: the Downloads folder; an existing file is never replaced unless you name it). It uses the running server when there is one, otherwise it loads the model for that single generation.
- **Settings > Connect an AI or a script**: the MCP declaration, the Claude Code command, how to start the server version, the command-line examples (PowerShell and Command Prompt) and complete instructions for an AI or a skill, each with a Copy button. The paths are those of the installation, wherever it is.
- **One archive only**: `voiceclone-server.exe` now ships in the same folder as `VoiceClone.exe` in `VoiceClone-<version>-windows-x64.zip`; the separate server-only archive is gone.

**Français**
- **Ligne de commande rapide** : `voiceclone-server.exe speak "texte" --lang fr --quality ameliore --voice Zen` génère un fichier audio et affiche son chemin. Options : langue (`fr`, `en`, `de`, `es`, `it`, `pt`, `ja`, `ko`, `zh`, `ru`), qualité (`standard` ou `enhanced`/`ameliore`), voix, texte (ou entrée standard), destination (défaut : le dossier Téléchargements ; un fichier existant n'est jamais remplacé sauf si vous le nommez). Elle utilise le serveur s'il tourne déjà, sinon charge le modèle pour cette seule génération.
- **Paramètres > Connecter une IA ou un script** : la déclaration MCP, la commande de Claude Code, le lancement de la version serveur, les exemples de ligne de commande (PowerShell et Invite de commandes) et des instructions complètes pour une IA ou un skill, chacun avec un bouton Copier. Les chemins sont ceux de l'installation, où qu'elle soit.
- **Une seule archive** : `voiceclone-server.exe` est maintenant dans le même dossier que `VoiceClone.exe` dans `VoiceClone-<version>-windows-x64.zip` ; l'archive « serveur seul » séparée n'existe plus.

## 0.1.1 - 2026-11-11

**English**
- **English and French interface.** Choose the language in Settings; it also applies to error messages and to the headless server (the choice is saved).
- **Update check.** VoiceClone looks for a newer release on GitHub (once a day at startup, or on demand; it can be turned off), shows a badge and the release notes, and opens the release page. One request, no data sent.
- **Components and tools** (Settings): shows what is present or missing (Vulkan, CUDA, MP3/LAME) and installs what is missing from the official sources: Vulkan runtime from LunarG (SHA-256 hash and signature checked, Windows asks for permission), CUDA 12 libraries from NVIDIA (SHA-256 from NVIDIA's manifest). LAME is built in. CUDA support itself is not shipped yet and was not tested on an NVIDIA card.
- **Standalone builds.** The archives now include the Visual C++ runtimes (checked with `scripts/check-deps.ps1`), so nothing needs installing. New *portable mode* (an empty file named `portable` next to the program keeps all data in a `data` folder). New **server-only archive** (`VoiceClone-Server-…zip`): the MCP connector without a window and without WebView2.
- Release tooling: `scripts/bump-version.ps1` (rule: 0.1.9 → 0.2.0), `scripts/release.ps1` (builds, checks, publishes both archives), `scripts/check-i18n.mjs`.
- Bilingual README with a donation link.

**Français**
- **Interface en anglais et en français.** La langue se choisit dans Paramètres ; elle s'applique aussi aux messages d'erreur et à la version serveur (le choix est enregistré).
- **Détection des mises à jour.** VoiceClone cherche une release plus récente sur GitHub (une fois par jour au démarrage, ou à la demande ; désactivable), affiche un badge et les notes de version, et ouvre la page de la release. Une seule requête, aucune donnée envoyée.
- **Composants et outils** (Paramètres) : indique ce qui est présent ou manquant (Vulkan, CUDA, MP3/LAME) et installe ce qui manque depuis les sources officielles : runtime Vulkan de LunarG (empreinte SHA-256 et signature vérifiées, Windows demande l'autorisation), bibliothèques CUDA 12 de NVIDIA (SHA-256 du manifeste NVIDIA). LAME est intégré. Le module CUDA lui-même n'est pas encore fourni et n'a pas été testé sur une carte NVIDIA.
- **Versions autonomes.** Les archives incluent maintenant les runtimes Visual C++ (contrôlés par `scripts/check-deps.ps1`) : rien à installer. Nouveau *mode portable* (un fichier vide nommé `portable` à côté du programme garde toutes les données dans un dossier `data`). Nouvelle **archive serveur seul** (`VoiceClone-Server-….zip`) : le connecteur MCP sans fenêtre et sans WebView2.
- Outils de publication : `scripts/bump-version.ps1` (règle : 0.1.9 → 0.2.0), `scripts/release.ps1` (compile, contrôle, publie les deux archives), `scripts/check-i18n.mjs`.
- README bilingue avec lien de soutien.

## 0.1.0 - 2026-11-11

**English**
- First release: local voice cloning and audio generation with Qwen3-TTS (qwentts.cpp), as a Tauri app and as a headless MCP server for Claude Code.
- Voices from samples recorded with the microphone or imported (no length limit), with transcripts.
- GPU computing through Vulkan (tested on an AMD RX 6700 XT), automatic device detection with fallback to the processor; CUDA path written but untested.
- Voice enhancement (equalizer + normalized volume), WAV and MP3 export with a save dialog, waveforms, copy text, four interface themes, model manager (download, switch, unload).
- MCP tools `speak` (with `enhanced`), `list_profiles`, `list_generations`.

**Français**
- Première version : clonage de voix et génération d'audio en local avec Qwen3-TTS (qwentts.cpp), en application Tauri et en serveur MCP sans fenêtre pour Claude Code.
- Voix à partir d'échantillons enregistrés au micro ou importés (sans limite de durée), avec transcriptions.
- Calcul sur GPU via Vulkan (testé sur une AMD RX 6700 XT), détection automatique avec repli sur le processeur ; chemin CUDA écrit mais non testé.
- Amélioration de la voix (égaliseur + volume normalisé), export WAV et MP3 avec boîte « Enregistrer sous », formes d'onde, copie du texte, quatre thèmes, gestionnaire de modèles (téléchargement, choix, déchargement).
- Outils MCP `speak` (avec `enhanced`), `list_profiles`, `list_generations`.
