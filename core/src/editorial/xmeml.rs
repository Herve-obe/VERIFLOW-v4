//! XMEML (Final Cut Pro 7 XML, lu par Premiere Pro) : les clips arrivent dans
//! un bin, sans séquence. Les chemins sont toujours absolus (URL
//! `file://localhost/`) : XMEML n'a pas de chemin relatif fiable.
//!
//! Les durées et positions sont en images à la cadence du clip. Un son
//! synchronisé est rangé à l'image près (le format n'a pas mieux) ; l'écart
//! est signalé par `rounding_warnings`.

use std::fmt::Write;

use crate::player::logs::{label, LogClip};
use crate::player::timecode::{FrameRate, Timecode};

use super::model::{EditClip, EditOptions, SyncedSound};
use super::paths::{url_xmeml, xml_attr};
use super::Warning;

/// Écart toléré entre le décalage exact et son arrondi, en image : au-delà
/// l'arrondi est signalé.
const ROUNDING_TOLERANCE: f64 = 0.001;

fn drop_frame(c: &LogClip) -> bool {
    c.drop_frame && c.rate.supports_drop_frame()
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Décalage du son en images et écart d'arrondi (en images).
fn offset_frames(c: &LogClip, s: &SyncedSound) -> (i64, f64) {
    let exact = s.offset * c.rate.as_f64();
    (exact.round() as i64, (exact - exact.round()).abs())
}

/// Avertissement d'arrondi, une seule fois : le nombre de clips dont le son
/// synchronisé n'est pas calé sur une image exacte, leurs noms, et l'écart
/// le plus grand. Les sons non écrits (sans recouvrement) ne comptent pas.
pub fn rounding_warnings(clips: &[EditClip]) -> Vec<Warning> {
    let mut names = Vec::new();
    let mut worst = 0.0_f64;
    for c in clips {
        let Some(s) = c.sound.as_ref().filter(|s| s.attachable(&c.log)) else {
            continue;
        };
        let (_, gap) = offset_frames(&c.log, s);
        if gap > ROUNDING_TOLERANCE {
            names.push(c.log.name.clone());
            worst = worst.max(gap * 1000.0 / c.log.rate.as_f64());
        }
    }
    if names.is_empty() {
        return Vec::new();
    }
    vec![Warning::for_clips(
        "xmeml_rounding",
        format!(
            "XMEML : {} clip(s) dont le son synchronisé est arrondi à l'image (écart maximal {worst:.1} ms) : {}",
            names.len(),
            names.join(", ")
        ),
        names,
    )]
}

/// Écrivain XML indenté.
struct Xml {
    out: String,
    depth: usize,
}

impl Xml {
    fn pad(&mut self) {
        for _ in 0..self.depth {
            self.out += "  ";
        }
    }

    fn open(&mut self, tag: &str) {
        self.pad();
        let _ = writeln!(self.out, "<{tag}>");
        self.depth += 1;
    }

    /// Ouvre un élément avec un identifiant.
    fn open_id(&mut self, tag: &str, id: &str) {
        self.pad();
        let _ = writeln!(self.out, "<{tag} id=\"{}\">", xml_attr(id));
        self.depth += 1;
    }

    fn close(&mut self, tag: &str) {
        self.depth -= 1;
        self.pad();
        let _ = writeln!(self.out, "</{tag}>");
    }

    fn text(&mut self, tag: &str, value: &str) {
        self.pad();
        let _ = writeln!(self.out, "<{tag}>{}</{tag}>", xml_attr(value));
    }

    fn num(&mut self, tag: &str, value: i64) {
        self.text(tag, &value.to_string());
    }

    fn empty_id(&mut self, tag: &str, id: &str) {
        self.pad();
        let _ = writeln!(self.out, "<{tag} id=\"{}\"/>", xml_attr(id));
    }

    /// `<rate>` : base nominale et drapeau NTSC (23.976, 29.97, 59.94).
    fn rate(&mut self, r: FrameRate) {
        self.open("rate");
        self.num("timebase", r.nominal() as i64);
        self.text("ntsc", if r.is_ntsc() { "TRUE" } else { "FALSE" });
        self.close("rate");
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Video,
    Audio,
}

/// Plan d'un clipitem d'un clip maître.
struct Item {
    id: String,
    kind: Kind,
    /// Rang de la piste dans son type (1 pour la vidéo, 1..n pour le son).
    track: u32,
    /// Canal du fichier source (numéroté depuis 1), pour `sourcetrack`.
    channel: u32,
    /// Fichier source : index dans la liste des fichiers du clip.
    file: usize,
    start: i64,
    end: i64,
    tin: i64,
    tout: i64,
}

/// Fichier source d'un clip maître.
struct Source<'a> {
    id: String,
    path: String,
    rate: FrameRate,
    drop: bool,
    /// Premier timecode (images à `rate`) et durée.
    start: i64,
    duration: i64,
    reel: String,
    video: Option<&'a LogClip>,
    channels: u32,
    sample_rate: u32,
    /// Profondeur des échantillons (16 si inconnue).
    depth: u32,
}

fn file(x: &mut Xml, s: &Source) {
    x.open_id("file", &s.id);
    x.text("name", file_name(&s.path));
    x.text("pathurl", &url_xmeml(&s.path));
    x.rate(s.rate);
    x.num("duration", s.duration);
    x.open("timecode");
    x.rate(s.rate);
    x.text(
        "string",
        &Timecode::from_frames(s.start, s.rate, s.drop).to_string(),
    );
    x.num("frame", s.start);
    x.text("displayformat", if s.drop { "DF" } else { "NDF" });
    x.open("reel");
    x.text("name", &s.reel);
    x.close("reel");
    x.close("timecode");
    x.open("media");
    if let Some(v) = s.video {
        x.open("video");
        x.open("samplecharacteristics");
        x.rate(s.rate);
        x.num("width", v.width as i64);
        x.num("height", v.height as i64);
        x.close("samplecharacteristics");
        x.close("video");
    }
    if s.channels > 0 {
        x.open("audio");
        x.open("samplecharacteristics");
        x.num("depth", s.depth as i64);
        x.num("samplerate", s.sample_rate as i64);
        x.close("samplecharacteristics");
        x.num("channelcount", s.channels as i64);
        x.close("audio");
    }
    x.close("media");
    x.close("file");
}

fn meta<'a>(c: &'a EditClip, key: &str) -> &'a str {
    c.meta.get(key).map(|s| s.trim()).unwrap_or("")
}

