# FFmpeg intégré à VERIFLOW

VERIFLOW livre `ffmpeg` et `ffprobe` dans son installeur (charte §5.2) : il fonctionne sans Internet ni installation annexe.

Organisation décidée par Hervé le 20/10/2026, pour être au plus près de la GPLv3 :
1. **Windows et Linux** : builds BtbN, dont le contenu est connu exactement.
2. **macOS** : FFmpeg compilé par VERIFLOW.
3. **Code source complet** joint à chaque Release.

## Builds utilisés (version 9.0.2)

| Système | Origine | Licence |
|---|---|---|
| Windows x64 | BtbN/FFmpeg-Builds, variante `win64-gpl`, branche 9.0, autobuild `autobuild-2026-09-30-13-08`, `ffmpeg-n9.0.2-17-g2a571b6068-win64-gpl-9.0.zip` | GPLv3, statique |
| Linux x64 | même autobuild, variante `linux64-gpl`, `ffmpeg-n9.0.2-17-g2a571b6068-linux64-gpl-9.0.tar.xz` | GPLv3, statique |
| macOS Apple Silicon et Intel | compilé par `scripts/build-ffmpeg-macos.sh` (job `ffmpeg-macos` du build), assemblé en binaire universel | GPLv3, statique |

**Windows et Linux** :
- `scripts/fetch-ffmpeg.sh` télécharge chaque build BtbN et vérifie son empreinte SHA-256.
- BtbN conserve durablement ses autobuilds de fin de mois : c'en est un qui est épinglé.
- Ses scripts (`scripts.d`) fixent le commit de chaque bibliothèque.

**macOS** :
- FFmpeg 9.0.2 (archive officielle, empreinte vérifiée) et chacune de ses bibliothèques, au même commit que dans les builds BtbN.
- Bibliothèques : x264, x265 (8, 10 et 12 bits), libvpx, SVT-AV1, Opus, LAME, libogg, libvorbis, FreeType, FriBidi, HarfBuzz, libass, libvmaf, snappy (HAP), libwebp.
- Fonctions du système : VideoToolbox, AudioToolbox, CoreText, zlib, bzip2, iconv. La détection automatique de FFmpeg est coupée (`--disable-autodetect`) : sinon, il lierait des bibliothèques de Homebrew présentes sur la machine de compilation (liblzma, X11).
- Le job `ffmpeg-macos` tourne sur `macos-15` (arm64, macOS 11 et plus) et `macos-15-intel` (x86_64, macOS 10.15 et plus, pour Catalina).
- Le résultat est gardé en cache tant que le script ne change pas.
- Le script refuse un binaire qui dépend d'une bibliothèque hors du système (Homebrew, dossier de compilation).
- Absents sur Mac, donc préréglages grisés : Xvid, Theora, H.266/VVC, JPEG XL.

Les builds de gyan.dev (Windows), martin-riedl.de et evermeet.cx (macOS), utilisés jusqu'au 20/10/2026, sont abandonnés : il n'était pas certain qu'ils publient le code source complet de toutes leurs bibliothèques. La branche de stockage `vendor/ffmpeg-evermeet` n'est plus utilisée.

## Contrôle de licence

- **Build**, étape « Contrôler FFmpeg » : le FFmpeg de chaque système est lancé avec `-buildconf`, et le build **échoue s'il contient `--enable-nonfree`**.
- **Script macOS** : il le vérifie aussi.
- **Composants tiers** : l'étape « Avis de licences des composants tiers » (`scripts/third-party.mjs`) échoue si une crate Rust ou un paquet JavaScript a une licence incompatible avec la GPLv3.

## Code source correspondant (GPLv3 §6 d)

Le job `sources` du build (`scripts/ffmpeg-sources.sh`) joint deux archives à chaque Release, au même endroit que les installeurs, avec leurs empreintes (`ffmpeg-sources-9.0.2.sha256`).

**`ffmpeg-sources-9.0.2-windows-linux.tar`** :
- FFmpeg au commit des builds BtbN (`release/9.0`, `2a571b6068`) ;
- les scripts BtbN de l'autobuild épinglé ;
- la source de chaque bibliothèque, téléchargée par leur propre `download.sh` ;
- un fichier LISEZMOI qui explique comment recompiler.

**`ffmpeg-sources-9.0.2-macos.tar`** :
- FFmpeg 9.0.2 et chaque bibliothèque au commit épinglé ;
- le script de compilation.

Lancement manuel du build avec l'option « sources » : les archives sont produites en artefact, pour vérification.

## Textes juridiques

- `legal/CLUF.md` : contrat de licence utilisateur final, fondé sur la GPLv3.
- `legal/CGU.md` : conditions générales d'utilisation.
- `legal/COMPOSANTS_TIERS.md` : FFmpeg et ses bibliothèques, polices, ASIO.
- `src-tauri/licenses/FFMPEG.txt` : avis FFmpeg livré avec l'application.
- `src-tauri/licenses/THIRD_PARTY.txt` : généré au build ; textes de licence des crates Rust et paquets JavaScript.

Ils sont copiés dans l'installeur (`licenses/`), avec le texte de la GPLv3. On les lit dans VERIFLOW : Aide > Licences, ou bouton « Licences, CLUF et CGU » d'À propos. La rubrique FFmpeg affiche la version et la configuration du FFmpeg réellement installé. L'installeur affiche le CLUF.

CLUF et CGU sont des **projets à faire relire par un juriste** avant toute diffusion commerciale. Points à compléter : adresse de contact, droit applicable.

## Mettre à jour FFmpeg

1. **Windows et Linux** : choisir un nouvel autobuild BtbN de fin de mois. Changer `BTBN_TAG`, les noms de fichiers et les empreintes dans `scripts/fetch-ffmpeg.sh`, et les mêmes valeurs dans `scripts/ffmpeg-sources.sh`.
2. **macOS** : reprendre les commits de `scripts.d` de ce même autobuild dans `scripts/build-ffmpeg-macos.sh`, ainsi que la version et l'empreinte de l'archive FFmpeg.
3. Mettre à jour `legal/COMPOSANTS_TIERS.md` et `src-tauri/licenses/FFMPEG.txt`.
4. Lancer le workflow « Build » sur la branche, avec l'option « sources ». Vérifier les trois installeurs et les archives de sources.
5. Relancer les tests du cœur avec le nouveau FFmpeg : `VERIFLOW_FFMPEG_DIR=... cargo test -p veriflow-core`.
