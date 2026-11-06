# Phase 2 : onglet MEDIA

Date : 06/10/2026
Objectif (charte §7.2) : parcourir les médias, les prévisualiser rapidement, consulter et éditer leurs métadonnées un par un ou par lot, sans jamais modifier les originaux.

## Ce qui est livré

| Fonction | Réalisation |
|---|---|
| Navigation | Ouverture d'un dossier (copie de carte, disque de rushes), sous-dossiers en option, fichiers système ignorés |
| Mode VIDEO / AUDIO | VIDEO : vidéos et images ; AUDIO : fichiers son |
| Affichage | Liste (vignette ou forme d'onde, durée, codec, format, TC, scène, prise, prise cerclée, taille) ou grille |
| Aperçus | Vignette à 10 % de la durée, **bande de 8 images qui suit la souris au survol**, forme d'onde ; tout est mis en cache (recalcul seulement si le fichier change) |
| Recherche et filtres | Nom, scène, prise, commentaire, bobine, mots-clés ; filtre « prises cerclées » |
| Sélection | Clic, Ctrl/Cmd+clic, Maj+clic (plage), Ctrl+A, flèches |
| Lecteur rapide | Espace ou double-clic : pop-up avec le lecteur vidéo ou la console audio, **indépendant de l'onglet PLAYER** ; Échap pour fermer |
| Inspecteur | Informations techniques complètes (conteneur, codec, résolution, cadence, TC, audio, BWF time reference) |
| Métadonnées | 38 champs en 7 groupes (production, identification, découpage, image, son, timecode, notes) + noms des pistes |
| Lecture des métadonnées intégrées | iXML (projet, scène, prise, bobine, note, prise cerclée, noms de pistes), étiquettes QuickTime / MXF, nom de bobine de la piste timecode |
| Édition | Un média ou **par lot** (« plusieurs valeurs » si la sélection diffère), enregistrée dans le fichier projet |
| Exports | **CSV** (Excel), **ALE** (Avid, DaVinci Resolve : Name, Tracks, Start, End, Source File, colonnes de métadonnées), **sidecars XMP** (Premiere, Resolve), **copies de travail** |
| Exports vers le montage (05/11/2026) | Bouton **Montage…** (EXPERT) : FCPXML, XMEML, EDL, ALE, OTIO, CSV Resolve, clips de chutier sans séquence. Branche `claude/exports-montage`, non fusionnée, non testée sur poste réel. Détail ci-dessous |

### Règle d'or respectée (charte §5.4)
Les originaux ne sont **jamais modifiés** :
1. les éditions sont écrites dans le fichier projet `.veriflow` ;
2. les sidecars XMP sont écrits dans le dossier de ton choix ;
3. les copies de travail sont créées ailleurs, sans jamais écraser un fichier existant :
   - WAV : bloc iXML réécrit, audio et autres blocs (bext, fmt...) copiés à l'identique ;
   - vidéo : recopie sans réencodage avec les étiquettes, timecode conservé.

## Explorateur de dossiers (OFFLOAD et MEDIA), ajouté à la demande d'Hervé
| Fonction | Réalisation |
|---|---|
| Contenu | Dossiers uniquement, en trois sections : **Volumes** (nom lisible, espace libre, HDD, amovible), **Favoris** (épinglés par clic droit), **Projet** (dossiers des offloads du projet ouvert) |
| Mise à jour en direct | Volumes branchés ou retirés détectés en 2 s ; dossiers dépliés surveillés par le système (FSEvents, ReadDirectoryChangesW, inotify) ; disques réseau relus toutes les 5 s |
| Légèreté | Sous-dossiers lus au dépliage, seuls les dossiers dépliés sont surveillés, dossiers système et cachés masqués |
| OFFLOAD | Glisser-déposer vers Source ou Destinations ; double-clic = source ; clic droit : utiliser comme source, ajouter comme destination, ouvrir dans le système, épingler |
| MEDIA | Clic sur un dossier = affichage de ses médias ; **la liste se met à jour en direct** (un clip copié pendant un offload apparaît avec sa vignette) ; inventaire plafonné à 5 000 médias pour éviter de parcourir un disque entier par erreur |
| Interface | Panneau redimensionnable, repliable (Ctrl+B), largeur et état mémorisés |

Essai réel (Linux) : arbre des volumes, dossier créé pendant que l'application tourne (apparu dans l'arbre), clip copié dans le dossier affiché (apparu dans la liste avec sa vignette et son TC), menu contextuel et glisser-déposer vers la source. Captures : `phase2_explorateur_*`.

