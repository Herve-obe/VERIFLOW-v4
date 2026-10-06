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

### Règle d'or respectée (charte §5.4)
Les originaux ne sont **jamais modifiés** :
1. les éditions sont écrites dans le fichier projet `.veriflow` ;
2. les sidecars XMP sont écrits dans le dossier de ton choix ;
3. les copies de travail sont créées ailleurs, sans jamais écraser un fichier existant :
   - WAV : bloc iXML réécrit, audio et autres blocs (bext, fmt...) copiés à l'identique ;
   - vidéo : recopie sans réencodage avec les étiquettes, timecode conservé.

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
2. Copie de travail des WAV RF64 (plus de 4 Go).
3. Vue arborescente des volumes à gauche de la liste.
