//! FCPXML (Final Cut Pro, DaVinci Resolve, Premiere via import) : les clips
//! arrivent dans un événement, sans projet ni séquence. Les chemins sont
//! toujours absolus (URL `file://`) : FCPXML n'a pas de chemin relatif.

use crate::player::logs::{fcp_time, label, LogClip, Marker};
use crate::player::timecode::FrameRate;

use super::model::{EditClip, EditOptions, FcpxmlVersion, SyncedSound};
use super::paths::{url_file, xml_attr};

/// Durée en échantillons : fraction exacte de seconde au pas de 1/`rate`.
fn sample_time(samples: i64, rate: u32) -> String {
    fcp_time(samples, FrameRate::new(rate.max(1), 1))
}

fn samples(seconds: f64, rate: u32) -> i64 {
    (seconds * rate as f64).round() as i64
}

/// Durée d'une image au format XML : au moins une image.
fn dur_frames(c: &LogClip) -> i64 {
    c.frame_count.max(1)
}

fn tc_format(c: &LogClip) -> &'static str {
    if c.drop_frame && c.rate.supports_drop_frame() {
        "DF"
    } else {
        "NDF"
    }
}

/// Source du média : attribut `src` jusqu'à la 1.9, `media-rep` ensuite.
fn asset(version: FcpxmlVersion, id: &str, attrs: &str, path: &str) -> String {
    let url = xml_attr(&url_file(path));
    if version == FcpxmlVersion::V1_9 {
        format!("    <asset id=\"{id}\" {attrs} src=\"{url}\"/>\n")
    } else {
        format!(
            "    <asset id=\"{id}\" {attrs}>\n      <media-rep kind=\"original-media\" src=\"{url}\"/>\n    </asset>\n"
        )
    }
}

fn truthy(s: Option<&String>) -> bool {
    s.is_some_and(|s| {
        matches!(
            s.trim().to_lowercase().as_str(),
            "true" | "1" | "oui" | "yes" | "vrai"
        )
    })
}

/// Marqueur de plage ou de point : début en images depuis le début du clip,
/// durée en images.
fn marker_span(m: &Marker) -> (i64, i64) {
    match m.range() {
        Some((a, b)) => (a, (b - a).max(1)),
        None => (m.frame, 1),
    }
}

/// Éléments portés par le clip : marqueurs, mot-clés, prise cerclée, puis
/// métadonnées (ordre imposé par le DTD).
fn extras(c: &EditClip, opts: &EditOptions, ind: &str) -> String {
    let l = &c.log;
    let mut out = String::new();
    if opts.markers {
        for m in &l.markers {
            let (from, len) = marker_span(m);
            let value = match label(m) {
                v if v.is_empty() => "Marqueur".to_owned(),
                v => v,
            };
            out += &format!(
                "{ind}<marker start=\"{}\" duration=\"{}\" value=\"{}\" note=\"{}\"/>\n",
                fcp_time(l.start_frame + from, l.rate),
                fcp_time(len, l.rate),
                xml_attr(&value),
                xml_attr(&format!("Couleur : {}", m.color.id())),
            );
        }
    }
    let span = format!(
        "start=\"{}\" duration=\"{}\"",
        fcp_time(l.start_frame, l.rate),
        fcp_time(dur_frames(l), l.rate)
    );
    if let Some(k) = c.meta.get("keywords").filter(|k| !k.trim().is_empty()) {
        out += &format!("{ind}<keyword {span} value=\"{}\"/>\n", xml_attr(k.trim()));
    }
    if truthy(c.meta.get("circled")) {
        out += &format!("{ind}<rating {span} value=\"favorite\"/>\n");
    }
    let mut md = String::new();
    let mut put = |key: &str, value: Option<&str>| {
        if let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) {
            md += &format!(
                "{ind}  <md key=\"com.apple.proapps.studio.{key}\" value=\"{}\"/>\n",
                xml_attr(v)
            );
        }
    };
    put("reel", l.reel.as_deref());
    put("scene", c.meta.get("scene").map(String::as_str));
    put("shot", c.meta.get("shot").map(String::as_str));
    put("angle", c.meta.get("angle").map(String::as_str));
    put("cameraName", c.meta.get("camera").map(String::as_str));
    if !md.is_empty() {
        out += &format!("{ind}<metadata>\n{md}{ind}</metadata>\n");
    }
    out
}

/// Note du clip (commentaire de MEDIA), à placer en premier.
fn note(c: &EditClip, ind: &str) -> String {
    match c.meta.get("comment").map(|s| s.trim()) {
        Some(n) if !n.is_empty() => format!("{ind}<note>{}</note>\n", xml_attr(n)),
        _ => String::new(),
    }
}

