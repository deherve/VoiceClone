# Changelog

Versions go up by 0.0.1; after the digit 9 the next tens digit is used: 0.1.8 → 0.1.9 → 0.2.0 → … → 0.9.9 → 1.0.0.
Each release on GitHub carries the notes of its section below (English, then French).

Les versions avancent de 0.0.1 ; après le chiffre 9 on passe à la dizaine suivante : 0.1.8 → 0.1.9 → 0.2.0 → … → 0.9.9 → 1.0.0.
Chaque release GitHub reprend les notes de sa section ci-dessous (anglais, puis français).

## 0.2.3 - 2026-11-13

**English**
- **Faster synthesis**: the engine now computes up to 4 pieces of a text together (chosen from the free video memory: 4 with about 11 GB free, 1 on a small card, 2 when it cannot be measured). Measured on an RX 6700 XT: a 12-minute text goes from 154 s to 85 s (-45 %), a one-minute text from 12 s to 7 s, the first generation from 17 s to 12 s. The output is unchanged in quality (same transcription accuracy) and still reproducible (identical files for identical requests). Video memory peak: 5.2 GB -> 7.9 GB.
- **Faster MP3 export**: the encoder uses LAME's recommended "near best" setting, 2.8 times faster on real speech, same file size.
- **Stability**: an empty audio buffer from the engine could crash the program; a text with nothing to say (punctuation only, a word replaced by nothing) is now ignored or reported as empty; the engine no longer stays blocked after a panic; a failing piece stops the others at once; Chinese and Japanese texts are split into sentences; the main voice is no longer required when the text only uses voice tags; the history never keeps a half-written generation; one memory reservation for the whole audio (lower peak on hour-long texts).
- **Processes and installs**: llama-server and whisper-cli now die with the application (no 2.4 GB server left behind when closing during a writing); downloads have network timeouts; Whisper and llama.cpp are unpacked in a temporary folder then renamed (no half-installed engine); concurrent transcriptions no longer overwrite each other's temporary file; the update script never copies while the application is still running and only stops servers of its own folder; unpacking checks the real size written.
- **Voices and exports**: an unreadable bundled voice no longer prevents the next ones (Zen included) from being imported; two voices with similar names no longer overwrite each other's export or track files; inconsistent data and NaN samples no longer make the quality assistant or the exports panic.
- **Interface** (12 fixes): a new generation could show the waveform of an older deleted one; the connection instructions showed an empty folder; "Clear" in phonetic mode lost the next text; keyboard focus was lost in the Settings tabs and when leaving a transcript; the writing window announced "0 MB" before Settings had been opened; fast clicks on two "Play" buttons left the buttons out of sync; Ctrl+Enter could start a generation for a voice without samples; an unknown quality code could break the voice list; confirmation boxes now focus "Cancel"; a failing recorder released neither the microphone nor its timer; a border colour fixed for one theme.
- Cleaning: no dead code left (one unused constant removed, redundant style rules removed), PowerShell calls without a flashing console window, help of the command line completed, 74 automated tests.

