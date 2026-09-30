# VERIFLOW : questionnaire de cadrage de la charte

Statut : en attente des réponses d'Hervé. Pour chaque question, une proposition par défaut est donnée entre crochets.
Répondre "défaut" suffit pour valider la proposition.

## A. Produit et modèle économique
1. Nom commercial définitif : "VERIFLOW" ? Le "v4" du repo signifie-t-il qu'il existe des versions précédentes (code, maquettes, specs) à reprendre ? [VERIFLOW, départ de zéro]
2. Utilisateurs cibles prioritaires : DIT, assistant caméra, ingé son tournage, monteur, petite prod, écoles ? [DIT + ingé son tournage]
3. Modèle de vente : licence perpétuelle, abonnement, freemium (version gratuite limitée) ? [licence perpétuelle + MAJ majeures payantes]
4. Protection : activation 100 % hors ligne par clé signée cryptographiquement ? [oui]
5. Langues de l'interface : [FR + EN dès la V1, architecture i18n]
6. "Windows, Mac et PC" : cibles = Windows + macOS + Linux ? Versions minimales ? [Win 10/11 x64, macOS 12+ Apple Silicon et Intel, Linux Ubuntu 22.04+ en bonus]
7. Machine minimale cible (poste mobile) : [laptop 4 cœurs, 8 Go RAM, SSD]

## B. Licences et budget zéro (décisions structurantes)
8. Code fermé propriétaire ou code ouvert (GPL) vendu ? Impact : en fermé, seules les briques LGPL/MIT/BSD/Apache sont utilisables. x264/x265 (GPL) sont alors exclus ; H.264/HEVC passeraient par les encodeurs matériels/OS (VideoToolbox, Media Foundation, NVENC, QSV, AMF). [propriétaire + FFmpeg compilé LGPL en liaison dynamique]
9. Brevets codecs (H.264, HEVC, AAC) et ProRes (encodeur FFmpeg non licencié par Apple) : risque juridique à accepter, à contourner, ou à reporter ? [s'appuyer sur les encodeurs fournis par l'OS, ProRes encodage via FFmpeg marqué "non certifié Apple" ou désactivable]
10. Signature de code : notarisation Apple (compte Apple Developer payant) et certificat Windows (payant) sont hors budget zéro. Sans eux : alertes Gatekeeper/SmartScreen à l'installation. On accepte pour la phase bêta ? [oui, signature à prévoir au premier revenu]
11. Repo public ou privé ? (impact quota GitHub Actions gratuit pour compiler Windows/macOS/Linux) [privé]

## C. Stack technique (proposition à valider)
12. [Tauri 2 : cœur Rust (copie, checksums, audio temps réel, FFmpeg) + interface web (Svelte ou React). Binaire léger, faible RAM, multiplateforme.] Préférence autre (Qt, Electron, Flutter) ?
13. Données : [un fichier projet par production + base SQLite locale, aucun serveur, aucune télémétrie]
14. Mises à jour : [vérification en ligne optionnelle, désactivable, installation manuelle possible hors ligne]

