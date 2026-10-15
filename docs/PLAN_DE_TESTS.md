# Plan de tests sur poste réel (V1)

Ordre conseillé : chaque étape s'appuie sur la précédente (installer, copier les rushes, les regarder, les synchroniser, les convertir). Cocher `[x]` au fur et à mesure. Source : `docs/TESTS_EN_ATTENTE.md` et les listes « À tester sur poste réel » de chaque phase.

**Postes** : PC Windows (W) et Mac Catalina (M). Un test marqué W+M est à faire sur les deux.

**Matériel à préparer**
- Une carte caméra (ou sa copie) avec des rushes réels : ProRes, XAVC ou H.264, avec timecode ; si possible un plan de plus de 10 min.
- Les WAV de l'enregistreur de la même journée (Sound Devices, Zoom ou Zaxcom), avec timecode.
- Un rush d'une caméra sans timecode (reflex, smartphone) avec son témoin, et son WAV.
- Si possible : un rush avec LTC sur une piste (Tentacle Sync E), un rush en 23,976 et un en 29,97 DF.
- Deux disques de destination, dont un mécanique (HDD) si possible.
- Une LUT .cube de la caméra, un logo PNG, un SRT reçu d'un client.
- Un second écran (HDMI ou DisplayPort).
- DaVinci Resolve, Premiere, Avid Media Composer, Final Cut Pro (Mac) : ceux disponibles.

**Signaler un problème** : système (W ou M), support (carte, disque), étape (numéro ci-dessous), ce qui s'est passé, et une capture si possible.

---

## 1. Installation et FFmpeg intégré ([#9](https://github.com/Herve-obe/VERIFLOW-v4/pull/9))

- [ ] **1.1 (W+M)** Désinstaller les anciennes versions de VERIFLOW. Vérifier que FFmpeg n'est pas installé sur le poste : dans un terminal, `ffmpeg -version` doit répondre « commande introuvable ».
- [ ] **1.2 (W)** Installer VERIFLOW avec l'installeur Windows du build. Si SmartScreen bloque : « Informations complémentaires », puis « Exécuter quand même ».
- [ ] **1.3 (M)** Installer VERIFLOW avec le .dmg. Au premier lancement, si macOS bloque : clic droit sur VERIFLOW dans Applications, « Ouvrir », puis « Ouvrir ».
- [ ] **1.4 (W+M)** Ouvrir un clip dans PLAYER. Attendu : l'image s'affiche, sans message « outil introuvable ». FFmpeg vient donc de l'installeur.

## 2. OFFLOAD ([#3](https://github.com/Herve-obe/VERIFLOW-v4/pull/3))

- [ ] **2.1 (W)** Copier la carte vers 2 disques en même temps. Attendu : copie et vérification réussies sur les deux, dossier `ascmhl/` présent dans chaque destination.
- [ ] **2.2 (W)** Choisir un disque mécanique comme destination. Attendu : avertissement « disque mécanique ».
- [ ] **2.3 (W)** Relancer l'offload de la même carte. Attendu : VERIFLOW signale la carte déjà copiée et propose la reprise.
- [ ] **2.4 (W)** Pendant une copie, débrancher une destination. Attendu : erreur claire sur cette destination, l'autre continue.
- [ ] **2.5 (W)** Fin de copie : éjecter la source depuis VERIFLOW. Attendu : carte éjectée proprement.
- [ ] **2.6 (M)** Refaire 2.1 et 2.5 sur Mac.

## 3. MEDIA et PLAYER ([#2](https://github.com/Herve-obe/VERIFLOW-v4/pull/2), [#5](https://github.com/Herve-obe/VERIFLOW-v4/pull/5))

- [ ] **3.1 (W+M)** Dans PLAYER, lire un rush ProRes, un XAVC et un H.264. Attendu : timecode juste, J/K/L (arrière, pause, avant) et flèches image par image fonctionnent.
- [ ] **3.2 (W+M)** Pistes son : noms des pistes affichés, SOLO et MUTE, mesure LUFS.
- [ ] **3.3 (W)** Ligne « Délais » (MEDIA et PLAYER), sur un rush caméra typique : noter les 4 valeurs (ouverture, première image, son prêt, départ) et le format du rush (caméra, codec, résolution, nombre de pistes).
- [ ] **3.4 (W)** Rush Tentacle Sync E (LTC sur la piste droite). Attendu : piste LTC coupée et trois choix proposés (laisser coupée, baisser de 30 dB, réactiver).

## 4. Scopes ([#12](https://github.com/Herve-obe/VERIFLOW-v4/pull/12))

- [ ] **4.1 (W+M)** PLAYER, bouton « Scopes ». Passer en revue forme d'onde, parade RGB, vectorscope et histogramme sur un rush Rec. 709. Attendu : affichage cohérent avec l'image.
- [ ] **4.2 (W+M)** Lire le rush avec les scopes ouverts. Attendu : lecture toujours fluide.
- [ ] **4.3 (W)** Ouvrir la même mire (ou le même plan) dans Resolve. Attendu : couleurs de l'aperçu identiques à l'œil ; sur une mire à 75 %, les barres tombent dans leurs cibles du vectorscope.

## 5. Sortie moniteur ([#13](https://github.com/Herve-obe/VERIFLOW-v4/pull/13))

