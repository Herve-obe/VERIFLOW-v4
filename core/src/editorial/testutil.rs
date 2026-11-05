//! Clips d'exemple partagés par les tests des formats.

use std::collections::BTreeMap;

use crate::player::logs::{Color, LogClip, Marker};
use crate::player::timecode::FrameRate;
use crate::sync::{SyncFile, TimeSource};

use super::model::{EditClip, SyncedSound};

/// Clip vidéo de 100 images, bobine A001.
pub fn clip(name: &str, rate: FrameRate, start: i64, drop: bool) -> EditClip {
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

/// Son d'enregistreur de 60 s, deux canaux, timecode 01:00:00.
pub fn sound(offset: f64) -> SyncedSound {
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

pub fn marker(clip: &EditClip, frame: i64, range: Option<(i64, i64)>, color: Color) -> Marker {
    Marker {
        id: 0,
        path: clip.log.path.clone(),
        frame,
        in_frame: range.map(|r| r.0),
        out_frame: range.map(|r| r.1),
        color,
        comment: "Bonne prise".into(),
        scene: String::new(),
        take: String::new(),
    }
}
