# Getting started

VERIFLOW manages the picture and sound dailies of a shoot: verified card copies, media browsing, playback, sync, transcoding and reports.

## Two modes, six tabs

- **VIDEO** (blue accent) and **AUDIO** (amber accent): the switch at the top right, or **Tab** (when not typing), or **Ctrl+Tab** anywhere. Each tab adapts to the mode: videos and images in VIDEO, sound files in AUDIO.
- Tabs follow the workflow: **OFFLOAD** (Alt+1), **MEDIA** (Alt+2), **PLAYER** (Alt+3), **SYNC** (Alt+4), **TRANSCODE** (Alt+5), **REPORT** (Alt+6).
- Switching tabs interrupts nothing: a running copy or an open clip carry on.

## The project

A project is a **.veriflow** file, stored wherever you like (for example on the production drive). It keeps the copy history, entered metadata, markers and reports. **Media files are never modified**: everything you enter goes into the project.

- **File > New Project** (Ctrl+N), **Open Project** (Ctrl+O), **Recent Projects**.
- **Save** (Ctrl+S): every change is already written when it is made; Ctrl+S confirms it.
- **Save As** (Ctrl+Shift+S): full copy of the project under another name, which becomes the open project. The original is untouched.
- The open project's name is shown in the middle of the top bar.

## The explorer

Left panel of OFFLOAD, MEDIA, SYNC and TRANSCODE:

- **Volumes** (free space, spinning disk marked "HDD", removable media marked ⏏), **Favorites** (right-click a folder: Pin), **Project** (copy folders of the open project).
- It updates by itself: card plugged in, folder created, file copied.
- Drag a folder onto a zone of the tab; right-click for actions; **Ctrl+B** hides or shows it.

## Menus, shortcuts and preferences

- **Edit > Preferences** (Ctrl+,): language, behaviour at opening, application data, shortcuts, OFFLOAD, PLAYER and TRANSCODE defaults.
- **Preferences > Shortcuts**: click "Edit" then press the new combination. A shortcut already in use is removed from the other action. **Help > Keyboard Shortcuts** shows the current list.
- On macOS, menus are in the system bar at the top of the screen.

# OFFLOAD: copying a card

Goal: copy a source to one or more destinations with **bit-for-bit integrity proof**.

## Making a copy

1. **Source**: a detected card is offered directly; otherwise "Browse", drag and drop, or double-click in the explorer.
2. **Destinations**: "+ Add a destination" (at least two drives, following the DIT 3-2-1 rule). The source is read **once** for all destinations.
3. **Folder structure**: template of the folder created in each destination. Variables: `{projet}`, `{jour}`, `{camera}`, `{carte}`, `{date}`. The example under the field shows the resulting path.
4. Shooting day, camera or recorder, operator, notes: included in the report.
5. **Checksums**: XXH128 by default (fastest, recognised by ASC MHL). Several checksums can be computed at once.
6. Option **Eject the source after a verified copy**.
7. **Start the secure copy**. Before copying, VERIFLOW checks free space, flags a spinning disk (slower copy) and a card already copied (in the destination or in the project history): you then choose to complete or to copy everything again.

## What the copy guarantees

- The card structure is kept as is, empty folders included, with the original dates.
- **Nothing is ever written to the source.**
- Each file is written under a temporary name and renamed once complete: never a half-copied file.
- Each destination is **read back from the disk**, not from the system memory, then compared with the source.
- A failing destination (full disk, unplugged cable) does not stop the others. A different file already present is never overwritten.
- Several cards can be queued.

## Result and reports

- Verdict: **CONFORME** (verified), **ÉCHEC** (failed) or **INTERROMPU** (interrupted), with file-by-file details and the cause of any failure.
- In each destination:
  - `ascmhl/`: **ASC MHL v2** manifest (validated with the ASC reference tool), chained from one copy to the next;
  - `_VERIFLOW/`: **PDF**, **HTML** and **CSV** report (project, operator, computer, duration, speed, checksum of each file per destination). The PDF prints the checksum of the CSV and MHL files, and its own checksum file comes with it: any change is visible.