- [ ] **5.1 (W+M)** Brancher le second écran. PLAYER, menu « Moniteur » : les deux écrans sont listés, l'écran principal est signalé.
- [ ] **5.2 (W+M)** Choisir le second écran. Attendu : image plein écran sur cet écran, sans interface ni timecode incrusté.
- [ ] **5.3 (W+M)** Lire le rush. Attendu : lecture fluide, même image sur les deux écrans.
- [ ] **5.4 (W+M)** Charger une LUT. Attendu : LUT appliquée aussi sur le moniteur.
- [ ] **5.5 (W+M)** Échap sur le second écran ferme le moniteur ; le menu revient sur « Moniteur ». Rouvrir le moniteur, puis fermer VERIFLOW : le moniteur se ferme aussi.

## 6. REPORT sur Mac ([#6](https://github.com/Herve-obe/VERIFLOW-v4/pull/6))

- [ ] **6.1 (M)** Onglet REPORT complet. Fenêtre « Colonnes » : glisser-déposer pour réordonner. Tri par colonne.
- [ ] **6.2 (M)** Exports PDF, XLSX et EDL : ouvrir chaque fichier produit.
- [ ] **6.3 (M)** Touche R dans PLAYER : ajout au rapport.

## 7. SYNC ([#8](https://github.com/Herve-obe/VERIFLOW-v4/pull/8), [#10](https://github.com/Herve-obe/VERIFLOW-v4/pull/10))

- [ ] **7.1 (W)** Rushes et WAV de la même journée, en lot. Attendu : chaque plan apparié au bon WAV, décalage et confiance affichés.
- [ ] **7.2 (W)** Caméra sans timecode avec son témoin. Attendu : plan trouvé par la forme d'onde. Puis méthode « Forme d'onde seule » sur un lot au timecode faux : plans retrouvés malgré le timecode.
- [ ] **7.3 (W)** Rush avec LTC sur une piste. Attendu : badge LTC, canal LTC exclu des fichiers produits.
- [ ] **7.4 (W)** Rushes en 23,976 et 29,97 DF. Attendu : timecodes et décalages justes.
- [ ] **7.5 (W)** Re-wrap MOV polyphonique, MOV une piste mono par canal, et MXF. Importer dans Resolve, Premiere et Avid. Attendu : pistes nommées, timecode juste, son caméra en plus.
- [ ] **7.6 (W+M)** FCPXML dans Final Cut Pro (M) et Resolve ; OTIO dans Resolve. Attendu : son attaché et calé.
- [ ] **7.7 (W)** Plan de plus de 10 min. Attendu : alerte de dérive cohérente avec l'écoute.
- [ ] **7.8 (W)** Même plan : re-wrap avec « Corriger la dérive mesurée » cochée. Dans le logiciel de montage, calage vérifié au début et à la fin du plan.

## 8. TRANSCODE ([#7](https://github.com/Herve-obe/VERIFLOW-v4/pull/7), [#11](https://github.com/Herve-obe/VERIFLOW-v4/pull/11), [#14](https://github.com/Herve-obe/VERIFLOW-v4/pull/14))

- [ ] **8.1 (W+M)** H.264, HEVC et AV1 : noter la ligne « Encodeur » (carte graphique ou système) et la vitesse.
- [ ] **8.2 (M)** ProRes 422 HQ : encodeur Apple VideoToolbox proposé ou non selon la version de macOS.
- [ ] **8.3 (W)** Rushes réels vers ProRes, DNxHR, DNxHD MXF, XDCAM HD422. Importer dans Resolve, Premiere et Avid. Attendu : timecode et pistes son justes.
- [ ] **8.4 (W)** MXF OP-Atom : préréglage DNxHD 1080, case « MXF OP-Atom pour Avid ». Copier les fichiers `_V1` et `_A1`, `_A2`... dans `Avid MediaFiles/MXF/<n>`. Attendu : Media Composer les reconnaît, avec le bon timecode.
- [ ] **8.5 (W)** Préréglages web : YouTube 1080p et vertical 9:16 depuis un rush 16:9.
- [ ] **8.6 (W)** Incrustations : timecode, nom du fichier, logo PNG, sous-titres SRT, LUT .cube de la caméra.
- [ ] **8.7 (W+M)** Éditeur de sous-titres : dans Incrustations, « Éditer » sur le SRT du client. Corriger les signalements, enregistrer, incruster dans une conversion. Attendu : accents et caractères spéciaux intacts.
- [ ] **8.8 (W+M)** « Créer des sous-titres » : créer un VTT de zéro, l'enregistrer, l'utiliser en piste.
- [ ] **8.9 (W)** Son : WAV Sound Devices vers WAV 44,1 kHz 16 bits, MP3, pistes séparées. Attendu : fichiers nommés d'après les pistes ; métadonnées visibles dans MEDIA.
- [ ] **8.10 (W)** Remplacement du son calé par timecode (vidéo et WAV BWF du même plan).
- [ ] **8.11 (W)** Analyses : détection de plans sur un montage (EDL importée dans Resolve), loudness d'un mix, VMAF d'un proxy.
- [ ] **8.12 (W+M)** Préréglage personnel : enregistrer sur un poste, exporter, importer sur l'autre.
