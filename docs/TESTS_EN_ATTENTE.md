# Tests en attente sur poste réel

Fonctions livrées et fusionnées, testées automatiquement et sous Linux, mais pas encore validées par Hervé sur ses postes. À traiter dès que possible.

| Phase | PR | Fusionnée le | Statut | À tester |
|---|---|---|---|---|
| 0 bis | [#2](https://github.com/Herve-obe/VERIFLOW-v4/pull/2) | 05/10/2026 | Partiellement testée (Windows, 06/10/2026) : lecture vidéo et audio dans le lecteur rapide de MEDIA, FFmpeg intégré | Onglet PLAYER lui-même (ProRes, XAVC, H.264, timecode, J/K/L), noms de pistes, SOLO/MUTE, LUFS, FFmpeg intégré sous macOS (blocage de sécurité possible) |
| 1 | [#3](https://github.com/Herve-obe/VERIFLOW-v4/pull/3) | 06/10/2026 | Partiellement testée (Windows, 06/10/2026) : offload d'une carte vers 1 disque et ouverture du rapport PDF, après correction de la relecture Windows dans [#4](https://github.com/Herve-obe/VERIFLOW-v4/pull/4) | Offload vers 2 disques, dossier `ascmhl/`, reprise d'une carte déjà copiée, éjection de la source, avertissement disque mécanique, coupure d'une destination pendant la copie, macOS |

Signaler tout problème en indiquant : système (Windows / macOS / Linux), support (carte, disque), et ce qui s'est passé.
