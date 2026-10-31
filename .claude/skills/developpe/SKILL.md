---
name: developpe
description: Développement automatique d'une demande d'Hervé pour VERIFLOW par l'équipe d'agents (architecte, dev-coeur, dev-ui, testeur, relecteur, doc-i18n, ci-build), de la demande à la PR et à l'installeur de test, sans intervention d'Hervé avant la fin. À utiliser quand Hervé écrit /developpe suivi de sa demande, ou demande un développement automatique.
---

# Développement automatique de VERIFLOW

Tu es l'agent principal. Tu ne codes pas toi-même : tu répartis, vérifies, relies et testes le travail des agents de `.claude/agents/`, et tu relances ce qui échoue. Hervé n'intervient qu'à la fin : ne lui pose aucune question en cours de route, sauf blocage décrit à l'étape 7.

Respecte `CLAUDE.md` en entier : une branche par fonction, PR validée par Hervé avant fusion (tu ne fusionnes jamais dans `main`), messages de commit en français avec les lignes d'attribution, aucun identifiant de modèle dans le dépôt.

## Choix des modèles (coût)
- `architecte`, `relecteur` : opus. Peu d'appels, mais c'est là que se jouent la justesse et les pièges.
- `dev-coeur`, `dev-ui` : sonnet. L'essentiel du code.
- `testeur`, `doc-i18n`, `ci-build` : haiku. Travail mécanique ou d'attente.
- Ne relance jamais un agent plus coûteux pour une tâche qu'un agent moins cher sait faire. Donne à chaque agent un message complet et autonome (il ne voit pas cette conversation) : objectif, fichiers, branche, critère de fin.

## Étapes

1. **Plan** : appelle `architecte` avec la demande d'Hervé, telle quelle. Garde son plan comme référence.
2. **Branche** : crée `claude/<sujet>` depuis `origin/main` (ou depuis la branche d'une PR dont elle dépend, comme l'indique le plan).
3. **Lots** : pour chaque lot du plan, appelle `dev-coeur` ou `dev-ui`.
   - Lots indépendants (fichiers différents) : en parallèle, chacun avec `isolation: "worktree"`, puis récupère leurs commits sur la branche (`git cherry-pick` ou fusion) et résous les conflits toi-même.
   - Lots dépendants : l'un après l'autre.
4. **Contrôle** : après chaque lot, appelle `testeur`. Un échec repart à l'agent qui a fait le lot, avec le compte rendu exact du testeur. Au plus 3 tentatives par lot.
5. **Relecture** : quand tous les lots passent, appelle `relecteur`. Chaque point BLOQUANT ou À CORRIGER repart à l'agent concerné, puis `testeur` à nouveau. Au plus 2 tours de relecture. Les points MINEURS : corrige-les s'ils sont sûrs et rapides, sinon note-les pour Hervé.
6. **Documentation** : appelle `doc-i18n` si le comportement visible change.
7. **Blocage** : si un lot échoue 3 fois, ou si la relecture bloque encore après 2 tours, arrête ce lot, garde le reste s'il tient debout seul, et mets le blocage dans le compte rendu final (cause exacte, ce qui a été essayé). Ne contourne jamais un test (pas de test supprimé, désactivé ou affaibli).
8. **Publication** :
   - pousse la branche (`git push -u origin <branche>`, nouvelles tentatives en cas d'erreur réseau) ;
   - ouvre la PR vers `main` avec le gabarit habituel du dépôt : Résumé, Vérifications, À tester par Hervé ;
   - abonne-toi à la PR (`subscribe_pr_activity`) et corrige toi-même un échec de CI de la même façon (agents, testeur, relecteur).
9. **Installeur de test** : fusionne la branche dans `claude/recette`, en résolvant les conflits avec le même soin. Pour les fichiers JSON de traduction, fusionne les clés par script. Puis appelle `ci-build` sur `claude/recette`. Un échec de build repart en étape 4.
10. **Compte rendu à Hervé**, seul message de fin :
    - ce qui est livré, en quelques lignes ;
    - lien de la PR et lien du build (artefact `veriflow-windows-latest`) ;
    - ce qu'il doit tester sur poste réel, en liste numérotée, dans l'ordre ;
    - blocages éventuels et points mineurs laissés de côté ;
    - mets aussi à jour le classeur `docs/recette/RECETTE_PR_*.xlsx` (nouvelles lignes de test, saisies d'Hervé conservées) et envoie-le.

## Ce que l'équipe ne peut pas faire
- Tester sur un vrai Windows, un vrai Mac, une carte son, un second écran ou une carte caméra : seul Hervé le peut. Les agents le disent dans « À tester par Hervé », sans le cacher.
- Fusionner dans `main` : c'est Hervé qui valide.
