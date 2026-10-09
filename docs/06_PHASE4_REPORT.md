# Phase 4 : onglet REPORT (rapports image et son)

Date : 09/10/2026
Objectif (charte §7.4) : produire les rapports image et son du tournage, au format des rapports papier de l'école, avec des colonnes choisies et ordonnées par l'utilisateur, à partir des médias et des saisies faites dans MEDIA et PLAYER.

## Ce qui est livré

| Fonction | Réalisation |
|---|---|
| Rapports | **Rapport image** en mode VIDEO, **rapport son** en mode AUDIO. Enregistrés dans le fichier projet (schéma v5), numérotés automatiquement par type (numéro modifiable) |
| Base | Reproduction des rapports papier de l'école (Drive VERIFLOW/V4 : `Rapport Image`, `Rapport Son 8 pistes`) : mêmes champs, mêmes cases à cocher, même ordre ; lignes vides en fin de feuillet pour la saisie à la main sur le plateau |
| **Colonnes** (révisé le 09/10/2026, remplace les modèles École et Pro) | Bouton « Colonnes » de l'éditeur : cocher les colonnes à ajouter (fichier, durée, TC son, objectif, focale, T-stop, ISO, obturateur, ND, balance des blancs, faux départ, son seul, MOS en image ; fichier, TC IN, durée, faux départ, son seul en son) et les **ordonner** par glisser-déposer ou flèches. Les colonnes du rapport papier restent toujours présentes. Le choix vaut pour l'éditeur et tous les exports, et sert de point de départ au rapport suivant du même type |
| **Tri** | Clic sur un titre de colonne : tri des prises (ordre naturel, 2 avant 10 ; cases vides en fin), second clic pour l'ordre inverse |
| Pistes | 1 à 32 pistes, noms lus dans l'iXML. Impression : **8 pistes par feuillet** comme le papier ; au-delà, feuillets supplémentaires pour les pistes 9 à 16, etc. |
| Pré-remplissage | Vidéos : TC IN et TC OUT, durée, audio ou muet, cadence, définition (HD, 4K), format image, modèle de caméra. WAV : ID (nom du fichier), scène, prise, prise cerclée, note, noms de pistes, TC BWF, fréquence, résolution. **Champs saisis dans MEDIA** : scène et plan (« 12/3 »), prise, cerclée, faux départ, son seul, MOS, objectif, focale, T-stop, ISO, obturateur, filtres, balance des blancs, commentaire, et pour l'en-tête projet, réalisateur, chef opérateur, ingénieur du son, caméra, enregistreur, date |
| Éditeur | En-tête en trois blocs (production, caméra ou enregistreur, feuillet et remarques) ; choix proposés (cases du papier et valeurs étendues : 23.976, 29.97, 50, 60 i/s, CFexpress, SxS, 44,1 et 192 kHz, 32 bits float) avec saisie libre ; tableau modifiable directement, lignes déplaçables et supprimables ; enregistrement automatique |
| Ajout de prises | Bouton dans le rapport (sélecteur de fichiers), bouton **« Ajouter au rapport »** dans MEDIA (sélection multiple), touche **R** dans le PLAYER |
| Touche **R** (PLAYER) | Fenêtre de saisie de la ligne du clip en cours dans le rapport le plus récent (ou un autre, ou un nouveau) ; ligne créée et pré-remplie si besoin ; Entrée enregistre, Échap ferme. Aussi par le bouton « Rapport (R) » à côté de « + Marqueur (M) » |
| Exports | **PDF**, **XLSX**, **CSV** (point-virgule, s'ouvre dans Excel en français), **HTML** imprimable, **EDL** CMX3600 des prises d'un rapport image (marqueurs et plages du PLAYER compris) |
| Présentation | Nom de l'école ou de la production et **logo** (PNG, JPEG, SVG) imprimés en haut des rapports, enregistrés dans le projet |
| Mise en page PDF (révisée le 09/10/2026) | Bandeau de titre avec pastilles Rapport, Feuillet et Support ; informations en encadrés ; cases d'option nettes ; tableau à titres foncés et lignes alternées, qui occupe toute la page ; **numéro de prise entouré** pour une prise cerclée ; date d'édition en pied de page |
| Prise cerclée | Case « Cerclée » en tête de chaque ligne de l’éditeur et dans la fenêtre de la touche R (vidéo et son) ; sur le PDF, seul le numéro de prise est entouré (pas de colonne) ; colonne « Cerclée » gardée dans les exports XLSX, CSV et HTML pour filtrer les bonnes prises |
| Champs à choix | Liste déroulante (cases du papier et valeurs étendues) avec **« Autre... »** pour saisir une valeur à la main ; « Format image » propose les formats d'enregistrement courants (ProRes, DNxHD/DNxHR, XAVC, XF-AVC, AVC-Intra, H.264, H.265, Blackmagic RAW, REDCODE RAW, ARRIRAW...) |

## Choix techniques

- **PDF : Typst 0.15 embarqué** (licence Apache 2.0), comme prévu par la charte (§5.2). La mise en page est décrite dans `templates/reports/` :
  - `common.typ` : police, couleurs, cases à cocher, tableau, pied de page ;
  - `image.typ` : rapport image (A4 portrait avec les colonnes de base, paysage dès qu'une colonne est ajoutée) ;
  - `son.typ` : rapport son (A4 paysage).

  Modifier ces fichiers suffit pour changer l'apparence des rapports, sans toucher au code. Les données leur arrivent dans un fichier `data.json` virtuel.
- **Police Inter** (licence SIL OFL 1.1, celle de l'interface), embarquée dans l'application (`templates/fonts/`) : le PDF est identique sur Windows, macOS et Linux, sans dépendre des polices installées. Timecodes en DejaVu Sans Mono, fournie avec Typst.
- **XLSX : rust_xlsxwriter** (MIT ou Apache 2.0).
- Licences de toutes les dépendances vérifiées : compatibles GPLv3.
- Le PDF est « balisé » (accessibilité). Certaines versions de l'outil `pdfinfo` (poppler) affichent « Suspects object is wrong type » : c'est un défaut connu de poppler, le PDF est conforme.

## Vérifications réalisées

### Tests automatiques (cœur)
94 tests passent, dont ceux de cette phase :
- colonnes de base identiques aux rapports papier ; ordre choisi conservé, colonnes de base rétablies si absentes ;
- champs MEDIA vers lignes (scène/plan, cerclée, MOS) ;
- en-tête saisi par l'utilisateur jamais écrasé par le pré-remplissage ;
- feuillets : lignes vides pour la saisie à la main, 8 pistes par feuillet ;
- cases à cocher et « autre » ;
- rendu PDF image et son, colonnes de base ou toutes les colonnes ;
- CSV (échappement, toutes les pistes), HTML (échappement), XLSX ;
- rapports enregistrés, numérotés par type, modifiés et supprimés dans le projet.

### Essai dans l'application (Linux, Xvfb)
- Rapport image créé depuis MEDIA avec 7 clips : TC, cadence, définition et format repris ; scène et prise reprises de MEDIA ; export PDF, EDL.
- Rapport son d'un WAV 32 pistes : 32 noms de pistes iXML, scène 12A, prise 3, 48 kHz, 24 bits ; PDF de 4 feuillets (pistes 1-8, 9-16, 17-24, 25-32).
- Touche R dans le PLAYER : ligne ajoutée au rapport en cours, visible dans l'onglet REPORT.
- Colonnes : ajout de Fichier et ISO, Fichier glissé en tête ; tableau, tri par Scène/Plan et PDF paysage conformes ; choix conservé à la réouverture.

## Limites connues
- Le modèle des rapports papier a été reconstitué d'après le contenu des fichiers Excel de l'école : **la mise en page exacte est à comparer avec les originaux** (marges, tailles, nombre de lignes par feuillet).
- Ouverture automatique de la fenêtre de saisie au changement de prise (charte : PROPOSÉ) : non faite ; la touche R suffit pour l'instant.

## À tester sur poste réel
1. Créer un rapport image et un rapport son ; comparer les PDF aux rapports papier de l'école. Puis ajouter et réordonner des colonnes (bouton « Colonnes »), trier en cliquant sur un titre, et vérifier le PDF (paysage) et le rapport suivant (mêmes colonnes).
2. Ajouter des prises depuis MEDIA (« Ajouter au rapport ») et depuis le PLAYER (touche R).
3. Exporter en PDF, XLSX (ouvrir dans Excel), CSV, HTML, et EDL (importer dans DaVinci Resolve).
4. Ajouter un logo et le nom de l'école, vérifier leur place sur le PDF.
5. Rapport son d'un enregistreur réel (Sound Devices) : noms de pistes, scène, prise, TC.
