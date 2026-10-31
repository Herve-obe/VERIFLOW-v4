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
| Contrôles avant copie | Chemins finaux, espace libre de chaque destination, disque dur mécanique (avertissement de lenteur), carte déjà copiée (dans la destination et dans l'historique du projet) avec confirmation ; portable sur batterie (voir ligne Batterie) |
| File d'attente | Plusieurs cartes à la suite, traitées l'une après l'autre en arrière-plan |
| Suivi | Progression, débit, temps restant, fichier en cours, tableau fichier par fichier, fichiers en échec avec la cause |
| Éjection | Option « éjecter la source après une copie conforme » (macOS diskutil, Windows, Linux gio/umount). Avant l'éjection, l'explorateur cesse de surveiller les dossiers du volume (sous Windows, un dossier surveillé garde le disque ouvert). Windows : disques durs et SSD USB, FireWire et cartes reconnus comme éjectables (le système ne déclare « amovibles » que les clés et les cartes), éjection vérifiée (la lettre doit disparaître dans les 15 s, sinon message « encore utilisé par un autre programme »). Corrigé le 18/10/2026 après le test d'Hervé : la source restait occupée jusqu'à la fermeture de VERIFLOW. Deuxième test (19/10/2026) : éjection encore refusée ; pendant l'éjection, la liste des volumes n'est plus relue (sysinfo ouvrait chaque volume toutes les 2 s), trois essais de 5 s, et en cas de refus le message nomme les programmes qui tiennent des fichiers de la source ouverts (gestionnaire de redémarrage de Windows). Troisième test (22/10/2026) : encore refusée ; le journal de Windows (Kernel-PnP, événement 225 du 18/10) désignait VERIFLOW lui-même, et aucun événement le 22/10, donc la commande « Éjecter » de l'Explorateur n'agissait sans doute pas sur un disque dur USB. Depuis le 23/10 : VERIFLOW quitte le disque s'il y a son dossier courant, puis éjecte par le gestionnaire de périphériques (`CM_Request_Device_EjectW`, comme « Retirer le périphérique en toute sécurité ») : disque retrouvé par son numéro, puis boîtier USB parent si besoin. Un refus donne la raison de Windows, les programmes qui ont des fichiers ouverts et l'événement 225. La commande de l'Explorateur reste en secours |
| Arborescence | Modèle `{date}/{carte}` par défaut ; variables `{projet}`, `{jour}`, `{camera}`, `{carte}`, `{bobine}`, `{support}`, `{date}` |
| Carte reconnue (20/10/2026) | D'après l'arborescence : Sony XAVC (`PRIVATE/M4ROOT`), Sony XDCAM (`XDROOT`), Canon XF (`CONTENTS/CLIPS001`), Panasonic P2 (`CONTENTS/VIDEO`), AVCHD, RED (`.RDM`, `.R3D`), Blackmagic RAW, ARRI (`.ari`, clips `A001C001_AAMMJJ_XXXX`), GoPro, DJI. Bobine d'après le nom des clips (`A001C001`, `A001_C001`, `A001_0101XX.RDM`, `A001_08151230_C001.braw`) et lettre de caméra (première lettre). Affiché sous la source. `{bobine}` vaut le nom de la carte si rien n'est reconnu ; `{camera}` prend la lettre reconnue si le champ Caméra est vide. Seuls les noms de dossiers et de fichiers sont lus |
| Historique | Chaque offload est enregistré dans le fichier projet (détection des cartes déjà copiées) |
| Carte réutilisée (31/10/2026) | Carte copiée lundi, recopiée mardi sans formatage : les rushes déjà copiés sont cherchés dans les dossiers des copies précédentes du projet (historique), en plus des dossiers de même nom, même si le dossier du jour porte un autre nom. « Reprendre dans ce dossier » puis, au lancement, « Compléter la copie » (nouveaux rushes seulement, les autres revérifiés) ou « Effacer et tout recopier » ; « Revenir au modèle » copie toute la carte dans le dossier du jour |
| Débit (31/10/2026) | Empreintes calculées à côté de la lecture, un fil par méthode (copie et vérification) : la lecture n'attend plus MD5 ou SHA-256, et plusieurs méthodes se calculent en même temps. Pas de mise en veille pendant une copie, une conversion ou une synchronisation : Windows SetThreadExecutionState, macOS NSProcessInfo (évite aussi App Nap), Linux systemd-inhibit ; rendu dès la fin du travail. Aucun réglage du système n'est modifié (stratégie de retrait des disques, antivirus, signature SMB : au choix de l'utilisateur) |
| Batterie (31/10/2026) | Portable sur batterie : avant copie, avertissement dans le préflight et dans la confirmation de lancement (réévalué au clic sur Lancer) si l'autonomie estimée est inférieure à 1,5 fois la durée de copie, si la batterie est à 20 % ou moins, ou si la copie est estimée à plus de 10 min. La durée est estimée à débit prudent (100 Mo/s, 60 Mo/s si un disque dur est en jeu) pour la lecture de la source plus la relecture de chaque destination. Pendant la copie, relevé toutes les 60 s : bandeau si le portable passe sur batterie, si la batterie descend à 20 %, si l'autonomie devient inférieure à la fin estimée, ou si la copie a démarré sur batterie avec plus de 10 min restantes ; nouvelle alerte seulement si la situation s'aggrave. Sources : Windows GetSystemPowerStatus, macOS pmset -g batt, Linux /sys/class/power_supply (batteries de périphériques ignorées). Aucun avertissement si l'information manque (poste fixe, VM). L'utilisateur peut toujours continuer |

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
1. Logo de la production et vignettes vidéo dans le rapport PDF : fait le 20/10/2026. Bloc « Rapport PDF » d'OFFLOAD : logo (PNG, JPEG, TIFF, BMP, WebP ; transparence sur fond blanc ; en haut à droite, 150 × 50 points au plus) et case « Vignettes des vidéos » (cochée par défaut). Une image par vidéo, à 10 % de sa durée, tirée de la copie vérifiée (jamais de la carte), 5 par ligne avec le nom du fichier, 60 au plus. Images insérées en JPEG par FFmpeg, sans bibliothèque supplémentaire. Le HTML n'a ni logo ni vignettes. Capture : `docs/captures/offload_pdf_logo_vignettes.png`.
2. Horodatage certifié RFC 3161 : fait le 20/10/2026. Case « Horodatage certifié RFC 3161 » du bloc « Rapport PDF » (décochée par défaut), autorité DigiCert (`http://timestamp.digicert.com`) modifiable. L'empreinte SHA-256 du PDF est envoyée à l'autorité ; le jeton signé est écrit à côté du PDF (`<rapport>.pdf.tsr`) dans chaque destination. VERIFLOW contrôle le statut, l'empreinte et le nonce ; la signature se vérifie avec `openssl ts -verify -in rapport.pdf.tsr -data rapport.pdf -CAfile <certificats racines>`. Sans connexion, les rapports sont écrits sans jeton et la carte de la copie l'indique. HTTP sans TLS : le jeton est signé, son transport n'a pas à l'être. Essai réel (DigiCert) : `Verification: OK` par OpenSSL avec les certificats racines du système ; PDF modifié d'un octet : `message imprint mismatch`.
3. Modèles de dossiers propres à chaque caméra : fait le 20/10/2026 (variables `{bobine}` et `{support}`, voir plus haut). Reste la lecture des métadonnées de carte (XML Sony, P2, Canon), avec des cartes exemples.
4. Éjection et détection HDD à vérifier sur de vrais supports Windows et macOS.
