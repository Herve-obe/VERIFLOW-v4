# Prise en main

VERIFLOW gère les rushes image et son d'un tournage : copie vérifiée des cartes, consultation des médias, lecture, synchronisation, conversion et rapports.

## Deux modes, six onglets

- **VIDEO** (accent bleu) et **AUDIO** (accent ambre) : l'interrupteur en haut à droite, ou **Tab** (hors saisie de texte), ou **Ctrl+Tab** partout. Chaque onglet s'adapte au mode : vidéos et images en VIDEO, fichiers son en AUDIO.
- Les onglets se suivent dans l'ordre du travail : **OFFLOAD** (Alt+1), **MEDIA** (Alt+2), **PLAYER** (Alt+3), **SYNC** (Alt+4), **TRANSCODE** (Alt+5), **REPORT** (Alt+6).
- Changer d'onglet n'interrompt rien : une copie en cours ou un clip ouvert continuent.

## Le projet

Un projet est un fichier **.veriflow**, rangé où tu veux (par exemple sur le disque de la production). Il garde l'historique des copies, les métadonnées saisies, les marqueurs et les rapports. **Les médias ne sont jamais modifiés** : tout ce que tu saisis va dans le projet.

- **Fichier > Nouveau projet** (Ctrl+N), **Ouvrir un projet** (Ctrl+O), **Projets récents**.
- **Enregistrer** (Ctrl+S) : chaque modification est déjà écrite au moment où elle est faite ; Ctrl+S le confirme.
- **Enregistrer sous** (Ctrl+Maj+S) : copie complète du projet sous un autre nom, qui devient le projet ouvert. L'original n'est pas modifié.
- Le nom du projet ouvert s'affiche au centre de la barre du haut.

## L'explorateur

Panneau de gauche d'OFFLOAD, MEDIA, SYNC et TRANSCODE :

- **Volumes** (espace libre, disque mécanique signalé « HDD », support amovible signalé ⏏), **Favoris** (clic droit sur un dossier : Épingler), **Projet** (dossiers des copies du projet ouvert).
- Il se met à jour tout seul : carte branchée, dossier créé, fichier copié.
- Glisser-déposer un dossier vers une zone de l'onglet ; clic droit pour les actions ; **Ctrl+B** le masque ou l'affiche.

## Menus, raccourcis et préférences

- **Édition > Préférences** (Ctrl+,) : langue, comportement à l'ouverture, données de l'application, raccourcis, valeurs par défaut d'OFFLOAD, du PLAYER et de TRANSCODE.
- **Préférences > Raccourcis** : clique sur « Modifier » puis appuie sur la nouvelle combinaison. Un raccourci déjà pris est retiré de l'autre action. **Aide > Raccourcis clavier** affiche la liste en vigueur.
- Sous macOS, les menus sont dans la barre du système, en haut de l'écran.

# OFFLOAD : copier une carte

But : copier une source vers une ou plusieurs destinations avec une **preuve d'intégrité bit à bit**.

## Faire une copie

1. **Source** : une carte détectée est proposée directement ; sinon « Parcourir », un glisser-déposer, ou double-clic dans l'explorateur.
2. **Destinations** : « + Ajouter une destination » (deux disques au moins, selon la règle 3-2-1 des DIT). La source n'est lue qu'**une fois** pour toutes les destinations.
3. **Arborescence** : modèle du dossier créé dans chaque destination. Variables : `{projet}`, `{jour}`, `{camera}`, `{carte}`, `{date}`. L'exemple sous le champ montre le chemin produit.
4. Jour de tournage, caméra ou enregistreur, opérateur, remarques : repris dans le rapport.
5. **Empreintes** : XXH128 par défaut (le plus rapide, reconnu par ASC MHL). Plusieurs empreintes possibles en même temps.
6. Option **Éjecter la source après une copie conforme**.
7. **Lancer la copie sécurisée**. Avant de copier, VERIFLOW contrôle l'espace libre, signale un disque mécanique (copie plus lente) et une carte déjà copiée (dans la destination ou dans l'historique du projet) : tu choisis alors de compléter ou de tout recopier.

