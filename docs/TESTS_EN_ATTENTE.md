# Tests en attente sur poste réel

Fonctions livrées et fusionnées, testées automatiquement et sous Linux, mais pas encore validées par Hervé sur ses postes. À traiter dès que possible.

| Phase | PR | Fusionnée le | Statut | À tester |
|---|---|---|---|---|
| 0 bis | [#2](https://github.com/Herve-obe/VERIFLOW-v4/pull/2) | 05/10/2026 | Non testée sur poste réel | PLAYER VIDEO (rushes ProRes, XAVC, H.264, timecode), PLAYER AUDIO (WAV Sound Devices, noms de pistes, SOLO/MUTE, LUFS), FFmpeg intégré (Windows et surtout macOS : blocage de sécurité possible) |
| 1 | [#3](https://github.com/Herve-obe/VERIFLOW-v4/pull/3) | 06/10/2026 | **Fusionnée sans test** (poste occupé par une restauration vidéo) | Offload d'une vraie carte vers 2 disques, rapport PDF, dossier `ascmhl/`, reprise d'une carte déjà copiée, éjection de la source, avertissement disque mécanique, coupure d'une destination pendant la copie |

Signaler tout problème en indiquant : système (Windows / macOS / Linux), support (carte, disque), et ce qui s'est passé.
