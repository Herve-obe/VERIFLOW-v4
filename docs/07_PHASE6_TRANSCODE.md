# Phase 6 : onglet TRANSCODE

Date : 09/10/2026
Objectif (charte §7.6, décision du 09/10/2026) : reprendre les capacités de conversion de Shutter Encoder (Paul Pacifico), avec un code et une interface propres à VERIFLOW, et y ajouter les fonctions propres à VERIFLOW (métadonnées de tournage, timecode, traçabilité).

## Ce qui est livré

### Préréglages (environ 80)

| Famille | Préréglages |
|---|---|
| Montage et étalonnage | ProRes 422 Proxy, LT, 422, HQ, 4444, 4444 XQ ; DNxHR LB, SQ, HQ, HQX, 444 ; DNxHD 1080 LB, SQ, HQ, HQX (MXF, débit selon la cadence) ; GoPro CineForm ; QuickTime Animation ; non compressé 10 bits (v210) ; FFV1 (archivage) |
| Broadcast | XDCAM HD422 (MPEG-2 50 Mbit/s), XDCAM HD 35, AVC-Intra 100, XAVC Intra (classe 100 en HD, 300 en UHD), XAVC Long GOP 4:2:2 10 bits, tous en MXF avec une piste mono 24 bits par canal ; HAP, HAP Alpha, HAP Q |
| Diffusion | H.264, HEVC, AV1, H.266/VVC (MP4), VP9, VP8 (WebM) |
| Plateformes web | YouTube 1080p et 4K (débits recommandés par YouTube), Vimeo 1080p, web léger 720p, vertical 9:16 (Reels, TikTok, Shorts), carré 1:1, portrait 4:5 : taille, recadrage centré, son stéréo AAC |
| Proxies | ProRes Proxy, DNxHR LB, H.264, à mi-taille, suffixe `_proxy`, timecode et pistes conservés |
| Anciens codecs | MPEG-2, MPEG-1, Motion JPEG, DV (PAL ou NTSC selon la cadence), Xvid, WMV, Theora |
| Images | JPEG, PNG, TIFF, DPX 10 bits, OpenEXR, WebP, JPEG XL : une image (position au choix, milieu du plan par défaut), une image toutes les N secondes, ou toute la séquence numérotée d'après le timecode |
| Sans réencodage | Changement de conteneur (MOV, MP4, MXF, MKV), découpe, remplacement du son, conformation de cadence, fusion, insert, sous-titres en piste, extraction de l'image, du son (WAV polyphonique) ou de chaque piste (WAV mono) |
| Son | WAV/BWF, AIFF, FLAC, ALAC, MP3, AAC, Opus, Ogg Vorbis, AC-3 |
| Analyses | Loudness et True Peak, détection de plans (EDL CMX 3600), de noir, de médias hors ligne, de silences (CSV), qualité VMAF, empreintes par image (FrameMD5) |

### Réglages