- SHA-256 does not exist in the ASC MHL v2 standard: it appears in the reports, not in the MHL.

## Ejection

If ejection fails, the message names the program keeping the drive open (Windows). Close it, then eject the drive from the system.

# MEDIA: browsing media

Goal: browse media, preview them and fill in their metadata, without ever modifying the files.

## Browsing

- Click a folder in the explorer: its media are listed (VIDEO mode: videos and images; AUDIO: sounds). The list follows running copies.
- **List** view (thumbnail or waveform, duration, codec, format, TC, scene, take, circled take, size) or **grid**.
- On hover, a strip of 8 frames follows the mouse.
- **Search** (name, scene, take, comment, reel, keywords) and **circled takes** filter.
- Selection: click, Ctrl+click, Shift+click, Ctrl+A, arrows.

## Quick player

**Space** or double-click: the media opens in a window, without leaving MEDIA. Esc closes it. "Open in PLAYER" switches to the PLAYER tab. Its "Monitor" menu sends the picture to a second screen.

## Metadata

- **Inspector**: technical information (container, codec, resolution, frame rate, TC, sound tracks, BWF time reference).
- 38 fields in 7 groups: production, identification, breakdown (scene, shot, take, circled, false start, wild track, MOS), picture (lens, focal length, T-stop, ISO, shutter, ND, white balance...), sound, timecode, notes, plus track names.
- Metadata already present in the files (iXML, QuickTime and MXF tags) are read and offered.
- Edit one media or **a batch**: "multiple values" is shown when the selection differs. Everything is saved in the project.

## Exports

- **CSV** (Excel), **ALE** (Avid, Resolve), **XMP sidecars** (Premiere, Resolve) in the folder of your choice.
- **Working copies**: new files with the metadata written inside (iXML for WAV, tags for videos, without re-encoding). Originals stay intact.
- **Add to report** (REPORT) and **Send to TRANSCODE** buttons.

# PLAYER: playback and logging

## Playback

- **Space** play/pause, **J/K/L** shuttle back, pause, forward (in VIDEO, repeated presses: ×2, ×4, ×8), **arrows** frame by frame, **Enter** stop and back to start.
- Frame-accurate playback. In normal playback, the picture follows the sound card clock.
- Video sound tracks: one strip per track (meter, volume, mute). A track carrying **LTC** (Tentacle Sync, for example) is detected and muted.
- **TC** button: burnt-in timecode (display only). **LUT** button: display LUT (.cube, .3dl, .dat, .m3d, .csp), the file is never modified. **Scopes** button: waveform, RGB parade, vectorscope, histogram.
- **Monitor**: full-screen picture, without interface, on another screen; the cursor disappears after 2 s without movement. Esc closes it.
- **Audio output**: choice of the sound card (WASAPI and ASIO on Windows, CoreAudio on macOS) and of the channel pair.

## AUDIO mode

Multitrack console (up to 32 tracks): levels, volume, pan, mute, solo per track, names read from iXML.

## Markers (logs)

An open project is required (VERIFLOW offers to create or open one).