## Ce que garantit la copie

- L'arborescence de la carte est conservée telle quelle, dossiers vides compris, avec les dates d'origine.
- **Aucune écriture sur la source.**
- Chaque fichier est écrit sous un nom provisoire puis renommé quand il est complet : jamais de fichier à moitié copié.
- Chaque destination est **relue depuis le disque**, et non depuis la mémoire du système, puis comparée à la source.
- Une destination en panne (disque plein, câble arraché) n'arrête pas les autres. Un fichier différent déjà présent n'est jamais écrasé.
- Plusieurs cartes peuvent être mises en file d'attente.

## Résultat et rapports

- Verdict : **CONFORME**, **ÉCHEC** ou **INTERROMPU**, avec le détail fichier par fichier et la cause d'un échec.
- Dans chaque destination :
  - `ascmhl/` : manifeste **ASC MHL v2** (validé par l'outil de référence de l'ASC), chaîné d'une copie à l'autre ;
  - `_VERIFLOW/` : rapport **PDF**, **HTML** et **CSV** (projet, opérateur, poste, durée, débit, empreintes de chaque fichier par destination). Le PDF imprime l'empreinte du CSV et des MHL, et son propre fichier d'empreinte accompagne le PDF : toute modification se voit.
- SHA-256 n'existe pas dans la norme ASC MHL v2 : il figure dans les rapports, pas dans le MHL.

## Éjection

Si l'éjection échoue, le message indique le programme qui garde le disque ouvert (Windows). Ferme-le puis éjecte le disque depuis le système.

# MEDIA : consulter les médias

But : parcourir les médias, les prévisualiser et renseigner leurs métadonnées, sans jamais modifier les fichiers.

## Parcourir

