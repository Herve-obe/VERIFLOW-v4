# VERIFLOW v4 : charte du logiciel

Version : 0.1 (brouillon soumis à validation)
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

## 4. Charte graphique (VALIDÉ : sombre, logo à créer ; détail PROPOSÉ)

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

---

## 5. Architecture technique (PROPOSÉ, validé sur le principe par Hervé)

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
| Cœur | Rust | Code propriétaire VERIFLOW | Logique métier |
| Copie et checksums | Rust (crates `xxhash-rust`, RustCrypto `md-5`, `sha1`, `sha2`) | BSL-1.0 / MIT / Apache 2.0 | OFFLOAD |
| Décodage, encodage, analyse | FFmpeg **compilé en LGPL**, lié dynamiquement | LGPL 2.1+ | MEDIA, PLAYER, SYNC, TRANSCODE |
| Lecture vidéo | Décodage FFmpeg + rendu GPU natif (wgpu) | MIT / Apache 2.0 | PLAYER VIDEO, image par image |
| Audio temps réel | `cpal` (WASAPI, CoreAudio, ALSA/JACK) | Apache 2.0 | PLAYER AUDIO |
| Rééchantillonnage | `rubato` | MIT | Conversion si la carte son ne suit pas |
| Mesures | `ebur128` (LUFS) + crête maison | MIT | Vumètres |
| Base de données | SQLite via `rusqlite` | Domaine public / MIT | Projets, logs, métadonnées |
| Rapports PDF | Typst (moteur embarqué) | Apache 2.0 | REPORT, rapports d'offload |
| Formats d'échange | Écrits par VERIFLOW : MHL v2 (XML), EDL CMX3600, ALE, CSV, FCPXML, OTIO (JSON) | Propriétaire | Exports |

Risque technique principal : **la lecture vidéo fluide et précise à l'image dans Tauri**. La WebView ne sait pas lire ProRes, DNxHR ou XAVC. Le décodage se fait donc en Rust via FFmpeg et le rendu dans une surface GPU native. **Ce point sera prototypé en premier (preuve de concept)** avant de développer l'onglet PLAYER. Solution de repli : libmpv compilée en LGPL.

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

---

## 6. Licences et contraintes légales (VALIDÉ sur le principe, détails PROPOSÉS)

### 6.1 Code propriétaire + FFmpeg LGPL
- VERIFLOW est un logiciel propriétaire.
- FFmpeg est compilé **sans** `--enable-gpl` ni `--enable-nonfree`, livré en bibliothèques dynamiques remplaçables par l'utilisateur, avec le texte de la licence LGPL et un lien vers les sources exactes utilisées (obligations LGPL).
- Exclus car sous GPL : x264, x265, Xvid, librubberband, etc.

### 6.2 Codecs brevetés : "l'idée stable" (réponse à ta question 9)
Principe : **déléguer l'encodage breveté aux encodeurs livrés avec le système d'exploitation ou la carte graphique**, dont les licences sont réglées par leur éditeur.

| Codec | macOS | Windows | Linux |
|---|---|---|---|
| H.264 / HEVC | VideoToolbox (Apple) | Media Foundation, NVENC, QSV, AMF | VAAPI / NVENC si disponible |
| AAC | AudioToolbox (Apple) | Media Foundation | Désactivé par défaut |
| ProRes | **VideoToolbox (encodeur officiel Apple)** | Encodeur FFmpeg `prores_ks` marqué **"ProRes compatible, non certifié Apple"** | Idem Windows |

Pour ProRes, Apple tient une liste de produits certifiés et avertit que les implémentations non autorisées (dont FFmpeg) peuvent poser des problèmes. Sur Mac on utilise l'encodeur Apple. Sur Windows et Linux on affiche la mention "non certifié", et on contactera le ProRes Program Office d'Apple (ProRes@apple.com) quand le produit sera commercialisé.

Codecs libres de droits ou dont les brevets ont expiré, encodés directement par FFmpeg LGPL : AV1 (SVT-AV1, BSD), VP9 (libvpx, BSD), FFV1, DNxHD/DNxHR, CineForm, MPEG-2 (XDCAM HD422), MJPEG, HAP, FLAC, ALAC, Opus, Vorbis, MP3 (LAME, LGPL), AC-3, WAV, AIFF.

