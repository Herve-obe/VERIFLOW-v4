# Phase 5 : onglet SYNC

Date : 10/10/2026
Objectif (charte §7.5) : synchroniser par lot les plans vidéo et les sons de l'enregistreur, par timecode, LTC ou forme d'onde, avec validation plan par plan, détection de la dérive et exports sans réencodage.

## Ce qui est livré

### Lecture de l'heure de chaque fichier

| Source | Détail |
|---|---|
| Métadonnées | Timecode de la piste tmcd QuickTime, du MXF ou du conteneur ; référence temporelle BWF (bext) des WAV |
| LTC | Cherché sur chaque canal pendant les 4 premières secondes quand le fichier n'a pas de timecode. Le canal LTC est ensuite exclu de la corrélation et du re-wrap. Cadences NTSC : heure corrigée de 1,001 |
| Aucune | Le plan peut être trouvé par la forme d'onde (option « Chercher par la forme d'onde ») |

### Méthode (menu, décision d'Hervé du 10/10/2026)

| Méthode | Fonctionnement | Usage |
|---|---|---|
| Automatique | Timecode ou LTC d'abord, puis affinage et recherche par la forme d'onde selon les cases | Cas général |
| Timecode seul | Calage par le timecode ou le LTC uniquement, sans corrélation (la dérive peut quand même être mesurée) | Tournage avec timecode fiable (jam sync) |
| Forme d'onde seule | L'heure des fichiers est ignorée : chaque plan est cherché dans tous les sons du lot | Timecode absent ou faux (jam sync oublié, horloge décalée) |

### Appariement (méthode automatique)

1. **Par l'heure** : chaque plan est associé au son dont la plage horaire le recouvre le plus (passage de minuit compris).
2. **Affinage par la forme d'onde** (option) : corrélation GCC-PHAT du son témoin de la caméra et du son de l'enregistreur, sur 10 s au milieu du passage commun, autour du décalage donné par le timecode (tolérance réglable, 2 s par défaut). Précision : l'échantillon à 16 kHz, soit environ 0,06 ms. Si le timecode était faux, la remarque l'indique (« timecode corrigé de … »).
3. **Recherche sans timecode** (option) : corrélation du son témoin sur toute la durée de chaque son, à 2 kHz, puis affinage.
4. **Confiance** : 1 moins le rapport du second pic au premier. En dessous de 25 %, le résultat est écarté ou signalé.
5. **Dérive** (option) : décalage mesuré au début et à la fin du passage commun (extraits de 4 s, passage commun d'au moins 60 s). La dérive est donnée en images sur la durée du plan ; alerte au-delà d'une image. Correction prévue en V2 (charte).

### Interface

- Ajout de fichiers ou de dossiers (boutons, glisser-déposer, explorateur) : tri automatique entre vidéos et sons.
- Tableau des paires : en mode VIDEO, une ligne par plan ; en mode AUDIO, regroupé par son, avec les sons non utilisés. Colonnes : timecode (badge LTC), méthode, décalage en secondes et en images, confiance, dérive, remarque.
- Détail d'une paire : formes d'onde superposées (son témoin en haut, enregistreur en bas, même instant), durée affichée de 0,5 à 30 s, curseur de position ; réglage à ±1 image, ±1 ms, ±1 échantillon à 48 kHz ; « Recalculer par la forme d'onde » ; choix d'un autre son ; case « Paire vérifiée ».
- Options mémorisées sur le poste.

### Exports

| Export | Détail |
|---|---|
| Re-wrap | Nouveau fichier MOV ou MXF : image copiée sans réencodage, son de l'enregistreur calé (PCM 24 bits ; en MXF une piste mono par canal ; en MOV une piste polyphonique, ou une piste mono par canal avec l'option « Une piste mono par canal »), son caméra conservé en pistes supplémentaires (option), noms de pistes, timecode d'origine. À côté de chaque vidéo ou dans un dossier, suffixe `_sync` par défaut, jamais d'écrasement de l'original |
| FCPXML 1.10 | Final Cut Pro et DaVinci Resolve : chaque plan avec son son attaché, calé à l'échantillon ; une seule ressource par fichier son |
| OTIO | DaVinci Resolve, Premiere (via plugin), Avid : pistes V1 et A1 |

La case « Paires validées seulement » limite l'export aux paires cochées (décochée par défaut).

