//! OpenTimelineIO (JSON `.otio`) : une collection de clips, sans timeline. Un
//! clip synchronisé devient une pile (`Stack.1`) avec sa piste vidéo et sa
//! piste son. Les durées sont comptées en images, à la cadence exacte du clip
//! (24000/1001 pour 23.976) ; le son est compté en échantillons.

use serde_json::{json, Value};

use crate::player::logs::{label, range_json, LogClip, Marker};

use super::model::{EditClip, EditOptions, PathMode, SyncedSound};
use super::paths::{encode, relative, url_file};

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// URL de la cible : absolue, ou relative au dossier `opts.base` (chemin en
/// pour cent, séparateurs « / »). Un média d'un autre lecteur reste absolu.
fn target_url(path: &str, opts: &EditOptions) -> String {
    if opts.paths == PathMode::Relative {
        if let Some(base) = &opts.base {
            let rel = relative(path, &base.display().to_string());
            let absolute = rel.starts_with('/')
                || rel.starts_with("//")
                || rel.as_bytes().get(1) == Some(&b':');
            if !absolute {
                return rel.split('/').map(encode).collect::<Vec<_>>().join("/");
            }
        }
    }
    url_file(path)
}

fn marker(m: &Marker, start: i64, dur: i64, rate: crate::player::timecode::FrameRate) -> Value {
    json!({
        "OTIO_SCHEMA": "Marker.2",
        "name": label(m),
        "color": m.color.upper(),
        "marked_range": range_json(start, dur, rate),
        "comment": m.comment,
        "metadata": { "veriflow": { "scene": m.scene, "take": m.take } },
    })
}

fn markers(c: &LogClip) -> Vec<Value> {
    c.markers
        .iter()
        .map(|m| match m.range() {
            Some((a, b)) => marker(m, c.start_frame + a, b - a, c.rate),
            None => marker(m, c.start_frame + m.frame, 0, c.rate),
        })
        .collect()
}

fn reference(path: &str, available: Value, opts: &EditOptions) -> Value {
    json!({
        "DEFAULT_MEDIA": {
            "OTIO_SCHEMA": "ExternalReference.1",
            "name": file_name(path),
            "target_url": target_url(path, opts),
            "available_range": available,
            "available_image_bounds": null,
            "metadata": {},
        }
    })
}

fn clip_json(
    name: &str,
    source_range: Value,
    media: Value,
    markers: Vec<Value>,
    metadata: Value,
) -> Value {
    json!({
        "OTIO_SCHEMA": "Clip.2",
        "name": name,
        "source_range": source_range,
        "media_references": media,
        "active_media_reference_key": "DEFAULT_MEDIA",
        "markers": markers,
        "effects": [],
        "enabled": true,
        "metadata": metadata,
    })
}

fn track(name: &str, kind: &str, children: Vec<Value>) -> Value {
    json!({
        "OTIO_SCHEMA": "Track.1",
        "name": name,
        "kind": kind,
        "children": children,
        "source_range": null,
        "effects": [],
        "markers": [],
        "enabled": true,
        "metadata": {},
    })
}

fn veriflow_meta(c: &EditClip) -> Value {
    json!({ "veriflow": { "reel": c.log.reel, "meta": c.meta } })
}

/// Son synchronisé : piste son avec un blanc si le son démarre après l'image.
fn sound_track(c: &EditClip, s: &SyncedSound, opts: &EditOptions) -> Value {
    let f = &s.file;
    let sr = if f.sample_rate > 0 {
        f.sample_rate
    } else {
        48_000
    };
    let srate = crate::player::timecode::FrameRate::new(sr, 1);
    let samples = |secs: f64| (secs * sr as f64).round() as i64;
    let origin = samples(f.start.unwrap_or(0.0));
    let total = samples(f.duration);
    let off = samples(s.offset);
    let video_len = samples(c.log.frame_count as f64 / c.log.rate.as_f64());
    // Son en retard : blanc de `off` ; en avance : on entre dans le son.
    let (gap, skip) = if off >= 0 { (off, 0) } else { (0, -off) };
    let len = (total - skip).min(video_len - gap).max(1);
    let path = f.path.display().to_string();
    let clip = clip_json(
        &f.name,
        range_json(origin + skip, len, srate),
        reference(&path, range_json(origin, total.max(1), srate), opts),
        vec![],
        json!({ "veriflow": { "synced_with": c.log.name } }),
    );
    let mut children = Vec::new();
    if gap > 0 {
        children.push(json!({
            "OTIO_SCHEMA": "Gap.1",
            "name": "",
            "source_range": range_json(0, gap, srate),
            "effects": [],
            "markers": [],
            "enabled": true,
            "metadata": {},
        }));
    }
    children.push(clip);
    track("A1", "Audio", children)
}

