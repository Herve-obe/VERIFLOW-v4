# Barre de menus et Préférences

Demande d'Hervé du 19/10/2026 : une barre de menus classique pour ranger les préférences, les réglages, la création et l'ouverture des projets. Décisions validées : menu natif sous macOS, barre dessinée sous Windows et Linux ; boutons de projet retirés du bandeau (le nom du projet ouvert y reste) ; Préférences dans une fenêtre aux couleurs de VERIFLOW, à sections.

## Ce qui est livré

| Menu | Entrées |
|---|---|
| Fichier | Nouveau projet (Ctrl+N), Ouvrir un projet (Ctrl+O), Projets récents (10 derniers, « Effacer la liste »), Enregistrer le projet (Ctrl+S), Enregistrer le projet sous (Ctrl+Maj+S), Fermer le projet (Ctrl+W), Quitter (Ctrl+Q) |
| Édition | Annuler (Ctrl+Z), Rétablir (Ctrl+Y ; ⇧⌘Z sur macOS), Couper, Copier, Coller, Tout sélectionner, Préférences (Ctrl+,) |
| Affichage | OFFLOAD à REPORT (Alt+1 à 6), Basculer VIDEO / AUDIO (Tab, Ctrl+Tab partout), Afficher/masquer l'explorateur (Ctrl+B), Plein écran (F11) |
| Lecture | Lecture/pause (Espace), Stop (Entrée), J/K/L, image précédente/suivante (flèches), entrée (I), sortie (O), marqueur (M), saisie du rapport (R) |
| Aide | Raccourcis clavier (F1), Licences, À propos de VERIFLOW |

