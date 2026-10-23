# Welcome

VERIFLOW handles the picture and sound rushes of a shoot, from offloading the cards to the files delivered to editing:

- **OFFLOAD** copies the cards with bit-for-bit proof of integrity;
- **MEDIA** shows the media and their metadata;
- **PLAYER** plays the rushes and lets you place markers;
- **REPORT** writes the camera and sound reports;
- **SYNC** syncs the picture with the recorder sound;
- **TRANSCODE** converts files (proxies, deliveries, sound, stills).

Three principles apply throughout:

1. **Originals are never modified.** What you enter goes into the project file, and every file produced is a new file.
2. **Everything can be checked.** Copies read back from disk, checksums, ASC MHL manifests, dated reports.
3. **Nothing needs the Internet**, except the certified timestamp of reports (an OFFLOAD option). FFmpeg is included in the installation.

This guide follows the order of work. The **A shooting day** chapter shows the whole sequence; the following chapters detail each tab. At the end: **Common problems** and **Glossary**.

# Getting started

## The screen

- **Top bar**: menus (File, Edit, View, Playback, Help), tabs, name of the open project in the centre, **VIDEO / AUDIO** switch on the right.
- **Explorer**, on the left in OFFLOAD, MEDIA, SYNC and TRANSCODE: drives and folders.
- **Work area** in the centre: the tab content.
- **Status bar** at the bottom: messages, version.

On macOS, the menus are in the system menu bar at the top of the screen.

## Two modes: VIDEO and AUDIO

- **VIDEO** (blue accent): videos and stills. **AUDIO** (amber accent): sound files.
- To switch: the switch at the top right, the **Tab** key (outside text entry) or **Ctrl+Tab** (anywhere).
- Each tab adapts: MEDIA only shows sounds in AUDIO, REPORT creates a sound report, the PLAYER opens its multitrack console.

## Six tabs

They follow the order of work: **OFFLOAD** (Alt+1), **MEDIA** (Alt+2), **PLAYER** (Alt+3), **SYNC** (Alt+4), **TRANSCODE** (Alt+5), **REPORT** (Alt+6).

Switching tabs interrupts nothing: a copy, an analysis or a conversion keeps running in the background, and an open clip stays where it is.

## The project

A project is a **.veriflow** file. Store it wherever you like, for example on the production drive next to the rushes. It holds:

- the copy history (so OFFLOAD recognises a card already copied);
- the metadata entered in MEDIA;
- the PLAYER markers;
- the REPORT reports.

The media themselves are never modified. Without an open project you can copy, play and convert, but not save metadata, markers or reports. VERIFLOW then offers to create or open a project.

- **File > New Project** (Ctrl+N): choose the file name and location.
- **File > Open Project** (Ctrl+O), or **Recent Projects**.
- **Save Project** (Ctrl+S): every change is already written when it is made; Ctrl+S confirms it.
- **Save Project As** (Ctrl+Shift+S): full copy of the project under another name, which becomes the open project. The original stays as it is. Handy to keep one version per shooting day.

Tip: one project per film or per exercise. Back up the .veriflow file with the rushes.

## The explorer

It shows folders only, in three sections:

- **Volumes**: each drive with its name and free space. "HDD" flags a spinning drive (slower copy), ⏏ a removable medium.
- **Favorites**: right-click a folder > **Pin**.
- **Project**: the copy folders of the open project.

It updates by itself: card plugged in, folder created, file copied. A double-click expands or collapses a folder.

Uses:
- **drag and drop** a folder onto an area of the tab (Source, Destinations, file list);
- **right-click** for the tab actions (use as source, add as destination, open in the system...);
- **Ctrl+B** hides or shows the explorer; drag its edge to resize it.

## Preferences

**Edit > Preferences** (Ctrl+,). One section per topic, on the left:

- **General**: language, startup behaviour, application data (see the **Data and uninstalling** chapter). These settings apply as soon as you change them.
- **Shortcuts**: see below. They also apply at once.
- **OFFLOAD**: folder template, operator, checksums, eject.
- **PLAYER**: burnt-in timecode, scopes, colour of new markers, report entry opened on each new take.
- **TRANSCODE**: destination (next to each source file, or a folder), in VIDEO and AUDIO.