- Clic sur un dossier de l'explorateur : ses médias s'affichent (mode VIDEO : vidéos et images ; AUDIO : sons). La liste suit les copies en cours.
- Vue **liste** (vignette ou forme d'onde, durée, codec, format, TC, scène, prise, prise cerclée, taille) ou **grille**.
- Au survol, une bande de 8 images défile sous la souris.
- **Recherche** (nom, scène, prise, commentaire, bobine, mots-clés) et filtre **prises cerclées**.
- Sélection : clic, Ctrl+clic, Maj+clic, Ctrl+A, flèches.

## Lecteur rapide

**Espace** ou double-clic : le média s'ouvre dans une fenêtre, sans quitter MEDIA. Échap la ferme. « Ouvrir dans le PLAYER » passe à l'onglet PLAYER. Son menu « Moniteur » envoie l'image sur un second écran.

## Métadonnées

- **Inspecteur** : informations techniques (conteneur, codec, résolution, cadence, TC, pistes son, référence temporelle BWF).
- 38 champs en 7 groupes : production, identification, découpage (scène, plan, prise, cerclée, faux départ, son seul, MOS), image (objectif, focale, T-stop, ISO, obturateur, ND, balance des blancs...), son, timecode, notes, plus les noms des pistes.
- Les métadonnées déjà présentes dans les fichiers (iXML, étiquettes QuickTime et MXF) sont lues et proposées.
- Édition d'un média ou **par lot** : « plusieurs valeurs » s'affiche quand la sélection diffère. Tout est enregistré dans le projet.

## Exports

- **CSV** (Excel), **ALE** (Avid, Resolve), **sidecars XMP** (Premiere, Resolve) dans le dossier de ton choix.
- **Copies de travail** : nouveaux fichiers avec les métadonnées écrites dedans (iXML pour les WAV, étiquettes pour les vidéos, sans réencodage). Les originaux restent intacts.
- Boutons **Ajouter au rapport** (REPORT) et **Envoyer vers TRANSCODE**.

# PLAYER : lire et marquer

## Lecture

- **Espace** lecture/pause, **J/K/L** shuttle arrière, pause, avant (en VIDEO, appuis répétés : vitesse ×2, ×4, ×8), **flèches** image par image, **Entrée** stop et retour au début.
- Lecture précise à l'image. En lecture normale, l'image suit l'horloge de la carte son.
- Pistes son de la vidéo : une tranche par piste (vumètre, volume, coupure). Une piste qui porte du **LTC** (Tentacle Sync, par exemple) est détectée et coupée d'office.
- Bouton **TC** : timecode incrusté (affichage seulement). Bouton **LUT** : LUT d'affichage (.cube, .3dl, .dat, .m3d, .csp), le fichier n'est jamais modifié. Bouton **Scopes** : forme d'onde, parade RGB, vectorscope, histogramme.
- **Moniteur** : image plein écran, sans interface, sur un autre écran ; le curseur disparaît après 2 s d'immobilité. Échap la ferme.
- **Sortie audio** : choix de la carte (WASAPI et ASIO sous Windows, CoreAudio sous macOS) et de la paire de canaux.

## Mode AUDIO

Console multipiste (jusqu'à 32 pistes) : niveaux, volume, panoramique, coupure, solo par piste, noms lus dans l'iXML.

## Marqueurs (logs)

Un projet ouvert est nécessaire (VERIFLOW propose d'en créer ou d'en ouvrir un).

- **M** : marqueur à l'image courante, commentaire à taper tout de suite.
- **I** puis **O** puis **M** : marqueur de plage (entrée-sortie).
- 8 couleurs (celles d'Avid), scène et prise ; clic sur un TC pour y aller.
- **Exports** : EDL CMX3600, ALE, CSV, FCPXML, OTIO, pour le clip courant ou tous les clips marqués du projet. Une plage devient un sous-clip ; un marqueur hors de toute plage n'apparaît que dans le CSV.

## Saisie du rapport

**R** ou « Rapport (R) » : fenêtre de saisie de la ligne du clip en cours dans le rapport (REPORT). Entrée enregistre, Échap ferme.

# REPORT : rapports image et son

- **Rapport image** en mode VIDEO, **rapport son** en mode AUDIO, au format des rapports papier de l'école, enregistrés dans le projet et numérotés automatiquement.
- **Ajouter des prises** : bouton du rapport, « Ajouter au rapport » dans MEDIA, ou touche R dans le PLAYER. Les lignes sont pré-remplies : TC, durée, cadence, définition, caméra, scène, prise, cerclée, champs saisis dans MEDIA ; pour les WAV, noms de pistes, TC BWF, fréquence, résolution.
- **Éditeur** : en-tête (production, caméra ou enregistreur, feuillet, remarques), tableau modifiable directement, lignes déplaçables, enregistrement automatique. Listes déroulantes avec « Autre... » pour une valeur libre.
- **Colonnes** : ajoute et ordonne les colonnes voulues (objectif, ISO, ND...). Clic sur un titre de colonne pour trier.
- 1 à 32 pistes ; à l'impression, 8 pistes par feuillet comme sur le papier.
- **Logo** et nom de l'école ou de la production en haut des rapports.
- **Exports** : PDF, XLSX, CSV, HTML, EDL (rapport image).

# SYNC : synchroniser image et son

But : associer chaque plan vidéo au son de l'enregistreur, au décalage près, puis produire des fichiers synchronisés.

## Analyse

1. Ajoute les vidéos et les sons (boutons, glisser-déposer, explorateur) : le tri entre vidéos et sons se fait seul.
2. **Méthode** :
   - **Automatique** : timecode ou LTC d'abord, affiné et complété par la forme d'onde ;
   - **Timecode seul** : tournage avec un timecode fiable (jam sync) ;
   - **Forme d'onde seule** : timecode absent ou faux ; chaque plan est cherché dans tous les sons.
3. **Analyser**. Pour chaque paire : méthode utilisée, décalage (secondes et images), **confiance**, **dérive** d'horloge, remarque (par exemple « timecode corrigé »).

## Vérifier une paire

Clic sur une ligne : formes d'onde superposées (son témoin de la caméra en haut, enregistreur en bas). Réglage à ±1 image, ±1 ms, ±1 échantillon, « Recalculer par la forme d'onde », choix d'un autre son, case **Paire vérifiée**.

## Créer les fichiers synchronisés (re-wrap)

- Nouveau fichier par plan : **image recopiée sans réencodage** et son de l'enregistreur calé (PCM 24 bits), son caméra conservé en option, timecode d'origine.
- **Destination** : à côté de chaque vidéo, ou dans un dossier choisi. Suffixe `_sync` par défaut ; l'original n'est jamais écrasé.
- Conteneur MOV ou MXF (une piste mono par canal), option « une piste mono par canal » en MOV.
- **Corriger la dérive** : le son est rééchantillonné pour suivre l'horloge de la caméra, calé au milieu du plan.
- « Paires validées seulement » limite l'export aux paires cochées.

# TRANSCODE : convertir

## Principe

1. Ajoute les fichiers (boutons, glisser-déposer, ou « Envoyer vers TRANSCODE » depuis MEDIA).
2. Choisis un **préréglage** (environ 80) : montage et étalonnage (ProRes, DNxHR, DNxHD, CineForm...), broadcast (XDCAM, AVC-Intra, XAVC), diffusion (H.264, HEVC, AV1, VP9...), plateformes web, proxies, anciens codecs, images (JPEG, PNG, TIFF, DPX, EXR...), son (WAV, AIFF, FLAC, MP3, AAC...), sans réencodage, analyses.
3. Ajuste les **réglages** par section, la **destination** et le **nommage**, puis lance.

## Sections de réglages

- **Image** : débit, taille, recadrage, rotation, désentrelacement, cadence, LUT, corrections, étiquettes couleur.
- **Incrustations** : timecode, nom du fichier, texte, logo, sous-titres. Éditeur de sous-titres SRT et WebVTT (« Éditer » ou « Créer des sous-titres ») avec contrôles de lisibilité.
- **Son** : choix des pistes, fréquence, résolution, débit, normalisation (EBU R 128, ATSC A/85, streaming, podcast) plafonnée au True Peak.
- **Traitement** selon le préréglage : son de remplacement calé par timecode, conformation de cadence, insert, seuils de détection.
- **MXF OP-Atom (Avid)** pour les préréglages DNxHD 1080.

## Sans réencodage

Changement de conteneur, découpe (points d'entrée et de sortie par fichier, en timecode ou en secondes), remplacement du son, fusion, insert, sous-titres en piste, extraction de l'image, du son ou de chaque piste.

## Ce que VERIFLOW conserve

Métadonnées de tournage (bext, iXML : scène, prise, noms de pistes), **timecode** (décalé avec la découpe), pistes séparées nommées d'après l'iXML. En option : empreinte XXH128 de chaque fichier produit, qualité VMAF, rapport CSV du lot.

## Préréglages personnels

Enregistre tes réglages sous un nom ; exporte-les et importe-les en fichier `.vfpreset` pour les partager.

# Données et désinstallation

- **Préférences > Général > À l'ouverture de VERIFLOW** : *Reprendre où j'en étais*, ou *Repartir à zéro* (destinations, options et affichages effacés à chaque ouverture ; langue, favoris, projets récents, préréglages et préférences gardés).
- **Remettre à zéro maintenant** et **Effacer toutes les données** : même chose, tout de suite.
- Les projets, rapports et copies ne sont jamais effacés par VERIFLOW.
- **Windows** : désinstalle par Paramètres > Applications > VERIFLOW. Coche « Supprimer les données de l'application » pour repartir d'une base propre. Relancer `setup.exe` puis choisir « Désinstaller » enchaîne sur une réinstallation : il suffit d'annuler.
