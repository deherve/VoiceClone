// Interface bilingue : francais et anglais. Les textes statiques portent data-i18n (ou data-i18n-attr="attribut:cle"),
// les textes construits par main.js passent par t(). Les deux dictionnaires doivent avoir exactement les memes cles
// (verifie par scripts/check-i18n.mjs).
const I18N = {
  fr: {
    'nav.pages': 'Pages', 'nav.studio': 'Studio', 'nav.settings': 'Paramètres',
    'mcp.active': 'Connecteur MCP actif · {url}', 'mcp.stopped': 'Connecteur MCP arrêté : {error}',
    'common.play': 'Écouter', 'common.stop': 'Arrêter', 'common.stop_playback': 'Arrêter la lecture', 'common.delete': 'Supprimer',
    'common.cancel': 'Annuler', 'common.error': 'Erreur : {msg}',

    // Voix
    'voices.title': 'Voix', 'voices.new': '+ Nouvelle voix', 'voices.name': 'Nom', 'voices.language': 'Langue', 'voices.add': 'Ajouter la voix',
    'voices.empty': 'Aucune voix. Créez-en une ci-dessous.', 'voices.created': 'Voix créée. Ajoutez maintenant un échantillon.',
    'voice.meta_one': '{lang} · {n} échantillon', 'voice.meta_other': '{lang} · {n} échantillons',
    'voice.delete': 'Supprimer cette voix', 'voice.delete_aria': 'Supprimer la voix {name}',
    'voice.delete_confirm': 'Supprimer la voix « {name} » et ses échantillons ? Cette action est définitive.',
    'lang.french': 'Français', 'lang.english': 'Anglais', 'lang.german': 'Allemand', 'lang.spanish': 'Espagnol', 'lang.italian': 'Italien',
    'lang.portuguese': 'Portugais', 'lang.japanese': 'Japonais', 'lang.korean': 'Coréen', 'lang.chinese': 'Chinois', 'lang.russian': 'Russe',

    // Echantillons
    'samples.title': 'Échantillons de {name}',
    'samples.hint': "Enregistrez-vous ou importez un fichier audio, sans limite de durée. La transcription exacte rend le clonage plus fidèle.",
    'samples.rec': 'Enregistrer', 'samples.import': 'Importer', 'samples.stop_rec': 'Arrêter ({time})',
    'sample.label': 'Échantillon {n} · {dur}', 'sample.placeholder': 'Transcription exacte de cet échantillon',
    'sample.transcript_aria': "Transcription de l'échantillon {n}", 'sample.delete_aria': "Supprimer l'échantillon {n}",
    'sample.delete_confirm': "Supprimer l'échantillon {n} ? Cette action est définitive.", 'sample.saved': 'Transcription enregistrée.',
    'sample.converting': "Conversion de l'audio…",
    'sample.added_guided': 'Échantillon ajouté. Corrigez la transcription si vous avez dit autre chose.',
    'sample.added': 'Échantillon ajouté. Saisissez sa transcription exacte.',
    'sample.rec_guided': 'Enregistrement en cours. Lisez le texte ci-dessus, ou dites autre chose et corrigez la transcription ensuite.',
    'sample.rec_plain': 'Enregistrement en cours.',
    'sample.mic_error': 'micro inaccessible ({msg})', 'sample.file_error': 'fichier audio illisible ({msg})',
    'notes.none': 'Ajoutez un échantillon pour pouvoir générer avec cette voix.',
    'notes.no_text': 'Transcription manquante : le clonage se limite au timbre de la voix, il est moins fidèle.',
    'notes.long': "{total} d'échantillons au total : au-delà de {limit} environ, la génération ralentit et peut dérailler.",
    'readme.prefix': 'Texte à lire : ',
    'readme.french': "Bonjour, et bienvenue. Aujourd'hui, je prends un moment pour enregistrer ma voix, calmement, sans me presser. Est-ce que le ton est naturel ? Je crois que oui. Je parle comme d'habitude, avec des phrases courtes, puis d'autres un peu plus longues, pour donner du rythme. Voilà, c'est terminé : merci de m'avoir écouté !",
    'readme.english': "Hello, and welcome. Today I am taking a moment to record my voice, calmly, without rushing. Does the tone sound natural? I think so. I speak the way I usually do, with short sentences, then a few longer ones, to give it some rhythm. There, that's it: thank you for listening!",

    // Saisie
    'compose.title': 'Texte à dire', 'compose.placeholder': 'Saisissez le texte à générer avec la voix sélectionnée.',
    'compose.clear': 'Effacer le texte', 'compose.hint': 'Ctrl + Entrée pour lancer', 'compose.go': 'Générer',
    'compose.line_one': 'Voix : {name} · {n} caractère', 'compose.line_other': 'Voix : {name} · {n} caractères',
    'compose.no_voice': 'Aucune voix sélectionnée',
    'compose.generating': 'Génération en cours… Le premier appel charge le modèle, il est plus long.',
    'compose.done': "{dur} d'audio générées en {s} s.",

    // Generations
    'gen.title': 'Générations', 'gen.count_one': '{n} audio', 'gen.count_other': '{n} audios',
    'gen.empty': 'Les audios générés apparaîtront ici, y compris ceux demandés par Claude Code.',
    'gen.voice': 'Voix : {name}', 'gen.voice_deleted': 'supprimée',
    'gen.copy': 'Copier le texte', 'gen.copy_aria': 'Copier le texte de cette génération', 'gen.copied': 'Texte copié.', 'gen.copy_failed': 'copie impossible',
    'gen.folder': 'Dossier', 'gen.folder_aria': "Montrer le fichier dans l'Explorateur",
    'gen.original': 'Original', 'gen.original_title': 'Audio tel que généré', 'gen.enhanced': 'Amélioré',
    'gen.enhanced_title': 'Égaliseur de voix et volume normalisé', 'gen.version_aria': "Version de l'audio",
    'gen.enh_on': 'Amélioration activée : voix égalisée et volume normalisé.', 'gen.enh_off': "Audio d'origine rétabli.",
    'gen.delete': 'Supprimer cette génération', 'gen.delete_confirm': 'Supprimer cette génération et son fichier audio ? Cette action est définitive.',
    'gen.dl_aria': 'Télécharger en {fmt}', 'gen.dl_aria_enh': 'Télécharger en {fmt} (version améliorée)',
    'gen.dl_choose': 'Choisissez le dossier et le nom du {fmt}…', 'gen.saved': 'Enregistré : {path}', 'gen.cancelled': 'Téléchargement annulé.',

    // Parametres : langue, apparence
    'set.language': 'Langue', 'set.language_hint': "Langue de l'interface et des messages. Le choix est mémorisé.",
    'lang.ui.fr': 'Français', 'lang.ui.en': 'English',
    'set.appearance': 'Apparence', 'set.appearance_hint': "Thème de l'interface. Le choix est mémorisé.",
    'theme.nocturne': 'Nocturne', 'theme.nocturne_desc': 'Sombre, accent ambre',
    'theme.atelier': 'Atelier', 'theme.atelier_desc': 'Clair, papier crème, titres en serif',
    'theme.console': 'Console', 'theme.console_desc': 'Graphite et turquoise, style studio audio',
    'theme.conversation': 'Conversation', 'theme.conversation_desc': 'Clair et arrondi, générations en bulles de discussion',

    // Connexion IA / ligne de commande
    'conn.title': 'Connecter une IA ou un script',
    'conn.hint': "Copiez ces éléments dans votre IA, votre skill ou votre script. Les chemins sont ceux de cette installation de VoiceClone : ils suivent l'endroit où le programme se trouve.",
    'conn.no_server': "Le fichier voiceclone-server.exe est introuvable à côté de l'application : la version serveur et la ligne de commande ne sont pas disponibles dans cette installation. Le connecteur MCP de la fenêtre, lui, fonctionne.",
    'conn.copy': 'Copier', 'conn.copied': 'Copié', 'conn.copy_aria': 'Copier : {title}', 'conn.copied_status': 'Copié dans le presse-papiers.',
    'conn.mcp_json': 'Déclaration MCP (.mcp.json du projet)', 'conn.claude_cmd': 'Ou avec la commande de Claude Code',
    'conn.start': 'Lancer la version serveur, sans fenêtre (PowerShell)',
    'conn.start_note': "Inutile si VoiceClone est déjà ouvert : sa fenêtre expose le même connecteur. Ne lancez pas les deux en même temps (même port).",
    'conn.cli_ps': 'Génération rapide en ligne de commande (PowerShell)', 'conn.cli_cmd': 'Génération rapide en ligne de commande (Invite de commandes)',
    'conn.cmt_std': 'qualité standard', 'conn.cmt_enh': 'qualité améliorée (égaliseur + volume normalisé)', 'conn.cmt_out': 'destination personnalisée (sans --out : dossier Audios)',
    'conn.cli_note': "Langue : fr, en, de, es, it, pt, ja, ko, zh, ru. Qualité : standard ou enhanced (ameliore). Voix : le nom d'une voix. Sans --out, le fichier est créé dans {audios}. La commande affiche le chemin du fichier. Elle utilise le serveur s'il tourne déjà, sinon elle charge le modèle pour cette seule génération.",
    'conn.ai': 'Instructions complètes pour une IA ou un skill', 'conn.ai_copy': 'Copier les instructions pour une IA', 'conn.ai_preview': 'Aperçu des instructions',
    'conn.sample': 'Bonjour à tous', 'conn.sample_lang': 'fr',
    'conn.doc': `## Voix de synthèse : VoiceClone (connecteur MCP)

### Connexion
Serveur MCP en HTTP, local : {url} (aucune authentification, écoute seulement sur 127.0.0.1).

Déclaration dans le \`.mcp.json\` du projet :
\`\`\`json
{ "mcpServers": { "voiceclone": { "type": "http", "url": "{url}" } } }
\`\`\`
Ou : \`claude mcp add --transport http voiceclone {url}\`
Outils : \`mcp__voiceclone__speak\`, \`mcp__voiceclone__list_profiles\`, \`mcp__voiceclone__list_generations\`.

### Lancer la version serveur (sans fenêtre)
1. Tester si le serveur répond : POST JSON-RPC \`{"jsonrpc":"2.0","id":1,"method":"tools/list"}\` sur {url}.
2. Sinon le lancer en arrière-plan (PowerShell), puis attendre 2 s : \`Start-Process "{server}" -WindowStyle Hidden\`
3. Ne pas le relancer s'il tourne déjà. Ne jamais lancer en même temps VoiceClone.exe (même port) ; si la fenêtre est ouverte, s'en servir telle quelle.
4. La première génération charge le modèle (environ 10 s), ensuite environ 1 s par phrase.

### Outil \`speak\` (bloquant)
- \`text\` (obligatoire) : texte à dire, nombres en toutes lettres, une phrase par appel (80 à 130 caractères).
- \`output_path\` : chemin ABSOLU en .wav (dossiers créés). Sans lui, le fichier est dans le dossier Audios du programme (le chemin est renvoyé).
- \`profile\` : nom de la voix (\`list_profiles\`), par exemple "{voice}". Défaut : la première voix.
- \`language\` : french, english, german, spanish, italian, portuguese, japanese, korean, chinese, russian. Défaut : langue de la voix.
- \`enhanced\` : true = version améliorée (égaliseur de voix + volume normalisé à -18 dBFS, crêtes sous -1 dBFS) ; false (défaut) = brute (filtre anti-souffle seulement).
- \`seed\` : défaut 7 (résultat reproductible).

Avec amélioration : \`{ "text": "{sample}", "profile": "{voice}", "language": "{lang}", "enhanced": true, "output_path": "C:\\\\projet\\\\voix\\\\01.wav" }\`
Sans amélioration : même appel avec \`"enhanced": false\`.
Résultat : JSON \`{"id":…,"path":"…","duration_sec":…,"profile":"…","seed":7,"enhanced":true,"elapsed_sec":…}\` ; fichier WAV 24 kHz mono 16 bits. En cas d'échec : \`isError: true\` et un message.

### Ligne de commande (sans MCP)
PowerShell : \`& "{server}" speak "{sample}" --lang {lang} --quality enhanced --voice {voice}\`
Invite de commandes : \`"{server}" speak "{sample}" --lang {lang} --quality enhanced --voice {voice}\`
Options : --text, --lang (fr, en, de, es, it, pt, ja, ko, zh, ru), --quality (standard | enhanced), --voice, --out (fichier .wav ou dossier ; défaut : le dossier Audios du programme), --seed.
Affiche le chemin du fichier créé. Utilise le serveur s'il tourne déjà, sinon charge le modèle pour cette génération.

### Autres outils
\`list_profiles\` (id, name, language, samples), \`list_generations\` (limit ; path, duration_sec, enhanced, text).

### Prérequis
Modèle et codec téléchargés (VoiceClone, Paramètres, Modèles) ; la voix doit avoir au moins un échantillon et sa transcription.`,

    // Mises a jour
    'upd.title': 'Mises à jour', 'upd.installed': 'Version installée : {version}', 'upd.check': 'Vérifier maintenant',
    'upd.auto': 'Vérifier au démarrage (une requête à GitHub, aucune donnée envoyée)', 'upd.checking': 'Vérification…',
    'upd.uptodate': 'VoiceClone est à jour (version {version}).', 'upd.available': 'Version {latest} disponible (vous avez la {current}).',
    'upd.notes': 'Nouveautés', 'upd.open': 'Voir et télécharger la version', 'upd.badge': 'Mise à jour {latest}',
    'upd.how': "Téléchargez l'archive sur la page de la version, fermez VoiceClone puis remplacez ses fichiers : vos voix et vos générations (dans %APPDATA%\\VoiceClone) sont conservées.",
    'upd.size': 'Archive : {name} · {size}',

    // Carte graphique
    'aud.title': 'Dossier des audios', 'aud.hint': "Tous les audios générés (application, MCP, ligne de commande) sont enregistrés dans le sous-dossier Audios du programme.", 'aud.open': 'Ouvrir le dossier',
    'gpu.title': 'Carte graphique', 'gpu.none': 'Aucun périphérique de calcul détecté.',
    'gpu.summary': 'Meilleur choix détecté : {best}. {state}', 'gpu.loaded': 'Calcul en cours sur {name}.', 'gpu.not_loaded': 'Le modèle est chargé à la prochaine génération.',
    'gpu.device': 'Périphérique de calcul', 'gpu.auto': 'Automatique : {name} (recommandé)', 'gpu.cpu_name': 'processeur', 'gpu.recommended': 'recommandé',
    'gpu.best_hint': "Le meilleur choix détecté est {name} ({desc}). Choisissez « Automatique » pour l'utiliser.",
    'gpu.nvidia': "Une carte NVIDIA est détectée via Vulkan : elle fonctionne, mais la version CUDA de VoiceClone (fichier ggml-cuda.dll) serait plus rapide.",
    'gpu.fallback': 'Repli automatique : {note}.', 'gpu.advanced': 'Options avancées',
    'gpu.advanced_hint': "« Automatique » convient dans presque tous les cas. À modifier seulement si le son est abîmé ou si la génération échoue sur votre carte.",
    'gpu.fa': 'Attention « flash » (calcul plus rapide sur certaines cartes)', 'gpu.clamp': 'Garde FP16 (évite les débordements des cartes NVIDIA anciennes, un peu plus lent)',
    'gpu.saved': 'Enregistré. Le modèle sera rechargé avec ce réglage à la prochaine génération.', 'gpu.gb': '{n} Go',
    'tri.auto': 'Automatique (recommandé)', 'tri.on': 'Activée', 'tri.off': 'Désactivée',

    // Composants
    'comp.title': 'Composants et outils',
    'comp.hint': "Ce dont VoiceClone peut avoir besoin sur cet ordinateur. Les installations viennent des sources officielles et sont vérifiées avant d'être utilisées.",
    'comp.mp3': 'MP3 (LAME)', 'comp.mp3_state': "Intégré à l'application : rien à installer.",
    'comp.vulkan': 'Vulkan (calcul sur carte graphique)', 'comp.vulkan_ok': 'Installé.',
    'comp.vulkan_missing': 'Absent : le calcul se fait sur le processeur, beaucoup plus lent. Le plus simple est de mettre à jour le pilote de la carte graphique.',
    'comp.cuda': 'CUDA (cartes NVIDIA)', 'comp.cuda_na': 'Aucune carte NVIDIA détectée : sans objet.',
    'comp.cuda_driver': 'Carte NVIDIA détectée, mais le pilote NVIDIA (CUDA) est absent.',
    'comp.cuda_module': "Pilote NVIDIA présent. Cette version de VoiceClone n'inclut pas le module CUDA (ggml-cuda.dll) : le calcul passe par Vulkan.",
    'comp.cuda_runtime': 'Les bibliothèques CUDA 12 (cudart, cublas) sont absentes.', 'comp.cuda_ok': 'Installé.',
    'comp.install_vulkan': 'Installer le runtime Vulkan (LunarG, {size})', 'comp.install_cuda': 'Installer les bibliothèques CUDA (NVIDIA, {size})',
    'comp.drivers_nvidia': 'Pilotes NVIDIA', 'comp.drivers_amd': 'Pilotes AMD', 'comp.drivers_intel': 'Pilotes Intel',
    'comp.confirm_vulkan': "Télécharger le programme d'installation du runtime Vulkan depuis sdk.lunarg.com (21 Mo) ?\n\nIl est vérifié (empreinte SHA-256 et signature « LunarG, Inc. ») avant d'être lancé. Windows demandera votre autorisation.",
    'comp.confirm_cuda': "Télécharger les bibliothèques CUDA 12 depuis developer.download.nvidia.com (environ 528 Mo) ?\n\nElles sont vérifiées avec l'empreinte SHA-256 du manifeste officiel de NVIDIA, puis copiées dans le dossier de données de VoiceClone.",
    'comp.stage_manifest': 'Lecture du manifeste NVIDIA…', 'comp.stage_download': 'Téléchargement… {pct} %', 'comp.stage_verify': 'Vérification…',
    'comp.stage_extract': 'Extraction…', 'comp.stage_launch': "Lancement de l'installation…",
    'comp.done_launched': "Le programme d'installation est lancé : confirmez dans la fenêtre Windows, puis relancez VoiceClone.",
    'comp.done_installed': 'Bibliothèques installées. Relancez VoiceClone pour les utiliser.',

    // Modeles et memoire
    'models.title': 'Modèles',
    'models.hint': 'Un modèle de synthèse et le codec audio sont nécessaires. Les fichiers viennent de {source}. Changer de modèle prend effet à la génération suivante.',
    'models.state_active': 'utilisé pour les générations', 'models.state_installed': 'installé', 'models.state_missing': 'non installé', 'models.state_loaded': 'en mémoire',
    'models.use': 'Utiliser', 'models.download': 'Télécharger ({size})', 'models.downloading': 'Téléchargement de {label}…',
    'models.progress_aria': 'Téléchargement de {label}', 'models.progress': '{pct} % · {done} sur {total}',
    'models.use_done': '{label} sera utilisé à la prochaine génération.', 'models.installed_done': '{label} est installé.',
    'models.delete_confirm': "Supprimer {label} ({size}) du disque ? Il faudra le retélécharger pour s'en resservir.", 'models.deleted': '{label} est supprimé.',
    'models.action_aria': '{action} : {label}',
    'mem.title': 'Mémoire', 'mem.loaded': 'Un modèle est chargé en mémoire (environ 2,5 Go de RAM, plus la mémoire de la carte graphique). Le décharger libère cette mémoire ; il sera rechargé à la prochaine génération.',
    'mem.none': 'Aucun modèle en mémoire. Il sera chargé à la prochaine génération.', 'mem.unload': 'Décharger le modèle', 'mem.unloaded': 'Modèle déchargé.',
    'mem.open': 'Ouvrir le dossier des modèles', 'mem.folder': 'Dossier : {dir}',
    'unit.gb': 'Go', 'unit.mb': 'Mo'
  },

  en: {
    'nav.pages': 'Pages', 'nav.studio': 'Studio', 'nav.settings': 'Settings',
    'mcp.active': 'MCP connector active · {url}', 'mcp.stopped': 'MCP connector stopped: {error}',
    'common.play': 'Play', 'common.stop': 'Stop', 'common.stop_playback': 'Stop playback', 'common.delete': 'Delete',
    'common.cancel': 'Cancel', 'common.error': 'Error: {msg}',

    'voices.title': 'Voices', 'voices.new': '+ New voice', 'voices.name': 'Name', 'voices.language': 'Language', 'voices.add': 'Add the voice',
    'voices.empty': 'No voice yet. Create one below.', 'voices.created': 'Voice created. Now add a sample.',
    'voice.meta_one': '{lang} · {n} sample', 'voice.meta_other': '{lang} · {n} samples',
    'voice.delete': 'Delete this voice', 'voice.delete_aria': 'Delete the voice {name}',
    'voice.delete_confirm': 'Delete the voice "{name}" and its samples? This cannot be undone.',
    'lang.french': 'French', 'lang.english': 'English', 'lang.german': 'German', 'lang.spanish': 'Spanish', 'lang.italian': 'Italian',
    'lang.portuguese': 'Portuguese', 'lang.japanese': 'Japanese', 'lang.korean': 'Korean', 'lang.chinese': 'Chinese', 'lang.russian': 'Russian',

    'samples.title': 'Samples of {name}',
    'samples.hint': 'Record yourself or import an audio file, with no length limit. The exact transcript makes the cloning more faithful.',
    'samples.rec': 'Record', 'samples.import': 'Import', 'samples.stop_rec': 'Stop ({time})',
    'sample.label': 'Sample {n} · {dur}', 'sample.placeholder': 'Exact transcript of this sample',
    'sample.transcript_aria': 'Transcript of sample {n}', 'sample.delete_aria': 'Delete sample {n}',
    'sample.delete_confirm': 'Delete sample {n}? This cannot be undone.', 'sample.saved': 'Transcript saved.',
    'sample.converting': 'Converting the audio…',
    'sample.added_guided': 'Sample added. Correct the transcript if you said something else.',
    'sample.added': 'Sample added. Now enter its exact transcript.',
    'sample.rec_guided': 'Recording. Read the text above, or say something else and correct the transcript afterwards.',
    'sample.rec_plain': 'Recording.',
    'sample.mic_error': 'microphone unavailable ({msg})', 'sample.file_error': 'unreadable audio file ({msg})',
    'notes.none': 'Add a sample to be able to generate with this voice.',
    'notes.no_text': 'Transcript missing: cloning is limited to the timbre of the voice and is less faithful.',
    'notes.long': '{total} of samples in total: beyond about {limit}, generation slows down and may go wrong.',
    'readme.prefix': 'Text to read: ',
    'readme.french': "Bonjour, et bienvenue. Aujourd'hui, je prends un moment pour enregistrer ma voix, calmement, sans me presser. Est-ce que le ton est naturel ? Je crois que oui. Je parle comme d'habitude, avec des phrases courtes, puis d'autres un peu plus longues, pour donner du rythme. Voilà, c'est terminé : merci de m'avoir écouté !",
    'readme.english': "Hello, and welcome. Today I am taking a moment to record my voice, calmly, without rushing. Does the tone sound natural? I think so. I speak the way I usually do, with short sentences, then a few longer ones, to give it some rhythm. There, that's it: thank you for listening!",

    'compose.title': 'Text to say', 'compose.placeholder': 'Type the text to generate with the selected voice.',
    'compose.clear': 'Clear the text', 'compose.hint': 'Ctrl + Enter to generate', 'compose.go': 'Generate',
    'compose.line_one': 'Voice: {name} · {n} character', 'compose.line_other': 'Voice: {name} · {n} characters',
    'compose.no_voice': 'No voice selected',
    'compose.generating': 'Generating… The first call loads the model, so it takes longer.',
    'compose.done': '{dur} of audio generated in {s} s.',

    'gen.title': 'Generations', 'gen.count_one': '{n} audio', 'gen.count_other': '{n} audios',
    'gen.empty': 'Generated audio will appear here, including the audio requested by Claude Code.',
    'gen.voice': 'Voice: {name}', 'gen.voice_deleted': 'deleted',
    'gen.copy': 'Copy the text', 'gen.copy_aria': 'Copy the text of this generation', 'gen.copied': 'Text copied.', 'gen.copy_failed': 'copy failed',
    'gen.folder': 'Folder', 'gen.folder_aria': 'Show the file in Explorer',
    'gen.original': 'Original', 'gen.original_title': 'Audio as generated', 'gen.enhanced': 'Enhanced',
    'gen.enhanced_title': 'Voice equalizer and normalized volume', 'gen.version_aria': 'Audio version',
    'gen.enh_on': 'Enhancement on: voice equalized and volume normalized.', 'gen.enh_off': 'Original audio restored.',
    'gen.delete': 'Delete this generation', 'gen.delete_confirm': 'Delete this generation and its audio file? This cannot be undone.',
    'gen.dl_aria': 'Download as {fmt}', 'gen.dl_aria_enh': 'Download as {fmt} (enhanced version)',
    'gen.dl_choose': 'Choose the folder and the name of the {fmt}…', 'gen.saved': 'Saved: {path}', 'gen.cancelled': 'Download cancelled.',

    'set.language': 'Language', 'set.language_hint': 'Language of the interface and of the messages. Your choice is remembered.',
    'lang.ui.fr': 'Français', 'lang.ui.en': 'English',
    'set.appearance': 'Appearance', 'set.appearance_hint': 'Interface theme. Your choice is remembered.',
    'theme.nocturne': 'Nocturne', 'theme.nocturne_desc': 'Dark, amber accent',
    'theme.atelier': 'Atelier', 'theme.atelier_desc': 'Light, cream paper, serif headings',
    'theme.console': 'Console', 'theme.console_desc': 'Graphite and turquoise, audio studio style',
    'theme.conversation': 'Conversation', 'theme.conversation_desc': 'Light and rounded, generations as chat bubbles',

    'conn.title': 'Connect an AI or a script',
    'conn.hint': 'Copy these into your AI, skill or script. The paths are those of this VoiceClone installation: they follow wherever the program is located.',
    'conn.no_server': 'The file voiceclone-server.exe was not found next to the application: the server version and the command line are not available in this installation. The MCP connector of the window still works.',
    'conn.copy': 'Copy', 'conn.copied': 'Copied', 'conn.copy_aria': 'Copy: {title}', 'conn.copied_status': 'Copied to the clipboard.',
    'conn.mcp_json': 'MCP declaration (the project .mcp.json)', 'conn.claude_cmd': 'Or with the Claude Code command',
    'conn.start': 'Start the server version, without a window (PowerShell)',
    'conn.start_note': 'Not needed if VoiceClone is already open: its window exposes the same connector. Do not start both at the same time (same port).',
    'conn.cli_ps': 'Quick generation from the command line (PowerShell)', 'conn.cli_cmd': 'Quick generation from the command line (Command Prompt)',
    'conn.cmt_std': 'standard quality', 'conn.cmt_enh': 'enhanced quality (equalizer + normalized volume)', 'conn.cmt_out': 'custom destination (without --out: the Audios folder)',
    'conn.cli_note': 'Language: fr, en, de, es, it, pt, ja, ko, zh, ru. Quality: standard or enhanced. Voice: the name of a voice. Without --out, the file is created in {audios}. The command prints the path of the file. It uses the server if it is already running, otherwise it loads the model for this single generation.',
    'conn.ai': 'Complete instructions for an AI or a skill', 'conn.ai_copy': 'Copy the instructions for an AI', 'conn.ai_preview': 'Instructions preview',
    'conn.sample': 'Hello everyone', 'conn.sample_lang': 'en',
    'conn.doc': `## Speech synthesis: VoiceClone (MCP connector)

### Connection
Local MCP server over HTTP: {url} (no authentication, listens on 127.0.0.1 only).

Declaration in the project's \`.mcp.json\`:
\`\`\`json
{ "mcpServers": { "voiceclone": { "type": "http", "url": "{url}" } } }
\`\`\`
Or: \`claude mcp add --transport http voiceclone {url}\`
Tools: \`mcp__voiceclone__speak\`, \`mcp__voiceclone__list_profiles\`, \`mcp__voiceclone__list_generations\`.

### Start the server version (no window)
1. Check whether the server answers: JSON-RPC POST \`{"jsonrpc":"2.0","id":1,"method":"tools/list"}\` to {url}.
2. If not, start it in the background (PowerShell), then wait 2 s: \`Start-Process "{server}" -WindowStyle Hidden\`
3. Do not start it again if it is already running. Never start VoiceClone.exe at the same time (same port); if the window is open, just use it.
4. The first generation loads the model (about 10 s), then about 1 s per sentence.

### \`speak\` tool (blocking)
- \`text\` (required): text to say, numbers written out in words, one sentence per call (80 to 130 characters).
- \`output_path\`: ABSOLUTE path ending in .wav (folders are created). Without it, the file stays in VoiceClone's data folder.
- \`profile\`: name of the voice (\`list_profiles\`), for example "{voice}". Default: the first voice.
- \`language\`: french, english, german, spanish, italian, portuguese, japanese, korean, chinese, russian. Default: the voice's language.
- \`enhanced\`: true = enhanced version (voice equalizer + volume normalized to -18 dBFS, peaks below -1 dBFS); false (default) = raw (only a hiss filter).
- \`seed\`: default 7 (reproducible result).

Enhanced: \`{ "text": "{sample}", "profile": "{voice}", "language": "{lang}", "enhanced": true, "output_path": "C:\\\\project\\\\voice\\\\01.wav" }\`
Not enhanced: same call with \`"enhanced": false\`.
Result: JSON \`{"id":…,"path":"…","duration_sec":…,"profile":"…","seed":7,"enhanced":true,"elapsed_sec":…}\`; WAV file, 24 kHz mono 16-bit. On failure: \`isError: true\` and a message.

### Command line (no MCP)
PowerShell: \`& "{server}" speak "{sample}" --lang {lang} --quality enhanced --voice {voice}\`
Command Prompt: \`"{server}" speak "{sample}" --lang {lang} --quality enhanced --voice {voice}\`
Options: --text, --lang (fr, en, de, es, it, pt, ja, ko, zh, ru), --quality (standard | enhanced), --voice, --out (.wav file or folder; default: the program's Audios folder), --seed.
Prints the path of the file created. Uses the server if it is already running, otherwise loads the model for this generation.

### Other tools
\`list_profiles\` (id, name, language, samples), \`list_generations\` (limit; path, duration_sec, enhanced, text).

### Requirements
Model and codec downloaded (VoiceClone, Settings, Models); the voice needs at least one sample and its transcript.`,

    'upd.title': 'Updates', 'upd.installed': 'Installed version: {version}', 'upd.check': 'Check now',
    'upd.auto': 'Check at startup (one request to GitHub, no data sent)', 'upd.checking': 'Checking…',
    'upd.uptodate': 'VoiceClone is up to date (version {version}).', 'upd.available': 'Version {latest} is available (you have {current}).',
    'upd.notes': "What's new", 'upd.open': 'View and download the version', 'upd.badge': 'Update {latest}',
    'upd.how': 'Download the archive from the release page, close VoiceClone, then replace its files: your voices and generations (in %APPDATA%\\VoiceClone) are kept.',
    'upd.size': 'Archive: {name} · {size}',

    'aud.title': 'Audios folder', 'aud.hint': "Every generated audio (app, MCP, command line) is saved in the Audios subfolder of the program.", 'aud.open': 'Open folder',
    'gpu.title': 'Graphics card', 'gpu.none': 'No compute device detected.',
    'gpu.summary': 'Best choice detected: {best}. {state}', 'gpu.loaded': 'Computing on {name}.', 'gpu.not_loaded': 'The model is loaded at the next generation.',
    'gpu.device': 'Compute device', 'gpu.auto': 'Automatic: {name} (recommended)', 'gpu.cpu_name': 'processor', 'gpu.recommended': 'recommended',
    'gpu.best_hint': 'The best choice detected is {name} ({desc}). Choose "Automatic" to use it.',
    'gpu.nvidia': 'An NVIDIA card is detected through Vulkan: it works, but the CUDA version of VoiceClone (ggml-cuda.dll file) would be faster.',
    'gpu.fallback': 'Automatic fallback: {note}.', 'gpu.advanced': 'Advanced options',
    'gpu.advanced_hint': '"Automatic" is right in almost every case. Change it only if the sound is damaged or generation fails on your card.',
    'gpu.fa': 'Flash attention (faster on some cards)', 'gpu.clamp': 'FP16 guard (prevents overflows on older NVIDIA cards, slightly slower)',
    'gpu.saved': 'Saved. The model will be reloaded with this setting at the next generation.', 'gpu.gb': '{n} GB',
    'tri.auto': 'Automatic (recommended)', 'tri.on': 'On', 'tri.off': 'Off',

    'comp.title': 'Components and tools',
    'comp.hint': 'What VoiceClone may need on this computer. Installations come from the official sources and are verified before use.',
    'comp.mp3': 'MP3 (LAME)', 'comp.mp3_state': 'Built into the application: nothing to install.',
    'comp.vulkan': 'Vulkan (graphics card computing)', 'comp.vulkan_ok': 'Installed.',
    'comp.vulkan_missing': 'Missing: computing runs on the processor, much slower. The simplest fix is to update the graphics card driver.',
    'comp.cuda': 'CUDA (NVIDIA cards)', 'comp.cuda_na': 'No NVIDIA card detected: not applicable.',
    'comp.cuda_driver': 'NVIDIA card detected, but the NVIDIA (CUDA) driver is missing.',
    'comp.cuda_module': 'NVIDIA driver present. This version of VoiceClone does not include the CUDA module (ggml-cuda.dll): computing goes through Vulkan.',
    'comp.cuda_runtime': 'The CUDA 12 libraries (cudart, cublas) are missing.', 'comp.cuda_ok': 'Installed.',
    'comp.install_vulkan': 'Install the Vulkan runtime (LunarG, {size})', 'comp.install_cuda': 'Install the CUDA libraries (NVIDIA, {size})',
    'comp.drivers_nvidia': 'NVIDIA drivers', 'comp.drivers_amd': 'AMD drivers', 'comp.drivers_intel': 'Intel drivers',
    'comp.confirm_vulkan': 'Download the Vulkan runtime installer from sdk.lunarg.com (21 MB)?\n\nIt is verified (SHA-256 hash and "LunarG, Inc." signature) before it is started. Windows will ask for your permission.',
    'comp.confirm_cuda': 'Download the CUDA 12 libraries from developer.download.nvidia.com (about 528 MB)?\n\nThey are verified against the SHA-256 hash of the official NVIDIA manifest, then copied into the VoiceClone data folder.',
    'comp.stage_manifest': 'Reading the NVIDIA manifest…', 'comp.stage_download': 'Downloading… {pct}%', 'comp.stage_verify': 'Verifying…',
    'comp.stage_extract': 'Extracting…', 'comp.stage_launch': 'Starting the installer…',
    'comp.done_launched': 'The installer is started: confirm in the Windows window, then restart VoiceClone.',
    'comp.done_installed': 'Libraries installed. Restart VoiceClone to use them.',

    'models.title': 'Models',
    'models.hint': 'A synthesis model and the audio codec are required. The files come from {source}. Changing the model takes effect at the next generation.',
    'models.state_active': 'used for generations', 'models.state_installed': 'installed', 'models.state_missing': 'not installed', 'models.state_loaded': 'in memory',
    'models.use': 'Use', 'models.download': 'Download ({size})', 'models.downloading': 'Downloading {label}…',
    'models.progress_aria': 'Downloading {label}', 'models.progress': '{pct}% · {done} of {total}',
    'models.use_done': '{label} will be used at the next generation.', 'models.installed_done': '{label} is installed.',
    'models.delete_confirm': 'Delete {label} ({size}) from disk? You will have to download it again to use it.', 'models.deleted': '{label} is deleted.',
    'models.action_aria': '{action}: {label}',
    'mem.title': 'Memory', 'mem.loaded': 'A model is loaded in memory (about 2.5 GB of RAM, plus graphics card memory). Unloading frees this memory; the model is reloaded at the next generation.',
    'mem.none': 'No model in memory. It is loaded at the next generation.', 'mem.unload': 'Unload the model', 'mem.unloaded': 'Model unloaded.',
    'mem.open': 'Open the models folder', 'mem.folder': 'Folder: {dir}',
    'unit.gb': 'GB', 'unit.mb': 'MB'
  }
};