## Champs éditables (résultat de la recherche demandée en question 24)
| Groupe | Champs | Correspondances |
|---|---|---|
| Production | Projet, jour, date, société, réalisateur, chef opérateur, ingé son, DIT | ALE, XMP (xmpDM), iXML PROJECT |
| Identification | Bobine/carte, nom du clip, caméra, enregistreur | ALE Tape/Name/Camera, iXML TAPE |
| Découpage | Séquence, scène, plan, prise, prise cerclée, faux départ, son seul, MOS | ALE Scene/Shot/Take/Circled/MOS, iXML SCENE/TAKE/CIRCLED |
| Image | Objectif, focale, T-stop, mise au point, ISO/EI, obturateur, balance des blancs, teinte, filtres/ND, LUT, cadence capteur | ALE |
| Son | Micros, cadence TC, noms des pistes | iXML TRACK_LIST |
| Timecode | User bits | |
| Notes | Commentaire, note, mots-clés, étiquette de couleur | ALE Comments/Keywords, iXML NOTE, XMP |

**03/11/2026, niveau DÉBUTANT** : métadonnées en lecture seule, texte sélectionnable et copiable, aucun enregistrement.

## Mode Débutant (ajout du 03/11/2026)

En niveau DÉBUTANT, les métadonnées sont en lecture seule : les champs ne sont pas modifiables, leur texte reste sélectionnable et copiable, et le bouton Enregistrer est remplacé par une note. Les listes Oui/Non sont désactivées. Aucun enregistrement n'est fait. Les exports CSV, ALE, XMP, la copie de travail et « Ajouter au rapport » sont masqués. « Ouvrir dans le PLAYER » et « Envoyer vers TRANSCODE » restent disponibles. L'avertissement « pas de projet ouvert » n'apparaît qu'en EXPERT. Détail dans la charte, §3.2.

## Exports vers le montage (05/11/2026)

Le bouton **Montage…** de l'onglet MEDIA ouvre la boîte « Exporter vers le montage ». Il est visible en EXPERT seulement, dans le bloc des exports.

**Logiciel de montage.** Le choix du logiciel coche les formats par défaut, modifiables ensuite :

| Logiciel | Formats cochés par défaut |
|---|---|
| Final Cut Pro | FCPXML 1.11 |
| Premiere Pro | XMEML |
| DaVinci Resolve | FCPXML 1.10 + CSV Resolve |
| Avid Media Composer | ALE + EDL |
| Autre | OTIO + EDL |

Formats cochables : FCPXML (version 1.9, 1.10 ou 1.11 au choix), XMEML (v5), EDL (CMX3600), ALE, OTIO, CSV Resolve. Plusieurs formats s'écrivent en un seul clic.

**Portée.** Les clips sélectionnés, ou les clips affichés du dossier, filtres compris.

**Pas d'écrasement.** Un fichier existant n'est jamais remplacé. Le nouveau fichier prend le suffixe `_2`, `_3`, etc. (un nouvel export dans le même dossier donne `..._2`).

**Clips sans séquence.** Les clips sont exportés comme des clips de chutier, sans séquence à vider ni à construire. FCPXML écrit un événement sans projet, XMEML un bin, OTIO une collection. Seule l'EDL est une séquence.

**Métadonnées et marqueurs.** Scène, plan, prise, caméra, bobine, notes, mots-clés et prise cerclée sont écrits dans chaque format qui les accepte. Les marqueurs du PLAYER (point ou plage, avec leur couleur) sont inclus si l'option est cochée.