/// Son attaché : décalage au pas de 1/`sample_rate` s, départ d'après l'heure
/// du fichier (time reference BWF). Rien si le son ne recouvre pas l'image.
fn attached_sound(c: &LogClip, s: &SyncedSound, id: &str) -> Option<String> {
    let sr = s.file.sample_rate.max(1);
    let v_sec = c.start_frame as f64 / c.rate.as_f64();
    let v_dur = dur_frames(c) as f64 / c.rate.as_f64();
    let a_start = samples(s.file.start.unwrap_or(0.0), sr);
    let lead = s.offset.max(0.0);
    let skip = (-s.offset).max(0.0);
    let dur = (v_dur - lead).min(s.file.duration - skip);
    if dur <= 0.0 {
        return None;
    }
    Some(format!(
        "        <asset-clip ref=\"{id}\" lane=\"-1\" offset=\"{}\" name=\"{}\" start=\"{}\" duration=\"{}\" audioRole=\"dialogue\"/>\n",
        sample_time(samples(v_sec + lead, sr), sr),
        xml_attr(&s.file.name),
        sample_time(a_start + samples(skip, sr), sr),
        sample_time(samples(dur, sr).max(1), sr),
    ))
}

/// FCPXML : une bibliothèque, un événement, un clip par entrée (`asset-clip`,
/// ou `sync-clip` quand un son d'enregistreur est synchronisé), sans projet.
pub fn to_fcpxml(clips: &[EditClip], opts: &EditOptions) -> String {
    let version = opts.fcpxml_version;
    // Un format par couple (taille, cadence), pour les clips vidéo.
    let mut formats: Vec<(u32, u32, FrameRate)> = Vec::new();
    let key = |c: &LogClip| {
        (
            if c.width > 0 { c.width } else { 1920 },
            if c.height > 0 { c.height } else { 1080 },
            c.rate,
        )
    };
    for c in clips.iter().filter(|c| c.log.has_video) {
        let k = key(&c.log);
        if !formats.contains(&k) {
            formats.push(k);
        }
    }
    let mut res = String::new();
    for (i, (w, h, r)) in formats.iter().enumerate() {
        res += &format!(
            "    <format id=\"r{}\" frameDuration=\"{}\" width=\"{w}\" height=\"{h}\"/>\n",
            i + 1,
            fcp_time(1, *r)
        );
    }
    // Un même son peut servir à plusieurs clips : une seule ressource par fichier.
    let mut sounds: Vec<&std::path::Path> = Vec::new();
    let mut body = String::new();
    for (i, e) in clips.iter().enumerate() {
        let c = &e.log;
        let id = format!("a{}", i + 1);
        let fmt = if c.has_video {
            let k = key(c);
            let n = formats.iter().position(|f| *f == k).unwrap_or(0) + 1;
            format!(" format=\"r{n}\"")
        } else {
            String::new()
        };
        let audio = if c.audio_channels > 0 {
            format!(
                " hasAudio=\"1\" audioSources=\"1\" audioChannels=\"{}\" audioRate=\"{}\"",
                c.audio_channels,
                c.audio_rate.max(1)
            )
        } else {
            String::new()
        };
        res += &asset(
            version,
            &id,
            &format!(
                "name=\"{}\" start=\"{}\" duration=\"{}\" hasVideo=\"{}\"{fmt}{audio}",
                xml_attr(&c.name),
                fcp_time(c.start_frame, c.rate),
                fcp_time(dur_frames(c), c.rate),
                u8::from(c.has_video),
            ),
            &c.path,
        );
        let head = format!(
            "name=\"{}\" start=\"{}\" duration=\"{}\" tcFormat=\"{}\"",
            xml_attr(&c.name),
            fcp_time(c.start_frame, c.rate),
            fcp_time(dur_frames(c), c.rate),
            tc_format(c)
        );
        let attached = match (&e.sound, c.has_video) {
            (Some(s), true) => {
                let sr = s.file.sample_rate.max(1);
                let k = match sounds.iter().position(|p| *p == s.file.path.as_path()) {
                    Some(k) => k,
                    None => {
                        sounds.push(&s.file.path);
                        let k = sounds.len();
                        res += &asset(
                            version,
                            &format!("s{k}"),
                            &format!(
                                "name=\"{}\" start=\"{}\" duration=\"{}\" hasAudio=\"1\" audioSources=\"1\" audioChannels=\"{}\" audioRate=\"{sr}\"",
                                xml_attr(&s.file.name),
                                sample_time(samples(s.file.start.unwrap_or(0.0), sr), sr),
                                sample_time(samples(s.file.duration, sr).max(1), sr),
                                s.file.channels.max(1),
                            ),
                            &s.file.path.to_string_lossy(),
                        );
                        k
                    }
                };
                attached_sound(c, s, &format!("s{k}"))
            }
            _ => None,
        };
        match attached {
            Some(snd) => {
                body += &format!(
                    "      <sync-clip offset=\"0s\" {head}>\n{}        <asset-clip ref=\"{id}\" offset=\"{}\" {head}/>\n{snd}{}      </sync-clip>\n",
                    note(e, "        "),
                    fcp_time(c.start_frame, c.rate),
                    extras(e, opts, "        "),
                );
            }
            None => {
                let inner = format!("{}{}", note(e, "        "), extras(e, opts, "        "));
                if inner.is_empty() {
                    body += &format!("      <asset-clip ref=\"{id}\" offset=\"0s\" {head}/>\n");
                } else {
                    body += &format!(
                        "      <asset-clip ref=\"{id}\" offset=\"0s\" {head}>\n{inner}      </asset-clip>\n"
                    );
                }
            }
        }
    }
    let title = if opts.title.trim().is_empty() {
        "VERIFLOW"
    } else {
        opts.title.trim()
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"{}\">\n  <resources>\n{res}  </resources>\n  <library>\n    <event name=\"{}\">\n{body}    </event>\n  </library>\n</fcpxml>\n",
        version.as_str(),
        xml_attr(title),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::logs::Color;
    use crate::sync::{SyncFile, TimeSource};
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use std::collections::{BTreeMap, BTreeSet};

    fn clip(name: &str, rate: FrameRate, start: i64, drop: bool) -> EditClip {
        EditClip {
            log: LogClip {
                path: format!("/m/{name}.mov"),
                name: name.into(),
                reel: Some("A001".into()),
                start_frame: start,
                frame_count: 100,
                rate,
                drop_frame: drop,
                width: 1920,
                height: 1080,
                has_video: true,
                audio_channels: 2,
                audio_rate: 48000,
                markers: vec![],
            },
            meta: BTreeMap::new(),
            sound: None,
        }
    }

    fn wav(offset: f64) -> SyncedSound {
        SyncedSound {
            file: SyncFile {
                path: "/m/son.wav".into(),
                name: "son".into(),
                video: false,
                start: Some(3600.0),
                source: TimeSource::Metadata,
                timecode: None,
                duration: 60.0,
                rate: None,
                width: 0,
                height: 0,
                sample_rate: 48000,
                channels: 2,
                ltc_channel: None,
                tracks: vec![],
                error: None,
            },
            offset,
        }
    }

    /// Éléments avec leurs attributs, dans l'ordre du document.
    fn parse(xml: &str) -> Vec<(String, BTreeMap<String, String>)> {
        let mut r = Reader::from_str(xml);
        let mut out = Vec::new();
        loop {
            match r.read_event().expect("XML mal formé") {
                Event::Start(e) | Event::Empty(e) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    let attrs = e
                        .attributes()
                        .map(|a| {
                            let a = a.unwrap();
                            (
                                String::from_utf8_lossy(a.key.as_ref()).into_owned(),
                                a.unescape_value().unwrap().into_owned(),
                            )
                        })
                        .collect();
                    out.push((name, attrs));
                }
                Event::Eof => break,
                _ => {}
            }
        }
        out
    }

    fn find<'a>(
        els: &'a [(String, BTreeMap<String, String>)],
        name: &str,
    ) -> Vec<&'a BTreeMap<String, String>> {
        els.iter()
            .filter(|(n, _)| n == name)
            .map(|(_, a)| a)
            .collect()
    }

    #[test]
    fn fcpxml_event_without_project() {
        let x = to_fcpxml(
            &[clip("A", FrameRate::new(25, 1), 0, false)],
            &EditOptions::default(),
        );
        let els = parse(&x);
        assert_eq!(find(&els, "event").len(), 1);
        assert!(find(&els, "project").is_empty() && find(&els, "sequence").is_empty());
        assert_eq!(find(&els, "asset-clip").len(), 1);
    }

    #[test]
    fn fcpxml_version_attr() {
        let c = [clip("A", FrameRate::new(25, 1), 0, false)];
        for v in [
            FcpxmlVersion::V1_9,
            FcpxmlVersion::V1_10,
            FcpxmlVersion::V1_11,
        ] {
            let o = EditOptions {
                fcpxml_version: v,
                ..Default::default()
            };
            let els = parse(&to_fcpxml(&c, &o));
            assert_eq!(find(&els, "fcpxml")[0]["version"], v.as_str());
            // media-rep depuis la 1.10, attribut src avant.
            assert_eq!(
                find(&els, "media-rep").len(),
                usize::from(v != FcpxmlVersion::V1_9)
            );
            assert_eq!(
                find(&els, "asset")[0].contains_key("src"),
                v == FcpxmlVersion::V1_9
            );
        }
    }

    #[test]
    fn fcpxml_2997df_times() {
        let c = [clip("A", FrameRate::new(30000, 1001), 107_894, true)];
        let els = parse(&to_fcpxml(&c, &EditOptions::default()));
        let ac = find(&els, "asset-clip")[0];
        assert_eq!(ac["tcFormat"], "DF");
        for k in ["start", "duration"] {
            let t = ac[k].trim_end_matches('s');
            let (n, d) = t.split_once('/').unwrap();
            let (n, d): (u64, u64) = (n.parse().unwrap(), d.parse().unwrap());
            // Un multiple entier de 1001/30000 s : n * 30000 = images * 1001 * d.
            assert_eq!((n * 30000) % (1001 * d), 0, "{k} = {}", ac[k]);
        }
    }

    #[test]
    fn fcpxml_2398_start() {
        let c = [clip("A", FrameRate::new(24000, 1001), 86_400, false)];
        let els = parse(&to_fcpxml(&c, &EditOptions::default()));
        let ac = find(&els, "asset-clip")[0];
        assert_eq!(ac["tcFormat"], "NDF");
        // 86400 images à 1001/24000 s = 86486400/24000 s = 3603.6 s.
        assert_eq!(ac["start"], "18018/5s");
        assert_eq!(find(&els, "format")[0]["frameDuration"], "1001/24000s");
    }

    #[test]
    fn fcpxml_sync_clip_offset_sample() {
        let mut e = clip("A", FrameRate::new(25, 1), 0, false);
        // 3 échantillons de retard : 3/48000 s = 1/16000 s.
        e.sound = Some(wav(3.0 / 48000.0));
        let els = parse(&to_fcpxml(&[e.clone()], &EditOptions::default()));
        assert_eq!(find(&els, "sync-clip").len(), 1);
        let snd = find(&els, "asset-clip")
            .into_iter()
            .find(|a| a.contains_key("lane"))
            .unwrap();
        assert_eq!(snd["lane"], "-1");
        assert_eq!(snd["offset"], "1/16000s");
        // Le son démarre à l'heure du fichier (3600 s).
        assert_eq!(snd["start"], "3600s");
        // Son en avance : on entre dans le son, le clip commence à l'image 0.
        e.sound = Some(wav(-0.5));
        let els = parse(&to_fcpxml(&[e], &EditOptions::default()));
        let snd = find(&els, "asset-clip")
            .into_iter()
            .find(|a| a.contains_key("lane"))
            .unwrap();
        assert_eq!(snd["offset"], "0s");
        assert_eq!(snd["start"], "7201/2s");
    }

    #[test]
    fn fcpxml_wellformed_refs() {
        let mut a = clip("A", FrameRate::new(25, 1), 0, false);
        a.sound = Some(wav(0.25));
        a.meta.insert("scene".into(), "12 \"bis\"".into());
        a.meta.insert("camera".into(), "A".into());
        a.meta.insert("comment".into(), "bon <plan>\nsuite".into());
        a.meta.insert("keywords".into(), "ext, nuit".into());
        a.meta.insert("circled".into(), "true".into());
        a.log.markers.push(Marker {
            id: 0,
            path: a.log.path.clone(),
            frame: 10,
            in_frame: Some(10),
            out_frame: Some(19),
            color: Color::Red,
            comment: "r&d".into(),
            scene: String::new(),
            take: String::new(),
        });
        let mut b = clip("B", FrameRate::new(30000, 1001), 0, true);
        b.log.width = 3840;
        b.log.height = 2160;
        let mut son = clip("Son", FrameRate::new(25, 1), 0, false);
        son.log.has_video = false;
        son.log.width = 0;
        let x = to_fcpxml(&[a, b, son], &EditOptions::default());
        let els = parse(&x);
        let ids: BTreeSet<&str> = els
            .iter()
            .filter(|(n, _)| n == "asset" || n == "format")
            .map(|(_, a)| a["id"].as_str())
            .collect();
        let mut refs = 0;
        for (_, a) in &els {
            for k in ["ref", "format"] {
                if let Some(v) = a.get(k) {
                    refs += 1;
                    assert!(ids.contains(v.as_str()), "{k}={v} sans ressource");
                }
            }
        }
        assert!(refs >= 6);
        assert_eq!(find(&els, "format").len(), 2);
        assert_eq!(find(&els, "marker")[0]["note"], "Couleur : red");
        assert_eq!(find(&els, "marker")[0]["duration"], "2/5s");
        assert_eq!(find(&els, "rating")[0]["value"], "favorite");
        assert_eq!(find(&els, "keyword")[0]["value"], "ext, nuit");
        let md = find(&els, "md");
        assert!(md
            .iter()
            .any(|m| m["key"].ends_with(".cameraName") && m["value"] == "A"));
        assert!(md
            .iter()
            .any(|m| m["key"].ends_with(".scene") && m["value"] == "12 \"bis\""));
        assert!(x.contains("<note>bon &lt;plan&gt;&#10;suite</note>"));
    }
}
