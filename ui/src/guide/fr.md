# Bienvenue

VERIFLOW accompagne les rushes image et son d'un tournage, du déchargement des cartes jusqu'aux fichiers livrés au montage :

- **OFFLOAD** copie les cartes avec une preuve d'intégrité bit à bit ;
- **MEDIA** affiche les médias et leurs métadonnées ;
- **PLAYER** lit les rushes et permet de poser des marqueurs ;
- **REPORT** rédige les rapports image et son ;
- **SYNC** synchronise l'image et le son de l'enregistreur ;
- **TRANSCODE** convertit les fichiers (proxies, livraisons, sons, images).

Trois principes valent dans tout le logiciel :

1. **Les originaux ne sont jamais modifiés.** Ce que tu saisis va dans le fichier projet, et tout fichier produit est un nouveau fichier.
2. **Tout est vérifiable.** Copies relues depuis le disque, empreintes, manifestes ASC MHL, rapports datés.
3. **Rien ne demande Internet**, sauf l'horodatage certifié des rapports (option d'OFFLOAD). FFmpeg est intégré à l'installation.

Ce guide suit l'ordre du travail. Le chapitre **Une journée de tournage** montre l'enchaînement complet ; les chapitres suivants détaillent chaque onglet. En fin de guide : **Problèmes fréquents** et **Glossaire**.

# Prise en main

## L'écran

- **Barre du haut** : menus (Fichier, Édition, Affichage, Lecture, Aide), onglets, nom du projet ouvert au centre, interrupteur **VIDEO / AUDIO** à droite.
- **Explorateur**, à gauche dans OFFLOAD, MEDIA, SYNC et TRANSCODE : disques et dossiers.
- **Zone de travail** au centre : le contenu de l'onglet.
- **Barre d'état** en bas : messages, version.

Sous macOS, les menus sont dans la barre du système, en haut de l'écran.

## Deux modes : VIDEO et AUDIO

- **VIDEO** (accent bleu) : vidéos et images. **AUDIO** (accent ambre) : fichiers son.
- Pour changer : l'interrupteur en haut à droite, la touche **Tab** (hors saisie de texte) ou **Ctrl+Tab** (partout).
- Chaque onglet s'adapte : MEDIA n'affiche que les sons en AUDIO, REPORT crée un rapport son, le PLAYER ouvre sa console multipiste.

## Six onglets

Ils se suivent dans l'ordre du travail : **OFFLOAD** (Alt+1), **MEDIA** (Alt+2), **PLAYER** (Alt+3), **SYNC** (Alt+4), **TRANSCODE** (Alt+5), **REPORT** (Alt+6).

Changer d'onglet n'interrompt rien : une copie, une analyse ou une conversion continuent en arrière-plan, et un clip ouvert reste à sa place.

## Le projet

Un projet est un fichier **.veriflow**. Range-le où tu veux, par exemple sur le disque de la production, à côté des rushes. Il contient :

- l'historique des copies (OFFLOAD reconnaît ainsi une carte déjà copiée) ;
- les métadonnées saisies dans MEDIA ;
- les marqueurs du PLAYER ;
- les rapports de REPORT.

Les médias eux-mêmes ne sont jamais modifiés. Sans projet ouvert, tu peux copier, lire et convertir, mais pas enregistrer de métadonnées, de marqueurs ni de rapports. VERIFLOW propose alors de créer ou d'ouvrir un projet.

- **Fichier > Nouveau projet** (Ctrl+N) : choisis le nom et l'emplacement du fichier.
- **Fichier > Ouvrir un projet** (Ctrl+O), ou **Projets récents**.
- **Enregistrer** (Ctrl+S) : chaque modification est déjà écrite au moment où elle est faite ; Ctrl+S le confirme.
- **Enregistrer sous** (Ctrl+Maj+S) : copie complète du projet sous un autre nom, qui devient le projet ouvert. L'original reste tel quel. Pratique pour garder une version par jour de tournage.

Conseil : un projet par film ou par exercice. Sauvegarde le fichier .veriflow avec les rushes.

## L'explorateur

Il affiche des dossiers seulement, en trois sections :

- **Volumes** : chaque disque avec son nom et son espace libre. « HDD » signale un disque mécanique (copie plus lente), ⏏ un support amovible.
- **Favoris** : clic droit sur un dossier > **Épingler**.
- **Projet** : les dossiers des copies du projet ouvert.

Il se met à jour tout seul : carte branchée, dossier créé, fichier copié. Un double-clic déplie ou replie un dossier.

Usages :
- **glisser-déposer** un dossier vers une zone de l'onglet (Source, Destinations, liste de fichiers) ;
- **clic droit** pour les actions de l'onglet (utiliser comme source, ajouter comme destination, ouvrir dans le système...) ;
- **Ctrl+B** masque ou affiche l'explorateur ; sa largeur se règle en tirant son bord.

## Préférences

**Édition > Préférences** (Ctrl+,). Une rubrique par sujet, à gauche :

- **Général** : langue, comportement à l'ouverture, données de l'application (voir le chapitre **Données et désinstallation**). Ces réglages s'appliquent dès qu'on les change.
- **Raccourcis** : voir plus bas. Ils s'appliquent aussi tout de suite.
- **OFFLOAD** : arborescence, opérateur, empreintes, éjection.
- **PLAYER** : timecode incrusté, scopes, couleur des nouveaux marqueurs, ouverture de la saisie du rapport à chaque nouvelle prise.
- **TRANSCODE** : destination (à côté de chaque fichier source, ou un dossier), en VIDEO comme en AUDIO.

