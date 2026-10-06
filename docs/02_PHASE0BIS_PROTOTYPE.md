# Phase 0 bis : prototype du PLAYER (vidéo et audio)

Date : 05/10/2026
Objectif (charte §8) : lever le risque technique principal avant de développer les onglets, en prouvant :
1. la lecture vidéo fluide et **précise à l'image** de rushes professionnels ;
2. un moteur audio **32 pistes en 192 kHz / 32 bits flottant** avec SOLO, MUTE, niveau et panoramique.

Verdict : **les deux objectifs sont atteints.** L'architecture est validée pour la suite.

---

## 1. Vidéo

### Choix d'architecture
| Option | Décision | Raison |
|---|---|---|
| FFmpeg lié au programme (bibliothèques) | Écarté | Compilation différente par système, versions incompatibles (Ubuntu 22.04 = FFmpeg 4.4, 24.04 = 6.1), un fichier corrompu peut faire planter l'application |
| **FFmpeg en processus séparé** | **Retenu** | Indépendant des versions, isolé (un rush corrompu ne plante pas VERIFLOW), même code sur les 3 systèmes |
| Images RGBA brutes vers l'interface | Écarté | 3,6 Mo par image 1280x720 : 40 ms de transfert, plafond à 19 i/s |
| **Aperçu JPEG qualité maximale 4:4:4** | **Retenu** | Environ 20 fois plus léger : 6 ms de transfert, lecture à pleine cadence |

### Saut exact à l'image
FFmpeg seul n'est pas fiable sur les codecs à GOP long avec images B (H.264, HEVC, XAVC Long GOP, MPEG-2) : un saut vers l'image 11 renvoyait l'image 12.
Méthode retenue : recul d'une seconde avant la cible en conservant les horodatages d'origine, puis filtrage exact de la première image dont l'horodatage atteint la cible. Pour les codecs intra (ProRes, DNxHR, MJPEG...), saut direct sans recul.

Tests automatiques (`core/tests/video_accuracy.rs`) : chaque saut doit renvoyer **exactement les mêmes octets** que la lecture continue.
| Cas testé | Résultat |
|---|---|
| MP4 GOP long 12 images avec images B | Exact |
| MPEG-2 en flux programme (horodatage de départ non nul) | Exact |
| MJPEG 29.97 i/s | Exact |
| Pas à pas arrière puis avant | Exact |

Vérification visuelle dans l'application : l'incrustation du signal de test « 00:00:02.960 / 74 » correspond à l'image 74 affichée par VERIFLOW.

### Mesures (ProRes 422 HQ 1920x1080 25 i/s, aperçu 1280x720, conteneur Linux sans carte graphique)
| Mesure | Résultat | Besoin |
|---|---|---|
| Saut vers une image quelconque | 120 à 200 ms | Acceptable pour un saut ; pas à pas instantané grâce au cache |
| Décodage continu (cœur seul) | 120 i/s | 25 à 60 i/s |
| Retour arrière sur les 24 dernières images | Instantané (cache) | |
| Lecture dans l'application | **25,2 i/s** (6 ms transfert + 10 ms décodage/dessin) | 25 i/s |

### Fonctions disponibles dans le prototype
Espace lecture/pause, J/K/L shuttle (x1, x2, x4, x8, avant et arrière), flèches image par image, Entrée stop, I/O points d'entrée et de sortie avec durée, timecode SMPTE DF/NDF (23.976 à 60 i/s) issu des métadonnées, barre de défilement, informations codec/résolution/cadence/TC.

## 2. Audio

### Architecture temps réel
- Un fil de lecture lit les fichiers, mixe, mesure la loudness et rééchantillonne si nécessaire.
- Le signal stéréo passe par un tampon circulaire sans verrou d'environ 50 ms.
- La fonction de rappel de la carte son ne fait que vider ce tampon : aucune allocation, aucun verrou, aucun accès disque (règle d'or de l'audio temps réel).
- Réglages (gain, pan, SOLO, MUTE) et mesures partagés par variables atomiques : l'interface ne bloque jamais l'audio.