**FCPXML (1.9 à 1.11).** Le chemin des médias est dans `media-rep`. Correspondances : Scène = « scène/plan », Take = prise, Camera Name = caméra, Camera Angle = champ angle, à défaut la caméra si elle n'a qu'une lettre. Ces correspondances sont à confirmer sur un vrai Final Cut.

**EDL.** Un événement par clip entier. Les marqueurs du PLAYER (points et plages) sont en commentaires `* LOC` à leur entrée.

**Chemins.** Chemins absolus par défaut. Les chemins relatifs ne sont proposés que pour OTIO : ALE ne garde que le nom du fichier (colonne Source File), EDL, FCPXML et XMEML restent en absolu. Les lecteurs réseau (UNC) et les noms avec espaces, accents ou caractères Unicode sont encodés.

**ALE.** Cadence écrite `23.976` (et non 23.98). Si les clips ont des cadences mêlées, un fichier ALE est écrit par cadence. Option « ALE en ASCII (sans accents) » pour les logiciels qui n'acceptent pas les accents.

**CSV Resolve.** Séparateur virgule, UTF-8 sans BOM. Les intitulés des colonnes doivent être confirmés sur un vrai export de DaVinci Resolve (voir `docs/TESTS_EN_ATTENTE.md`).

**Avertissements affichés après l'export.**
- Cadences mêlées : un ALE par cadence ; l'EDL suit la cadence du premier clip, les durées sont à vérifier.
- Son synchronisé en XMEML : le décalage est arrondi à l'image entière.

**Son synchronisé (SYNC).** Le cœur sait l'écrire en FCPXML, XMEML, OTIO et ALE, mais la boîte Montage… ne l'envoie pas encore. Il viendra dans une PR suivante, avec l'accord d'Hervé. La piste LTC est retirée du son exporté, sauf en OTIO où elle n'est exclue qu'en métadonnées (pas de sélection de canaux standard).

**Limites.**
- AAF non pris en charge.

**Contrôle externe.** `scripts/validate-exports.py` relit les fichiers avec OpenTimelineIO (adaptateurs cmx_3600, ale et otio_json). Le FCPXML est contrôlé par ElementTree, car l'adaptateur OTIO fcpx_xml ne lit pas les clips d'un événement. Le XMEML n'a pas d'adaptateur publié : il est seulement vérifié comme XML bien formé.

## Essais
- Tests automatiques : catalogue et lecture iXML, champs, édition par lot dans le projet, forme d'onde WAV et FFmpeg (avec découpage de lecture), vignettes et bande d'images, CSV/ALE/XMP, copie de travail WAV (audio identique octet pour octet, original intact, aucun écrasement), copie de travail vidéo (timecode conservé).
- Essai réel dans l'application (Linux) : 3 clips ProRes avec TC et 2 WAV 8 pistes avec iXML.
  - Liste, grille et mode AUDIO : OK.
  - Édition par lot de la scène et de la prise sur 3 clips : OK.
  - Lecteur rapide : OK.
  - Export ALE avec TC de début et de fin : OK.
  - Captures : `docs/captures/phase2_*`.

## Reste à faire (hors de cette PR)
1. Lecture des fichiers XML de métadonnées des caméras (Sony `M01.XML`, Panasonic P2, Blackmagic) pour préremplir objectif, ISO, obturateur...
2. Copie de travail des WAV RF64 (plus de 4 Go) : fait le 20/10/2026. Source RIFF, RF64 ou BW64 ; taille réelle des données lue dans le bloc `ds64`. La copie est écrite en RF64 (EBU Tech 3306 : `ds64` en premier bloc, tailles 64 bits, bloc `data` à 0xFFFFFFFF) si la source l'est déjà ou si elle dépasse 4 Gio ; sinon en RIFF comme avant. Tests : RF64 produit relu par VERIFLOW et FFprobe, échantillons identiques, iXML mis à jour, RF64 recopié en RF64. Pas d'essai sur un vrai fichier de plus de 4 Gio ici (seuil abaissé dans le test).
3. Vue arborescente des volumes à gauche de la liste.