Pour OFFLOAD, PLAYER et TRANSCODE :
- **Valider** (en bas de la fenêtre) enregistre les valeurs et les applique aussitôt aux onglets et aux lecteurs ouverts ;
- **Annuler**, ou la croix, abandonne les changements ;
- **Valeurs d'usine** remet la rubrique à ses valeurs d'origine. Il faut ensuite cliquer sur **Valider**.

Les valeurs validées servent aussi de point de départ quand l'état de travail est vidé (voir **Données et désinstallation**).

## Raccourcis clavier

- **Aide > Raccourcis clavier** affiche la liste en vigueur.
- **Préférences > Raccourcis** : clique sur **Modifier** puis appuie sur la nouvelle combinaison (Échap annule).
  - Un raccourci déjà pris est retiré de l'autre action, et un message le signale.
  - **×** retire un raccourci, **Par défaut** rétablit celui d'origine, **Tout rétablir** remet tous les raccourcis d'origine.
- Couper, copier, coller, annuler et tout sélectionner restent ceux du système.

Raccourcis principaux par défaut :

| Action | Touche |
|---|---|
| Lecture / pause | Espace |
| Arrière, pause, avant (shuttle) | J, K, L |
| Image précédente / suivante | ← / → |
| Stop et retour au début | Entrée |
| Marqueur | M |
| Point d'entrée, de sortie | I, O |
| Saisie du rapport | R |
| Basculer VIDEO / AUDIO | Tab, ou Ctrl+Tab pendant une saisie |
| Onglets | Alt+1 à Alt+6 |
| Explorateur | Ctrl+B |
| Guide d'utilisation | F1 |

## Aide

- **Guide d'utilisation** (F1) : ce guide, en français ou en anglais selon la langue de l'interface.
- **Licences** : textes de la licence de VERIFLOW (GPLv3), du contrat de licence (CLUF), des conditions d'utilisation (CGU), de FFmpeg (avec la version et la configuration réellement installées) et des composants tiers.
- **À propos** : version de VERIFLOW et de FFmpeg, accès aux licences.

# Une journée de tournage

Exemple d'enchaînement, de la carte au montage. Chaque étape est détaillée dans le chapitre de l'onglet.

1. **Le matin** : ouvre le projet du film (Fichier > Ouvrir un projet), ou crée-le le premier jour.
2. **À chaque carte** (OFFLOAD) :
   - branche la carte : elle est proposée comme source ;
   - vérifie les deux destinations (disque de travail et disque de sauvegarde), le jour de tournage et la caméra ;
   - lance la copie sécurisée ;
   - attends le verdict **CONFORME** avant d'effacer ou de reformater la carte dans la caméra.
3. **Pendant les pauses** (MEDIA) : ouvre le dossier de la copie, renseigne scène, prise et prise cerclée par lot, vérifie les métadonnées techniques.
4. **Contrôle des rushes** (PLAYER) : regarde les plans importants, pose des marqueurs (point net, problème de son, bonne réplique), remplis le rapport avec la touche **R**.
5. **Le soir** (SYNC) : synchronise les plans avec les sons de l'enregistreur, vérifie les paires douteuses, crée les fichiers synchronisés.
6. **Livraison** (TRANSCODE) : fabrique les proxies de montage ou les fichiers demandés par la post-production.
7. **Rapports** (REPORT) : finalise les rapports image et son et exporte-les en PDF pour la production.

À la fin de la journée, chaque destination contient la copie des cartes, les manifestes ASC MHL et les rapports de copie. Le fichier projet contient le reste.

# OFFLOAD : copier une carte

But : copier une source (carte, disque, dossier) vers une ou plusieurs destinations avec une **preuve d'intégrité bit à bit**, que la production ou une assurance peut contrôler.

## Préparer la copie

1. **Source** :
   - un support amovible branché est proposé directement : un clic suffit ;
   - sinon **Parcourir**, un glisser-déposer depuis l'explorateur ou le système, ou un double-clic sur un dossier de l'explorateur ;
   - **Actualiser** relit la liste des supports.
2. **Carte reconnue** : sous la source, VERIFLOW indique le type de carte et la bobine, d'après les noms des dossiers et des fichiers (exemple : « Carte reconnue : RED, bobine A002 »). Types reconnus : Sony XAVC et XDCAM, Canon XF, Panasonic P2, AVCHD, RED, Blackmagic RAW, ARRI, GoPro, DJI. Rien n'est écrit sur la carte.
3. **Destinations** : **Ajouter une destination**, autant que nécessaire. Règle des DIT (3-2-1) : au moins deux copies sur deux disques différents, dont une gardée ailleurs. La source n'est lue **qu'une fois** pour toutes les destinations.
4. **Arborescence** : modèle du dossier créé dans chaque destination. L'exemple sous le champ montre le chemin produit. Variables :
   - `{projet}` : nom du projet ouvert ;
   - `{jour}` : jour de tournage saisi ;
   - `{camera}` : caméra saisie, sinon la lettre reconnue sur la carte ;
   - `{carte}` : nom du volume ou du dossier source ;
   - `{bobine}` : bobine reconnue, sinon le nom de la carte ;
   - `{support}` : type de carte reconnu ;
   - `{date}` : date de la copie.

   Exemple : `{date}/{camera}/{bobine}` donne `2026-10-22/A/A002`.
