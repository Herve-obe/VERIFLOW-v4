# VERIFLOW : contrat de licence utilisateur final (CLUF)

Version du 20/10/2026. **Projet à faire relire par un juriste avant toute diffusion commerciale.**

## 1. Objet

Ce document décrit les conditions d'utilisation et de redistribution du logiciel VERIFLOW (l'« Application ») et des composants livrés avec lui. Il ne retire ni ne limite aucun des droits accordés par la licence publique générale GNU, version 3 (GPLv3) : en cas de contradiction, la GPLv3 prévaut.

Éditeur et auteur : Hervé Obejero.

## 2. Licence de l'Application

L'Application est un logiciel libre, distribué sous **GNU General Public License version 3 ou toute version ultérieure** (GPL-3.0-or-later). Le texte complet est fourni avec l'Application (fichier `LICENSE-GPL-3.0.txt`) et sur https://www.gnu.org/licenses/gpl-3.0.html.

Vous pouvez notamment, aux conditions de la GPLv3 :
- utiliser l'Application pour tout usage, privé ou professionnel, sur autant de postes que vous voulez ;
- étudier et modifier son code source ;
- redistribuer des copies, modifiées ou non, gratuitement ou contre paiement, à condition de les distribuer sous la même licence avec leur code source.

Exécuter l'Application ne demande pas d'accepter la GPLv3 (article 9). La GPLv3 ne s'impose qu'à celui qui modifie ou redistribue l'Application.

## 3. Clé d'activation et versions officielles

Les installeurs officiels, les mises à jour et l'assistance peuvent être vendus. Une éventuelle clé d'activation ne limite aucun droit accordé par la GPLv3 : quiconque reçoit l'Application garde le droit d'en obtenir le code source, de le modifier et de le redistribuer.

## 4. Conditions additionnelles (GPLv3, article 7)

Conformément à l'article 7 de la GPLv3, s'ajoutent à la licence :
- **a) Mentions** : les mentions d'auteur et de licence affichées dans l'Application (fenêtres « À propos » et « Licences ») doivent être conservées dans toute version redistribuée.
- **b) Marque** : la GPLv3 ne concède aucun droit sur le nom « VERIFLOW » ni sur son logo. Une version modifiée doit porter un autre nom, ou indiquer clairement qu'elle est modifiée et qu'elle ne provient pas de l'éditeur.

## 5. Composants tiers

L'Application est livrée avec des composants d'autres auteurs, chacun sous sa propre licence, toutes compatibles avec la GPLv3 :
- **FFmpeg** (programmes `ffmpeg` et `ffprobe`, version 9.0.2), sous GNU GPL version 3, avec les bibliothèques compilées dedans. Ils sont exécutés comme des programmes séparés. Voir `FFMPEG.txt` et la section 6.
- **Bibliothèques Rust et JavaScript** (Tauri, Svelte, SQLite, Typst et d'autres), sous licences MIT, Apache 2.0, BSD, MPL 2.0, Unicode, Zlib, ISC et assimilées. La liste complète et les textes de licence sont dans `THIRD_PARTY.txt`.
- **Polices** Inter et JetBrains Mono, sous SIL Open Font License 1.1.
- **SDK ASIO de Steinberg** (version Windows), sous GNU GPL version 3.

La liste détaillée figure dans `COMPOSANTS_TIERS.md` et dans la fenêtre « Licences » de l'Application.

## 6. Code source

Le code source de l'Application est publié avec chaque version, au même endroit que ses installeurs.

Le code source correspondant de FFmpeg et de chaque bibliothèque compilée dedans est joint à chaque version publiée, au même endroit que les installeurs (article 6 d de la GPLv3). Pour la version 9.0.2, les fichiers sont :
- `ffmpeg-sources-9.0.2-windows-linux.tar` : sources des builds Windows et Linux ;
- `ffmpeg-sources-9.0.2-macos.tar` : sources du build macOS.

Ce code source reste disponible tant que la version correspondante est proposée au téléchargement.

## 7. Brevets

La GPLv3 porte sur le droit d'auteur. Certains formats que l'Application sait produire (H.264, HEVC, ProRes, AAC notamment) peuvent être couverts par des brevets dans certains pays. L'Application n'accorde aucune licence de brevet de tiers. Par défaut, elle utilise les encodeurs du système ou de la carte graphique quand ils existent. ProRes produit hors Mac est marqué « compatible, non certifié Apple ».

## 8. Données personnelles

L'Application fonctionne sur votre poste. Elle n'a pas de compte en ligne, ne fait aucune télémétrie et n'envoie aucune donnée à l'éditeur.

Une seule fonction utilise Internet, et seulement si vous la cochez : l'horodatage certifié RFC 3161 du rapport PDF d'OFFLOAD.
- Elle envoie l'empreinte SHA-256 du PDF, jamais le PDF lui-même.
- Le destinataire est l'autorité d'horodatage choisie (DigiCert par défaut), qui voit aussi votre adresse IP.
- Ses propres conditions s'appliquent.

## 9. Absence de garantie et responsabilité

L'Application est fournie « en l'état », sans garantie, dans les limites permises par la loi (articles 15 et 16 de la GPLv3).

L'Application aide à vérifier des copies et des fichiers, mais ne dispense pas de garder plusieurs copies des rushes ni de contrôler les rapports qu'elle produit. Dans la mesure permise par la loi, l'éditeur n'est pas responsable :
- des pertes de données ;
- des pertes d'exploitation ;
- des dommages indirects liés à son utilisation.

Cette clause ne limite pas les droits que la loi applicable reconnaît aux consommateurs. Elle n'exclut pas la responsabilité en cas de faute lourde ou intentionnelle.

## 10. Droit applicable

Ce contrat est soumis au droit français (à confirmer). En cas de litige, les parties cherchent d'abord une solution amiable.

## 11. Contact

Pour toute question sur les licences ou pour obtenir le code source : [adresse de contact à compléter].