fn circled(c: &EditClip) -> bool {
    matches!(
        meta(c, "circled").to_lowercase().as_str(),
        "true" | "1" | "oui" | "yes" | "vrai"
    )
}

/// Un clip maître du bin.
fn master(x: &mut Xml, n: usize, c: &EditClip, item_base: &mut usize) {
    let l = &c.log;
    let dur = l.frame_count.max(1);
    // Un son qui ne recouvre pas l'image n'est pas écrit (avertissement
    // `sound_no_overlap`) : le son de la caméra reste alors.
    let sound = c.sound.as_ref().filter(|s| s.attachable(l));
    let reel = l.reel.clone().unwrap_or_else(|| l.name.clone());
    let mut sources: Vec<Source> = vec![Source {
        id: format!("file-{n}"),
        path: l.path.clone(),
        rate: l.rate,
        drop: drop_frame(l),
        start: l.start_frame,
        duration: dur,
        reel,
        video: l.has_video.then_some(l),
        // Avec un son synchronisé, le son de la caméra n'est pas repris.
        channels: if sound.is_some() { 0 } else { l.audio_channels },
        sample_rate: l.audio_rate,
        depth: 16,
    }];
    let mut items: Vec<Item> = Vec::new();
    let id = |k: usize| format!("clipitem-{}", *item_base + k);
    if l.has_video {
        items.push(Item {
            id: id(items.len() + 1),
            kind: Kind::Video,
            track: 1,
            channel: 1,
            file: 0,
            start: 0,
            end: dur,
            tin: 0,
            tout: dur,
        });
    }
    match sound {
        None => {
            for ch in 1..=l.audio_channels {
                items.push(Item {
                    id: id(items.len() + 1),
                    kind: Kind::Audio,
                    track: ch,
                    channel: ch,
                    file: 0,
                    start: 0,
                    end: dur,
                    tin: 0,
                    tout: dur,
                });
            }
        }
        Some(s) => {
            let f = &s.file;
            sources.push(Source {
                id: format!("file-{n}-son"),
                path: f.path.display().to_string(),
                rate: l.rate,
                drop: drop_frame(l),
                start: (f.start.unwrap_or(0.0) * l.rate.as_f64()).round() as i64,
                duration: (f.duration * l.rate.as_f64()).round() as i64,
                // Bobine du son : le sound roll, pas la bobine de la caméra.
                reel: s.roll_name(),
                video: None,
                channels: f.channels,
                sample_rate: f.sample_rate,
                depth: s.bits.map_or(16, u32::from),
            });
            let (off, _) = offset_frames(l, s);
            let snd_len = (f.duration * l.rate.as_f64()).round() as i64;
            // Son en retard : il démarre dans le clip ; en avance : on entre dans le son.
            let (start, tin) = if off >= 0 { (off, 0) } else { (0, -off) };
            let len = (snd_len - tin).min(dur - start).max(1);
            // Rang de piste 1..n sans la piste de LTC ; le canal source reste celui du fichier.
            for (rank, ch) in s.channels().into_iter().enumerate() {
                items.push(Item {
                    id: id(items.len() + 1),
                    kind: Kind::Audio,
                    track: rank as u32 + 1,
                    channel: ch,
                    file: 1,
                    start,
                    end: start + len,
                    tin,
                    tout: tin + len,
                });
            }
        }
    }
    let mut written = vec![false; sources.len()];

    let mid = format!("masterclip-{n}");
    x.open_id("clip", &mid);
    x.text("masterclipid", &mid);
    x.text("ismasterclip", "TRUE");
    x.text("name", &l.name);
    x.num("duration", dur);
    x.rate(l.rate);
    x.num("in", -1);
    x.num("out", -1);
    for m in &l.markers {
        let text = label(m);
        x.open("marker");
        x.text(
            "name",
            if text.is_empty() {
                m.color.upper()
            } else {
                &text
            },
        );
        x.text("comment", m.color.upper());
        x.num("in", m.range().map(|r| r.0).unwrap_or(m.frame));
        // Un point n'a pas de fin (-1).
        x.num("out", m.range().map(|r| r.1).unwrap_or(-1));
        x.close("marker");
    }
    let (scene, take, note) = (meta(c, "scene"), meta(c, "take"), meta(c, "comment"));
    let good = circled(c);
    if [scene, take, note].iter().any(|s| !s.is_empty()) || good {
        x.open("logginginfo");
        for (tag, v) in [("scene", scene), ("shottake", take), ("lognote", note)] {
            if !v.is_empty() {
                x.text(tag, v);
            }
        }
        if good {
            x.text("good", "TRUE");
        }
        x.close("logginginfo");
    }
    let keywords = meta(c, "keywords");
    if !note.is_empty() || !keywords.is_empty() {
        x.open("comments");
        if !note.is_empty() {
            x.text("mastercomment1", note);
        }
        if !keywords.is_empty() {
            x.text("mastercomment2", keywords);
        }
        x.close("comments");
    }
    x.open("media");
    for kind in [Kind::Video, Kind::Audio] {
        let group: Vec<&Item> = items.iter().filter(|i| i.kind == kind).collect();
        if group.is_empty() {
            continue;
        }
        x.open(if kind == Kind::Video {
            "video"
        } else {
            "audio"
        });
        for it in group {
            x.open("track");
            x.open_id("clipitem", &it.id);
            x.text("masterclipid", &mid);
            x.text("name", &l.name);
            x.num("duration", sources[it.file].duration);
            x.rate(l.rate);
            x.num("start", it.start);
            x.num("end", it.end);
            x.num("in", it.tin);
            x.num("out", it.tout);
            if written[it.file] {
                x.empty_id("file", &sources[it.file].id);
            } else {
                written[it.file] = true;
                file(x, &sources[it.file]);
            }
            if kind == Kind::Audio {
                x.open("sourcetrack");
                x.text("mediatype", "audio");
                x.num("trackindex", it.channel as i64);
                x.close("sourcetrack");
            }
            // Chaque clipitem est relié à tous ceux du clip, lui compris.
            for other in &items {
                x.open("link");
                x.text("linkclipref", &other.id);
                x.text(
                    "mediatype",
                    if other.kind == Kind::Video {
                        "video"
                    } else {
                        "audio"
                    },
                );
                x.num("trackindex", other.track as i64);
                x.num("clipindex", 1);
                if other.kind == Kind::Audio {
                    x.num("groupindex", 1);
                }
                x.close("link");
            }
            x.close("clipitem");
            x.close("track");
        }
        x.close(if kind == Kind::Video {
            "video"
        } else {
            "audio"
        });
    }
    x.close("media");
    x.close("clip");
    *item_base += items.len();
}

