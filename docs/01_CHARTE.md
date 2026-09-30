# VERIFLOW v4 : charte du logiciel

Version : 0.2 (validée par Hervé le 30/09/2026, amendable)
Date : 30/09/2026
Auteur du produit : Hervé Obejero
Développement : Claude (Claude Code)

Statut de chaque décision : **VALIDÉ** (réponse d'Hervé), **PROPOSÉ** (choix de Claude à valider), **OUVERT** (information manquante).

---

## 1. Vision

VERIFLOW est une application de bureau locale, légère et multiplateforme qui prend en charge les rushes audio et vidéo **à la fin du tournage** : copie sécurisée et certifiée, visionnage, log, synchronisation, transcodage et rapports.

Promesse : **un seul outil, sur un portable, sans réseau, qui protège juridiquement l'utilisateur** (preuve d'intégrité des rushes opposable aux assurances).

### 1.1 Utilisateurs cibles (VALIDÉ)
1. DIT et assistants caméra.
2. Ingénieurs du son tournage.
3. Tout professionnel de l'audiovisuel (monteur, scripte, directeur de production).
4. Étudiants en audiovisuel (priorité pédagogique : interface lisible, aide contextuelle).

### 1.2 Modèle commercial (VALIDÉ)
- Licence perpétuelle, activation **100 % hors ligne** par clé signée (Ed25519) liée à l'utilisateur.
- Code source ouvert sous **GPLv3** (voir §6.1) : on vend les installeurs officiels, les mises à jour et le support.
- Mises à jour : vérification en ligne optionnelle et désactivable, installation manuelle possible.
- Aucune télémétrie, aucun compte en ligne, aucun serveur.

### 1.3 Ce que VERIFLOW n'est pas
- Pas un logiciel de montage, ni d'étalonnage, ni de mixage.
- Pas un outil cloud ou collaboratif en temps réel.

---

## 2. Plateformes et matériel (VALIDÉ)