## Choix techniques

- Cœur `core/src/sync/` : `mod.rs` (lecture de l'heure, appariement, affinage, dérive), `correlate.rs` (décodage mono par FFmpeg avec passe-haut à 120 Hz, GCC-PHAT avec fenêtre de Hann, enveloppes), `export.rs` (re-wrap, FCPXML, OTIO).
- FFT : crate `realfft` 3.5 (MIT ou Apache 2.0, compatible GPLv3).
- Convention : `décalage = début du son − début de la vidéo` (positif : le son commence après l'image).
- Pont Tauri `src-tauri/src/sync.rs` : analyse et re-wrap annulables, avancement par événements.

## Vérifications réalisées

### Tests automatiques (cœur)
9 tests SYNC passent avec FFmpeg 9.0.2, 6.1 et 4.4 (celui de la CI) :
- timecode BWF faux de 0,3 s corrigé par la forme d'onde ;
- plan sans timecode trouvé par la forme d'onde (−12,5 s) ;
- LTC 14:30:00:00 lu sur le canal 1 ;
- dérive de 600 ppm détectée (environ −1,35 image, signe juste) ;
- re-wrap MOV et MXF : pistes, durée, timecode ;
- méthodes : « timecode seul » garde le son à la même heure sans affinage, « forme d'onde seule » trouve le vrai son malgré un leurre à la même heure ;
- MOV avec une piste mono par canal : enregistreur stéréo en 2 pistes mono, plus le son caméra ;
- FCPXML et OTIO : temps attendus.

### Essai dans l'application (Linux, Xvfb)
Deux plans MPEG-2 (l'un à 10:00:00:00, l'autre sans timecode) et un WAV BWF couvrant les deux, horloge fausse de 0,3 s :
- A001C001 : timecode puis forme d'onde, −10,000 s, confiance 98 %, remarque « timecode corrigé de -0,300 s par la forme d'onde » ;
- A001C002 : trouvé par la forme d'onde à −50,000 s, confiance 98 % ;
- re-wrap MOV des deux plans : décalage résiduel mesuré entre la piste de l'enregistreur et la piste caméra des fichiers produits : 0 échantillon ;
- FCPXML : son attaché à 36 000,3 s dans le BWF pour le premier plan, soit 10 s après son début (valeur attendue).

Cas « jam sync oublié » : un plan à 10:00:00:00, un son leurre à la même heure, le vrai son (stéréo) daté de 08:00:00 :
- méthode Automatique : leurre choisi, confiance 11 % signalée en orange, remarque « forme d'onde peu ressemblante » ;
- méthode Forme d'onde seule : vrai son trouvé, −10,000 s, confiance 99 % ;
- re-wrap MOV « une piste mono par canal » : 3 pistes mono (enregistreur gauche et droite, caméra), timecode 10:00:00:00, décalage résiduel 0 échantillon.

## Limites connues
- Correction de la dérive : V2 (charte). V1 : mesure et alerte.
- AAF : V2 (charte).
- Les remarques produites par le cœur (« timecode corrigé de … ») sont en français quelle que soit la langue de l'interface.
- Son témoin très faible ou absent : seule la méthode par timecode est possible (message dans le détail de la paire).
- Plusieurs plans dans un même son : bien gérés ; un plan à cheval sur deux sons : associé au son qui le recouvre le plus.

## À tester sur poste réel
1. Rushes réels d'une même journée (caméra avec timecode) et WAV d'un enregistreur (Sound Devices, Zoom, Zaxcom) : appariement par lot, décalages, confiance.
2. Caméra sans timecode (reflex, smartphone) avec son témoin : recherche par la forme d'onde, puis méthode « Forme d'onde seule » sur un lot au timecode faux.
3. LTC enregistré sur une piste caméra (Tentacle Sync, Deity TC-1) : badge LTC, canal exclu des fichiers produits.
4. Rushes en 23,976 et 29,97 DF : timecodes affichés et décalages.
5. Re-wrap MOV (polyphonique et une piste mono par canal) et MXF : import dans DaVinci Resolve, Premiere et Avid ; pistes nommées, timecode, son caméra en plus.
6. FCPXML dans Final Cut Pro et Resolve, OTIO dans Resolve : son attaché et calé.
7. Long plan (plus de 10 min) : alerte de dérive cohérente avec l'écoute.