### Formats lus
WAV, BWF, RF64/BW64 (fichiers de plus de 4 Go, fréquents en 32 pistes 192 kHz), PCM 8/16/24/32 bits, flottant 32/64 bits, WAVE_FORMAT_EXTENSIBLE. Métadonnées iXML : noms de pistes, projet, scène, prise, bobine, note, prise cerclée. BWF : time reference (timecode de début).
Sessions : fichiers polyphoniques et/ou monos d'une même prise (à la même fréquence), assemblés en une seule console.

### Mixage et mesures
- Panoramique à puissance constante (-3 dB au centre), gain de -inf à +12 dB, SOLO (prioritaire), MUTE.
- Crêtemètres en dBFS par piste (post-fader) et master L/R, retombée 20 dB/s, maintien 1,5 s.
- Loudness EBU R128 : momentanée, court terme, intégrée (LUFS).
- Sortie à la fréquence native des fichiers si la carte la gère (jusqu'à 192 kHz, flottant 32 bits de préférence), sinon conversion à la volée vers la fréquence de la carte. L'interface indique « native » ou « conversion ».

### Mesures
| Session | Résultat |
|---|---|
| 32 pistes, 192 kHz, 32 bits flottant, sortie 192 kHz | 15 fois le temps réel sur un seul cœur |
| Même session, sortie convertie en 48 kHz | 14 fois le temps réel |
| Débit disque nécessaire | 32 x 192 000 x 4 octets = 24,6 Mo/s (23,4 Mio/s) |

Tests automatiques : lecture de tous les formats PCM et flottants, iXML (y compris caractères accentués et entités XML), mixage SOLO/MUTE/gain/pan, crêtes, loudness d'un sinus 1 kHz à -20 dBFS crête centré (-23 LUFS attendus, -23,0 mesurés), conversion 192 vers 48 kHz, refus des fréquences mélangées.

## 3. Limites connues du prototype (à traiter dans la phase PLAYER)
1. ~~FFmpeg à installer séparément~~ **Réglé (05/10/2026)** : FFmpeg 9.0.2 (GPL, statique) est intégré aux installeurs Windows, macOS (universel) et Linux, sous les noms `veriflow-ffmpeg` et `veriflow-ffprobe` pour ne jamais entrer en conflit avec un FFmpeg du système. VERIFLOW fonctionne sans Internet ni installation annexe. Les binaires sont téléchargés à la compilation depuis des sources épinglées et vérifiés par empreinte SHA-256 (`scripts/fetch-ffmpeg.sh`) ; ils ne sont pas stockés dans Git. Vérifié sur le paquet Linux installé, FFmpeg système masqué : ouverture et lecture ProRes OK, et le FFmpeg 9.0.2 embarqué est plus rapide que le 6.1 du système (170 i/s en continu, 75 ms par saut).
2. Lecture arrière continue (J) lente sur les codecs GOP long : chaque image nécessite un saut. Solution prévue : décodage par blocs à l'envers.
3. Aperçu JPEG : parfait pour vérifier cadrage, point et jeu, pas pour juger finement la couleur. Un mode « qualité maximale » (RGBA) pourra être proposé pour l'image arrêtée.
4. Audio : sortie par défaut du système uniquement (choix de la carte son et ASIO à venir), vidéo et audio pas encore lus ensemble.
5. Sous Linux sans carte son, la sortie de test « null » n'a pas d'horloge : la position avance trop vite. Sans effet sur une vraie carte son.

## 4. Reproduire les mesures
```bash
# Clip ProRes de test
ffmpeg -f lavfi -i testsrc2=size=1920x1080:rate=25:duration=20 -c:v prores_ks -profile:v 3 -timecode 10:00:00:00 test.mov
cargo run --release -p veriflow-core --example seek_bench -- test.mov
# Moteur audio 32 pistes 192 kHz
cargo run --release -p veriflow-core --example audio_bench
# Session de démonstration 32 pistes avec noms iXML
cargo run --release -p veriflow-core --example make_session -- session.wav 32 48000
```