### 6.3 ASIO (réponse à ta question 32)
Depuis octobre 2025, Steinberg propose le SDK ASIO sous **double licence : GPLv3 ou licence propriétaire Steinberg**. La GPLv3 est incompatible avec un code fermé. VERIFLOW devra donc signer la licence propriétaire Steinberg (conditions et gratuité **à confirmer** auprès de Steinberg). En attendant : **WASAPI (mode exclusif) + CoreAudio** en V1, ASIO dès que la licence est obtenue.

### 6.4 Signature de code (réponse à ta question 10 : "c'est-à-dire ?")
Windows et macOS vérifient qu'un programme téléchargé est "signé" par un éditeur identifié. Sans signature :
- **Windows** affiche un écran bleu SmartScreen "Windows a protégé votre ordinateur". L'utilisateur doit cliquer sur "Informations complémentaires" puis "Exécuter quand même".
- **macOS** bloque l'ouverture. L'utilisateur doit passer par Réglages Système > Confidentialité et sécurité > "Ouvrir quand même".

Le logiciel fonctionne normalement ensuite, mais cela fait peu professionnel pour un produit vendu. Coûts :
- Apple Developer Program : 99 USD par an (tarif affiché par Apple, à revérifier au moment de l'achat).
- Certificat Windows : service Microsoft ou autorité de certification, de l'ordre de quelques dizaines à quelques centaines d'euros par an selon l'offre (**à chiffrer précisément avant la sortie**).

**Décision PROPOSÉE** : bêta non signée, avec une notice d'installation illustrée ; signature achetée avec les premières recettes, avant la vente publique.

### 6.5 Repo public ou privé (réponse à ta question 11)
- **Public** : GitHub Actions est gratuit et illimité, mais tout le monde peut lire et copier le code source d'un logiciel que tu veux vendre.
- **Privé** (compte gratuit) : 2 000 minutes de compilation par mois. Une minute macOS en consomme 10 et une minute Windows en consomme 2. Un cycle complet (Linux 10 min + Windows 15 min + macOS 15 min) coûte environ 10 + 30 + 150 = 190 minutes, soit une dizaine de compilations complètes par mois.

**Recommandation** : **passer le repo en privé maintenant**, avant d'y mettre du code. Stratégie CI : vérifications rapides sous Linux à chaque PR ; compilation Windows et macOS uniquement pour les versions taguées ou à la demande. C'est suffisant pour notre rythme.

### 6.6 Polices, icônes, bibliothèques
Uniquement des licences permissives (MIT, BSD, Apache 2.0, SIL OFL, domaine public). Le fichier `THIRD_PARTY_LICENSES` est généré automatiquement à chaque version.

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
- Pilotes : WASAPI + CoreAudio en V1, ASIO dès que la licence est obtenue (§6.3).

Calcul du débit maximal : 32 pistes × 192 000 échantillons/s × 4 octets = 24 576 000 octets/s ≈ 24,6 Mo/s (23,4 Mio/s), soit environ 88,5 Go par heure. C'est compatible avec un SSD. Sur un HDD, un tampon de lecture anticipée sera nécessaire.

Rappel des niveaux : les vumètres sont en dBFS (0 dBFS = plafond numérique). L'alignement analogique (+4 dBu = 0 VU, soit -18 dBFS en EBU R68 ou -20 dBFS en SMPTE RP155) sera un réglage d'affichage.

### 7.4 REPORT (priorité 4)

| Fonction | Décision |
|---|---|
| Types | Rapport **son**, rapport **image**, **EDL** |
| Lien PLAYER | Chaque champ est éditable à la volée depuis le PLAYER via le pop-up |
| Modèles | Modèles de l'école Saint-Genès + standards du métier (colonnes : scène, prise, fichier, TC, pistes, prises cerclées, notes, réglages caméra) |
| Personnalisation | Logo, en-tête, colonnes affichées, ordre |
| Exports | **PDF, CSV, XLSX, HTML** (PROPOSÉ, validé implicitement à la question 41) |

**OUVERT** : les dossiers Drive "01 - RAPPORT IMAGE" et "03 - RAPPORT SON 8 PISTES" appartiennent au compte herve.obejero@saint-genes.com. Je vois les dossiers mais pas leur contenu depuis ton compte Gmail. Il faut soit copier les fichiers dans ton dossier Drive "VERIFLOW/V4", soit partager les fichiers eux-mêmes avec ton adresse Gmail.

### 7.5 SYNC (priorité 5)

| Fonction | Décision |
|---|---|
| Méthodes | **TC des métadonnées** (BWF timeReference, piste tmcd QuickTime, TC MXF), **décodage LTC** enregistré sur une piste audio, **corrélation de formes d'onde** (VALIDÉ) |
| Mode de travail | Par lot : appariement automatique vidéo/son, validation manuelle possible clip par clip |
| Dérive d'horloge | Détection et alerte en V1, correction en V2 (VALIDÉ) |
| Cadences | 23.976, 24, 25, 29.97 DF et NDF, 30, 47.95, 48, 50, 59.94 DF et NDF, 60 (VALIDÉ) |
| Exports | **Re-wrap sans réencodage** (vidéo + son synchronisé dans un nouveau fichier), **FCPXML** et **OTIO**. AAF en V2 (VALIDÉ) |

### 7.6 TRANSCODE (priorité 6)

Liste de référence : celle de Shutter Encoder (VALIDÉ), filtrée par les contraintes de licence du §6.

| Catégorie | V1 | V2 ou à étudier |
|---|---|---|
| Vidéo intermédiaire | ProRes (toutes variantes), DNxHD, DNxHR, CineForm, QT Animation, non compressé, FFV1 | XAVC Intra, AVC-Intra 100 (nécessitent un encodeur H.264 10 bits 4:2:2 intra, absent en LGPL) |
| Vidéo diffusion | H.264, HEVC (encodeurs OS/GPU), AV1, VP9, MPEG-2 / XDCAM HD422 et HD35 | H.266/VVC (maturité et brevets à évaluer) |
| Vidéo autres | MJPEG, HAP, VP8, DV, MPEG-1, Theora, WMV | Xvid : exclu (GPL) |
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

| # | Sujet | Action attendue |
|---|---|---|
| O1 | Modèles de rapports de l'école inaccessibles | Hervé : copier les fichiers dans Drive "VERIFLOW/V4" |
| O2 | Repo public ou privé | Hervé : valider le passage en privé (recommandé) |
| O3 | Couleurs d'accent VIDEO/AUDIO et règle TAB | Hervé : valider ou ajuster |
| O4 | Licence ASIO propriétaire Steinberg | Claude : vérifier les conditions avant la phase 3 |
| O5 | Coût réel de la signature Windows et macOS | Claude : chiffrer avant la phase 7 |
| O6 | Branche `main` inexistante | Créer `main` à partir de cette charte une fois validée |

## 11. Sources

- ASC MHL, spécification et outils de référence : https://github.com/ascmitc/mhl-specification et https://github.com/ascmitc/mhl
- Algorithmes MHL v2 (MD5, SHA1, C4, XXH64, XXH3, XXH128) : https://www.imagineproducts.com/news/blog/new-asc-mhl-guidelines-and-our-role/
- Licences FFmpeg : https://ffmpeg.org/legal.html
- Apple ProRes, produits autorisés : https://support.apple.com/en-il/118584
- Encodeurs ProRes dans FFmpeg : https://academysoftwarefoundation.github.io/EncodingGuidelines/EncodeProres.html
- Steinberg ASIO en GPLv3 (octobre 2025) : https://www.kvraudio.com/news/steinberg-moves-vst-3-sdk-to-mit-open-source-license-asio-now-gplv3-65179 et https://librearts.org/2025/11/steinberg-relicenses-vst3-and-asio/
- Facturation GitHub Actions : https://docs.github.com/billing/managing-billing-for-github-actions/about-billing-for-github-actions
- Tauri et Electron, comparatif 2026 : https://www.pkgpulse.com/guides/electron-vs-tauri-2026
- Shutter Encoder, fonctions et codecs : https://www.shutterencoder.com/
- iXML (métadonnées son de tournage) : https://en.wikipedia.org/wiki/IXML
- Contenu d'un rapport caméra : https://www.studiobinder.com/blog/camera-report-template-pdf-download/
- Contenu d'un rapport son : https://clapper.in/templates/sound-report-template/