For OFFLOAD, PLAYER and TRANSCODE:
- **Apply** (at the bottom of the window) saves the values and applies them right away to open tabs and players;
- **Cancel**, or the close button, discards the changes;
- **Factory values** resets the section to its original values. Then click **Apply**.

The applied values are also the starting point when the working state is cleared (see **Data and uninstalling**).

## Keyboard shortcuts

- **Help > Keyboard Shortcuts** shows the current list.
- **Preferences > Shortcuts**: click **Edit**, then press the new combination (Esc cancels).
  - A shortcut already in use is removed from the other action, with a message.
  - **×** removes a shortcut, **Default** restores the original one, **Restore all** restores every original shortcut.
- Cut, copy, paste, undo and select all stay those of the system.

Main default shortcuts:

| Action | Key |
|---|---|
| Play / pause | Space |
| Reverse, pause, forward (shuttle) | J, K, L |
| Previous / next frame | ← / → |
| Stop and back to start | Enter |
| Marker | M |
| In point, out point | I, O |
| Report entry | R |
| Switch VIDEO / AUDIO | Tab, or Ctrl+Tab while typing |
| Tabs | Alt+1 to Alt+6 |
| Explorer | Ctrl+B |
| User guide | F1 |

## Help

- **User Guide** (F1): this guide, in English or French depending on the interface language.
- **Licenses**: texts of the VERIFLOW licence (GPLv3), the end-user licence agreement (EULA), the terms of use, FFmpeg (with the version and configuration actually installed) and third-party components.
- **About VERIFLOW**: VERIFLOW and FFmpeg versions, access to the licences.

# A shooting day

Example sequence, from card to edit. Each step is detailed in the chapter of its tab.

1. **In the morning**: open the film project (File > Open Project), or create it on the first day.
2. **For each card** (OFFLOAD):
   - plug in the card: it is offered as the source;
   - check the two destinations (working drive and backup drive), the shooting day and the camera;
   - start the secure copy;
   - wait for the **VERIFIED** verdict before erasing or formatting the card in the camera.
3. **During breaks** (MEDIA): open the copy folder, enter scene, take and circled take in batch, check the technical metadata.
4. **Reviewing the rushes** (PLAYER): watch the key shots, place markers (focus, sound problem, good line), fill in the report with the **R** key.
5. **In the evening** (SYNC): sync the shots with the recorder sounds, check doubtful pairs, create the synced files.
6. **Delivery** (TRANSCODE): make the editing proxies or the files requested by post-production.
7. **Reports** (REPORT): finish the camera and sound reports and export them to PDF for the production.

At the end of the day, each destination holds the card copies, the ASC MHL manifests and the copy reports. The project file holds the rest.

# OFFLOAD: copying a card

Goal: copy a source (card, drive, folder) to one or more destinations with **bit-for-bit proof of integrity** that the production or an insurer can check.

## Preparing the copy

1. **Source**:
   - a removable medium that is plugged in is offered directly: one click is enough;
   - otherwise **Browse**, drag and drop from the explorer or the system, or double-click a folder in the explorer;
   - **Refresh** reads the list of media again.
2. **Card recognised**: under the source, VERIFLOW shows the card type and reel, from the folder and file names (example: "Card recognised: RED, reel A002"). Recognised types: Sony XAVC and XDCAM, Canon XF, Panasonic P2, AVCHD, RED, Blackmagic RAW, ARRI, GoPro, DJI. Nothing is written to the card.
3. **Destinations**: **Add a destination**, as many as needed. DIT rule (3-2-1): at least two copies on two different drives, one kept elsewhere. The source is read **only once** for all destinations.
4. **Folder template**: the folder created in each destination. The example under the field shows the resulting path. Variables:
   - `{projet}`: name of the open project;
   - `{jour}`: shooting day entered;
   - `{camera}`: camera entered, otherwise the letter recognised on the card;
   - `{carte}`: name of the source volume or folder;
   - `{bobine}`: recognised reel, otherwise the card name;
   - `{support}`: recognised card type;
   - `{date}`: date of the copy.

   Example: `{date}/{camera}/{bobine}` gives `2026-10-22/A/A002`.
