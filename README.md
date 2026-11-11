# VoiceClone

Génération d'audio avec une voix clonée, en local (Qwen3-TTS via `qwentts.cpp`), pilotable par Claude Code (MCP).
Application Tauri (Rust) : `target\release\VoiceClone.exe`.

## Construire

```bat
build-qwen.cmd          :: moteur en Vulkan (AMD, NVIDIA, Intel)  → vendor\qwentts\build
cargo build --release   :: l'application ; copie les DLL du moteur à côté de VoiceClone.exe
cargo test --release -p voiceclone
cargo test --release -p voiceclone -- --ignored   :: repli du GPU, avec les vrais modèles installés
```

Prérequis : Rust, CMake, Visual Studio 2022 Build Tools, Vulkan SDK.

## Cartes graphiques

Au démarrage de la page Paramètres (ou à la première génération), VoiceClone liste les périphériques vus par ggml
et propose le meilleur : **CUDA** (NVIDIA) > ROCm > **Vulkan** (GPU dédié avant GPU intégré) > processeur.
Le choix est « Automatique » par défaut ; on peut forcer un périphérique. Si le chargement échoue sur le périphérique
choisi (pilote absent, mémoire insuffisante), le suivant est essayé, jusqu'au processeur, et Paramètres explique le repli.

| Matériel | Statut |
|---|---|
| AMD, Vulkan (RX 6700 XT) | **Testé** : génération plus rapide que le temps réel |
| NVIDIA, Vulkan | Même code que l'AMD, **non testé** sur une carte NVIDIA |
| NVIDIA, CUDA | **Non testé, non compilé** : aucune carte NVIDIA ni CUDA Toolkit sur la machine de développement |
| Processeur | Testé (environ 4 fois plus lent que le GPU) |

### Variante CUDA (NVIDIA)

```bat
build-qwen.cmd cuda                     :: nécessite le CUDA Toolkit 12.x ; produit vendor\qwentts\build-cuda
set VOICECLONE_QWEN_BUILD=build-cuda
cargo build --release
```

`ggml-cuda.dll` et les DLL d'exécution CUDA (`cudart64_*`, `cublas*`) sont copiées à côté de l'application : l'utilisateur
final n'a pas besoin du toolkit, seulement d'un pilote NVIDIA récent. Sans CUDA, la version Vulkan fonctionne aussi sur NVIDIA.

Réglages par défaut du moteur : CUDA → attention « flash » activée, garde FP16 activée sauf pour les familles Ampere et plus
récentes reconnues (RTX 30/40/50, RTX A, A100, H100, L40...) ; Vulkan → ni l'un ni l'autre (réglage validé sur AMD).
Les deux sont modifiables dans Paramètres › Carte graphique › Options avancées, utile si le son est abîmé sur une carte NVIDIA.

## Variables d'environnement

| Variable | Rôle |
|---|---|
| `VOICECLONE_DATA` | dossier de données (défaut `%APPDATA%\VoiceClone`) |
| `VOICECLONE_MODELS` | dossier des modèles (défaut `<données>\models`) |
| `VOICECLONE_PORT` | port du connecteur MCP (défaut 17494) |
| `VOICECLONE_DEVICE` | force un périphérique (`CUDA0`, `Vulkan0`, `CPU`), prioritaire sur Paramètres |
| `VOICECLONE_QWEN_BUILD` | variante du moteur à lier à la compilation (`build` ou `build-cuda`) |

## Éléments tiers

- `vendor/qwentts` : copie du commit `de2ad83` de [ServeurpersoCom/qwentts.cpp](https://github.com/ServeurpersoCom/qwentts.cpp)
  (licence MIT, voir `vendor/qwentts/LICENSE` et `vendor/qwentts/ggml/LICENSE`), portage C++ de Qwen3-TTS (Alibaba, Qwen team).
- Modèles GGUF téléchargés depuis [Serveurperso/Qwen3-TTS-GGUF](https://huggingface.co/Serveurperso/Qwen3-TTS-GGUF) sur Hugging Face
  (non inclus dans ce dépôt ; leurs licences s'appliquent).
- Interface : [Tauri](https://tauri.app). MP3 : LAME (via `mp3lame-encoder`).