| Élément | Cible |
|---|---|
| Windows | 10 et 11, x64 |
| macOS | 12 Monterey et plus, Apple Silicon et Intel |
| Linux | Bonus (Ubuntu 22.04+ / Debian 12+), sans engagement de support |
| Processeur | 4 cœurs minimum |
| RAM | 8 Go minimum |
| Stockage | SSD recommandé. HDD accepté avec **avertissement de lenteur** détecté automatiquement |
| Langues | Français + anglais dès la V1 (architecture i18n, ajout d'autres langues sans recompiler) |

---

## 3. Principe d'interface : deux modes, six onglets (VALIDÉ)

VERIFLOW possède **deux modes d'affichage** : **VIDEO** et **AUDIO**. Chacun contient les six onglets :

`OFFLOAD | MEDIA | PLAYER | SYNC | TRANSCODE | REPORT`

- Bascule par la touche **TAB** ou par l'**interrupteur glissant VIDEO/AUDIO** présent dans chaque onglet.
- Chaque mode a sa propre **couleur d'accent** pour savoir d'un coup d'œil où l'on est (PROPOSÉ : bleu-cyan pour VIDEO, ambre pour AUDIO).
- L'onglet actif est conservé lors de la bascule (OFFLOAD VIDEO devient OFFLOAD AUDIO).
- Contrainte technique : TAB sert aussi à passer d'un champ à l'autre dans les formulaires. **Règle PROPOSÉE** : TAB bascule le mode sauf quand un champ de saisie a le focus ; Ctrl+TAB (Cmd+TAB étant réservé sur macOS, donc Ctrl+TAB partout) bascule dans tous les cas.

### 3.1 Différences entre les modes

| Onglet | Mode VIDEO | Mode AUDIO |
|---|---|---|
| OFFLOAD | Cartes caméra (Sony, Panasonic, Blackmagic en priorité) | Cartes enregistreur (Sound Devices en priorité) |
| MEDIA | Vignettes, filmstrip, métadonnées caméra | Forme d'onde, métadonnées BWF/iXML, noms de pistes |
| PLAYER | Lecteur vidéo image par image, LUT, TC incrusté, logs | Multipiste 32 pistes, SOLO/MUTE/niveau/pan, mesures |
| SYNC | Vue centrée sur les clips vidéo à habiller de son | Vue centrée sur les fichiers son à répartir sur les clips |
| TRANSCODE | Presets vidéo | Presets audio (conversion, rééchantillonnage, normalisation) |
| REPORT | Rapport image + EDL | Rapport son |

---

## 4. Charte graphique (VALIDÉ, amendable)

Références d'ergonomie : logiciels de post-production et de DIT actuels (interfaces sombres, denses, lisibles de nuit sur le plateau). **Aucune reprise du visuel de la v3.**

| Jeton | Valeur proposée | Usage |
|---|---|---|
| Fond | `#0F1115` | Arrière-plan général |
| Surface | `#171A20` | Panneaux |
| Surface haute | `#20242C` | Pop-ups, menus |
| Bordure | `#2A2F38` | Séparateurs |
| Texte principal | `#E6E8EB` | |
| Texte secondaire | `#9AA1AC` | |
| Accent VIDEO | `#3BA7FF` | Mode VIDEO |
| Accent AUDIO | `#FFB020` | Mode AUDIO |
| Succès | `#2ECC71` | Checksum OK, copie vérifiée |
| Alerte | `#F5A623` | Avertissements |
| Erreur | `#FF4D4F` | Échec de vérification |

- Police : **Inter** (licence SIL OFL, libre et commercialisable) pour l'interface, **JetBrains Mono** (OFL) pour les timecodes et checksums.
- Timecodes toujours en police à chasse fixe, grande taille dans le PLAYER.
- Contraste conforme WCAG AA minimum.
- Logo : à créer (monogramme "V" + coche de vérification + forme d'onde). Propositions en phase de maquettes.
- Ergonomie : tout est au clavier, tous les raccourcis sont reconfigurables, pas d'action destructive sans confirmation.
- **Modifiabilité (exigence d'Hervé)** : tout le visuel est isolé dans le dossier `ui/`. Couleurs, polices, tailles, espacements et arrondis sont définis **une seule fois** dans `ui/src/theme/tokens.css`. Changer une valeur à cet endroit change toute l'application. Aucune couleur n'est écrite en dur dans les composants.

---

## 5. Architecture technique (VALIDÉ)

### 5.1 Choix : Tauri 2 (Rust + interface web)

| Critère | Tauri 2 | Electron | Qt (C++) |
|---|---|---|---|
| Taille d'installeur | Quelques Mo (hors FFmpeg) | 85 Mo et plus | 30 Mo et plus |
| RAM au repos | Environ 40 Mo | Environ 170 Mo | Faible |
| Licence | MIT / Apache 2.0 | MIT | LGPL (contraignante en statique) |
| Performances E/S et audio | Rust natif | Node.js, à compléter en natif | Natif |

Tauri 2 utilise la WebView du système au lieu d'embarquer Chromium, d'où sa légèreté. Rust apporte la sécurité mémoire, indispensable pour un logiciel qui manipule des rushes.

### 5.2 Composants

| Couche | Technologie | Licence | Rôle |
|---|---|---|---|
| Interface | Svelte 5 + TypeScript | MIT | Onglets, formulaires, tableaux |
| Cœur | Rust | GPL-3.0-or-later (VERIFLOW) | Logique métier |
| Copie et checksums | Rust (crates `xxhash-rust`, RustCrypto `md-5`, `sha1`, `sha2`) | BSL-1.0 / MIT / Apache 2.0 | OFFLOAD |
| Décodage, encodage, analyse | FFmpeg compilé avec `--enable-gpl` (x264, x265), lié dynamiquement | GPL | MEDIA, PLAYER, SYNC, TRANSCODE |
| Lecture vidéo | Décodage FFmpeg + rendu GPU natif (wgpu) | MIT / Apache 2.0 | PLAYER VIDEO, image par image |
| Audio temps réel | `cpal` (WASAPI, CoreAudio, ALSA/JACK) | Apache 2.0 | PLAYER AUDIO |
| Rééchantillonnage | `rubato` | MIT | Conversion si la carte son ne suit pas |
| Mesures | `ebur128` (LUFS) + crête maison | MIT | Vumètres |
| Base de données | SQLite via `rusqlite` | Domaine public / MIT | Projets, logs, métadonnées |
| Rapports PDF | Typst (moteur embarqué) | Apache 2.0 | REPORT, rapports d'offload |
| Formats d'échange | Écrits par VERIFLOW : MHL v2 (XML), EDL CMX3600, ALE, CSV, FCPXML, OTIO (JSON) | Propriétaire | Exports |

Risque technique principal : **la lecture vidéo fluide et précise à l'image dans Tauri**. La WebView ne sait pas lire ProRes, DNxHR ou XAVC. Le décodage se fait donc en Rust via FFmpeg et le rendu dans une surface GPU native. **Ce point sera prototypé en premier (preuve de concept)** avant de développer l'onglet PLAYER. Solution de repli : libmpv (GPL/LGPL).

### 5.3 Données : "un fichier projet + SQLite", expliqué

- Un **projet** = une production (un film, une série, un clip). Il est enregistré dans **un seul fichier** `NomDuProjet.veriflow`, que tu peux copier sur un disque, envoyer à la post-prod ou archiver avec les rushes. C'est l'équivalent d'un fichier projet Premiere ou Resolve.
- Ce fichier est en réalité une **base SQLite** : un format de base de données contenu dans un simple fichier, sans serveur à installer. Il contient l'historique des offloads, la liste des médias, les métadonnées éditées, les logs, les marqueurs et les rapports.
- Les **réglages de l'application** (raccourcis, préférences, presets) sont stockés à part, dans le dossier utilisateur du système.
- Rien ne sort du poste. Aucune donnée n'est envoyée sur Internet.

### 5.4 Règle d'or sur les originaux (VALIDÉ)
Les fichiers originaux vérifiés ne sont **jamais modifiés, renommés ni déplacés** par VERIFLOW. Toute édition de métadonnées est écrite :
1. dans le projet `.veriflow` (toujours) ;
2. dans un fichier sidecar (XMP ou iXML externe) ;
3. ou dans une **copie de travail**, sur demande explicite.

Toute action qui toucherait un fichier déjà vérifié déclenche un **pop-up d'avertissement**.

## 5.5 Organisation du code

Règle : **l'interface graphique et le cœur métier sont séparés**. Le cœur Rust ne connaît rien de l'interface, et l'interface ne fait aucun calcul métier. On peut ainsi refaire tout le visuel sans toucher à la copie, aux checksums ou à l'audio.

```
VERIFLOW-v4/
├── docs/                    Charte, spécifications, notices
├── ui/                      TOUTE l'interface graphique (Svelte + TypeScript)
│   └── src/
│       ├── theme/           Couleurs, polices, espacements : modifier le visuel ici
│       ├── components/      Éléments réutilisables (boutons, tableaux, vumètres, pop-ups)
│       ├── layout/          Barre d'onglets, interrupteur VIDEO/AUDIO, fenêtres
│       ├── tabs/            Un dossier par onglet : offload, media, player, sync, transcode, report
│       ├── i18n/            Textes de l'interface : fr.json, en.json
│       └── shortcuts/       Raccourcis clavier par défaut
├── src-tauri/               Pont entre l'interface et le cœur (commandes Tauri)
├── core/                    Cœur métier en Rust, un module par fonction
│   ├── offload/  media/  player/  sync/  transcode/  report/  project/
├── templates/reports/       Mise en page des rapports PDF : modifier les rapports ici
└── tests/                   Tests automatisés
```

---

## 6. Licences et contraintes légales (VALIDÉ : open source GPL, 30/09/2026)

### 6.1 VERIFLOW est un logiciel libre sous GPLv3
- Licence du code : **GPL-3.0-or-later**. Le fichier `LICENSE` est à la racine du repo.
- Conséquence positive : on peut intégrer légalement toutes les briques GPL gratuites. FFmpeg peut être compilé **avec** `--enable-gpl` (x264, x265 et les filtres GPL), et le SDK ASIO est utilisable sous sa licence GPLv3.
- Conséquence sur la vente : la GPL **autorise la vente**, mais chaque acheteur a le droit de redistribuer le logiciel et son code source, gratuitement ou non, et de retirer la vérification de clé. La clé d'activation reste en place mais ne protège pas juridiquement contre la copie.
- Ce que l'on vend réellement : les **installeurs officiels signés**, les mises à jour, le support, la documentation et la confiance dans une version certifiée (argument fort pour un outil de preuve d'intégrité).
- Protection possible : le **nom et le logo VERIFLOW** ne sont pas couverts par la GPL. Un dépôt de marque (INPI, payant) interdirait à un tiers de redistribuer une copie sous ce nom. À envisager au lancement commercial.
- Toute dépendance doit être compatible GPLv3 : MIT, BSD, Apache 2.0, LGPL, GPLv2+, SIL OFL. Interdit : bibliothèques sous licence propriétaire incompatible, "GPLv2 only", "nonfree" de FFmpeg (fdk-aac, NDI SDK...).
- Les SDK RAW caméra propriétaires (V2) devront être évalués un par un : un SDK propriétaire ne peut pas être lié à un programme GPL sans exception de licence. Solution envisagée : module externe optionnel, installé séparément par l'utilisateur.

### 6.2 Codecs brevetés : la licence GPL ne règle pas les brevets
Le droit d'auteur (GPL) et les brevets sont deux sujets distincts. Utiliser x264 est libre, mais H.264 reste couvert par des brevets.

| Codec | Stratégie |
|---|---|
| H.264 | **Encodeurs OS/GPU par défaut** (VideoToolbox, Media Foundation, NVENC, QSV, AMF). x264 disponible en option (qualité, XAVC Intra, AVC-Intra). Le pool Via LA ne facture pas de redevance sous 100 000 unités par an, sous réserve de signer son accord de licence : à faire avant la vente |
| HEVC | Encodeurs OS/GPU par défaut, x265 en option. Pools de brevets distincts (Access Advance, Via LA) : conditions à vérifier avant la vente |
| AAC | Encodeurs OS (AudioToolbox, Media Foundation) par défaut, encodeur natif FFmpeg en option |
| ProRes | **VideoToolbox (encodeur officiel Apple) sur Mac**. Sur Windows et Linux, `prores_ks` marqué **"ProRes compatible, non certifié Apple"**. Contact Apple ProRes Program Office (ProRes@apple.com) avant la vente |
| Libres ou brevets expirés | AV1, VP9, VP8, FFV1, DNxHD/DNxHR, CineForm, MPEG-2, MPEG-1, MJPEG, HAP, FLAC, ALAC, Opus, Vorbis, MP3, AC-3, WAV, AIFF : encodés directement |

### 6.3 ASIO
Depuis octobre 2025, Steinberg propose le SDK ASIO sous double licence, dont **GPLv3**. VERIFLOW étant sous GPLv3, **ASIO est utilisable gratuitement**. V1 : WASAPI + CoreAudio + ASIO sous Windows.

### 6.4 Signature de code (VALIDÉ : reportée)
La signature Apple et Windows sera achetée **une fois le logiciel fonctionnel à 100 % et validé par les testeurs**. D'ici là, installeurs non signés + notice d'installation illustrée (contournement Gatekeeper et SmartScreen).

### 6.5 Repo
Avec un code sous GPL, rien ne justifie de cacher le code. **Recommandation** : repasser le repo en **public**, ce qui rend GitHub Actions **gratuit et illimité** (compilation Windows, macOS et Linux à chaque PR). En privé, le quota est de 2 000 minutes par mois, avec un coefficient 10 pour macOS et 2 pour Windows.

### 6.6 Polices, icônes, bibliothèques
Uniquement des licences compatibles GPLv3 (MIT, BSD, Apache 2.0, SIL OFL, domaine public, LGPL, GPL). Le fichier `THIRD_PARTY_LICENSES` est généré automatiquement à chaque version.

---

## 7. Spécifications fonctionnelles par onglet

### 7.1 OFFLOAD (priorité 1)

**Objectif** : copier une source vers N destinations, avec une preuve d'intégrité bit à bit opposable aux assurances.

| Fonction | Décision |
|---|---|
| Sources | Tout volume monté : carte SD/CFexpress/SxS, clé USB, disque externe, dossier |
| Destinations | 1 par défaut, bouton **"+"** pour en ajouter. Écriture **parallèle** à partir d'**une seule lecture** de la source (VALIDÉ) |
| Checksum | Calculé pendant la lecture source (aucune relecture supplémentaire de la carte) |
| Vérification | **Relecture complète obligatoire** de chaque destination, comparée au hash source (VALIDÉ) |
| Algorithme par défaut | **XXH128 (xxHash3 128 bits)** (PROPOSÉ, voir ci-dessous) |
| Algorithmes au choix | XXH64, XXH3-64, MD5, SHA-1, SHA-256, C4. Plusieurs peuvent être calculés simultanément si un assureur l'exige |
| Structure | Arborescence de la carte **conservée intégralement**. Modèles de dossiers personnalisables : `{Projet}/{Jour}/{Date}/{Caméra}/{Carte}` |
| Originaux | Jamais renommés. Pop-up d'avertissement avant toute action qui modifierait un fichier |
| Suivi | Tableau d'avancement **fichier par fichier** : nom, taille, débit, hash source, hash de chaque destination, statut |
| Doublons | Détection des cartes et fichiers déjà copiés (par hash et par empreinte de volume) + avertissement |
| Reprise | Reprise après coupure (secteur, câble débranché, plantage) sans recopier les fichiers déjà vérifiés |
| File d'attente | Plusieurs cartes à la suite, traitement séquentiel automatique |
| Éjection | Éjection sécurisée de la source à la fin, en option |
| Disque lent | Détection d'un HDD et avertissement sur la durée estimée |
| Espace | Vérification de l'espace libre sur toutes les destinations **avant** de démarrer |

**Algorithme : pourquoi XXH128 par défaut (réponse à ta question 16)**
- xxHash n'est pas un hash cryptographique : il est conçu pour détecter les erreurs de copie, pas pour résister à un attaquant. C'est exactement le besoin d'un offload.
- XXH128 est plus rapide que XXH64 sur les processeurs actuels (instructions SIMD) et offre 128 bits au lieu de 64, donc un risque de collision négligeable.
- Il est reconnu par **ASC MHL v2**, avec XXH64, XXH3, MD5, SHA-1 et C4.
- En pratique, le débit est limité par les disques et non par le calcul : un SSD USB 3.2 plafonne vers 1 000 Mo/s, bien en dessous de la vitesse de calcul de xxHash.
- XXH64 reste disponible pour la compatibilité avec les outils plus anciens, MD5 et SHA-256 pour les assureurs ou cahiers des charges qui les exigent.

**Preuve d'intégrité ("légalement conforme", question 15)**
Aucun logiciel ne peut garantir à lui seul une valeur juridique. VERIFLOW fournit en revanche une **chaîne de preuve conforme aux usages de l'industrie** :
1. **ASC MHL v2** : standard de l'American Society of Cinematographers, qui trace l'historique de chaque copie (qui, quand, quel outil, quels hashs).
2. **Rapport PDF horodaté** : projet, opérateur, poste, source, destinations, liste des fichiers avec hashs, durée, débit, résultat, vignettes.
3. **Hash du rapport lui-même**, imprimé dans le rapport et dans le MHL, pour prouver qu'il n'a pas été modifié après coup.
4. **Horodatage certifié RFC 3161 en option** si le poste est en ligne (via une autorité d'horodatage gratuite), sinon horloge locale signalée comme telle.
5. Journal d'événements (erreurs, reprises, avertissements) inclus dans le rapport.

**Rapports** : PDF + HTML + CSV + MHL, avec logo de la production et vignettes (VALIDÉ).

**Supports prioritaires** (VALIDÉ) : Sony (XAVC, XDCAM), Panasonic (P2, AVCHD), Blackmagic (BRAW en copie, ProRes), Sound Devices (BWF poly et mono). Les autres suivront.

### 7.2 MEDIA (priorité 2)

| Fonction | Décision |
|---|---|
| Navigation | Volumes et dossiers du poste + médias du projet |
| Affichage | Liste et grille, vignettes, filmstrip au survol, forme d'onde pour l'audio, filtres et recherche |
| Lecteur pop-up | Lecture rapide (Espace) sans quitter l'onglet |
| Métadonnées | Lecture complète : conteneur, codec, résolution, cadence, TC, pistes, BWF, iXML, XMP, métadonnées caméra |
| Édition | Un par un ou **par lot**, écrite selon la règle du §5.4 |
| RAW caméra | R3D, ARRIRAW, BRAW, X-OCN en **V2** (SDK propriétaires, licences à vérifier une par une) |

**Champs éditables (réponse à ta question 24, issus des rapports caméra/son et du standard iXML)**

| Groupe | Champs |
|---|---|
| Production | Projet, jour de tournage, date, société de production, réalisateur, chef opérateur, ingé son, DIT |
| Identification | Bobine / roll / carte, nom du clip, caméra (A, B, C...), enregistreur |
| Découpage | Séquence, scène, plan, prise, **prise cerclée (bonne prise)**, faux départ, son seul (wild track), plan sans son (MOS) |
| Image | Objectif, focale, T-stop/diaphragme, distance de mise au point, ISO/EI, obturateur (angle ou vitesse), balance des blancs, teinte, filtres/ND, LUT, cadence de prise de vue, résolution, format |
| Son | Noms des pistes (TRACK_LIST iXML), micros par piste, fréquence d'échantillonnage, résolution, cadence TC, type de TC, bande/tape |
| Timecode | TC début/fin, cadence, DF/NDF, user bits |
| Notes | Commentaire libre, note qualité, mots-clés, couleur/étiquette |

### 7.3 PLAYER (priorité 3)

**Commun**
- Raccourcis (VALIDÉ, tous reconfigurables) : **Espace** lecture/pause, **J/K/L** shuttle arrière/pause/avant (appuis répétés = vitesse ×2, ×4, ×8), **flèches gauche/droite** image par image, **I/O** points d'entrée/sortie, **Entrée** stop et retour au début, **M** marqueur, une touche dédiée pour ouvrir le pop-up REPORT.
- Pop-up de saisie vers REPORT : raccourci dédié + ouverture automatique au changement de prise (PROPOSÉ).

**Mode VIDEO**
- Lecture fluide et **précise à l'image**, shuttle, jog.
- **LUT d'affichage** (.cube) et **TC incrusté** en V1 (VALIDÉ). Scopes et sortie moniteur externe en V2.
- Logs : marqueurs avec couleur, commentaire, in/out, scène/prise.
- Exports : **EDL CMX3600, ALE, CSV, FCPXML, OTIO** (VALIDÉ).

**Mode AUDIO**
- Multipiste **jusqu'à 32 pistes, 192 kHz, 32 bits float**.
- Sources : **WAV polyphoniques et fichiers mono regroupés** par scène/prise (VALIDÉ).
- Noms des pistes lus dans iXML (TRACK_LIST) ou BWF, éditables.
- Par piste : **SOLO, MUTE, niveau (fader), panoramique**.
- Master **stéréo**, mesures **crête dBFS + LUFS** (intégré, court terme, momentané) (VALIDÉ).
- Si la carte son ne gère pas la fréquence ou le nombre de canaux du fichier : **rééchantillonnage et réduction à la volée** en mémoire, sans jamais modifier le fichier.
- Pilotes : WASAPI + CoreAudio + ASIO (GPLv3) dès la V1 (§6.3).

Calcul du débit maximal : 32 pistes × 192 000 échantillons/s × 4 octets = 24 576 000 octets/s ≈ 24,6 Mo/s (23,4 Mio/s), soit environ 88,5 Go par heure. C'est compatible avec un SSD. Sur un HDD, un tampon de lecture anticipée sera nécessaire.

Rappel des niveaux : les vumètres sont en dBFS (0 dBFS = plafond numérique). L'alignement analogique (+4 dBu = 0 VU, soit -18 dBFS en EBU R68 ou -20 dBFS en SMPTE RP155) sera un réglage d'affichage.

### 7.4 REPORT (priorité 4)

| Fonction | Décision |
|---|---|
| Types | Rapport **son**, rapport **image**, **EDL** |
| Lien PLAYER | Chaque champ est éditable à la volée depuis le PLAYER via le pop-up |
| Modèles | Modèle **École** (reproduction des rapports Saint-Genès, voir ci-dessous) + modèle **Pro** (colonnes étendues) + modèles personnalisés |
| Personnalisation | Logo, en-tête, colonnes affichées, ordre |
| Exports | **PDF, CSV, XLSX, HTML** (PROPOSÉ, validé implicitement à la question 41) |

**Modèles de référence fournis par Hervé** (Drive VERIFLOW/V4 : `Rapport Image.xlsx/pdf`, `Rapport Son 8 pistes.xlsx/pdf`)

| Rapport | En-tête | Colonnes du tableau |
|---|---|---|
| **Image** | Rapport N°, feuillet N°/N, support de sauvegarde N°, date, titre du film, réalisateur, directeur photo, OPV. Image : HD / 4K / autre, 24 / 25 / autre. Média : SD / P2 / SSD / CF / autre. Caméra, format image, référence son (dBFS), remarques | SEQ/Plan, prise, TC IN, TC OUT, audio/muet, effets/observations |
| **Son** | Rapport N°, feuillet N°/N, support de sauvegarde N°, date, titre du film, réalisateur, ingénieur du son, perchman. Image : 35 / 16 mm / autre, 24 / 25 i/s / autre. Enregistreur, time-code, référence (dBFS), autre. Numérisation : 48 / 96 kHz, 16 / 24 bits, autre. Remarques | ID, plan, prise, pistes 1 à 8, observations |

Améliorations apportées par VERIFLOW (PROPOSÉ) :
- Champs **pré-remplis automatiquement** depuis les métadonnées : TC IN/OUT, cadence, fréquence, résolution, caméra, enregistreur, noms de pistes.
- Nombre de pistes **dynamique de 1 à 32**, lu dans l'iXML (le modèle 8 pistes reste le format d'impression par défaut, avec feuillets supplémentaires au-delà).
- Choix étendus : 23.976 / 29.97 / 30 / 50 / 60 i/s, 44,1 / 192 kHz, 32 bits float, supports CFexpress et SxS.
- Colonnes ajoutées dans le modèle Pro : nom du fichier, durée, **prise cerclée**, faux départ, son seul, MOS, TC son pour le rapport image, réglages caméra (objectif, focale, T-stop, ISO, obturateur, ND, balance des blancs).
- Numéro de rapport et de feuillet incrémentés automatiquement, support de sauvegarde lié à l'OFFLOAD.


### 7.5 SYNC (priorité 5)

| Fonction | Décision |
|---|---|
| Méthodes | **TC des métadonnées** (BWF timeReference, piste tmcd QuickTime, TC MXF), **décodage LTC** enregistré sur une piste audio, **corrélation de formes d'onde** (VALIDÉ) |
| Mode de travail | Par lot : appariement automatique vidéo/son, validation manuelle possible clip par clip |
| Dérive d'horloge | Détection et alerte en V1, correction en V2 (VALIDÉ) |
| Cadences | 23.976, 24, 25, 29.97 DF et NDF, 30, 47.95, 48, 50, 59.94 DF et NDF, 60 (VALIDÉ) |
| Exports | **Re-wrap sans réencodage** (vidéo + son synchronisé dans un nouveau fichier), **FCPXML** et **OTIO**. AAF en V2 (VALIDÉ) |

### 7.6 TRANSCODE (priorité 6)

Liste de référence : celle de Shutter Encoder (VALIDÉ). Grâce à la GPL, x264 et x265 sont disponibles (§6.2).

| Catégorie | V1 | V2 ou à étudier |
|---|---|---|
| Vidéo intermédiaire | ProRes (toutes variantes), DNxHD, DNxHR, CineForm, QT Animation, non compressé, FFV1, XAVC Intra et AVC-Intra 100 (via x264) | |
| Vidéo diffusion | H.264, HEVC (encodeurs OS/GPU par défaut, x264/x265 en option), XAVC Long GOP, AV1, VP9, MPEG-2 / XDCAM HD422 et HD35 | H.266/VVC (maturité et brevets à évaluer) |
| Vidéo autres | MJPEG, HAP, VP8, DV, MPEG-1, Theora, WMV, Xvid | |
| Audio | WAV, AIFF, FLAC, ALAC, MP3, AAC (encodeurs OS), AC-3, Opus, Vorbis | Dolby Digital Plus, TrueHD (licences Dolby à vérifier) |
| Images | JPEG, PNG, TIFF, DPX, OpenEXR | JPEG XL, PSD |
| Conteneurs | MOV, MP4, MXF OP1a, MXF OP-Atom, MKV, WebM, AVI, WAV/BWF | |
| Fonctions sans réencodage | Rewrap, remplacement audio, découpe, extraction, fusion, conformation de cadence | Sous-titres |
| Traitements | Presets, file d'attente, proxies, redimensionnement, LUT, TC incrusté, watermark, normalisation loudness (EBU R128 / ATSC A/85), analyse True Peak | |
| Hors périmètre | Outils IA de Shutter Encoder (séparation, transcription, etc.), gravure DVD/Blu-ray, téléchargement web, FTP | |

---

## 8. Plan de développement

Ordre VALIDÉ : OFFLOAD, MEDIA, PLAYER, REPORT, SYNC, TRANSCODE.

| Phase | Contenu | Livrable |
|---|---|---|
| 0 | Socle : projet Tauri, structure, CI, i18n, thème, bascule VIDEO/AUDIO, fichier projet | Application vide navigable, compilée sur 3 OS |
| 0 bis | Preuve de concept lecture vidéo précise à l'image + audio 32 pistes | Démo technique, validation du choix §5.2 |
| 1 | OFFLOAD complet + rapports d'offload + MHL v2 | **Bêta 1 utilisable en tournage** |
| 2 | MEDIA | Bêta 2 |
| 3 | PLAYER VIDEO + AUDIO | Bêta 3 |
| 4 | REPORT | Bêta 4 |
| 5 | SYNC | Bêta 5 |
| 6 | TRANSCODE | Release candidate V1 |
| 7 | Licence et activation, signature de code, installeurs, documentation | **V1 commerciale** |

## 9. Méthode de travail (VALIDÉ)

- Tout est sauvegardé sur GitHub, rien sur les postes.
- **Une branche par fonctionnalité**, une **Pull Request** que tu valides avant fusion dans `main`.
- Chaque PR contient : description, captures d'écran, tests automatisés, procédure de test manuelle.
- Versions taguées `v4.0.0-beta.N`, installeurs téléchargeables dans les Releases GitHub.
- Les rushes de test restent hors Git (trop lourds). Tu les testeras sur tes postes.

## 10. Points ouverts

| # | Sujet | Statut |
|---|---|---|
| O1 | Modèles de rapports de l'école | **Réglé** : intégrés au §7.4 |
| O2 | Repo public ou privé | Recommandation : repasser en **public** (code GPL, CI gratuite illimitée). Décision d'Hervé |
| O3 | Couleurs d'accent VIDEO/AUDIO et règle TAB | **Validé**, amendable via `ui/src/theme/` |
| O4 | ASIO | **Réglé** : GPLv3 compatible |
| O5 | Signature de code | **Reporté** : après validation complète par les testeurs |
| O6 | Branche `main` inexistante | À créer à partir de cette charte (accord d'Hervé demandé) |
| O7 | Accords de brevets H.264 / HEVC et contact Apple ProRes | Avant la vente, pas avant |
| O8 | Dépôt de la marque VERIFLOW | Au lancement commercial |

## 11. Sources

- ASC MHL, spécification et outils de référence : https://github.com/ascmitc/mhl-specification et https://github.com/ascmitc/mhl
- Algorithmes MHL v2 (MD5, SHA1, C4, XXH64, XXH3, XXH128) : https://www.imagineproducts.com/news/blog/new-asc-mhl-guidelines-and-our-role/
- Licences FFmpeg : https://ffmpeg.org/legal.html
- Apple ProRes, produits autorisés : https://support.apple.com/en-il/118584
- Encodeurs ProRes dans FFmpeg : https://academysoftwarefoundation.github.io/EncodingGuidelines/EncodeProres.html
- Steinberg ASIO en double licence GPLv3 (octobre 2025) : https://www.kvraudio.com/news/steinberg-moves-vst-3-sdk-to-mit-open-source-license-asio-now-gplv3-65179 et https://librearts.org/2025/11/steinberg-relicenses-vst3-and-asio/
- Facturation GitHub Actions : https://docs.github.com/billing/managing-billing-for-github-actions/about-billing-for-github-actions
- Tauri et Electron, comparatif 2026 : https://www.pkgpulse.com/guides/electron-vs-tauri-2026
- Pool de brevets AVC/H.264 (Via LA) : https://www.via-la.com/licensing-programs/avc-h-264/
- Licence GPLv3 : https://www.gnu.org/licenses/gpl-3.0.fr.html
- Shutter Encoder, fonctions et codecs : https://www.shutterencoder.com/
- iXML (métadonnées son de tournage) : https://en.wikipedia.org/wiki/IXML
- Contenu d'un rapport caméra : https://www.studiobinder.com/blog/camera-report-template-pdf-download/
- Contenu d'un rapport son : https://clapper.in/templates/sound-report-template/
