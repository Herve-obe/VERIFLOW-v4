# VERIFLOW v4 : consignes pour Claude

## Règles de travail (Hervé)
- Langue : français, tutoiement professionnel. Pas d'émojis, pas de tirets cadratins ni de double-tirets dans les réponses.
- Rien n'est sauvegardé sur les postes : tout passe par ce repo GitHub. Pousser souvent (petits commits).
- **Si Hervé écrit "je dois partir" (ou équivalent) : committer et pousser immédiatement tout le travail en cours**, même inachevé (commit préfixé `wip:`), puis résumer en 3 lignes où on en est et quoi faire ensuite.
- Une branche par fonctionnalité, une PR validée par Hervé avant fusion.
- Ne jamais s'inspirer du code ou du visuel de la v3 (github.com/Herve-obe/Veriflow_v3.0).
- Tenir à jour `docs/TESTS_EN_ATTENTE.md` : toute PR fusionnée sans test d'Hervé sur poste réel y est inscrite, avec la liste des points à tester.

## Références
- Charte (source de vérité) : `docs/01_CHARTE.md`
- Réponses au questionnaire : `docs/00_REPONSES_QUESTIONNAIRE.md`
- Modèles de rapports école : Google Drive, dossier VERIFLOW/V4

## Architecture (voir charte §5)
- `ui/` : toute l'interface (Svelte 5 + TypeScript). Visuel centralisé dans `ui/src/theme/tokens.css`, aucune couleur en dur ailleurs.
- `core/` : cœur métier Rust, sans dépendance à l'interface.
- `src-tauri/` : pont Tauri 2 entre `ui/` et `core/`.
- Licence : GPL-3.0-or-later. Toute dépendance doit être compatible GPLv3.

## Commandes
- Installer : `npm ci`
- Lancer en dev : `npm run tauri dev`
- Tests cœur : `cargo test -p veriflow-core`
- Vérification UI : `npm run check`
- Build : `npm run tauri build`
