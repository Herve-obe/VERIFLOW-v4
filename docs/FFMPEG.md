# FFmpeg intégré à VERIFLOW

VERIFLOW livre `ffmpeg` et `ffprobe` dans son installeur (charte §5.2) : il fonctionne sans Internet ni installation annexe. Ils sont téléchargés au moment du build par `scripts/fetch-ffmpeg.sh`, chacun épinglé par son empreinte SHA-256 : un build donne toujours le même FFmpeg.

## Builds utilisés (version 9.0.2)

| Système | Source | Licence | Remarque |
|---|---|---|---|
| Windows x64 | gyan.dev, « essentials » | GPLv3 | Statique |
| macOS Apple Silicon | martin-riedl.de | GPLv3 | Statique, sans `--enable-nonfree` (vérifié le 11/10/2026) |
| macOS Intel | evermeet.cx | GPLv3 | Statique, macOS 10.13 et plus (Catalina) |
| Linux x64 | BtbN/FFmpeg-Builds, variante `linux64-gpl`, branche 9.0, autobuild `autobuild-2026-09-30-13-08`, fichier `ffmpeg-n9.0.2-17-g2a571b6068-linux64-gpl-9.0.tar.xz` | GPLv3 | Statique. Remplace le 11/10/2026 le build de martin-riedl.de, compilé avec DeckLink (`--enable-nonfree`), donc non redistribuable |

**Copie de secours macOS Intel** : le miroir d'evermeet.cx est parfois injoignable. Les deux fichiers (`ffmpeg-9.0.2.zip`, `ffprobe-9.0.2.zip`) sont aussi disponibles tels quels dans la Release `ffmpeg-mirror` du dépôt (si elle est créée) et dans le dossier Google Drive VERIFLOW d'Hervé (partagé par lien) ; le script s'en sert si evermeet ne répond pas, avec la même empreinte SHA-256 exigée.

BtbN conserve durablement les autobuilds de fin de mois : c'en est un qui est épinglé.

## Contrôle de licence

Le build (`.github/workflows/build.yml`, étape « Contrôler FFmpeg ») lance le FFmpeg téléchargé avec `-buildconf` et **échoue s'il contient `--enable-nonfree`**. Un FFmpeg nonfree (fdk-aac, DeckLink, NDI, CUDA NPP...) ne peut pas être distribué avec un programme GPL.

## Code source correspondant (GPL)

La GPL impose de pouvoir fournir le code source des binaires distribués :
- FFmpeg : dépôt officiel https://git.ffmpeg.org/ffmpeg.git (miroir https://github.com/FFmpeg/FFmpeg). Version Linux : commit `2a571b6068` de la branche `release/9.0` (n9.0.2 + 17 commits). Autres systèmes : version 9.0.2.
- Scripts et bibliothèques du build Linux : https://github.com/BtbN/FFmpeg-Builds (au même autobuild).
- Builds Windows et macOS : configuration lisible avec `ffmpeg -buildconf` ; la mise à disposition des sources par gyan.dev, evermeet.cx et martin-riedl.de reste à vérifier.

**Décision à prendre avant toute diffusion (Hervé)** : la manière de fournir ces sources (archive jointe aux Releases, ou offre écrite), conformément à la GPLv3 §6.

## Mettre à jour FFmpeg

1. Changer la version, les URL et les empreintes dans `scripts/fetch-ffmpeg.sh`. Pour Linux, une empreinte ou un nom de fichier laissé vide est affiché par le build (annotation « à épingler »), à recopier ensuite.
2. Lancer le workflow « Build » sur la branche, vérifier les trois installeurs.
3. Relancer les tests du cœur avec le nouveau FFmpeg (`VERIFLOW_FFMPEG_DIR=... cargo test -p veriflow-core`).
