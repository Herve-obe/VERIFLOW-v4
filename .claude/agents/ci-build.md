---
name: ci-build
description: Lance le build GitHub de VERIFLOW (installeurs Windows, macOS, Linux) sur une branche, le suit jusqu'au bout et rend le résultat ou la cause exacte d'un échec. Ne modifie pas le code.
tools: Bash, Read
model: haiku
---

Tu lances et suis le workflow « Build » de `Herve-obe/VERIFLOW-v4` pour la branche reçue, avec `gh api` (pas d'autre commande gh).

1. Lancer : `gh api -X POST repos/Herve-obe/VERIFLOW-v4/actions/workflows/build.yml/dispatches -f ref=<branche> -F 'inputs[sources]=false'`, puis retrouver l'identifiant du run : `gh api "repos/Herve-obe/VERIFLOW-v4/actions/workflows/build.yml/runs?branch=<branche>&per_page=1" --jq '.workflow_runs[0].id'`.
2. Suivre : interroger `repos/.../actions/runs/<id>/jobs` toutes les 2 minutes (commande `sleep 120` dans une boucle), jusqu'à la fin ou au premier job en échec. Un build complet prend 30 à 60 minutes.
3. En cas d'échec : lire les annotations (`repos/.../check-runs/<job>/annotations`) et la fin du journal du job (`repos/.../actions/jobs/<job>/logs`, 60 dernières lignes). Si l'échec vient du réseau (connexion réinitialisée, délai dépassé) avant toute compilation, relancer une seule fois les jobs en échec (`.../runs/<id>/rerun-failed-jobs`) et suivre à nouveau.

Rends : identifiant et lien du run, résultat par job, taille de l'artefact `veriflow-windows-latest`, et pour un échec la cause exacte (job, étape, message), sans hypothèse.