5. **Shooting day**, **Camera / recorder**, **Operator**, **Notes**: shown in the copy report.
6. **Checksums**: XXH128 by default, the fastest, recognised by the ASC MHL standard. You can tick several (MD5, SHA-1, SHA-256, C4...). SHA-256 is not part of the ASC MHL standard: it appears in the reports only.
7. **Eject the source after a verified copy**: the card is ejected as soon as the verdict is VERIFIED.

## The PDF report

In the **PDF report** block:

- **Production logo**: PNG, JPEG, TIFF, BMP or WebP. It is printed at the top right; transparency becomes white.
- **Video thumbnails**: one frame per shot, taken at 10 % of its duration from the verified copy (never from the card), 5 per row, 60 at most. Extracting them adds a little time at the end of the copy.
- **Certified RFC 3161 timestamp** (Internet required):
  - the PDF checksum is sent to a timestamp authority (DigiCert by default, address can be changed);
  - the authority returns a signed token, written next to the PDF (`.pdf.tsr`) in each destination;
  - this token proves that the report existed in this form at this date;
  - only the PDF checksum is sent, never the report or the rushes;
  - without a connection, the reports are still written and the copy card shows **Timestamp failed**.

## Starting and following the copy

Click **Start secure copy**. Before copying, VERIFLOW checks:

- the free space of each destination ("not enough space, missing ...");
- the drive speed: a spinning drive is flagged, the copy will be slower;
- existing copies: a card already copied to the destination or in the project history is flagged (see **Resuming a copy**).

During the copy, the **Queue** shows one card per copy: progress, speed, time left, current file. You can prepare and start the next card: it waits for its turn. **Cancel** stops a copy without leaving a half-written file.

## What the copy guarantees

- The card folder structure is kept as it is, empty folders included, with the original file and folder dates.
- **Nothing is written to the source.**
- Each file is written under a temporary name, then renamed once complete: never a half-copied file.
- Each destination is **read back from the disk**, not from the system memory, then compared with the source.
- A failing destination (disk full, cable pulled) does not stop the others.
- A different file already present is never overwritten without your consent.

## Reading the result

- **VERIFIED**: every copy is checked bit for bit. You can reuse the card.
- **Failed**: one or more files do not match. The **Failed files** list gives the destination and the cause. Do not reuse the card: fix the cause (drive, cable), then start the copy again; it resumes the missing files.
- **Copy cancelled**: cancelled or cut off. Start it again to resume.

Card buttons: **PDF report** opens the report, **Folder** opens the copy in the system. **Remove** takes the card out of the queue; the copied rushes, reports and MHL stay on the drives.

## Files produced

In each destination, next to the rushes:

- `ascmhl/`: **ASC MHL v2** manifest of the copy, chained to the previous copies on the same destination. It is the format used by labs and DIT software to check a copy later.
- `_VERIFLOW/`: **PDF**, **HTML** and **CSV** report. It gives the project, operator, computer, duration, speed, and the checksum of every file in every destination.
- The PDF prints the checksum of the CSV and of the MHL, and its own checksum file (`.pdf.sha256`) comes with it: any change shows.

## Resuming a copy

If a card has already been copied, fully or partly, VERIFLOW reads back the rushes present and the source, then compares their checksums. It shows how many are **Identical**, **Different** or **Missing**. Two choices:

- **Complete the copy**: identical rushes are kept, only missing or different ones are copied and checked;
- **Delete and copy everything again**: the rushes of this source already present are deleted, then the whole copy is made again (confirmation asked).

If the previous copy is in another folder (copy interrupted on another day), **Resume in this folder** continues it in the same place; **Back to the template** keeps the folder given by the template.

## Ejecting

