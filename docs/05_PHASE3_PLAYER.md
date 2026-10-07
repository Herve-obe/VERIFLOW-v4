# Phase 3 : onglet PLAYER (VIDEO + AUDIO)

Date : 06/10/2026
Objectif (charte §7.3) : finaliser le lecteur posé en phase 0 bis. Cette phase ajoute :
- les logs (marqueurs) et leurs exports ;
- la lecture vidéo avec son synchronisé ;
- la LUT d'affichage et le TC incrusté ;
- le choix de la carte son, ASIO compris.

## Ce qui est livré

| Fonction | Réalisation |
|---|---|
| Marqueurs | Touche **M** : marqueur à l'image courante. **I** puis **O** puis **M** : plage entrée-sortie. Couleur au choix (8 couleurs Avid, reconnues par Premiere, Resolve et OpenTimelineIO), commentaire saisi tout de suite, scène, prise. Clic sur le TC pour y aller ; repères affichés sur la barre de temps |
| Enregistrement | Dans le fichier projet `.veriflow` (schéma v4), jamais dans les médias. Un projet ouvert est nécessaire |
| Exports | **EDL CMX3600**, **ALE**, **CSV**, **FCPXML 1.10**, **OTIO**, pour le clip courant ou pour tous les clips marqués du projet |
| Son des vidéos | Pistes son des MOV, MXF, MP4 lues par le moteur audio (décodage FFmpeg en flux, un processus par piste) ; en lecture normale, **l'image suit l'horloge de la carte son** ; son coupé en shuttle (J/L) ; bouton « Son » |
| LUT d'affichage | Fichiers .cube, .3dl, .dat, .m3d, .csp appliqués sur l'image d'aperçu par le filtre `lut3d` de FFmpeg (interpolation tétraédrique) ; le fichier n'est jamais modifié ; LUT vérifiée avant application, message clair si elle est illisible |
| TC incrusté | Bouton « TC » : timecode affiché sur l'image (affichage seulement) |
| Sortie audio | Liste de toutes les sorties, par pilote : **WASAPI et ASIO** (Windows), CoreAudio (macOS), ALSA/JACK (Linux). Choix de la **paire de canaux** (1-2, 3-4...) pour les cartes multicanal. Réglage mémorisé et commun au PLAYER AUDIO, au son des vidéos et au lecteur rapide de MEDIA. Un changement rouvre le lecteur en gardant la position et les réglages de la console |
| PLAYER AUDIO | Mêmes logs qu'en VIDEO, sur une grille de 25 i/s calée sur le TC BWF ; le moteur lit aussi désormais MP3, FLAC, AIFF, M4A, OGG (décodage FFmpeg) |

### Règles d'export
- **Plage → événement** : un marqueur plage devient un événement (sous-clip) de la timeline exportée.
- **Clip sans plage** : il est exporté en entier.
- **Marqueurs point** : ils sont attachés à l'événement qui les contient. **Un marqueur point situé hors de toute plage n'apparaît que dans le CSV**, puisque la timeline ne contient pas ce passage.
- **Timeline** : elle commence à 01:00:00:00, à la cadence du premier clip.
- **EDL** :
  - norme CMX3600, bobine sur 8 caractères (nom complet en `* FROM CLIP NAME` et `* SOURCE FILE`) ;
  - texte en ASCII (accents retirés) ;
  - marqueurs en `* LOC:` au **timecode source**, la convention Avid reprise par OpenTimelineIO ;
  - une EDL n'indique pas sa cadence : il faut la choisir à l'import dans le logiciel de montage.
- **FCPXML** : ce format n'a pas de couleur de marqueur ; la couleur est écrite dans la note du marqueur.
- **TC de sortie** : il est exclusif (image qui suit la dernière image de la plage), comme dans toutes les EDL et ALE.

## Vérifications réalisées

### Tests automatiques (cœur)
73 tests passent, dont ceux de cette phase :
- découpage en événements, lignes EDL, CSV, fractions de temps FCPXML, URL de fichiers, OTIO ;
- marqueurs enregistrés, modifiés et supprimés dans le projet ;
- description d'un vrai MOV (TC 10:00:00:00) et d'un BWF (time reference 01:00:00:00) pour l'export ;
- son d'une vidéo à 2 pistes mono : décodage, lecture suivie, sauts avant et arrière, mixage dans le moteur ;
- LUT d'inversion appliquée puis retirée ; LUT invalide refusée ; noms de fichiers contenant espace, apostrophe, « : » et « , ».