- **M**: marker at the current frame, comment typed right away.
- **I** then **O** then **M**: range marker (in-out).
- 8 colours (Avid's), scene and take; click a TC to go there.
- **Exports**: CMX3600 EDL, ALE, CSV, FCPXML, OTIO, for the current clip or all marked clips of the project. A range becomes a subclip; a marker outside any range only appears in the CSV.

## Report entry

**R** or "Report (R)": entry window for the current clip's line in the report (REPORT). Enter saves, Esc closes.

# REPORT: camera and sound reports

- **Camera report** in VIDEO mode, **sound report** in AUDIO mode, in the layout of the school's paper reports, saved in the project and numbered automatically.
- **Adding takes**: report button, "Add to report" in MEDIA, or the R key in the PLAYER. Lines are pre-filled: TC, duration, frame rate, resolution, camera, scene, take, circled, fields entered in MEDIA; for WAV files, track names, BWF TC, sample rate, bit depth.
- **Editor**: header (production, camera or recorder, sheet, notes), table editable in place, movable lines, automatic saving. Drop-down lists with "Other..." for a free value.
- **Columns**: add and order the columns you want (lens, ISO, ND...). Click a column title to sort.
- 1 to 32 tracks; when printing, 8 tracks per sheet as on paper.
- **Logo** and name of the school or production at the top of the reports.
- **Exports**: PDF, XLSX, CSV, HTML, EDL (camera report).

# SYNC: syncing picture and sound

Goal: match each video shot with the recorder's sound, to the exact offset, then produce synced files.

## Analysis

1. Add videos and sounds (buttons, drag and drop, explorer): they are sorted automatically.
2. **Method**:
   - **Automatic**: timecode or LTC first, refined and completed by the waveform;
   - **Timecode only**: shoot with reliable timecode (jam sync);
   - **Waveform only**: missing or wrong timecode; each shot is searched in all sounds.
3. **Analyse**. For each pair: method used, offset (seconds and frames), **confidence**, clock **drift**, remark (for example "timecode corrected").

## Checking a pair

Click a line: overlaid waveforms (camera guide track on top, recorder below). Adjust by ±1 frame, ±1 ms, ±1 sample, "Recompute from the waveform", choose another sound, **Pair checked** box.

## Creating synced files (re-wrap)

- New file per shot: **picture copied without re-encoding** and the recorder's sound in sync (24-bit PCM), camera sound kept as an option, original timecode.
- **Destination**: next to each video, or in a chosen folder. `_sync` suffix by default; the original is never overwritten.
- MOV or MXF container (one mono track per channel), "one mono track per channel" option in MOV.
- **Correct drift**: the sound is resampled to follow the camera clock, synced in the middle of the shot.
- "Validated pairs only" limits the export to checked pairs.

# TRANSCODE: converting

## Principle

1. Add files (buttons, drag and drop, or "Send to TRANSCODE" from MEDIA).
2. Choose a **preset** (about 80): editing and grading (ProRes, DNxHR, DNxHD, CineForm...), broadcast (XDCAM, AVC-Intra, XAVC), delivery (H.264, HEVC, AV1, VP9...), web platforms, proxies, legacy codecs, images (JPEG, PNG, TIFF, DPX, EXR...), sound (WAV, AIFF, FLAC, MP3, AAC...), no re-encoding, analyses.
3. Adjust the **settings** by section, the **destination** and the **naming**, then start.

## Settings sections

- **Picture**: bitrate, size, crop, rotation, deinterlacing, frame rate, LUT, corrections, colour tags.
- **Overlays**: timecode, file name, text, logo, subtitles. SRT and WebVTT subtitle editor ("Edit" or "Create subtitles") with readability checks.
- **Sound**: track selection, sample rate, bit depth, bitrate, loudness normalisation (EBU R 128, ATSC A/85, streaming, podcast) capped at True Peak.
- **Processing** depending on the preset: replacement sound synced by timecode, frame rate conform, insert, detection thresholds.
- **MXF OP-Atom (Avid)** for DNxHD 1080 presets.

## Without re-encoding

Container change, trimming (in and out points per file, in timecode or seconds), sound replacement, merge, insert, subtitle tracks, extraction of the picture, the sound or each track.

## What VERIFLOW keeps

Shooting metadata (bext, iXML: scene, take, track names), **timecode** (shifted with trimming), separate tracks named after the iXML. Optionally: XXH128 checksum of each output file, VMAF quality, CSV report of the batch.

## Personal presets

Save your settings under a name; export and import them as `.vfpreset` files to share them.

# Data and uninstalling

- **Preferences > General > When VERIFLOW opens**: *Resume where I left off*, or *Start fresh* (destinations, options and display settings cleared at each opening; language, favorites, recent projects, presets and preferences kept).
- **Reset now** and **Clear all data**: the same, right away.
- Projects, reports and copies are never deleted by VERIFLOW.
- **Windows**: uninstall from Settings > Apps > VERIFLOW. Tick "Delete application data" to start from a clean slate. Running `setup.exe` again and choosing "Uninstall" continues with a reinstallation: just cancel it.