| Section | Contenu |
|---|---|
| Image | Débit et encodeur ; taille (source, moitié, quart, 2160p à 540p, taille libre avec bandes ou recadrage) ; rapport d'image (16:9 à 9:16, 2.39:1...) ; rotation et miroirs ; recadrage au pixel ; désentrelacement (rapide, qualité, une image par trame) ; cadence (images dupliquées, fondu, interpolation de mouvement) ; suppression des images dupliquées ; LUT 3D ; luminosité, contraste, saturation, gamma ; étiquettes Rec. 709, 2020, 601 |
| Incrustations | Timecode (de la source, ou imposé, avec décalage), nom du fichier, texte libre, logo (position, taille, opacité), sous-titres SRT, VTT, ASS incrustés ou en piste. Police Inter intégrée : rendu identique sur les trois systèmes |
| Son | Pistes des vidéos : toutes, pistes 1 et 2, mix stéréo (impaires à gauche, paires à droite), sans son ; fréquence, résolution, débit ; normalisation EBU R 128, ATSC A/85, streaming, podcast ou cible libre, par gain simple plafonné au True Peak |
| Traitement | Selon le préréglage : images, cadence de conformation, fichiers son de remplacement (associés par le nom) et calage par timecode, plan à insérer, original pour VMAF, seuils de détection |
| Destination et nommage | Dossier ou à côté des originaux ; préfixe, suffixe, remplacement de texte, numérotation ; fichier déjà présent : nouveau nom, ignorer ou remplacer (jamais l'original) ; aperçu du nom produit |
| Vérification | Empreinte XXH128 de chaque fichier produit, qualité VMAF du fichier produit, rapport CSV du lot |

### Fichiers et préréglages personnels

- Liste de fichiers : boutons, glisser-déposer (dossiers parcourus), bouton « Envoyer vers TRANSCODE » de MEDIA ; par fichier : points d'entrée et de sortie (ciseaux, timecode ou secondes), fiche d'informations, ordre (fusion).
- Préréglages personnels : réglages complets enregistrés sous un nom, exportés et importés en fichier `.vfpreset` pour les partager (sans chemins propres au poste).

### Ce que VERIFLOW ajoute

- **Métadonnées de tournage** : bext et iXML (scène, prise, noms de pistes) recopiés dans les WAV produits, référence temporelle recalculée si la fréquence change ou si le début est coupé ; son extrait d'une vidéo calé sur le timecode de l'image ; pistes séparées nommées d'après l'iXML (`_A01_Perche.wav`).
- **Timecode** conservé et décalé avec la découpe ; séquences d'images numérotées d'après le timecode ; EDL et rapports de détection en timecode de la source.
- **Son de remplacement calé par timecode** (référence BWF du son, timecode de l'image).
- **Traçabilité** : empreinte XXH128 (celle de l'OFFLOAD et des ASC MHL), rapport CSV du lot, VMAF du fichier produit.
- **Normalisation sans compression ni limiteur** : la dynamique n'est jamais modifiée sans le dire.
- **Sécurité** : écriture sous un nom `.part`, renommé à la fin ; l'original n'est jamais remplacé.

## Choix techniques

- Cœur `core/src/transcode/` : `catalog` (préréglages), `settings`, `video` (encodeurs), `filters` (chaîne d'image, incrustations, graphes son), `build` (commande FFmpeg), `special` (fusion, insert, pistes), `analysis`, `loudness`, `bwf`, `report`, `run` (exécution, avancement, annulation).
- Les fichiers utilisés par les filtres (LUT, police, textes, sous-titres) sont copiés dans un dossier de travail temporaire sous des noms simples : aucun chemin Windows n'a à être échappé dans un filtre FFmpeg.
- **Disponibilité selon le FFmpeg du poste** : liste des encodeurs et filtres lue au démarrage ; un préréglage impossible est grisé avec la raison. Encodeurs du système et de la carte graphique (VideoToolbox, NVENC, Quick Sync, AMF, Media Foundation) essayés une fois, encodeur logiciel sinon.
- FFmpeg intégrés (version 9.0.2), constaté le 09/10/2026 :
  - Windows (gyan.dev, « essentials ») : ni HAP, ni H.266, ni SVT-AV1 (AV1 par libaom), ni JPEG XL ;
  - Linux (martin-riedl.de) : ni Xvid, ni JPEG XL ;
  - macOS : à vérifier sur le poste.
  - **Point à traiter** : le FFmpeg Linux de martin-riedl.de est compilé avec `--enable-nonfree`, ce qui le rend en principe non redistribuable. À remplacer avant toute diffusion.
- Hors périmètre (charte) : outils IA, gravure DVD/Blu-ray, téléchargement web, FTP, e-mail. Dolby Digital Plus et TrueHD : licences Dolby à vérifier avant d'être proposés.

## Vérifications réalisées

### Tests automatiques (cœur)
128 tests du cœur passent, dont plus de 50 pour TRANSCODE, avec le FFmpeg 9.0.2 livré et avec le FFmpeg 6.1 du système :
- chaque codec vidéo produit un fichier lisible, avec son et timecode 10:00:00:00 (Xvid ignoré : absent du FFmpeg Linux) ; XDCAM : pistes mono et 1920×1080 ; DV : 720×576 ;
- préréglage vertical : 1080×1920, son stéréo ;
- images : JPEG unique, PNG toutes les 0,5 s, DPX numérotés à partir de l'image 900 000 (10:00:00:00 à 25 i/s) ;
- incrustations (timecode, nom, texte avec « % » et guillemets, logo), LUT, sous-titres incrustés et en piste ;
- découpe (durée et timecode 10:00:00:12), fusion, insert, conformation 25 vers 24 i/s (durée × 25/24), extraction, pistes séparées, son remplacé et calé par timecode ;
- détections de plans, de noir, de rouge « hors ligne », de silence ; FrameMD5 ; VMAF d'un proxy ;
- nommage (préfixe, remplacement, numérotation), empreinte XXH128, rapport CSV ;
- BWF : iXML et bext recopiés, référence décalée par la découpe ; normalisation à ±0,3 LU ; annulation sans fichier laissé.

### Essai dans l'application (Linux, Xvfb)
- 2 rushes H.264 1080p avec timecode 14:20:00:00 vers XDCAM HD422 avec timecode incrusté et VMAF : MXF 1920×1080, deux pistes mono 24 bits, timecode 14:20:00:00, timecode incrusté exact, VMAF 98,95.
- Détection de noir sur un montage : passage 09:00:02:00 à 09:00:03:00 trouvé, fichier CSV écrit.

## Limites connues
- MXF OP-Atom (Avid) : non fait (DNxHD est livré en MXF OP1a, lu par Avid via AMA et par Resolve).
- Éditeur de sous-titres : non fait (import SRT, VTT, ASS seulement).
- Découpe : points saisis en timecode ou secondes, sans lecteur dans l'onglet (le PLAYER donne les timecodes).
- Encodeurs matériels non essayés ici (pas de carte graphique ni de Mac sur la machine de test).
- ProRes 4444 : couche alpha non transmise.

## À tester sur poste réel
1. Windows et Mac : H.264, HEVC et AV1, ligne « Encodeur » (carte graphique ou système) et vitesse.
2. Mac : ProRes 422 HQ, encodeur Apple VideoToolbox proposé ou non selon la version de macOS.
3. Rushes caméra réels vers ProRes, DNxHR, DNxHD MXF, XDCAM HD422 : import dans DaVinci Resolve, Premiere et Avid ; timecode et pistes son.
4. Préréglages web : YouTube 1080p et vertical 9:16 à partir d'un rush 16:9.
5. Incrustations : timecode, nom, logo PNG, sous-titres SRT ; LUT .cube de la caméra.
6. Son : WAV Sound Devices vers WAV 44,1 kHz 16 bits, MP3, pistes séparées (noms des fichiers d'après les pistes) ; métadonnées vues dans MEDIA.
7. Remplacement du son calé par timecode (vidéo et WAV BWF du même plan).
8. Analyses : détection de plans sur un montage (import de l'EDL dans Resolve), loudness d'un mix, VMAF d'un proxy.
9. Préréglage personnel : enregistrer, exporter, importer sur l'autre poste.