- **macOS** : menu natif en haut de l'écran, avec le menu VERIFLOW (À propos, Préférences ⌘,, Masquer, Quitter ⌘Q) et le menu Fenêtre (réduire, plein écran). Couper, copier, coller sont les entrées du système, indispensables pour les champs de texte. Les raccourcis d'une seule touche (Espace, J, K, L, Tab...) sont affichés dans la barre dessinée mais pas donnés au menu natif, qui les prendrait aux champs de texte.
- **En-tête en deux lignes** (option B de la maquette, onglets centrés, choix d'Hervé du 19/10/2026) :
  1. barre de titre : logo, menus, projet ouvert (nom · dossier) au centre de l'espace libre. Sous Windows, elle remplace la barre du système : déplacement de la fenêtre en la tirant, double-clic pour agrandir, boutons réduire, agrandir/restaurer et fermer redessinés (fermer en rouge au survol). Sous Linux, la barre du système reste au-dessus (pas de boutons dessinés). Sous macOS, cette ligne n'existe pas : les menus sont dans la barre du système ;
  2. onglets centrés, interrupteur VIDEO / AUDIO à droite (sous macOS, logo à gauche et projet à côté de l'interrupteur).
- **Windows et Linux** : barre de menus dessinée dans la barre de titre. Elle ne prend pas le focus : couper, copier, coller agissent sur le champ en cours (presse-papiers par le module officiel de Tauri, le moteur web refusant « coller » depuis un menu).
- **Enregistrer** : chaque modification du projet est déjà écrite dans le fichier `.veriflow` au moment où elle est faite (SQLite) ; Ctrl+S le confirme dans la barre d'état et le note dans le journal du projet.
- **Enregistrer sous** : copie complète et compacte du projet (`VACUUM INTO`) sous un autre nom ; la copie devient le projet ouvert, son nom prend celui du fichier, l'original n'est pas modifié. Boîte proposée dans le dossier du projet, nom « … copie ».
- **Préférences** (fenêtre à sections, complétée le 20/10/2026) :
  - **Général** : langue, ouverture (reprendre ou repartir à zéro), données de l'application ;
  - **Raccourcis** : toutes les actions des menus Fichier, Édition (Préférences), Affichage, Lecture et Aide, plus la bascule VIDEO / AUDIO forcée (Ctrl+Tab). « Modifier » puis la nouvelle combinaison (Échap annule) ; un raccourci déjà pris est retiré de l'autre action, avec un message ; « × » retire un raccourci ; « Par défaut » et « Tout rétablir ». Couper, copier, coller, annuler et tout sélectionner restent ceux du système. Les raccourcis modifiés valent partout : clavier, menus affichés, menu natif de macOS, fenêtre Aide > Raccourcis clavier (charte §4 : « tous les raccourcis sont reconfigurables ») ;
  - **OFFLOAD** : valeurs de départ (arborescence, opérateur, empreintes, éjection), utilisées à la première ouverture, après une remise à zéro ou en mode « Repartir à zéro » ;
  - **PLAYER** : TC incrusté et scopes au premier lancement d'un lecteur, couleur des nouveaux marqueurs, ouverture de la saisie du rapport à chaque nouvelle prise ;
  - **TRANSCODE** : destination proposée à chaque ouverture (à côté de chaque source, ou un dossier).
  - Enregistrées sous `veriflow.prefs` (gardées par la remise à zéro), code `ui/src/stores/prefs.svelte.ts`, `ui/src/menu/ShortcutsEditor.svelte`.
- **Données de l'application** (demande d'Hervé du 20/10/2026, Préférences > Général) :
  - « À l'ouverture de VERIFLOW » : *Reprendre où j'en étais* (par défaut) ou *Repartir à zéro*. À zéro, l'état de travail est effacé à chaque ouverture : destinations et options d'OFFLOAD, réglages de SYNC, dossiers dépliés de l'explorateur, affichages du PLAYER, sections ouvertes de TRANSCODE. Restent : langue, projets récents, favoris, préréglages TRANSCODE créés, sortie audio ;
  - « Remettre à zéro maintenant » (même effacement) et « Effacer toutes les données » (tout, réglages compris), avec confirmation, puis rechargement ;
  - les projets `.veriflow`, rapports et copies ne sont jamais touchés ;
  - désinstallation sous Windows : seul l'installeur NSIS (`setup.exe`) est publié, en français et en anglais ; son désinstalleur propose « Supprimer les données de l'application » (dossiers `%APPDATA%` et `%LOCALAPPDATA%\com.veriflow.app`, où le moteur web garde les réglages). Le MSI, qui ne le propose pas, n'est plus publié. macOS et Linux n'ont pas de désinstalleur : « Effacer toutes les données » fait le même travail.
- Code : `ui/src/lib/appdata.ts` (clés `veriflow.*` gardées ou effacées), appliqué dans `ui/src/main.ts` avant le chargement des vues.
- **Raccourcis clavier** (Aide) : liste tirée des menus, toujours à jour.
- En-tête : tient dans la largeur minimale de la fenêtre (1024 px).
- Limite sous Windows 11 : la barre étant redessinée, le menu « Snap Layouts » au survol du bouton agrandir n'apparaît pas ; le Snap par glisser vers les bords et Win+flèches fonctionnent.

## Choix techniques

- `ui/src/menu/commands.svelte.ts` : liste unique des commandes (libellé, raccourci, condition, action) et des menus. Sert à la barre dessinée, au menu natif et à la fenêtre des raccourcis.
- `ui/src/menu/MenuBar.svelte` (barre dessinée), `MenuDialogs.svelte` (Préférences, Raccourcis, Licences, À propos).
- `ui/src/layout/TopBar.svelte` (en-tête en deux lignes), `WindowControls.svelte` (boutons de la fenêtre) ; barre du système retirée sous Windows dans `src-tauri/src/lib.rs` (`set_decorations(false)`), zones de déplacement `data-tauri-drag-region`.
- `src-tauri/src/menu.rs` : construit le menu natif de macOS à partir de la description envoyée par l'interface, relaie les clics (événement `menu`). Reconstruit à chaque changement de langue, de projet ou de récents.
- Menu Lecture : événement `veriflow-player-action`, reçu par le lecteur actif (PLAYER ou lecteur rapide), qui exécute la même action que la touche.
- Cœur : `Project::save` et `Project::save_as` (`core/src/project.rs`), test `save_as_copies_everything_and_switches_to_the_copy`.
- Nouvelle dépendance : `tauri-plugin-clipboard-manager` 2 (MIT ou Apache 2.0, compatible GPLv3).

## Vérifications réalisées (Linux, Xvfb)

- Barre et menus déroulants affichés à 1280 et 1024 px de large, sans débordement.
- Ctrl+N : projet créé ; Ctrl+S : « Projet enregistré » ; Ctrl+Maj+S : copie `menutest copie.veriflow` créée et ouverte, les deux fichiers dans les projets récents.
- Ctrl+, : Préférences ; F1 : raccourcis ; Ctrl+B : explorateur masqué puis réaffiché.
- Édition : tout sélectionner, copier dans un champ, coller dans un autre, depuis le menu.
- Lecture > Lecture/pause : le clip est lu.
- `cargo test -p veriflow-core` (projet : 11 tests), clippy, `npm run check` propres.
- Menu natif de macOS : compilé (même API de Tauri sous Linux), pas affiché ici (pas de Mac).

## À tester sur poste réel

- Raccourcis : changer « Nouveau projet » en Ctrl+Maj+N, vérifier le menu et le clavier ; donner à une action un raccourci déjà pris (message, l'autre action perd le sien) ; « Tout rétablir ». Sur macOS, le menu natif doit afficher le nouveau raccourci.
- OFFLOAD, PLAYER, TRANSCODE : changer les valeurs, « Remettre à zéro maintenant » (Général) : l'onglet reprend les valeurs des Préférences.

- Données : désinstaller avec la case « Supprimer les données de l'application » cochée, réinstaller : plus aucune destination d'OFFLOAD. « Repartir à zéro » : fermer, rouvrir, l'état de travail est vide, la langue et les favoris restent.

0. Windows : barre de titre dessinée (déplacer la fenêtre, double-clic pour agrandir, réduire, agrandir/restaurer, fermer ; redimensionner par les bords ; Snap vers les bords, Win+flèches ; deux écrans).
1. Windows : barre de menus, survol d'un menu à l'autre, raccourcis Ctrl+N, O, S, Maj+S, W, Q, F11, F1.
2. Couper, copier, coller depuis le menu Édition dans un champ (OFFLOAD, REPORT).
3. Enregistrer sous : copie ouverte, original intact, projets récents.
4. macOS : menu natif (menu VERIFLOW, Préférences ⌘,, Quitter ⌘Q), copier/coller dans les champs, menus traduits en anglais après changement de langue.
5. Menu Lecture depuis le PLAYER et le lecteur rapide de MEDIA.