/// Bin XMEML 5 : un clip maître par clip, pistes vidéo et audio reliées.
/// Les chemins sont toujours absolus (`opts.paths` est ignoré).
pub fn to_xmeml(clips: &[EditClip], opts: &EditOptions) -> String {
    let mut x = Xml {
        out: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n"),
        depth: 0,
    };
    x.pad();
    x.out += "<xmeml version=\"5\">\n";
    x.depth += 1;
    x.open("bin");
    x.text("name", &opts.title);
    x.open("children");
    let mut item_base = 0;
    for (i, c) in clips.iter().enumerate() {
        master(&mut x, i + 1, c, &mut item_base);
    }
    x.close("children");
    x.close("bin");
    x.close("xmeml");
    x.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editorial::testutil::{clip, marker, sound};
    use crate::player::logs::Color;
    use quick_xml::events::Event;
    use quick_xml::Reader;

    /// Éléments `(nom, texte, chemin)` dans l'ordre du document.
    fn parse(xml: &str) -> Vec<(String, String, Vec<String>)> {
        let mut r = Reader::from_str(xml);
        let mut stack: Vec<String> = Vec::new();
        let mut out = Vec::new();
        loop {
            match r.read_event().expect("XML mal formé") {
                Event::Start(e) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    out.push((name.clone(), String::new(), stack.clone()));
                    stack.push(name);
                }
                Event::Empty(e) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    out.push((name, String::new(), stack.clone()));
                }
                Event::Text(t) => {
                    // Le texte compte seulement juste après l'ouverture de l'élément.
                    if let Some(last) = out.last_mut().filter(|l| l.2.len() + 1 == stack.len()) {
                        last.1 += &quick_xml::escape::unescape(&t.decode().unwrap()).unwrap();
                    }
                }
                Event::GeneralRef(r) => {
                    let name = r.decode().unwrap();
                    let c = match name.as_ref() {
                        "quot" => '"',
                        "amp" => '&',
                        "lt" => '<',
                        "gt" => '>',
                        "apos" => '\'',
                        _ => r.resolve_char_ref().unwrap().expect("référence connue"),
                    };
                    if let Some(last) = out.last_mut().filter(|l| l.2.len() + 1 == stack.len()) {
                        last.1.push(c);
                    }
                }
                Event::End(_) => {
                    stack.pop();
                }
                Event::Eof => break,
                _ => {}
            }
        }
        assert!(stack.is_empty());
        out
    }

    fn texts<'a>(els: &'a [(String, String, Vec<String>)], tag: &str) -> Vec<&'a str> {
        els.iter()
            .filter(|(n, _, _)| n == tag)
            .map(|(_, t, _)| t.as_str())
            .collect()
    }

    #[test]
    fn xmeml_ntsc_flag() {
        let mut c = clip("A", FrameRate::new(24000, 1001), 86_400, false);
        c.log.audio_channels = 0;
        let els = parse(&to_xmeml(&[c], &EditOptions::default()));
        assert!(texts(&els, "ntsc").iter().all(|t| *t == "TRUE"));
        assert!(texts(&els, "timebase").iter().all(|t| *t == "24"));
        assert_eq!(texts(&els, "displayformat"), ["NDF"]);
        let c = clip("B", FrameRate::new(25, 1), 0, false);
        let els = parse(&to_xmeml(&[c], &EditOptions::default()));
        assert!(texts(&els, "ntsc").iter().all(|t| *t == "FALSE"));
        let c = clip("C", FrameRate::new(30000, 1001), 107_892, true);
        let els = parse(&to_xmeml(&[c], &EditOptions::default()));
        assert_eq!(texts(&els, "displayformat"), ["DF"]);
        assert_eq!(texts(&els, "string"), ["01:00:00;00"]);
    }

    #[test]
    fn xmeml_pathurl_windows() {
        let mut c = clip("A", FrameRate::new(25, 1), 0, false);
        c.log.path = r"C:\Rushes été\A 001.mov".into();
        let xml = to_xmeml(&[c], &EditOptions::default());
        assert!(xml.contains(
            "<pathurl>file://localhost/C%3A/Rushes%20%C3%A9t%C3%A9/A%20001.mov</pathurl>"
        ));
        assert!(xml.contains("<name>A 001.mov</name>"));
    }

    #[test]
    fn xmeml_markers_frames() {
        let mut c = clip("A", FrameRate::new(25, 1), 0, false);
        c.log.markers = vec![
            marker(&c, 10, None, Color::Red),
            marker(&c, 20, Some((20, 29)), Color::Green),
        ];
        let els = parse(&to_xmeml(&[c], &EditOptions::default()));
        let marks: Vec<_> = els.iter().filter(|(n, _, _)| n == "marker").collect();
        assert_eq!(marks.len(), 2);
        let ins: Vec<_> = els
            .iter()
            .filter(|(n, _, p)| {
                (n == "in" || n == "out") && p.last().is_some_and(|l| l == "marker")
            })
            .map(|(_, t, _)| t.as_str())
            .collect();
        // Point : fin -1 ; plage : fin exclusive.
        assert_eq!(ins, ["10", "-1", "20", "30"]);
    }

    #[test]
    fn xmeml_merged_links() {
        let mut c = clip("A", FrameRate::new(25, 1), 0, false);
        c.sound = Some(sound(0.5));
        let xml = to_xmeml(std::slice::from_ref(&c), &EditOptions::default());
        let els = parse(&xml);
        // 1 vidéo + 2 canaux du son, chacun relié aux trois.
        assert_eq!(texts(&els, "linkclipref").len(), 9);
        let items: Vec<_> = els.iter().filter(|(n, _, _)| n == "clipitem").collect();
        assert_eq!(items.len(), 3);
        assert!(xml.contains("<file id=\"file-1-son\">"));
        assert!(xml.contains("<channelcount>2</channelcount>"));
        // Son décalé de 0,5 s = 12,5 images : arrondi à 13 (arrondi au plus loin), signalé.
        assert!(xml.contains("<start>13</start>"));
        let w = rounding_warnings(&[c.clone()]);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, "xmeml_rounding");
        assert_eq!(w[0].clips, ["A"]);
        assert!(w[0].detail.contains("1 clip(s)") && w[0].detail.ends_with(": A"));
        c.sound = Some(sound(0.48));
        assert!(rounding_warnings(&[c]).is_empty());
    }

    #[test]
    fn xmeml_wellformed() {
        let mut a = clip("A & <B>", FrameRate::new(30000, 1001), 107_892, true);
        a.meta.insert("scene".into(), "12\"A".into());
        a.meta.insert("circled".into(), "true".into());
        a.meta.insert("comment".into(), "ligne 1\nligne 2".into());
        a.log.markers = vec![marker(&a, 3, None, Color::Blue)];
        let mut b = clip("B", FrameRate::new(25, 1), 0, false);
        b.sound = Some(sound(-0.2));
        let mut w = clip("W", FrameRate::new(25, 1), 0, false);
        w.log.has_video = false;
        w.log.path = "/m/W.wav".into();
        let xml = to_xmeml(&[a, b, w], &EditOptions::default());
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>"));
        let els = parse(&xml);
        assert_eq!(texts(&els, "good"), ["TRUE"]);
        assert_eq!(texts(&els, "scene"), ["12\"A"]);
        // Chaque référence de lien désigne un clipitem existant.
        let ids: Vec<String> = xml
            .match_indices("<clipitem id=\"")
            .map(|(i, m)| {
                let rest = &xml[i + m.len()..];
                rest[..rest.find('"').unwrap()].to_owned()
            })
            .collect();
        for r in texts(&els, "linkclipref") {
            assert!(ids.iter().any(|i| i == r), "lien orphelin {r}");
        }
        // Son en avance : on entre dans le son de 5 images.
        assert!(xml.contains("<in>5</in>"));
    }

    #[test]
    fn xmeml_link_trackindex_is_track_rank() {
        let mut c = clip("A", FrameRate::new(25, 1), 0, false);
        let mut snd = sound(0.0);
        // Canaux 1, 2 (LTC) et 3 : pistes 1 et 2, canaux source 1 et 3.
        snd.file.channels = 3;
        snd.file.ltc_channel = Some(1);
        snd.roll = Some("ROLL07".into());
        snd.bits = Some(24);
        c.sound = Some(snd);
        let xml = to_xmeml(&[c], &EditOptions::default());
        let els = parse(&xml);
        let link_tracks: Vec<&str> = els
            .iter()
            .filter(|(n, _, p)| n == "trackindex" && p.last().is_some_and(|l| l == "link"))
            .map(|(_, t, _)| t.as_str())
            .collect();
        // Trois clipitems (1 vidéo, 2 sons), chacun relié aux trois.
        assert_eq!(link_tracks, ["1", "1", "2"].repeat(3));
        let source: Vec<&str> = els
            .iter()
            .filter(|(n, _, p)| n == "trackindex" && p.last().is_some_and(|l| l == "sourcetrack"))
            .map(|(_, t, _)| t.as_str())
            .collect();
        assert_eq!(source, ["1", "3"]);
        // Bobine du son et profondeur réelle.
        assert!(xml.contains("<name>ROLL07</name>"));
        assert!(xml.contains("<depth>24</depth>"));
        // Le son de la caméra n'est pas repris : une seule profondeur.
        assert_eq!(xml.matches("<depth>").count(), 1);
    }

    #[test]
    fn xmeml_sound_without_overlap_keeps_camera_audio() {
        let mut c = clip("A", FrameRate::new(25, 1), 0, false);
        c.sound = Some(sound(30.0));
        let xml = to_xmeml(std::slice::from_ref(&c), &EditOptions::default());
        assert!(!xml.contains("file-1-son"));
        // 1 vidéo + 2 canaux de la caméra.
        assert_eq!(xml.matches("<clipitem id=").count(), 3);
        assert!(rounding_warnings(&[c]).is_empty());
    }

    #[test]
    fn xmeml_rounding_single_warning_for_all_clips() {
        let mut a = clip("A", FrameRate::new(25, 1), 0, false);
        a.sound = Some(sound(0.5));
        let mut b = clip("B", FrameRate::new(25, 1), 0, false);
        b.sound = Some(sound(0.5));
        let w = rounding_warnings(&[a, b]);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].clips, ["A", "B"]);
        assert!(w[0].detail.contains("2 clip(s)"));
    }
}