### Contrôle externe des exports
`scripts/validate-logs.py` fait relire les exports par **OpenTimelineIO 0.18.1**, avec les adaptateurs officiels cmx_3600, fcpx_xml et ale. Les clips, les plages source et les marqueurs ont été retrouvés à l'identique dans les cinq formats.

### Essai dans l'application (Linux, Xvfb)
- ProRes 422 1280x720 avec TC et son :
  - marqueur point, plage I/O/M et commentaires ;
  - marqueurs relus après redémarrage ;
  - export EDL vérifié ;
  - LUT d'inversion et TC incrusté.
- Son synchronisé :
  - son actif, l'image suit l'horloge audio ;
  - son coupé, l'image défile à 25 i/s.
  - La carte son virtuelle de l'environnement de test n'a pas d'horloge réelle : **la synchronisation fine reste à vérifier sur une vraie carte son.**
- WAV 32 pistes : marqueur au TC BWF, sélecteur de sortie.
- Défaut corrigé pendant l'essai : la première lettre tapée juste après M pouvait partir vers les raccourcis.

### Compilation Windows avec ASIO
Elle a été vérifiée par la génération des installeurs : le SDK ASIO (asio-sys 0.4.0) et cpal 0.18.2 ont été compilés sur le poste Windows de GitHub.

Captures : `docs/captures/phase3_logs_tc_incruste.png`, `phase3_lut_affichage.png`, `phase3_audio_logs_sortie.png`.

## Procédure de test manuel
1. Ouvrir ou créer un projet.
2. PLAYER (`Alt+3`), mode VIDEO, ouvrir un rush avec son.
3. Lecture (Espace) : vérifier la synchronisation image et son, par exemple sur un clap.
4. M sur une image, taper un commentaire, puis Entrée. I, avancer, O, puis M : une plage apparaît.
5. Changer une couleur (clic sur la pastille), renseigner scène et prise.
6. Exporter en EDL, ALE, FCPXML et OTIO, puis importer dans Resolve, Avid ou Premiere. Vérifier les sous-clips, les TC et les marqueurs.
7. Bouton LUT : charger une LUT .cube de la caméra (LogC, S-Log3...).
8. Bouton TC : timecode incrusté.
9. Sortie audio :
   - choisir la carte son, puis tester ASIO sous Windows ;
   - sur une carte multicanal, essayer la paire 3-4.
10. Mode AUDIO : ouvrir un WAV polyphonique, poser des marqueurs, exporter le CSV.

## Points d'attention
- **ASIO** :
  - chaque pilote ASIO n'accepte souvent que ses propres fréquences et formats ;
  - si la fréquence du fichier n'est pas acceptée, VERIFLOW rééchantillonne à la volée, comme pour les autres pilotes ;
  - en ASIO, un seul logiciel à la fois peut généralement utiliser la carte.
- **Décalage son/image à la mise en lecture** : il peut atteindre une cinquantaine de millisecondes (tampon de la carte), puis il est rattrapé dès la première relève de position (10 fois par seconde).
- **Pistes son vidéo** : leur décalage éventuel de départ par rapport à l'image (start time différent dans le conteneur) n'est pas encore compensé.

## Compatibilité macOS Catalina (ajout du 07/10/2026)
- **FFmpeg Intel** : il vient désormais d'evermeet.cx (9.0.2, GPL, statique). Vérifié par la génération des installeurs : il exige macOS 10.13 et n'utilise que des bibliothèques du système. Le script refuse tout binaire Intel exigeant plus que 10.15.
- **Application** : version minimale déclarée à macOS 10.15 ; interface compilée pour Safari 13 ; affichage des images sans `createImageBitmap` sur les moteurs antérieurs à Safari 15.
- **Diagnostic de démarrage** (`ui/public/boot.js`, JavaScript ES5) : si l'interface ne démarre pas, la fenêtre affiche la cause et la version du moteur web au lieu d'un écran noir.

## Reste à faire (hors de cette PR)
- Pop-up de saisie vers REPORT (phase REPORT).
- Scopes et sortie moniteur externe (V2, charte §7.3).
- Compensation du décalage de départ entre pistes son et image.
