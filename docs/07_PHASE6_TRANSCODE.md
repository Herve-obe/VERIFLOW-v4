# Phase 6 : onglet TRANSCODE (lot 1)

Date : 09/10/2026
Objectif (charte §7.6) : convertir les rushes vers les codecs de montage, de diffusion et les formats son, par préréglages et file d'attente, avec normalisation du niveau et mesure du True Peak.

TRANSCODE est livré en trois lots, chacun dans sa propre PR :

| Lot | Contenu |
|---|---|
| **1 (cette PR)** | Moteur, file d'attente, ProRes, DNxHR, CineForm, Animation, non compressé, FFV1, H.264, HEVC, proxies, changement de conteneur, extraction du son, formats son, normalisation EBU R 128 / ATSC A/85, mesure loudness et True Peak |
| 2 | DNxHD et MXF OP-Atom (Avid), XAVC Intra, AVC-Intra 100, XAVC Long GOP, MPEG-2 / XDCAM HD422 et HD35, AV1, VP9, autres codecs (MJPEG, HAP, VP8, DV, MPEG-1, Theora, WMV, Xvid), images (JPEG, PNG, TIFF, DPX, OpenEXR), LUT, TC incrusté, watermark, préréglages personnels |
| 3 | Fonctions sans réencodage : remplacement du son, découpe, fusion, conformation de cadence |

## Ce qui est livré

| Fonction | Réalisation |
|---|---|
| Fichiers | Boutons « Fichiers » et « Dossier » (sous-dossiers compris), glisser-déposer depuis l'explorateur de VERIFLOW ou du système, clic droit dans l'explorateur, bouton **« Envoyer vers TRANSCODE »** dans MEDIA |
| Préréglages VIDEO | **Montage** : ProRes 422 Proxy, LT, 422, HQ, 4444, 4444 XQ ; DNxHR LB, SQ, HQ, HQX, 444 ; GoPro CineForm ; QuickTime Animation ; non compressé 10 bits (v210) ; FFV1 (archivage, MKV). **Diffusion** : H.264 et HEVC en MP4. **Proxies** : ProRes Proxy, DNxHR LB, H.264, à mi-taille et suffixe `_proxy`. **Sans réencodage** : changement de conteneur (MOV, MP4, MXF OP1a, MKV), extraction du son en WAV |
| Préréglages AUDIO | WAV / BWF, AIFF, FLAC, ALAC, MP3, AAC, Opus, Ogg Vorbis, AC-3 ; mesure loudness et True Peak sans fichier produit |
| Ce qui est conservé | Toutes les pistes son (PCM 24 bits en montage, AAC en diffusion), **timecode** de la source, métadonnées du conteneur. En WAV : blocs **bext** (référence temporelle recalculée si la fréquence change) et **iXML** (scène, prise, noms de pistes) recopiés de l'original ; son extrait d'une vidéo : référence BWF calculée depuis le timecode de l'image, pour la synchro |
| Options | Taille d'image (source, moitié, quart, 2160p, 1080p, 720p, 540p) ; débit H.264/HEVC (automatique selon la taille, ou en Mbit/s) ; fréquence (44,1 à 192 kHz) avec rééchantillonnage de qualité ; résolution 16, 24 bits ou 32 bits flottant, avec dither en réduction ; débit des formats avec perte |
| Normalisation | EBU R 128 (−23 LUFS, −1 dBTP), ATSC A/85 (−24 LKFS, −2 dBTP), streaming (−14 LUFS, −1 dBTP), podcast (−16 LUFS, −1 dBTP), ou cible personnalisée. **Gain simple**, sans compression ni limiteur : si le True Peak maximal serait dépassé, le gain est réduit et le fichier est signalé « cible non atteinte » |
| Mesure | Niveau intégré, plage de loudness (LRA), True Peak, crête échantillon ; écart à une référence au choix |
| Encodeurs | H.264, HEVC et AAC : encodeur du système ou de la carte graphique par défaut (Apple VideoToolbox, NVIDIA NVENC, Intel Quick Sync, AMD AMF, Windows Media Foundation), essayé une fois sur le poste ; x264/x265 sinon ou sur demande. ProRes : VideoToolbox sur Mac s'il fonctionne, sinon `prores_ks` affiché **« ProRes compatible, non certifié Apple »** (charte §6.2) |
| Destination | Dossier choisi ou à côté de chaque original ; suffixe ; fichier déjà présent : nouveau nom (`_1`, `_2`), ne pas convertir, ou remplacer. **L'original n'est jamais remplacé** |
| File d'attente | Lots convertis l'un après l'autre ; avancement, fichier en cours, vitesse par rapport au temps réel, temps restant ; annulation ; fiche retirée par la croix avec confirmation ; « Ouvrir le dossier » |
| Sécurité | Fichier écrit sous le nom `….part` puis renommé une fois terminé : une conversion interrompue ne laisse jamais de fichier incomplet sous son nom final |
| Mémorisation | Réglages retenus séparément pour les modes VIDEO et AUDIO, sur le poste |