## D. OFFLOAD
15. Que signifie pour toi "légalement conforme" : norme ou exigence précise (assurance, contrat de prod, huissier) ? [standard ASC MHL v2 + rapport PDF horodaté]
16. Algorithmes de checksum : [xxHash64 par défaut, choix MD5 / SHA-1 / SHA-256 / xxHash3-128]
17. Destinations : 2 maximum ou extensible ? Copie simultanée ? [2 dans l'UI, moteur extensible, écriture parallèle, lecture source unique]
18. Vérification : relecture complète des destinations après copie, comparée au hash source ? [oui, obligatoire]
19. Arborescence et nommage : templates (Projet/Jour/Caméra/Carte) ? Renommage des clips autorisé ? [templates oui, structure de carte conservée intégralement, pas de renommage des originaux]
20. Robustesse : reprise après coupure, détection carte déjà copiée, doublons, éjection sécurisée, file d'attente de cartes ? [tout oui]
21. Formats de rapport : [PDF + HTML + CSV + MHL, logo prod, vignettes vidéo optionnelles]
22. Supports/caméras à gérer en priorité (ARRI, RED, Sony, Canon, Blackmagic, Panasonic, DJI, GoPro, Zoom, Sound Devices, Zaxcom...) ?

## E. MEDIA
23. Formats de lecture prioritaires ? Les RAW caméra (R3D, ARRIRAW, BRAW, X-OCN, ProRes RAW) passent par des SDK gratuits mais propriétaires, à conditions de redistribution à vérifier une par une. V1 ou V2 ? [V2, V1 = formats FFmpeg]
24. Champs de métadonnées éditables : scène, prise, bobine, notes, champs BWF bext, iXML, QuickTime/XMP ? [tous ceux-là]
25. Écriture des métadonnées : dans le fichier (ce qui casse le checksum de l'offload) ou en sidecar/copie ? [jamais sur les originaux vérifiés : sidecar + écriture uniquement sur copies de travail]
26. Affichage : vignettes, filmstrip, forme d'onde, recherche/filtre ? [oui]

## F. PLAYER
27. Raccourcis : [standard J/K/L + Espace + flèches gauche/droite image par image + I/O, reconfigurables]
28. Exports de logs : [EDL CMX3600 + ALE + CSV + FCPXML + OTIO]. Marqueurs avec couleur, commentaire, in/out ?
29. Vidéo : LUT d'affichage (Log vers Rec.709) ? TC incrusté ? Scopes ? Sortie moniteur externe ? [LUT .cube + TC oui, scopes et sortie externe en V2]
30. Audio multipiste : sources = WAV polyphoniques et/ou monos multiples regroupés par scène/prise ? [les deux]
31. Sortie audio : master stéréo seulement ou routage multi-sorties ? Mesures : crête dBFS, LUFS, PPM ? [master stéréo, crête dBFS + LUFS]
32. ASIO sous Windows (licence Steinberg à vérifier) ou WASAPI exclusif ? [WASAPI + CoreAudio, ASIO si licence compatible]
33. Lecture vidéo + multipiste audio simultanée et synchronisée dans le PLAYER ? [oui]

## G. SYNC
34. Sources de synchro : TC métadonnées (BWF timeReference, piste tmcd QuickTime), décodage LTC enregistré sur une piste audio, détection de clap, waveform ? [TC métadonnées + LTC + waveform]
35. Gestion de la dérive entre horloges (tournages longs) ? [détection + alerte V1, correction V2]
36. Export : re-wrap sans réencodage de la vidéo avec audio embarqué ? Et/ou export timeline pour le montage (FCPXML, OTIO, AAF) ? [re-wrap + FCPXML/OTIO, AAF en V2]
37. Cadences : 23.976, 24, 25, 29.97 DF/NDF, 30, 50, 59.94, 60 ? [toutes]

## H. TRANSCODE
38. Codecs/conteneurs prioritaires (ProRes, DNxHR, H.264, HEVC, AV1, VP9, FFV1, WAV, FLAC, AAC, MP3, MXF OP1a/OP-Atom, MOV, MP4, MKV) ?
39. Fonctions : presets, file d'attente, proxies montage, redimensionnement, LUT et TC incrustés, watermark ? [tout oui]

## I. REPORT
40. As-tu des modèles de rapports son / image existants (PDF, Excel) à me donner comme référence ?
41. Formats d'export des rapports : [PDF + CSV + XLSX + HTML]
42. Pop-up PLAYER vers REPORT : à chaque marqueur, ou sur raccourci dédié ? [raccourci dédié + auto sur changement de prise]

## J. Interface et méthode
43. Charte graphique : thème sombre, couleurs, logo existant ? [sombre, accent unique, logo à créer]
44. Priorité de développement (MVP) ? [OFFLOAD, MEDIA, PLAYER, REPORT, SYNC, TRANSCODE]
45. Rushes de test : peux-tu fournir des échantillons (hors Git, trop lourds) via un lien de partage ?
46. Workflow Git : [une branche par fonctionnalité, PR que tu valides, releases taguées]
