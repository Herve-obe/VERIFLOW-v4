# Tests en attente sur poste réel

Fonctions livrées et fusionnées, testées automatiquement et sous Linux, mais pas encore validées par Hervé sur ses postes. À traiter dès que possible.

| Phase | PR | Fusionnée le | Statut | À tester |
|---|---|---|---|---|
| 0 bis | [#2](https://github.com/Herve-obe/VERIFLOW-v4/pull/2) | 05/10/2026 | Partiellement testée (Windows, 06/10/2026) : lecture vidéo et audio dans le lecteur rapide de MEDIA, FFmpeg intégré | Onglet PLAYER lui-même (ProRes, XAVC, H.264, timecode, J/K/L), noms de pistes, SOLO/MUTE, LUFS, FFmpeg intégré sous macOS (blocage de sécurité possible) |
| 1 | [#3](https://github.com/Herve-obe/VERIFLOW-v4/pull/3) | 06/10/2026 | Partiellement testée (Windows, 06/10/2026) : offload d'une carte vers 1 disque et ouverture du rapport PDF, après correction de la relecture Windows dans [#4](https://github.com/Herve-obe/VERIFLOW-v4/pull/4) | Offload vers 2 disques, dossier `ascmhl/`, reprise d'une carte déjà copiée, éjection de la source, avertissement disque mécanique, coupure d'une destination pendant la copie, macOS |
| 3 | [#5](https://github.com/Herve-obe/VERIFLOW-v4/pull/5) | 09/10/2026 | Testée par Hervé (Windows et macOS Catalina) : PLAYER, son synchronisé, reprise OFFLOAD, fenêtres de confirmation | Points en stand-by ci-dessous |
| 4 | [#6](https://github.com/Herve-obe/VERIFLOW-v4/pull/6) | 09/10/2026 | Testée par Hervé sur Windows ; macOS non testé (Mac indisponible) | Sur macOS Catalina : onglet REPORT complet, en particulier le glisser-déposer de la fenêtre « Colonnes », le tri par colonne, les exports PDF, XLSX et EDL, la touche R du PLAYER |
| 5 | [#8](https://github.com/Herve-obe/VERIFLOW-v4/pull/8) | 11/10/2026 | Non testée sur poste réel (essais automatiques et sous Linux seulement) | Liste « À tester sur poste réel » de `docs/08_PHASE5_SYNC.md` : rushes et WAV d'enregistreur réels, les 3 méthodes (dont « Forme d'onde seule » sur un timecode faux), LTC Tentacle, 23,976 et 29,97 DF, re-wrap MOV (polyphonique et pistes mono) et MXF importés dans Resolve, Premiere et Avid, FCPXML et OTIO, alerte de dérive sur un plan long |
| 6 | [#7](https://github.com/Herve-obe/VERIFLOW-v4/pull/7) | 11/10/2026 | Non testée sur poste réel (essais automatiques et sous Linux seulement) | Liste « À tester sur poste réel » de `docs/07_PHASE6_TRANSCODE.md` : encodeurs matériels Windows et Mac, ProRes sur Mac, rushes réels vers ProRes, DNxHR, XDCAM importés dans Resolve, Premiere et Avid, préréglages web, incrustations et LUT, son et métadonnées, analyses, préréglages personnels |
| FFmpeg | [#9](https://github.com/Herve-obe/VERIFLOW-v4/pull/9) | 15/10/2026 | Non testée sur poste réel (builds des 3 installeurs verts) | Sur un poste sans FFmpeg installé (Windows, macOS Catalina, Linux) : installer VERIFLOW, ouvrir un clip dans PLAYER et MEDIA (FFmpeg intégré trouvé), lancer une conversion TRANSCODE ; sous Linux, nouveau FFmpeg BtbN (encodeurs H.264, ProRes, DNxHR, AAC) |
| SYNC | [#10](https://github.com/Herve-obe/VERIFLOW-v4/pull/10) | 15/10/2026 | Non testée sur poste réel (test automatique : 600 ppm sur 90 s, reste moins de 2 ms) | Correction de la dérive au re-wrap : plan long (plus de 10 min) avec dérive signalée, re-wrap avec « Corriger la dérive mesurée », calage vérifié au début et à la fin dans le logiciel de montage |

## En stand-by (demandé par Hervé le 08/10/2026)

Points de la PR [#5](https://github.com/Herve-obe/VERIFLOW-v4/pull/5) (phase 3, PLAYER, fusionnée le 09/10/2026) reportés, à tester sur PC Windows :

- Ligne « Délais » du lecteur vidéo (MEDIA et PLAYER) : relever les 4 valeurs (ouverture, première image, son prêt, départ) sur un rush caméra typique, avec son format (caméra, codec, résolution, nombre de pistes son), pour décider quelle étape optimiser.
- Alerte LTC avec un rush Tentacle Sync E (LTC sur la piste droite) : la piste doit être coupée et les trois choix proposés (laisser coupée, baisser de 30 dB, réactiver).

Signaler tout problème en indiquant : système (Windows / macOS / Linux), support (carte, disque), et ce qui s'est passé.