## Choix techniques

- **FFmpeg** intégré (charte §5.2), piloté en processus séparé ; progression lue sur `-progress`.
- **Mesure loudness** : filtre `ebur128` de FFmpeg (ITU-R BS.1770, True Peak par suréchantillonnage).
- **Normalisation** par gain fixe plutôt que par le filtre `loudnorm`, qui comprime la dynamique quand la cible n'est pas atteignable : la dynamique d'un mix n'est jamais modifiée sans le dire.
- **Pistes son** : réunies en un seul fichier polyphonique pour les formats son. Formats avec perte limités à 2 canaux (sauf 5.1) : seules les pistes 1 et 2 sont gardées, soit le mix gauche/droite des enregistreurs de tournage.
- **Rééchantillonnage** : `aresample` (filtre de 64 points, dither triangulaire en réduction de résolution).

## Vérifications réalisées

### Tests automatiques (cœur)
19 tests TRANSCODE passent (114 au total pour le cœur), dont des conversions réelles :
- ProRes, DNxHR, H.264 et changement de conteneur : codec, deux pistes son et timecode 10:00:00:00 retrouvés dans le fichier produit ; proxy à mi-taille ; ProRes logiciel signalé non certifié ;
- son extrait d'une vidéo : WAV polyphonique 2 pistes 24 bits, référence BWF 1 728 000 000 échantillons (10:00:00:00 à 48 kHz) ;
- WAV 48 kHz 24 bits converti en 96 kHz 16 bits : scène, prise et noms de pistes iXML conservés, référence bext doublée ;
- normalisation EBU R 128 d'un fichier vers −23 LUFS, vérifiée par une seconde mesure (écart inférieur à 0,3 LU) ;
- WAV 4 pistes vers MP3, AAC, Opus (2 pistes), AIFF et ALAC (4 pistes) ;
- annulation : arrêt immédiat, aucun fichier laissé ;
- noms de sortie : jamais l'original, deux sources du même nom ne s'écrasent pas.

### Essai dans l'application (Linux, Xvfb)
- Dossier de 2 rushes H.264 1080p ajouté, conversion ProRes 422 : progression, vitesse et temps restant affichés, fichiers ProRes produits avec timecode 14:20:00:00 et 2 pistes PCM.
- Mode AUDIO : mesure des 2 rushes avec la référence EBU R 128 : −18,5 LUFS, True Peak −18,1 dBTP, écart +4,5 dB.

## Limites connues
- Encodeurs matériels (VideoToolbox, NVENC, Quick Sync, AMF, Media Foundation) : **non essayés** ici (machine de test sans carte graphique ni Mac) ; leur détection est automatique et l'encodeur logiciel prend le relais si l'essai échoue.
- Report bext/iXML : WAV de moins de 4 Go seulement (au-delà, format RF64 : les métadonnées de FFmpeg restent, sans iXML).
- ProRes avec couche alpha (4444) : la couche alpha n'est pas encore transmise.

## À tester sur poste réel
1. Windows et Mac : préréglage H.264, vérifier la ligne « Encodeur » (carte graphique ou système) et la vitesse.
2. Mac : ProRes 422 HQ, vérifier si l'encodeur Apple VideoToolbox est proposé (selon la version de macOS) ou `prores_ks` non certifié.
3. Rushes caméra réels (Sony XAVC, Panasonic, Canon) vers ProRes et DNxHR (le Blackmagic RAW n'est pas lisible par FFmpeg) : import dans DaVinci Resolve et Premiere, timecode et pistes son.
4. WAV d'enregistreur (Sound Devices) vers WAV 44,1 kHz 16 bits et vers MP3 : scène, prise, noms de pistes et timecode dans le WAV produit (onglet MEDIA).
5. Normalisation EBU R 128 d'un mix, puis mesure du fichier produit.
6. Annuler une conversion en cours, retirer une fiche de la file.