- With the option ticked, the source is ejected after a verified copy: the **Source ejected.** message appears.
- On Windows, USB hard drives and SSDs can be ejected like cards.
- If ejecting is refused, the message names what keeps the drive open, when Windows knows it: a file open in a video player, File Explorer, antivirus or indexing. Close that program, then eject the drive from the system.

# MEDIA: browsing media

Goal: browse the media, preview them and fill in their metadata, without ever modifying the files.

## Browsing

- Click a folder in the explorer, or **Open a folder**: its media are shown. **Subfolders** includes the content of subfolders. Above 5,000 media, choose a more precise folder.
- VIDEO mode: videos and stills; AUDIO mode: sounds.
- The list follows running copies: a copied clip appears with its thumbnail.
- **List** view (thumbnail or waveform, duration, codec, format, TC, scene, take, circled take, size) or **Grid**.
- When hovering a thumbnail, a strip of 8 frames follows the mouse: you see the shot without opening it.
- **Search** (name, scene, take, comment, reel, keywords) and **Circled takes** filter.
- Selection: click, Ctrl+click (add), Shift+click (range), Ctrl+A (all), arrow keys.

## Quick player

**Space** or double-click a media: it opens in a separate window, without leaving MEDIA.

- The window can be moved to another display, resized and minimised; it comes back to the same place next time. If that display is no longer connected, it opens in the centre.
- Opening another media reloads the same window.
- Same controls as the PLAYER (playback, J/K/L, sound tracks).
- **Open in PLAYER** closes the window and opens the media in the PLAYER tab.
- **Monitor** menu: full-screen picture on a second display (see the PLAYER).
- **Esc** closes the window.

## Metadata

Select one or more media: the right panel shows their information.

- **Technical information**: container, codec, resolution, frame rate, TC, sound tracks, BWF time reference.
- **38 fields in 7 groups**:
  - Production: project, day, date, company, director, director of photography, sound engineer, DIT;
  - Identification: reel or card, clip name, camera, recorder;
  - Breakdown: sequence, scene, shot, take, circled take, false start, wild track, MOS;
  - Picture: lens, focal length, T-stop, focus, ISO, shutter, white balance, tint, filters and ND, LUT, sensor frame rate;
  - Sound: microphones, TC rate;
  - Timecode: user bits;
  - Notes: comment, rating, keywords, colour label.
- **Track names**.
- Metadata already in the files (iXML from recorders, QuickTime and MXF tags, reel from the timecode track) are read and offered.

**Batch entry**: select several media, change a field, then **Apply to selection**. "(multiple values)" appears when the media differ; a field left that way is not changed. Everything is saved in the project (**Saved in the project.**).

## Exporting

**Export** button:

- **Export as CSV**: table for Excel;
- **Export as ALE**: metadata for Avid and DaVinci Resolve (name, tracks, start and end TC, scene, take...);
- **XMP sidecar folder**: one XMP file per media, read by Premiere and Resolve;
- **Working copies**: new files with the metadata written inside (iXML for WAV, tags for videos, no re-encoding). The originals stay intact. A WAV over 4 GB is written as RF64, read by professional audio software.

Other buttons: **Add to report** (REPORT), **Send to TRANSCODE**, **Open in PLAYER**.

# PLAYER: playing and logging

## Opening a media

**Open in PLAYER** from MEDIA or the quick player, or the file picker of the tab. The PLAYER switches to the media mode (VIDEO or AUDIO).

## Playback

- **Space**: play or pause.
- **J / K / L**: reverse, pause, forward. In VIDEO, repeated presses speed up (×2, ×4, ×8). Sound is muted when shuttling.
- **Arrow keys**: frame by frame. **Enter**: stop and back to start.
- Click the timeline to jump to a point. Markers are shown on it.
- In normal playback, the picture follows the sound card clock: picture and sound stay in sync. A start offset between sound and picture in the file (common in MP4 and after editing) is compensated.

## Video sound tracks

Next to the picture, one strip per track: name, peak meter, volume (-60 to +12 dB), mute. The meter measures before the volume: it moves even when the track is muted.