fn one(c: &EditClip, opts: &EditOptions) -> Value {
    let l = &c.log;
    let clip = clip_json(
        &l.name,
        range_json(l.start_frame, l.frame_count.max(1), l.rate),
        reference(
            &l.path,
            range_json(l.start_frame, l.frame_count.max(1), l.rate),
            opts,
        ),
        markers(l),
        veriflow_meta(c),
    );
    let Some(s) = &c.sound else { return clip };
    json!({
        "OTIO_SCHEMA": "Stack.1",
        "name": l.name,
        "children": [
            track("V1", "Video", vec![clip]),
            sound_track(c, s, opts),
        ],
        "source_range": range_json(0, l.frame_count.max(1), l.rate),
        "effects": [],
        "markers": [],
        "enabled": true,
        "metadata": veriflow_meta(c),
    })
}

/// Collection OTIO (`SerializableCollection.1`) d'un clip par entrée.
pub fn to_otio_bin(clips: &[EditClip], opts: &EditOptions) -> String {
    let doc = json!({
        "OTIO_SCHEMA": "SerializableCollection.1",
        "name": opts.title,
        "children": clips.iter().map(|c| one(c, opts)).collect::<Vec<_>>(),
        "metadata": { "veriflow": { "version": crate::VERSION } },
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editorial::testutil::{clip, marker as mk, sound};
    use crate::player::logs::Color;
    use crate::player::timecode::FrameRate;

    fn parse(clips: &[EditClip], opts: &EditOptions) -> Value {
        serde_json::from_str(&to_otio_bin(clips, opts)).expect("JSON valide")
    }

    #[test]
    fn otio_bin_parses_json() {
        let a = clip("A", FrameRate::new(24000, 1001), 86_400, false);
        let mut b = clip("B", FrameRate::new(25, 1), 0, false);
        b.sound = Some(sound(0.1));
        let v = parse(&[a, b], &EditOptions::default());
        assert_eq!(v["OTIO_SCHEMA"], "SerializableCollection.1");
        let ch = v["children"].as_array().unwrap();
        assert_eq!(ch[0]["OTIO_SCHEMA"], "Clip.2");
        let r = &ch[0]["media_references"]["DEFAULT_MEDIA"];
        assert_eq!(r["OTIO_SCHEMA"], "ExternalReference.1");
        assert_eq!(r["target_url"], "file:///m/A.mov");
        // 23.976 : cadence exacte 24000/1001, comptée en images.
        let rate = ch[0]["source_range"]["start_time"]["rate"]
            .as_f64()
            .unwrap();
        assert!((rate - 24000.0 / 1001.0).abs() < 1e-12);
        assert_eq!(ch[0]["source_range"]["start_time"]["value"], 86_400.0);
        assert_eq!(ch[0]["metadata"]["veriflow"]["reel"], "A001");
        // Clip synchronisé : pile vidéo + son avec un blanc de 0,1 s = 4800 échantillons.
        assert_eq!(ch[1]["OTIO_SCHEMA"], "Stack.1");
        let audio = &ch[1]["children"][1]["children"];
        assert_eq!(audio[0]["OTIO_SCHEMA"], "Gap.1");
        assert_eq!(audio[0]["source_range"]["duration"]["value"], 4800.0);
        assert_eq!(
            audio[1]["source_range"]["start_time"]["value"],
            172_800_000.0
        );
    }

    #[test]
    fn otio_relative_url() {
        let mut a = clip("A", FrameRate::new(25, 1), 0, false);
        a.log.path = "/rushes/jour 1/A é.mov".into();
        let mut o = EditOptions {
            paths: PathMode::Relative,
            base: Some("/rushes/export".into()),
            ..Default::default()
        };
        let v = parse(std::slice::from_ref(&a), &o);
        assert_eq!(
            v["children"][0]["media_references"]["DEFAULT_MEDIA"]["target_url"],
            "../jour%201/A%20%C3%A9.mov"
        );
        // Autre lecteur Windows : l'URL reste absolue.
        a.log.path = r"D:\Rushes\A.mov".into();
        o.base = Some(r"C:\Export".into());
        let v = parse(std::slice::from_ref(&a), &o);
        assert_eq!(
            v["children"][0]["media_references"]["DEFAULT_MEDIA"]["target_url"],
            "file:///D:/Rushes/A.mov"
        );
    }

    #[test]
    fn otio_marker_colors() {
        let mut a = clip("A", FrameRate::new(25, 1), 1000, false);
        a.log.markers = vec![
            mk(&a, 10, None, Color::Magenta),
            mk(&a, 20, Some((20, 29)), Color::Cyan),
        ];
        let v = parse(&[a], &EditOptions::default());
        let m = v["children"][0]["markers"].as_array().unwrap();
        assert_eq!(m[0]["OTIO_SCHEMA"], "Marker.2");
        assert_eq!(m[0]["color"], "MAGENTA");
        assert_eq!(m[0]["marked_range"]["start_time"]["value"], 1010.0);
        assert_eq!(m[0]["marked_range"]["duration"]["value"], 0.0);
        assert_eq!(m[1]["color"], "CYAN");
        assert_eq!(m[1]["marked_range"]["duration"]["value"], 10.0);
    }
}