5. **Jour de tournage**, **Caméra / enregistreur**, **Opérateur**, **Remarques** : repris dans le rapport de copie.
6. **Empreintes** : XXH128 par défaut, le plus rapide, reconnu par la norme ASC MHL. Tu peux en cocher plusieurs (MD5, SHA-1, SHA-256, C4...). SHA-256 n'existe pas dans la norme ASC MHL : il figure dans les rapports seulement.
7. **Éjecter la source après une copie conforme** : la carte est éjectée dès le verdict CONFORME.

## Le rapport PDF

Dans le bloc **Rapport PDF** :

- **Logo de la production** : PNG, JPEG, TIFF, BMP ou WebP. Il est imprimé en haut à droite ; une transparence devient du blanc.
- **Vignettes des vidéos** : une image par plan, prise à 10 % de sa durée sur la copie vérifiée (jamais sur la carte), 5 par ligne, 60 au plus. L'extraction ajoute un peu de temps à la fin de la copie.
- **Horodatage certifié RFC 3161** (Internet requis) :
  - l'empreinte du PDF est envoyée à une autorité d'horodatage (DigiCert par défaut, adresse modifiable) ;
  - l'autorité renvoie un jeton signé, écrit à côté du PDF (`.pdf.tsr`) dans chaque destination ;
  - ce jeton prouve que le rapport existait sous cette forme à cette date ;
  - seule l'empreinte du PDF est envoyée, jamais le rapport ni les rushes ;
  - sans connexion, les rapports sont écrits quand même, et la fiche de la copie affiche **Horodatage impossible**.

## Lancer et suivre la copie

Clique sur **Lancer la copie sécurisée**. Avant de copier, VERIFLOW contrôle :

- l'espace libre de chaque destination (« espace insuffisant, manque ... ») ;
- la vitesse des disques : un disque mécanique est signalé, la copie sera plus lente ;
- les copies existantes : une carte déjà copiée dans la destination ou dans l'historique du projet est signalée (voir **Reprendre une copie**).

Pendant la copie, la **File d'attente** affiche une fiche par carte : progression, débit, temps restant, fichier en cours. Tu peux préparer et lancer la carte suivante : elle attend son tour. **Annuler** arrête une copie sans laisser de fichier à moitié écrit.

## Ce que garantit la copie

- L'arborescence de la carte est conservée telle quelle, dossiers vides compris, avec les dates d'origine des fichiers et des dossiers.
- **Aucune écriture sur la source.**
- Chaque fichier est écrit sous un nom provisoire, puis renommé quand il est complet : jamais de fichier à moitié copié.
- Chaque destination est **relue depuis le disque**, et non depuis la mémoire du système, puis comparée à la source.
- Une destination en panne (disque plein, câble arraché) n'arrête pas les autres.
- Un fichier différent déjà présent n'est jamais écrasé sans ton accord.

## Lire le résultat

- **CONFORME** : toutes les copies sont vérifiées bit à bit. Tu peux réutiliser la carte.
- **Échec** : un ou plusieurs fichiers ne sont pas conformes. La liste **Fichiers en échec** donne la destination et la cause. Ne réutilise pas la carte : corrige la cause (disque, câble), puis relance la copie, qui reprendra les fichiers manquants.
- **Copie interrompue** : annulée ou coupée. Relance-la pour la reprendre.

Boutons de la fiche : **Rapport PDF** ouvre le rapport, **Dossier** ouvre la copie dans le système. **Retirer** enlève la fiche de la file ; les rushes copiés, les rapports et les MHL restent sur les disques.

## Les fichiers produits

Dans chaque destination, à côté des rushes :

- `ascmhl/` : manifeste **ASC MHL v2** de la copie, chaîné aux copies précédentes de la même destination. C'est le format reconnu par les laboratoires et les logiciels de DIT pour vérifier une copie plus tard.
- `_VERIFLOW/` : rapport **PDF**, **HTML** et **CSV**. On y trouve le projet, l'opérateur, le poste, la durée, le débit, et l'empreinte de chaque fichier dans chaque destination.
- Le PDF imprime l'empreinte du CSV et des MHL, et son propre fichier d'empreinte (`.pdf.sha256`) l'accompagne : toute modification se voit.

## Reprendre une copie

Si une carte a déjà été copiée, en tout ou en partie, VERIFLOW relit les rushes présents et la source, puis compare leurs empreintes. Il affiche combien sont **Identiques**, **Différents** ou **Absents**. Deux choix :

- **Compléter la copie** : les rushes identiques sont gardés, seuls les absents ou différents sont copiés puis vérifiés ;
- **Effacer et tout recopier** : les rushes de cette source déjà présents sont effacés, puis la copie est refaite en entier (confirmation demandée).

Si la copie précédente est dans un autre dossier (copie interrompue un autre jour), **Reprendre dans ce dossier** la continue au même endroit ; **Revenir au modèle** garde le dossier prévu par l'arborescence.

## Éjection