const LANG_KEY = 'voiceclone-lang';
let LANG = 'fr';

// Langue de depart : choix memorise, sinon celle du systeme (anglais sauf si le systeme est en francais).
function detectLang() {
  try { const l = localStorage.getItem(LANG_KEY); if (l in I18N) return l; } catch { /* stockage indisponible */ }
  return (navigator.language || 'fr').toLowerCase().startsWith('fr') ? 'fr' : 'en';
}

function t(key, vars) {
  const s = I18N[LANG][key] ?? I18N.fr[key] ?? key;
  return vars ? s.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? '')) : s;
}
// Pluriel : cles « xxx_one » et « xxx_other » ; vars.n decide.
const tp = (key, vars) => t(`${key}_${vars.n === 1 ? 'one' : 'other'}`, vars);

function applyStatic() {
  document.documentElement.lang = LANG;
  for (const n of document.querySelectorAll('[data-i18n]')) n.textContent = t(n.dataset.i18n);
  for (const n of document.querySelectorAll('[data-i18n-attr]')) {
    for (const pair of n.dataset.i18nAttr.split(';')) {
      const [attr, key] = pair.split(':');
      n.setAttribute(attr, t(key));
    }
  }
}

function setLanguage(lang) {
  LANG = lang in I18N ? lang : 'fr';
  try { localStorage.setItem(LANG_KEY, LANG); } catch { /* choix valable pour cette session seulement */ }
  applyStatic();
}

LANG = detectLang();
