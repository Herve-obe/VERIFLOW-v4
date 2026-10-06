# Phase 1 : onglet OFFLOAD

Date : 06/10/2026
Objectif (charte §7.1) : copier une source vers une ou plusieurs destinations avec une preuve d'intégrité bit à bit, opposable aux assurances.

## Ce qui est livré

### Copie et vérification
| Fonction | Réalisation |
|---|---|
| Source | Carte, disque, dossier. Les supports amovibles branchés sont proposés directement |
| Destinations | 1 à N (bouton « + »), écriture **simultanée** à partir d'**une seule lecture** de la source |
| Structure | Arborescence de la carte conservée intégralement, **dossiers vides compris**, dates d'origine des fichiers et des dossiers rétablies |
| Originaux | Jamais modifiés : aucune écriture sur la source (le MHL est écrit dans les destinations) |
| Fichier final | Toujours complet : écriture dans `.nom.vfpart`, synchronisation disque, puis renommage |
| Vérification | Relecture **intégrale** de chaque destination **depuis le support physique** : le cache mémoire du système est contourné (F_NOCACHE macOS, fadvise Linux, FILE_FLAG_NO_BUFFERING Windows), sinon on relirait la mémoire et non le disque |
| Empreintes | XXH128 (défaut), XXH64, XXH3-64, MD5, SHA-1, SHA-256, C4, plusieurs à la fois |
| Robustesse | Une destination en échec (disque plein, câble arraché) n'arrête pas les autres ; un fichier différent déjà présent n'est jamais écrasé ; annulation sans fichier partiel |
| Reprise | Les fichiers complets déjà présents ne sont pas recopiés, seulement revérifiés |
| Contrôles avant copie | Chemins finaux, espace libre de chaque destination, disque dur mécanique (avertissement de lenteur), carte déjà copiée (dans la destination et dans l'historique du projet) avec confirmation |
| File d'attente | Plusieurs cartes à la suite, traitées l'une après l'autre en arrière-plan |
| Suivi | Progression, débit, temps restant, fichier en cours, tableau fichier par fichier, fichiers en échec avec la cause |
| Éjection | Option « éjecter la source après une copie conforme » (macOS diskutil, Windows, Linux gio/umount) |
| Arborescence | Modèle `{date}/{carte}` par défaut ; variables `{projet}`, `{jour}`, `{camera}`, `{carte}`, `{date}` |
| Historique | Chaque offload est enregistré dans le fichier projet (détection des cartes déjà copiées) |

### ASC MHL v2
- Une génération par copie dans `<destination>/ascmhl/NNNN_<carte>_<date>.mhl`, chaînée dans `ascmhl_chain.xml` par empreinte C4.
- Empreintes de fichiers, de dossiers (contenu et structure) et racine.
- **Validé avec l'implémentation de référence de l'ASC** (ascmhl 1.2, https://github.com/ascmitc/mhl) :
  - schéma XSD : conforme (manifeste et chaîne) ;
  - empreintes de dossiers et racine : identiques au caractère près à celles calculées par l'outil de référence ;
  - `ascmhl verify` : OK ; corruption d'un seul octet : détectée.
- Script de contrôle : `scripts/validate-mhl.sh <dossier copié>`.
- Limite de la norme : SHA-256 n'existe pas dans ASC MHL v2. Il figure dans les rapports CSV, HTML et PDF, pas dans le MHL.

### Rapports
- `<destination>/_VERIFLOW/` : CSV (séparateur « ; », compatible Excel), HTML autonome, PDF A4 multipage.
- Contenu : verdict (CONFORME / ÉCHEC / INTERROMPU), projet, opérateur, poste, logiciel, début, fin, durée, débit, source et empreinte d'inventaire, volume, empreintes utilisées, méthode de vérification, chaque destination et son MHL, liste des fichiers avec empreinte et statut par destination.
- Preuve de non-modification : le PDF et le HTML impriment l'empreinte SHA-256 du CSV de données et des MHL ; l'empreinte du PDF est dans `<rapport>.pdf.sha256`.

## Essais
- 50 tests automatiques du cœur passent (49 unitaires, 1 d'intégration vidéo), dont :
  - empreintes identiques aux vecteurs de l'outil de référence ASC ;
  - copie vers deux destinations ;
  - reprise sans recopie ;
  - corruption silencieuse détectée ;
  - fichier existant jamais écrasé ;
  - destination en panne sans effet sur l'autre ;
  - annulation sans fichier partiel ;
  - destination à l'intérieur de la source refusée ;
  - rapports et historique dans le projet.
- Essai réel depuis l'interface (Linux) : fausse carte Sony (13 fichiers, 240 Mo, dont un dossier vide) vers deux destinations. Résultat : 5 s, 52 Mo/s sur disque mécanique, CONFORME, contenu identique, MHL validé par l'outil de référence. Captures : `docs/captures/phase1_offload_en_cours.png`, `phase1_offload_termine.png`, `phase1_rapport_pdf.png`.

## Reste à faire (prévu, hors de cette PR)
1. Logo de la production et vignettes vidéo dans le rapport PDF.
2. Horodatage certifié RFC 3161 optionnel quand le poste est connecté.
3. Modèles de dossiers propres à chaque caméra (noms de bobine Sony, Panasonic P2, Blackmagic) et lecture des métadonnées de carte.
4. Éjection et détection HDD à vérifier sur de vrais supports Windows et macOS.