- Avec l'option cochée, la source est éjectée après une copie conforme : le message **Source éjectée** s'affiche.
- Sous Windows, les disques durs et SSD USB sont éjectables comme les cartes.
- Si l'éjection est refusée, le message nomme ce qui garde le disque ouvert, quand Windows le sait : un fichier ouvert dans un lecteur vidéo, l'Explorateur, l'antivirus ou l'indexation. Ferme ce programme, puis éjecte le disque depuis le système.

# MEDIA : consulter les médias

But : parcourir les médias, les prévisualiser et renseigner leurs métadonnées, sans jamais modifier les fichiers.

## Parcourir

- Clic sur un dossier de l'explorateur, ou **Ouvrir un dossier** : ses médias s'affichent. **Sous-dossiers** inclut le contenu des sous-dossiers. Au-delà de 5 000 médias, choisis un dossier plus précis.
- Mode VIDEO : vidéos et images ; mode AUDIO : sons.
- La liste suit les copies en cours : un clip copié apparaît avec sa vignette.
- Vue **Liste** (vignette ou forme d'onde, durée, codec, format, TC, scène, prise, prise cerclée, taille) ou **Grille**.
- Au survol d'une vignette, une bande de 8 images défile sous la souris : tu vois le plan sans l'ouvrir.
- **Rechercher** (nom, scène, prise, commentaire, bobine, mots-clés) et filtre **Prises cerclées**.
- Sélection : clic, Ctrl+clic (ajouter), Maj+clic (plage), Ctrl+A (tout), flèches.

## Lecteur rapide

**Espace** ou double-clic sur un média : il s'ouvre dans une fenêtre à part, sans quitter MEDIA.

- La fenêtre se déplace sur un autre écran, se redimensionne et se réduit ; elle revient à la même place à l'ouverture suivante. Si cet écran n'est plus branché, elle s'ouvre au centre.
- Ouvrir un autre média recharge la même fenêtre.
- Mêmes commandes que le PLAYER (lecture, J/K/L, pistes son).
- **Ouvrir dans le PLAYER** ferme la fenêtre et ouvre le média dans l'onglet PLAYER.
- Menu **Moniteur** : image plein écran sur un second écran (voir le PLAYER).
- **Échap** ferme la fenêtre.

## Métadonnées

Sélectionne un ou plusieurs médias : le panneau de droite affiche leurs informations.

- **Informations techniques** : conteneur, codec, résolution, cadence, TC, pistes son, référence temporelle BWF.
- **38 champs en 7 groupes** :
  - Production : projet, jour, date, société, réalisateur, chef opérateur, ingénieur du son, DIT ;
  - Identification : bobine ou carte, nom du clip, caméra, enregistreur ;
  - Découpage : séquence, scène, plan, prise, prise cerclée, faux départ, son seul, MOS ;
  - Image : objectif, focale, T-stop, mise au point, ISO, obturateur, balance des blancs, teinte, filtres et ND, LUT, cadence capteur ;
  - Son : micros, cadence TC ;
  - Timecode : user bits ;
  - Notes : commentaire, note, mots-clés, étiquette de couleur.
- **Noms des pistes** son.
- Les métadonnées déjà présentes dans les fichiers (iXML des enregistreurs, étiquettes QuickTime et MXF, bobine de la piste timecode) sont lues et proposées.

**Saisie par lot** : sélectionne plusieurs médias, change un champ, puis **Appliquer à la sélection**. « (plusieurs valeurs) » s'affiche quand les médias diffèrent ; un champ laissé ainsi n'est pas modifié. Tout est enregistré dans le projet (**Enregistré dans le projet.**).

## Exporter

Bouton **Exporter** :

- **Exporter en CSV** : tableau pour Excel ;
- **Exporter en ALE** : métadonnées pour Avid et DaVinci Resolve (nom, pistes, TC de début et de fin, scène, prise...) ;
- **Dossier des sidecars XMP** : un fichier XMP par média, lu par Premiere et Resolve ;
- **Copies de travail** : nouveaux fichiers avec les métadonnées écrites dedans (iXML pour les WAV, étiquettes pour les vidéos, sans réencodage). Les originaux restent intacts. Un WAV de plus de 4 Go est écrit au format RF64, lu par les logiciels audio professionnels.

Autres boutons : **Ajouter au rapport** (REPORT), **Envoyer vers TRANSCODE**, **Ouvrir dans le PLAYER**.

# PLAYER : lire et marquer

## Ouvrir un média

**Ouvrir dans le PLAYER** depuis MEDIA ou le lecteur rapide, ou le sélecteur de fichiers de l'onglet. Le PLAYER passe dans le mode du média (VIDEO ou AUDIO).

## Lecture

- **Espace** : lecture ou pause.
- **J / K / L** : arrière, pause, avant. En VIDEO, des appuis répétés accélèrent (×2, ×4, ×8). Le son est coupé en accéléré.
- **Flèches** : image par image. **Entrée** : stop et retour au début.
- Clic sur la barre de temps pour aller à un point. Les marqueurs y sont affichés.
- En lecture normale, l'image suit l'horloge de la carte son : image et son restent synchrones. Le décalage de départ entre son et image dans le fichier (fréquent en MP4 et après un montage) est compensé.

## Pistes son de la vidéo

À côté de l'image, une tranche par piste : nom, vumètre crête, volume (-60 à +12 dB), coupure. Le vumètre mesure avant le volume : il bouge même si la piste est coupée.

**LTC** : une piste qui porte du timecode audio (Tentacle Sync sur l'entrée droite d'une caméra, par exemple) est détectée à l'ouverture, marquée « LTC » et coupée d'office, pour t'éviter le sifflement. Un message propose de la laisser coupée, de la baisser de 30 dB ou de la réactiver.

## Affichage

- **TC** : timecode incrusté sur l'image (affichage seulement).
- **LUT** : LUT d'affichage (.cube, .3dl, .dat, .m3d, .csp), par exemple la LUT LogC ou S-Log3 de la caméra. Le fichier n'est jamais modifié.
- **Scopes** : forme d'onde, parade RGB, vectorscope (cibles des barres à 75 %, ligne des tons chair), histogramme. Calculés sur l'image affichée, LUT comprise.

Les choix de départ se règlent dans **Préférences > PLAYER**.

## Moniteur (second écran)

Menu **Moniteur** du lecteur : choisis un écran branché (téléviseur ou moniteur en HDMI ou DisplayPort).

- L'image s'affiche en plein écran, sans interface ni TC incrusté, LUT comprise.
- Le curseur de la souris disparaît après 2 s d'immobilité sur cet écran.
- **Échap** sur cet écran, ou le menu Moniteur, ferme la sortie.
- Un seul lecteur alimente le moniteur à la fois : le dernier qui l'a choisi.
- Pas de sortie SDI directe : pour un moniteur SDI, passe par un convertisseur HDMI vers SDI.

## Sortie audio

Choix de la carte son et, pour une carte multicanal, de la paire de canaux (1-2, 3-4...). Pilotes : WASAPI et ASIO sous Windows, CoreAudio sous macOS. Le choix vaut pour tout VERIFLOW.

En ASIO, un seul logiciel à la fois peut généralement utiliser la carte : ferme l'autre logiciel audio si le son ne sort pas.

## Mode AUDIO

Console multipiste, jusqu'à 32 pistes : niveaux, volume, panoramique, coupure, solo, noms des pistes lus dans l'iXML. Les marqueurs se posent comme en VIDEO, calés sur le timecode BWF du fichier.

## Marqueurs (logs)

Un projet ouvert est nécessaire. Sans projet, VERIFLOW propose d'en créer ou d'en ouvrir un, puis pose le marqueur demandé.

- **M** : marqueur à l'image courante ; tape le commentaire tout de suite, puis Entrée.
- **I**, avance, **O**, puis **M** : marqueur de plage (entrée et sortie).
- Pastille de couleur : 8 couleurs (celles d'Avid, reconnues par Premiere, Resolve et OpenTimelineIO). Scène et prise par marqueur.
- Clic sur le TC d'un marqueur pour y aller.

**Exports** : EDL CMX3600, ALE, CSV, FCPXML, OTIO, pour le clip courant ou pour tous les clips marqués du projet.
- Une plage devient un événement (sous-clip) de la timeline ; un clip sans plage est exporté en entier.
- Un marqueur hors de toute plage n'apparaît que dans le CSV.
- Une EDL n'indique pas sa cadence : choisis-la à l'import dans le logiciel de montage.

## Saisie du rapport

**R**, ou le bouton **Rapport (R)** : fenêtre de saisie de la ligne du clip en cours dans le rapport le plus récent (ou un autre, ou un nouveau). La ligne est créée et pré-remplie si besoin. Entrée enregistre, Échap ferme.

Option **Préférences > PLAYER > Ouvrir la saisie du rapport à chaque nouvelle prise** : la fenêtre s'ouvre seule dès qu'une autre prise est chargée (projet ouvert).

# REPORT : rapports image et son

But : produire les rapports image et son du tournage, au format des rapports papier de l'école, à partir des médias et des saisies faites dans MEDIA et le PLAYER.

## Créer un rapport

1. Un projet doit être ouvert : les rapports y sont enregistrés.
2. **Rapport image** en mode VIDEO, **Rapport son** en mode AUDIO. Les rapports sont numérotés automatiquement par type ; le numéro reste modifiable.
3. **Présentation des rapports** : nom de l'école ou de la production, et **logo**, imprimés en haut de chaque rapport.

## Ajouter des prises

Trois façons :
- **Ajouter des clips vidéo** ou **Ajouter des fichiers son** dans le rapport ;
- **Ajouter au rapport** dans MEDIA, pour les médias sélectionnés ;
- la touche **R** dans le PLAYER.

Les lignes sont pré-remplies :
- vidéos : TC IN et TC OUT, durée, son ou muet, cadence, définition, format image, modèle de caméra ;
- WAV : nom du fichier, scène, prise, prise cerclée, note, noms de pistes, TC BWF, fréquence, résolution ;
- champs saisis dans MEDIA : scène et plan, prise, cerclée, faux départ, son seul, MOS, objectif, focale, T-stop, ISO, obturateur, filtres, balance des blancs, commentaire.

**Ligne vide** ajoute une ligne à remplir à la main.

## Modifier

- **En-tête** en trois blocs : production, caméra (ou enregistreur), feuillet et remarques. Ce que tu saisis n'est jamais écrasé par le pré-remplissage.
- **Tableau** : les cellules se modifient directement. Lignes déplaçables (**Monter**, **Descendre**) et supprimables.
- Listes déroulantes avec les choix du papier et des valeurs étendues ; **Autre...** pour saisir une valeur libre.
- Case **Cerclée** en tête de ligne : sur le PDF, le numéro de prise est entouré.
- **Colonnes** : coche les colonnes à ajouter (objectif, ISO, ND, faux départ...) et ordonne-les par glisser-déposer. Les colonnes du rapport papier restent toujours présentes. Le choix sert aussi au rapport suivant du même type.
- **Tri** : clic sur un titre de colonne (2 avant 10), second clic pour l'ordre inverse.
- Pistes : 1 à 32. À l'impression, 8 pistes par feuillet comme sur le papier.
- L'enregistrement est automatique (**Enregistré**).

## Exporter

**PDF**, **XLSX**, **CSV** (s'ouvre dans Excel en français), **HTML** imprimable, et **EDL** CMX3600 pour un rapport image (chaque clip devient un événement, avec ses plages et marqueurs du PLAYER). Les PDF gardent des lignes vides en fin de feuillet pour la saisie à la main sur le plateau.

# SYNC : synchroniser image et son

But : associer chaque plan vidéo au son de l'enregistreur, au décalage près, puis produire des fichiers synchronisés pour le montage.

## Avant le tournage

La synchronisation est d'autant plus sûre que le tournage la prépare :
- **jam sync** : caméra et enregistreur sur le même timecode (Tentacle Sync, Ambient, timecode de l'enregistreur) ;
- **son témoin** : la caméra enregistre le son de son micro, même médiocre ; c'est lui que VERIFLOW compare à l'enregistreur ;
- **LTC** : si la caméra n'a pas d'entrée timecode, le boîtier peut enregistrer le timecode sur une piste audio de la caméra. VERIFLOW le lit.

## Analyser

1. Ajoute les vidéos et les sons : boutons, glisser-déposer de fichiers ou de dossiers, explorateur. Le tri entre vidéos et sons se fait seul.
2. **Méthode** :
   - **Automatique** : timecode ou LTC d'abord, puis affinage et recherche par la forme d'onde selon les cases. C'est le cas général ;
   - **Timecode seul** : tournage avec un timecode fiable ;
   - **Forme d'onde seule** : timecode absent ou faux (jam sync oublié, horloge décalée) ; chaque plan est cherché dans tous les sons.
3. Options :
   - **Affiner par la forme d'onde** : corrige un timecode légèrement faux, à l'échantillon près ;
   - **Chercher par la forme d'onde les plans sans timecode** ;
   - **Mesurer la dérive** : écart d'horloge entre caméra et enregistreur sur la durée du plan ;
   - **Tolérance du timecode (s)** : écart maximal cherché autour du timecode (2 s par défaut).
4. **Analyser**.

## Lire les résultats

Une ligne par plan (en VIDEO) ou par son (en AUDIO, avec les sons non utilisés) :

- **Timecode** du plan (badge « LTC » s'il vient d'une piste audio) ;
- **Méthode** utilisée ;
- **Décalage** en secondes et en images ;
- **Confiance** : ressemblance entre le son témoin et l'enregistreur. En dessous de 25 %, le résultat est écarté ou signalé en orange ;
- **Dérive**, en images sur la durée du plan, avec alerte au-delà d'une image ;
- **Remarque**, par exemple « timecode corrigé de +0,300 s par la forme d'onde ».

## Vérifier une paire

Clic sur une ligne : le détail s'ouvre sous le tableau.
- Formes d'onde superposées au même instant : son témoin de la caméra en haut, enregistreur en bas. Les pics (clap, consonnes) doivent tomber l'un sous l'autre.
- Durée affichée de 0,5 à 30 s, curseur de position.
- Réglage manuel à ±1 image, ±1 ms ou ±1 échantillon.
- **Recalculer par la forme d'onde**, ou choix d'un autre son.
- Case **Paire vérifiée** une fois contrôlée.

## Créer les fichiers synchronisés (re-wrap)

Pour chaque plan, un nouveau fichier :
- **image recopiée sans réencodage** : aucune perte, conversion rapide ;
- son de l'enregistreur calé (PCM 24 bits), avec les noms de pistes ;
- son de la caméra conservé en pistes supplémentaires (option) ;
- timecode d'origine.

Réglages :
- **Destination** : « À côté de chaque vidéo » ou « Dans un dossier » (Parcourir) ;
- suffixe `_sync` par défaut ; l'original n'est jamais écrasé ;
- conteneur **MOV** ou **MXF** (une piste mono par canal) ; en MOV, option « Une piste mono par canal » ;
- **Corriger la dérive** (cochée par défaut) : le son est rééchantillonné pour suivre l'horloge de la caméra, calé au milieu du plan ;
- **Paires validées seulement** : n'exporte que les paires cochées.

# TRANSCODE : convertir

But : convertir des fichiers vidéo, son et image, en gardant le timecode et les métadonnées de tournage.

## Principe

1. **Ajoute les fichiers** : boutons, glisser-déposer (les dossiers sont parcourus), ou **Envoyer vers TRANSCODE** depuis MEDIA.
2. **Choisis un préréglage** (environ 80), par famille :
   - montage et étalonnage : ProRes, DNxHR, DNxHD, CineForm, Animation, non compressé, FFV1 ;
   - broadcast : XDCAM, AVC-Intra, XAVC, HAP ;
   - diffusion : H.264, HEVC, AV1, VP9 ;
   - plateformes web : YouTube, Vimeo, vertical 9:16, carré, portrait ;
   - proxies : ProRes Proxy, DNxHR LB, H.264 à mi-taille, suffixe `_proxy` ;
   - anciens codecs ;
   - images : JPEG, PNG, TIFF, DPX, OpenEXR, WebP ;
   - son : WAV/BWF, AIFF, FLAC, ALAC, MP3, AAC, Opus, Vorbis, AC-3 ;
   - sans réencodage ;
   - analyses.

   Un préréglage impossible avec le FFmpeg installé est grisé, avec la raison.
3. **Ajuste les réglages** par section, la **destination** et le **nommage**.
4. **Lance** : la file affiche la progression de chaque fichier.

## Recettes courantes

- **Proxies pour le montage** : préréglage Proxies (ProRes Proxy ou DNxHR LB), destination « Dans un dossier ». Timecode et pistes conservés.
- **Fichier pour un client** : H.264 ou YouTube 1080p ; section Son, normalisation « streaming ».
- **Version pour les réseaux** : vertical 9:16 ; le recadrage centré est fait d'office.
- **Sons pour le montage** : WAV ; les métadonnées bext et iXML (scène, prise, noms de pistes) sont recopiées.
- **Extraire le son d'une vidéo** : préréglage sans réencodage, extraction du son (WAV polyphonique) ou de chaque piste (WAV mono nommés d'après les pistes).
- **Couper un plan** : préréglage sans réencodage avec découpe, points d'entrée et de sortie par fichier. Le PLAYER donne les timecodes.
- **Titre avec transparence** : ProRes 4444, Animation, PNG ou TIFF gardent la couche alpha de la source.

## Sections de réglages

- **Image** : débit et encodeur, taille, rapport d'image, rotation et miroirs, recadrage, désentrelacement, cadence, LUT 3D, luminosité, contraste, saturation, gamma, étiquettes couleur (Rec. 709, 2020, 601).
- **Incrustations** : timecode (de la source ou imposé), nom du fichier, texte libre, logo (position, taille, opacité), sous-titres incrustés ou en piste.
- **Son** : pistes (toutes, 1 et 2, mix stéréo, sans son), fréquence, résolution, débit, normalisation (EBU R 128, ATSC A/85, streaming, podcast ou cible libre), plafonnée au True Peak, sans compression ni limiteur.
- **Traitement**, selon le préréglage : son de remplacement calé par timecode, conformation de cadence, plan à insérer, seuils de détection.
- **MXF OP-Atom (Avid)**, pour les préréglages DNxHD 1080 : un fichier image et un fichier par canal son, à copier dans le dossier Avid MediaFiles.

## Sous-titres

Dans la section Incrustations : **Éditer** un fichier SRT, WebVTT, ASS ou SSA, ou **Créer des sous-titres**.
- Temps de début et de fin, texte ; ajout, coupe en deux, fusion, suppression, tri, décalage global.
- Contrôles de lisibilité en direct : 42 caractères par ligne, 2 lignes, 17 caractères par seconde, durées, écart entre sous-titres, chevauchements.
- ASS et SSA : les styles, positions et balises du fichier sont conservés ; seuls les temps et le texte se modifient ici. Pour changer les styles, utilise un logiciel dédié comme Aegisub.
- Le fichier enregistré devient celui de la conversion.

## Destination et nommage

- À côté de chaque fichier source, ou dans un dossier.
- Préfixe, suffixe, remplacement de texte, numérotation ; un aperçu montre le nom produit.
- Fichier déjà présent : nouveau nom, ignorer ou remplacer. L'original n'est jamais remplacé.
- Vérification en option : empreinte XXH128 de chaque fichier produit, qualité VMAF, rapport CSV du lot.

## Ce que VERIFLOW conserve

- **Timecode**, décalé avec la découpe.
- **Métadonnées de tournage** des WAV (bext, iXML), référence temporelle recalculée si la fréquence change ou si le début est coupé.
- **Pistes séparées** nommées d'après l'iXML (`_A01_Perche.wav`).
- Chaque fichier est écrit sous un nom provisoire, puis renommé à la fin.

## Préréglages personnels

Enregistre tes réglages sous un nom. Exporte-les et importe-les en fichier `.vfpreset` pour les partager entre postes (sans chemins propres au poste).

# Données et désinstallation

## À l'ouverture de VERIFLOW

**Préférences > Général > À l'ouverture de VERIFLOW** :
- **Reprendre où j'en étais** (par défaut) : destinations, options et affichages sont retrouvés tels que tu les as laissés ;
- **Repartir à zéro** : l'état de travail est vidé à chaque ouverture, et les onglets reprennent les valeurs des Préférences. Utile sur un poste partagé (salle de cours).

L'état de travail comprend : destinations et options d'OFFLOAD, réglages de SYNC, dossiers dépliés, affichages du PLAYER, sections de TRANSCODE. Restent toujours : la langue, les projets récents, les favoris, les préréglages TRANSCODE créés, la sortie audio et les Préférences.

## Vider ou tout effacer

**Préférences > Général > Données de l'application** :
- **Vider l'état de travail** : la même chose qu'une ouverture « à zéro », tout de suite. Les Préférences ne changent pas ;
- **Effacer toutes les données** : tout, réglages, favoris, projets récents et préréglages compris. Action définitive.

Une confirmation est demandée, puis VERIFLOW se recharge. Les fichiers de projet, les rapports et les copies ne sont jamais effacés.

## Désinstaller

- **Windows** : Paramètres > Applications > VERIFLOW > Désinstaller. Coche **Supprimer les données de l'application** pour repartir d'une base propre à la réinstallation.
- Relancer `setup.exe` puis choisir « Désinstaller » enchaîne sur une réinstallation : il suffit d'annuler.

# Problèmes fréquents

## La carte n'apparaît pas comme source

- Vérifie qu'elle est visible dans le système (Explorateur ou Finder), puis clique sur **Actualiser**.
- Sinon, choisis son dossier avec **Parcourir**.

## La copie est lente

- Un disque mécanique (« HDD ») écrit moins vite qu'un SSD. La copie écrit sur toutes les destinations à la fois : la plus lente donne le rythme.
- Les câbles et les ports USB comptent : un port USB 2 limite fortement le débit.
- La vérification relit tout depuis les disques : elle prend du temps, c'est le prix de la preuve.

## Une copie est en échec

Lis la cause dans **Fichiers en échec** : disque plein, disque débranché, erreur de lecture de la carte. Corrige-la, puis relance la copie : VERIFLOW propose de **Compléter la copie** sans recopier les fichiers déjà conformes. Ne réutilise pas la carte tant que le verdict n'est pas CONFORME.

## L'éjection est refusée

Un programme garde le disque ouvert. Le message le nomme quand Windows le sait. Ferme les fenêtres de l'Explorateur et les lecteurs ouverts sur ce disque, puis éjecte-le depuis le système. L'antivirus ou l'indexation peuvent aussi le retenir quelques secondes.

## Pas de son à la lecture

- Vérifie la sortie audio du lecteur (carte et paire de canaux).
- Une piste marquée « LTC » est coupée d'office : réactive-la si ce n'est pas du timecode.
- En ASIO, ferme les autres logiciels audio.

## Image et son décalés

Le décalage de départ dans le fichier est compensé. Si un décalage reste, il vient souvent du tournage lui-même : vérifie le plan sur un clap. Pour caler le son d'un enregistreur, utilise SYNC.

## SYNC ne trouve pas le bon son

- Confiance faible et remarque « forme d'onde peu ressemblante » : le timecode est sans doute faux. Relance avec la méthode **Forme d'onde seule**.
- Le son témoin de la caméra est absent ou très faible : seule la méthode par timecode est possible.
- Vérifie la paire dans le détail et choisis un autre son si besoin.

## Un préréglage TRANSCODE est grisé

Le FFmpeg installé ne contient pas l'encodeur nécessaire : la raison est indiquée. Choisis un préréglage voisin.

## Je ne peux pas enregistrer de marqueurs ou de rapports

Un projet doit être ouvert : Fichier > Nouveau projet ou Ouvrir un projet.

## Signaler un problème

Indique le système (Windows, macOS, Linux), le support (carte, disque), le fichier concerné (caméra, codec), ce que tu as fait et le message affiché.

# Glossaire

- **ALE** : Avid Log Exchange, fichier texte de métadonnées lu par Avid et DaVinci Resolve.
- **ASC MHL** : norme de l'American Society of Cinematographers qui décrit une copie et ses empreintes, pour la vérifier plus tard.
- **BWF** : Broadcast Wave Format, WAV avec un bloc bext qui contient notamment le timecode de début.
- **Dérive** : écart progressif entre deux horloges (caméra et enregistreur) sur la durée d'un plan.
- **EDL** : liste de montage (CMX3600), lue par tous les logiciels de montage.
- **Empreinte** : nombre calculé à partir du contenu d'un fichier ; un seul octet changé donne une empreinte différente. XXH128, MD5, SHA-256 sont des méthodes de calcul.
- **iXML** : métadonnées des enregistreurs son dans les WAV (scène, prise, noms de pistes).
- **Jam sync** : réglage de plusieurs appareils sur le même timecode.
- **LTC** : timecode enregistré comme un signal audio sur une piste son.
- **LUT** : table de conversion des couleurs, par exemple pour afficher une image Log en Rec. 709.
- **MOS** : plan tourné sans son.
- **Prise cerclée** : bonne prise, retenue pour le montage.
- **Proxy** : copie légère d'un rush, pour monter sur un poste modeste.
- **Re-wrap** : nouveau fichier qui reprend l'image sans la réencoder, avec un autre son ou d'autres métadonnées.
- **RF64** : variante du WAV pour les fichiers de plus de 4 Go.
- **RFC 3161** : norme d'horodatage certifié par une autorité tierce.
- **Son témoin** : son enregistré par la caméra, qui sert à caler le son de l'enregistreur.
- **Timecode (TC)** : heure inscrite dans chaque image ou échantillon, au format HH:MM:SS:II.
- **True Peak** : niveau de crête réel d'un son, entre les échantillons.