**LTC**: a track carrying audio timecode (Tentacle Sync on a camera's right input, for example) is detected on opening, marked "LTC" and muted, to spare you the noise. A message offers to keep it muted, lower it by 30 dB or turn it back on.

## Display

- **TC**: burnt-in timecode on the picture (display only).
- **LUT**: display LUT (.cube, .3dl, .dat, .m3d, .csp), for example the camera's LogC or S-Log3 LUT. The file is never modified.
- **Scopes**: waveform, RGB parade, vectorscope (75 % bar targets, skin tone line), histogram. Computed on the displayed picture, LUT included.

The starting choices are set in **Preferences > PLAYER**.

## Monitor (second display)

**Monitor** menu of the player: choose a connected display (TV or monitor on HDMI or DisplayPort).

- The picture is shown full screen, with no interface or burnt-in TC, LUT included.
- The mouse pointer disappears after 2 s without moving on that display.
- **Esc** on that display, or the Monitor menu, closes the output.
- Only one player feeds the monitor at a time: the last one that chose it.
- No direct SDI output: for an SDI monitor, use an HDMI to SDI converter.

## Audio output

Choice of the sound card and, for a multichannel card, the channel pair (1-2, 3-4...). Drivers: WASAPI and ASIO on Windows, CoreAudio on macOS. The choice applies to all of VERIFLOW.

With ASIO, usually only one program at a time can use the card: close the other audio program if no sound comes out.

## AUDIO mode

Multitrack console, up to 32 tracks: levels, volume, pan, mute, solo, track names read from the iXML. Markers are placed as in VIDEO, on the BWF timecode of the file.

## Markers (logs)

An open project is needed. Without a project, VERIFLOW offers to create or open one, then places the requested marker.

- **M**: marker at the current frame; type the comment right away, then Enter.
- **I**, move forward, **O**, then **M**: range marker (in and out).
- Colour dot: 8 colours (Avid's, recognised by Premiere, Resolve and OpenTimelineIO). Scene and take per marker.
- Click the TC of a marker to go there.

**Exports**: CMX3600 EDL, ALE, CSV, FCPXML, OTIO, for the current clip or for all marked clips of the project.
- A range becomes an event (subclip) of the timeline; a clip without a range is exported whole.
- A marker outside any range only appears in the CSV.
- An EDL does not state its frame rate: choose it when importing into the editing software.

## Report entry

**R**, or the **Report (R)** button: entry window for the row of the current clip in the latest report (or another one, or a new one). The row is created and pre-filled if needed. Enter saves, Esc closes.

Option **Preferences > PLAYER > Open the report entry on each new take**: the window opens by itself as soon as another take is loaded (project open).

# REPORT: camera and sound reports

Goal: produce the camera and sound reports of the shoot, in the layout of the school paper reports, from the media and what was entered in MEDIA and the PLAYER.

## Creating a report

1. A project must be open: reports are saved in it.
2. **Camera report** in VIDEO mode, **Sound report** in AUDIO mode. Reports are numbered automatically by type; the number can be changed.
3. **Report layout**: name of the school or production, and **logo**, printed at the top of each report.

## Adding takes

Three ways:
- **Add video clips** or **Add sound files** in the report;
- **Add to report** in MEDIA, for the selected media;
- the **R** key in the PLAYER.

Rows are pre-filled:
- videos: TC IN and TC OUT, duration, sound or mute, frame rate, definition, picture format, camera model;
- WAV: file name, scene, take, circled take, note, track names, BWF TC, sample rate, bit depth;
- fields entered in MEDIA: scene and shot, take, circled, false start, wild track, MOS, lens, focal length, T-stop, ISO, shutter, filters, white balance, comment.

**Empty row** adds a row to fill in by hand.

## Editing

- **Header** in three blocks: production, camera (or recorder), sheet and notes. What you enter is never overwritten by pre-filling.
- **Table**: cells are edited directly. Rows can be moved (**Move up**, **Move down**) and deleted.
- Drop-down lists with the paper choices and extended values; **Other...** to type a free value.
- **Circled** box at the start of a row: on the PDF, the take number is circled.
- **Columns**: tick the columns to add (lens, ISO, ND, false start...) and order them by drag and drop. The paper report columns are always present. The choice also applies to the next report of the same type.
- **Sorting**: click a column title (2 before 10), click again for reverse order.
- Tracks: 1 to 32. When printing, 8 tracks per sheet as on paper.
- Saving is automatic (**Saved**).

## Exporting

**PDF**, **XLSX**, **CSV**, printable **HTML**, and CMX3600 **EDL** for a camera report (each clip becomes an event, with its PLAYER ranges and markers). PDFs keep empty rows at the end of a sheet for handwriting on set.

# SYNC: syncing picture and sound

Goal: match each video shot with the recorder sound, to the exact offset, then produce synced files for editing.

## Before the shoot

Syncing is safer when the shoot prepares for it:
- **jam sync**: camera and recorder on the same timecode (Tentacle Sync, Ambient, recorder timecode);
- **reference sound**: the camera records the sound of its microphone, even a poor one; VERIFLOW compares it with the recorder;
- **LTC**: if the camera has no timecode input, the box can record timecode on an audio track of the camera. VERIFLOW reads it.

## Analysing

1. Add the videos and sounds: buttons, drag and drop of files or folders, explorer. Videos and sounds are sorted automatically.
2. **Method**:
   - **Automatic**: timecode or LTC first, then waveform refinement and search depending on the boxes. The general case;
   - **Timecode only**: shoot with reliable timecode;
   - **Waveform only**: timecode missing or wrong (jam sync forgotten, clock off); each shot is searched in all sounds.
3. Options:
   - **Refine with the waveform**: corrects a slightly wrong timecode, to the sample;
   - **Search clips without timecode by waveform**;
   - **Measure drift**: clock difference between camera and recorder over the shot;
   - **Timecode tolerance (s)**: largest difference searched around the timecode (2 s by default).
4. **Analyze**.

## Reading the results

One row per shot (in VIDEO) or per sound (in AUDIO, with unused sounds):

- **Timecode** of the shot ("LTC" badge if it comes from an audio track);
- **Method** used;
- **Offset** in seconds and frames;
- **Confidence**: similarity between the reference sound and the recorder. Below 25 %, the result is discarded or flagged in orange;
- **Drift**, in frames over the shot, with a warning above one frame;
- **Note**, for example "timecode corrected by +0.300 s by waveform".

## Checking a pair

Click a row: the detail opens under the table.
- Waveforms overlaid at the same instant: camera reference sound on top, recorder below. Peaks (slate, consonants) must line up.
- Displayed length from 0.5 to 30 s, position cursor.
- Manual adjustment by ±1 frame, ±1 ms or ±1 sample.
- **Recompute by waveform**, or choose another sound.
- **Verified pair** box once checked.

## Creating synced files (re-wrap)

For each shot, a new file:
- **picture copied without re-encoding**: no loss, fast;
- recorder sound aligned (24-bit PCM), with track names;
- camera sound kept as extra tracks (option);
- original timecode.

Settings:
- **Destination**: "Next to each video" or "In a folder" (Browse);
- `_sync` suffix by default; the original is never overwritten;
- **MOV** or **MXF** container (one mono track per channel); in MOV, "One mono track per channel" option;
- **Correct drift** (ticked by default): the sound is resampled to follow the camera clock, aligned at the middle of the shot;
- **Validated pairs only**: exports only the ticked pairs.

# TRANSCODE: converting

Goal: convert video, sound and image files, keeping the timecode and shooting metadata.

## How it works

1. **Add the files**: buttons, drag and drop (folders are scanned), or **Send to TRANSCODE** from MEDIA.
2. **Choose a preset** (about 80), by family:
   - editing and grading: ProRes, DNxHR, DNxHD, CineForm, Animation, uncompressed, FFV1;
   - broadcast: XDCAM, AVC-Intra, XAVC, HAP;
   - distribution: H.264, HEVC, AV1, VP9;
   - web platforms: YouTube, Vimeo, vertical 9:16, square, portrait;
   - proxies: ProRes Proxy, DNxHR LB, half-size H.264, `_proxy` suffix;
   - legacy codecs;
   - stills: JPEG, PNG, TIFF, DPX, OpenEXR, WebP;
   - sound: WAV/BWF, AIFF, FLAC, ALAC, MP3, AAC, Opus, Vorbis, AC-3;
   - without re-encoding;
   - analyses.

   A preset that the installed FFmpeg cannot do is greyed out, with the reason.
3. **Adjust the settings** by section, the **destination** and the **naming**.
4. **Start**: the list shows the progress of each file.

## Common recipes

- **Editing proxies**: Proxies preset (ProRes Proxy or DNxHR LB), destination "In a folder". Timecode and tracks kept.
- **File for a client**: H.264 or YouTube 1080p; Sound section, "streaming" normalisation.
- **Social media version**: vertical 9:16; centre cropping is done automatically.
- **Sounds for editing**: WAV; the bext and iXML metadata (scene, take, track names) are copied.
- **Extracting the sound of a video**: preset without re-encoding, sound extraction (polyphonic WAV) or each track (mono WAV named after the tracks).
- **Trimming a shot**: preset without re-encoding with trimming, in and out points per file. The PLAYER gives the timecodes.
- **Title with transparency**: ProRes 4444, Animation, PNG or TIFF keep the source alpha channel.

## Settings sections

- **Picture**: bitrate and encoder, size, aspect ratio, rotation and flips, crop, deinterlacing, frame rate, 3D LUT, brightness, contrast, saturation, gamma, colour tags (Rec. 709, 2020, 601).
- **Overlays**: timecode (from the source or set), file name, free text, logo (position, size, opacity), subtitles burnt in or as a track.
- **Sound**: tracks (all, 1 and 2, stereo mix, none), sample rate, bit depth, bitrate, normalisation (EBU R 128, ATSC A/85, streaming, podcast or free target), capped at True Peak, with no compression or limiter.
- **Processing**, depending on the preset: replacement sound aligned by timecode, frame rate conform, shot to insert, detection thresholds.
- **MXF OP-Atom (Avid)**, for DNxHD 1080 presets: one picture file and one file per sound channel, to copy into the Avid MediaFiles folder.

## Subtitles

In the Overlays section: **Edit** an SRT, WebVTT, ASS or SSA file, or **Create subtitles**.
- Start and end times, text; add, split, merge, delete, sort, global offset.
- Live readability checks: 42 characters per line, 2 lines, 17 characters per second, durations, gap between subtitles, overlaps.
- ASS and SSA: the file styles, positions and tags are kept; only times and text are edited here. To change styles, use dedicated software such as Aegisub.
- The saved file becomes the one used by the conversion.

## Destination and naming

- Next to each source file, or in a folder.
- Prefix, suffix, text replacement, numbering; a preview shows the resulting name.
- File already present: new name, skip or replace. The original is never replaced.
- Optional checks: XXH128 checksum of each file produced, VMAF quality, CSV report of the batch.

## What VERIFLOW keeps

- **Timecode**, offset with trimming.
- **Shooting metadata** of WAV files (bext, iXML), time reference recomputed when the sample rate changes or the start is trimmed.
- **Separate tracks** named after the iXML (`_A01_Boom.wav`).
- Each file is written under a temporary name, then renamed at the end.

## Personal presets

Save your settings under a name. Export and import them as `.vfpreset` files to share them between computers (without paths specific to a computer).

# Data and uninstalling

## When VERIFLOW opens

**Preferences > General > When VERIFLOW opens**:
- **Resume where I left off** (default): destinations, options and display come back as you left them;
- **Start fresh**: the working state is cleared at each opening, and the tabs take the Preferences values. Useful on a shared computer (classroom).

The working state includes: OFFLOAD destinations and options, SYNC settings, expanded folders, PLAYER display, TRANSCODE sections. Always kept: language, recent projects, favorites, TRANSCODE presets you created, audio output and Preferences.

## Clearing data

**Preferences > General > Application data**:
- **Clear working state**: the same as a fresh opening, right now. Preferences are not changed;
- **Clear all data**: everything, settings, favorites, recent projects and presets included. This cannot be undone.

A confirmation is asked, then VERIFLOW reloads. Project files, reports and copies are never deleted.

## Uninstalling

- **Windows**: Settings > Apps > VERIFLOW > Uninstall. Tick **Delete the application data** to start from a clean base when reinstalling.
- Running `setup.exe` again and choosing "Uninstall" goes on to a reinstallation: just cancel.

# Common problems

## The card does not appear as a source

- Check that it is visible in the system (File Explorer or Finder), then click **Refresh**.
- Otherwise, choose its folder with **Browse**.

## The copy is slow

- A spinning drive ("HDD") writes more slowly than an SSD. The copy writes to all destinations at once: the slowest sets the pace.
- Cables and USB ports matter: a USB 2 port strongly limits speed.
- Verification reads everything back from the drives: it takes time, it is the price of the proof.

## A copy failed

Read the cause in **Failed files**: disk full, drive unplugged, card read error. Fix it, then start the copy again: VERIFLOW offers to **Complete the copy** without copying the files already verified. Do not reuse the card until the verdict is VERIFIED.

## Ejecting is refused

A program keeps the drive open. The message names it when Windows knows it. Close File Explorer windows and players open on that drive, then eject it from the system. Antivirus or indexing can also hold it for a few seconds.

## No sound when playing

- Check the player audio output (card and channel pair).
- A track marked "LTC" is muted automatically: turn it back on if it is not timecode.
- With ASIO, close the other audio programs.

## Picture and sound out of sync

The start offset inside the file is compensated. If an offset remains, it often comes from the shoot itself: check the shot on a slate. To align a recorder sound, use SYNC.

## SYNC does not find the right sound

- Low confidence and a "waveform not similar" note: the timecode is probably wrong. Run again with the **Waveform only** method.
- The camera reference sound is missing or very weak: only the timecode method is possible.
- Check the pair in the detail view and choose another sound if needed.

## A TRANSCODE preset is greyed out

The installed FFmpeg does not include the required encoder: the reason is shown. Choose a similar preset.

## I cannot save markers or reports

A project must be open: File > New Project or Open Project.

## Reporting a problem

Give the system (Windows, macOS, Linux), the medium (card, drive), the file concerned (camera, codec), what you did and the message shown.

# Glossary

- **ALE**: Avid Log Exchange, metadata text file read by Avid and DaVinci Resolve.
- **ASC MHL**: American Society of Cinematographers standard describing a copy and its checksums, to check it later.
- **BWF**: Broadcast Wave Format, a WAV with a bext chunk that holds, among other things, the start timecode.
- **Checksum**: number computed from the content of a file; changing a single byte gives a different checksum. XXH128, MD5, SHA-256 are methods of computing it.
- **Circled take**: good take, kept for editing.
- **Drift**: gradual difference between two clocks (camera and recorder) over a shot.
- **EDL**: edit decision list (CMX3600), read by all editing software.
- **iXML**: sound recorder metadata in WAV files (scene, take, track names).
- **Jam sync**: setting several devices to the same timecode.
- **LTC**: timecode recorded as an audio signal on a sound track.
- **LUT**: colour conversion table, for example to view a Log picture in Rec. 709.
- **MOS**: shot filmed without sound.
- **Proxy**: lightweight copy of a rush, for editing on a modest computer.
- **Re-wrap**: new file that reuses the picture without re-encoding it, with another sound or other metadata.
- **Reference sound**: sound recorded by the camera, used to align the recorder sound.
- **RF64**: WAV variant for files over 4 GB.
- **RFC 3161**: standard for timestamps certified by a third-party authority.
- **Timecode (TC)**: time written in each frame or sample, in HH:MM:SS:FF format.
- **True Peak**: actual peak level of a sound, between samples.