**Français**
- **Synthèse plus rapide** : le moteur calcule maintenant jusqu'à 4 morceaux d'un texte ensemble (le nombre dépend de la mémoire vidéo libre : 4 avec environ 11 Go libres, 1 sur une petite carte, 2 quand elle ne se mesure pas). Mesuré sur une RX 6700 XT : un texte de 12 minutes passe de 154 s à 85 s (-45 %), un texte d'une minute de 12 s à 7 s, la première génération de 17 s à 12 s. La qualité est inchangée (même exactitude de transcription) et le résultat reste reproductible (fichiers identiques pour des demandes identiques). Pic de mémoire vidéo : 5,2 Go -> 7,9 Go.
- **Export MP3 plus rapide** : l'encodeur utilise le réglage « near best » recommandé par LAME, 2,8 fois plus rapide sur de la vraie parole, pour une taille de fichier identique.
- **Stabilité** : un tampon audio vide renvoyé par le moteur pouvait faire planter le programme ; un texte sans rien à dire (ponctuation seule, mot remplacé par du vide) est maintenant ignoré ou signalé vide ; le moteur ne reste plus bloqué après une panique ; un morceau qui échoue arrête aussitôt les autres ; les textes chinois et japonais sont découpés en phrases ; la voix principale n'est plus exigée quand le texte n'utilise que des balises de voix ; l'historique ne garde jamais une génération à moitié écrite ; une seule réservation de mémoire pour tout l'audio (pic plus bas sur les textes d'une heure).
- **Processus et installations** : llama-server et whisper-cli meurent maintenant avec l'application (plus de serveur de 2,4 Go oublié si on ferme pendant une écriture) ; les téléchargements ont des délais réseau ; Whisper et llama.cpp sont décompressés dans un dossier temporaire puis renommés (plus de moteur à moitié installé) ; deux transcriptions simultanées ne s'écrasent plus leur fichier temporaire ; le script de mise à jour ne copie jamais tant que l'application tourne et n'arrête que les serveurs de son propre dossier ; la décompression vérifie la taille réellement écrite.
- **Voix et exports** : une voix livrée illisible n'empêche plus l'import des suivantes (Zen comprise) ; deux voix aux noms proches ne s'écrasent plus leur export ni leurs pistes ; des données incohérentes ou des échantillons NaN ne font plus paniquer l'assistant de qualité ni les exports.
- **Interface** (12 corrections) : une nouvelle génération pouvait afficher la forme d'onde d'une ancienne supprimée ; les instructions de connexion affichaient un dossier vide ; « Effacer » en mode phonétique faisait perdre le texte suivant ; le focus clavier se perdait dans les onglets des Paramètres et en quittant une transcription ; la fenêtre d'écriture annonçait « 0 Mo » avant d'avoir ouvert les Paramètres ; deux clics rapides sur « Écouter » désynchronisaient les boutons ; Ctrl+Entrée pouvait lancer une génération pour une voix sans échantillon ; un code de qualité inconnu pouvait casser la liste des voix ; les confirmations placent le focus sur « Annuler » ; un enregistreur en échec ne rendait ni le micro ni son minuteur ; une couleur de bordure fixée pour un seul thème.
- Nettoyage : plus de code mort (une constante inutilisée et des règles de style redondantes retirées), appels PowerShell sans console qui clignote, aide de la ligne de commande complétée, 74 tests automatiques.

## 0.2.2 - 2026-11-13

**English**
- **Write with AI (local)**: an icon above the text box opens a window to describe a story or a dialogue; a light AI (Qwen3 4B Instruct, Apache 2.0, French and English, run by llama.cpp with Vulkan) writes it with your `[Voice]` tags and `[pause]` tags. Choose the type (dialogue or story), the language, the tone (natural, warm, cheerful, dramatic, funny, calm), the voices and the **reading time in minutes (15 seconds to 2 hours)**. The number of words comes from the measured speech rate of the synthetic voice (about 185 words per minute for a lively dialogue, 158 for a calm story); texts of more than about 280 words are planned first, then written in parts, each part getting the plan and the end of the previous one, so a one-hour podcast stays coherent. The text already written stays visible if you cancel. Measured: 60 minutes asked, 62.5 minutes read (11,350 words, 40 parts, written in under 5 minutes). Installed from Settings > Models (engine 37 MB and model 2.4 GB, SHA-256 checked); the server only runs during a request, on 127.0.0.1. Command line: `voiceclone-server write`.
- **Pauses and intonation in written texts**: the AI adds `[pause 0.5s]`, `[pause 1s]` and `[pause 2s]`, and writes with the punctuation and interjections that drive the voice's intonation (measured: lively text gives 8 to 13 semitones of pitch range, flat text about 5). Dialogues are checked so that every line has a speaker tag (one retry if not).
- **Phonetic version**: an "Original | Phonetic" switch at the top right of the text box rewrites a French text the way a French speaker says it: about sixty English or technical words respelled ("cloud" -> "claoude"), acronyms spelled out ("GPU" -> "gé pé u"), numbers in words. Tags are untouched, your dictionary comes first, each version stays editable.
- **Pronunciation dictionary and numbers in words** (new Settings tab): replacements applied before synthesis, and numbers (French and English: integers, decimals, %, euros, dollars) written out in words. The displayed text, the history and the subtitles keep the original spelling.
- **Automatic transcription (Whisper, local)**: "Transcribe" button on each sample; whisper.cpp and the small model (190 MB) are installed from Settings > Models, SHA-256 checked. Command line: `voiceclone-server transcribe`.
- **Subtitles and per-voice tracks**: every new generation keeps its timeline; "SRT" saves subtitles (one per sentence) and "Tracks" (multi-voice texts) writes one WAV per voice of the same length plus the subtitles.
- **Estimates under the text box**: characters, words, dialogue duration and generation time, calibrated on your last generation; the voice is no longer shown when the text contains voice tags.
- Behind the scenes: new modules for transcription, writing, pronunciation and export; 69 automated tests.

**Français**
- **Écrire avec l'IA (locale)** : une icône au-dessus de la zone de texte ouvre une fenêtre pour décrire un récit ou un dialogue ; une IA légère (Qwen3 4B Instruct, Apache 2.0, français et anglais, lancée par llama.cpp avec Vulkan) l'écrit avec vos balises `[Voix]` et `[pause]`. Choisissez le type (dialogue ou récit), la langue, le ton (naturel, chaleureux, enjoué, dramatique, humoristique, calme), les voix et la **durée de lecture en minutes (de 15 secondes à 2 heures)**. Le nombre de mots vient du débit mesuré de la voix de synthèse (environ 185 mots par minute pour un dialogue vif, 158 pour un récit calme) ; au-delà d'environ 280 mots, le texte est d'abord planifié puis écrit par parties, chacune recevant le plan et la fin de la précédente, pour qu'un podcast d'une heure reste cohérent. Le texte déjà écrit reste visible si vous annulez. Mesuré : 60 minutes demandées, 62,5 minutes lues (11 350 mots, 40 parties, écrites en moins de 5 minutes). Installation depuis Paramètres > Modèles (moteur 37 Mo et modèle 2,4 Go, SHA-256 vérifié) ; le serveur ne tourne que le temps d'une demande, sur 127.0.0.1. Ligne de commande : `voiceclone-server write`.
- **Pauses et intonation dans les textes écrits** : l'IA ajoute des `[pause 0.5s]`, `[pause 1s]` et `[pause 2s]`, et écrit avec la ponctuation et les interjections qui pilotent l'intonation de la voix (mesuré : un texte vivant donne 8 à 13 demi-tons d'amplitude de hauteur, un texte plat environ 5). Les dialogues sont contrôlés pour que chaque ligne ait sa balise de voix (une nouvelle tentative sinon).
- **Version phonétique** : un interrupteur « Original | Phonétique » en haut à droite de la zone de texte réécrit un texte français comme un francophone le dit : une soixantaine de mots anglais ou techniques respelés (« cloud » -> « claoude »), sigles épelés (« GPU » -> « gé pé u »), nombres en lettres. Les balises ne sont pas touchées, votre dictionnaire passe en premier, chaque version reste modifiable.
- **Dictionnaire de prononciation et nombres en lettres** (nouvel onglet des Paramètres) : remplacements appliqués avant la synthèse, et nombres (français et anglais : entiers, décimaux, %, euros, dollars) écrits en toutes lettres. Le texte affiché, l'historique et les sous-titres gardent l'écriture d'origine.
- **Transcription automatique (Whisper, en local)** : bouton « Transcrire » sur chaque échantillon ; whisper.cpp et le modèle small (190 Mo) s'installent depuis Paramètres > Modèles, SHA-256 vérifié. Ligne de commande : `voiceclone-server transcribe`.
- **Sous-titres et pistes par voix** : chaque nouvelle génération garde sa chronologie ; « SRT » enregistre les sous-titres (un par phrase) et « Pistes » (textes à plusieurs voix) écrit un WAV par voix de même durée, plus les sous-titres.
- **Estimations sous la zone de texte** : caractères, mots, durée du dialogue et temps de génération, calibrés sur votre dernière génération ; la voix n'est plus affichée quand le texte contient des balises de voix.
- En coulisses : nouveaux modules de transcription, d'écriture, de prononciation et d'export ; 69 tests automatiques.

## 0.2.1 - 2026-11-12

**English**
- The browser's right-click menu (Print, Save as, More tools...) no longer appears in the app. It stays in text fields (cut and paste) and on selected text (copy).

**Français**
- Le menu clic droit du navigateur (Imprimer, Enregistrer sous, Outils supplémentaires...) ne s'affiche plus dans l'application. Il reste dans les champs de saisie (couper, coller) et sur du texte sélectionné (copier).

## 0.2.0 - 2026-11-12

**English**
- **Voice quality assistant**: under each sample, pitch, peak and signal/noise measures, and advice when a sample is too short, clipping, too quiet, noisy, has no transcript or a transcript that does not fit, or when the samples of one voice come from different people (pitch gap). Command line: `voiceclone-server voice-check <name>`.
- **Export and import of voices** (Settings > Voices): a voice is exported as a folder (samples and transcripts) in the program's `Voix` subfolder; the import adds the voices found there, leaves existing ones untouched and reports unreadable ones without stopping. Command line: `export-voice <name> [folder]`, `import-voices [folder]`.
- **One-click update**: "Install the update" downloads the release archive, checks its SHA-256 against the hash published by GitHub (no hash, no automatic install), unpacks it safely, then a script replaces the program files after it closes and restarts it. The `data`, `Audios` and `Voix` folders and the `portable` file are never touched.
- **Settings in tabs**, with a layout for each theme, and confirmations in a "VoiceClone says" box instead of the browser's dialog. New banner image.

**Français**
- **Assistant de qualité des voix** : sous chaque échantillon, mesures de hauteur, de crête et d'écart signal/bruit, et conseils quand un échantillon est trop court, sature, est trop faible, bruité, sans transcription ou avec une transcription qui ne correspond pas, ou quand les échantillons d'une voix viennent de personnes différentes (écart de hauteur). Ligne de commande : `voiceclone-server voice-check <nom>`.
- **Export et import des voix** (Paramètres > Voix) : une voix est exportée sous forme de dossier (échantillons et transcriptions) dans le sous-dossier `Voix` du programme ; l'import ajoute les voix qui s'y trouvent, laisse les existantes intactes et signale les illisibles sans s'arrêter. Ligne de commande : `export-voice <nom> [dossier]`, `import-voices [dossier]`.
- **Mise à jour en un clic** : « Installer la mise à jour » télécharge l'archive de la release, vérifie son SHA-256 avec l'empreinte publiée par GitHub (pas d'empreinte, pas d'installation automatique), la décompresse en sécurité, puis un script remplace les fichiers du programme après sa fermeture et le relance. Les dossiers `data`, `Audios` et `Voix` et le fichier `portable` ne sont jamais touchés.
- **Paramètres en onglets**, avec une disposition par thème, et confirmations dans une boîte « VoiceClone indique » au lieu de la boîte du navigateur. Nouvelle image d'en-tête.

## 0.1.9 - 2026-11-11

**English**
- **Delete all generated audios**: a trash icon in the Generations header, with a confirmation that shows the number of audios (history and files, enhanced versions included).
- **"multiple" label**: a generation spoken by several voices is labelled "Voice: multiple" instead of the name of one voice.
- **Transcript check**: a sample whose transcript does not fit the length of its audio (about 7 to 30 characters per second is expected) is flagged in the samples panel and ignored for cloning (timbre only), with a warning after the generation. Such a transcript made voices drift and read the wrong text. Chinese, Japanese and Korean are not checked.
- New banner image in the README.

**Français**
- **Vider tous les audios générés** : une icône corbeille dans l'en-tête des Générations, avec une confirmation qui indique le nombre d'audios (historique et fichiers, versions améliorées comprises).
- **Étiquette « multiple »** : une génération dite par plusieurs voix est étiquetée « Voix : multiple » au lieu du nom d'une seule voix.
- **Contrôle des transcriptions** : un échantillon dont la transcription ne correspond pas à la durée de son audio (environ 7 à 30 caractères par seconde attendus) est signalé dans le panneau des échantillons et ignoré pour le clonage (timbre seul), avec un avertissement après la génération. Une telle transcription faisait dériver les voix, qui lisaient un autre texte. Le chinois, le japonais et le coréen ne sont pas contrôlés.
- Nouvelle image d'en-tête dans le README.

## 0.1.8 - 2026-11-11

**English**
- Fix: the voice colours (text, avatars) did not show in the app window, because its security policy blocks inline styles; the hue is now set through the CSS object model.

**Français**
- Correction : les couleurs des voix (texte, avatars) ne s'affichaient pas dans la fenêtre de l'application, dont la politique de sécurité bloque les styles en ligne ; la teinte est maintenant posée par le modèle objet CSS.

## 0.1.7 - 2026-11-11

**English**
- **A colour per voice**: each voice has its own colour (avatar, tag button, voice label in the list), and the text box repaints every part of the text in the colour of the voice that will speak it. Tags are bold, `[pause]` is grey italic, an unknown tag is underlined in red. Lightness and saturation follow the theme, so the colours stay readable in all four themes.

**Français**
- **Une couleur par voix** : chaque voix a sa couleur (avatar, bouton de balise, étiquette de voix dans la liste), et la zone de texte repeint chaque partie du texte avec la couleur de la voix qui la dira. Les balises sont en gras, `[pause]` en italique gris, une balise inconnue est soulignée en rouge. La clarté et la saturation suivent le thème : les couleurs restent lisibles dans les quatre thèmes.

## 0.1.6 - 2026-11-11

**English**
- The version number is shown in the title bar, next to the program name ("VoiceClone 0.1.6").

**Français**
- Le numéro de version s'affiche dans la barre de titre, à côté du nom du programme (« VoiceClone 0.1.6 »).

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
