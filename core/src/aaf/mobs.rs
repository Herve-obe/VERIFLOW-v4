//! Mobs AAF : média lié (fichier externe, sans essence copiée) et éléments
//! de montage (séquences, clips, blancs, timecode).
//!
//! Un fichier lié donne trois mobs, comme le lien AMA de Media Composer :
//! une source « bande » (avec son timecode), une source fichier (descripteur
//! et emplacement), et le master clip que la séquence utilise.

use std::path::Path;

use sha2::{Digest, Sha256};

use super::{
    auid, i64le, indirect_string, rational, string, timestamp, u16le, u32le, Object, DEF_KEY_PID,
    REF_CONTAINER_DEFS, REF_DATA_DEFS,
};

/// Identifiant de mob (UMID SMPTE 330M, 32 octets).
pub type MobId = [u8; 32];

pub const NULL_MOB: MobId = [0; 32];

/// Nouvel identifiant : en-tête UMID fixe, numéro de matériel unique.
pub fn mob_id(seed: &str) -> MobId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let hash = Sha256::digest(format!("{seed}|{now}|{n}|{}", std::process::id()));
    let mut id = [0u8; 32];
    id[..16].copy_from_slice(&[
        0x06, 0x0a, 0x2b, 0x34, 0x01, 0x01, 0x01, 0x05, 0x01, 0x01, 0x0f, 0x20, 0x13, 0, 0, 0,
    ]);
    id[16..].copy_from_slice(&hash[..16]);
    id
}

/// AUID aléatoire (même principe que `mob_id`).
pub fn new_auid(seed: &str) -> [u8; 16] {
    let id = mob_id(seed);
    let mut out = [0u8; 16];
    out.copy_from_slice(&id[16..]);
    // Version 4, variante RFC 4122 (octets enregistrés en petit-boutiste).
    out[7] = (out[7] & 0x0f) | 0x40;
    out[8] = (out[8] & 0x3f) | 0x80;
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Picture,
    Sound,
    Timecode,
}

impl Kind {
    fn def(self) -> Vec<u8> {
        auid(match self {
            Kind::Picture => "01030202-0100-0000-060e-2b3404010101",
            Kind::Sound => "01030202-0200-0000-060e-2b3404010101",
            Kind::Timecode => "01030201-0100-0000-060e-2b3404010101",
        })
        .to_vec()
    }
}

/// Cadence (numérateur, dénominateur).
pub type Rate = (i32, i32);

/// Composant : longueur en unités de la piste qui le contient.
pub struct Component {
    pub obj: Object,
    pub length: i64,
}

fn component(class: &str, kind: Kind, length: i64) -> Object {
    Object::new(class)
        .weak(0x0201, REF_DATA_DEFS, DEF_KEY_PID, kind.def())
        .data(0x0202, i64le(length))
}

pub fn source_clip(kind: Kind, length: i64, start: i64, source: &MobId, slot: u32) -> Component {
    Component {
        obj: component("0d010101-0101-1100-060e-2b3402060101", kind, length)
            .data(0x1201, i64le(start))
            .data(0x1101, source.to_vec())
            .data(0x1102, u32le(slot)),
        length,
    }
}

pub fn filler(kind: Kind, length: i64) -> Component {
    Component {
        obj: component("0d010101-0101-0900-060e-2b3402060101", kind, length),
        length,
    }
}

pub fn sequence(kind: Kind, parts: Vec<Component>) -> Component {
    let length = parts.iter().map(|c| c.length).sum();
    Component {
        obj: component("0d010101-0101-0f00-060e-2b3402060101", kind, length).vector(
            0x1001,
            "Components",
            parts.into_iter().map(|c| c.obj).collect(),
        ),
        length,
    }
}

/// Timecode continu : `start` en images depuis 00:00:00:00.
pub fn timecode(length: i64, start: i64, fps: u16, drop: bool) -> Component {
    Component {
        obj: component(
            "0d010101-0101-1400-060e-2b3402060101",
            Kind::Timecode,
            length,
        )
        .data(0x1501, i64le(start))
        .data(0x1502, u16le(fps))
        .data(0x1503, vec![u8::from(drop)]),
        length,
    }
}

/// Piste (TimelineMobSlot) : un segment, une cadence.
pub fn slot(id: u32, name: &str, rate: Rate, segment: Component, track: Option<u32>) -> Object {
    let mut o = Object::new("0d010101-0101-3b00-060e-2b3402060101")
        .data(0x4801, u32le(id))
        .data(0x4802, string(name))
        .data(0x4b02, i64le(0))
        .data(0x4b01, rational(rate.0, rate.1))
        .strong(0x4803, "Segment", segment.obj);
    if let Some(n) = track {
        o = o.data(0x4804, u32le(n));
    }
    o
}

/// Mob : composition (séquence), master clip ou source.
pub struct Mob {
    pub id: MobId,
    pub obj: Object,
}

pub const COMPOSITION: &str = "0d010101-0101-3500-060e-2b3402060101";
pub const MASTER: &str = "0d010101-0101-3600-060e-2b3402060101";
pub const SOURCE: &str = "0d010101-0101-3700-060e-2b3402060101";

pub fn mob(class: &str, id: MobId, name: &str, slots: Vec<Object>) -> Mob {
    let now = timestamp(&chrono::Local::now());
    Mob {
        id,
        obj: Object::new(class)
            .data(0x4402, string(name))
            .data(0x4401, id.to_vec())
            .data(0x4405, now.clone())
            .data(0x4404, now)
            .vector(0x4403, "Slots", slots),
    }
}

/// Commentaire de mob (TaggedValue).
pub fn comment(name: &str, value: &str) -> Object {
    Object::new("0d010101-0101-3f00-060e-2b3402060101")
        .data(0x5001, string(name))
        .data(0x5003, indirect_string(value))
}

fn locator(url: &str) -> Object {
    Object::new("0d010101-0101-3200-060e-2b3402060101").data(0x4001, string(url))
}

/// Image d'un fichier lié.
pub struct Picture {
    pub rate: Rate,
    pub frames: i64,
    pub width: u32,
    pub height: u32,
    /// Nom du codec selon FFmpeg (« prores », « h264 »...).
    pub codec: String,
    /// Format de pixel selon FFmpeg (« yuv422p10le »...).
    pub pix_fmt: String,
}

/// Flux son d'un fichier lié.
pub struct Sound {
    pub sample_rate: u32,
    pub channels: u32,
    pub bits: u32,
    /// Longueur en échantillons.
    pub length: i64,
}

/// Fichier à lier.
pub struct Media<'a> {
    pub path: &'a Path,
    pub url: String,
    pub name: String,
    pub picture: Option<Picture>,
    pub sounds: Vec<Sound>,
    /// Résumé du WAV (WAVEDescriptor) ; sinon lien générique.
    pub wave: Option<Vec<u8>>,
    /// Timecode de début : images depuis minuit, cadence de comptage, drop.
    pub timecode: Option<(i64, Rate, bool)>,
}

/// Fichier lié : mobs à ajouter au contenu et pistes du master clip.
pub struct Linked {
    pub mobs: Vec<Mob>,
    pub master: MobId,
    /// Piste image du master clip.
    pub picture: Option<u32>,
    /// Pistes son du master clip, une par canal, et leur cadence.
    pub sounds: Vec<(u32, Rate)>,
}

fn pixel_layout(pix_fmt: &str) -> (u32, u32, u32) {
    let depth = [16, 12, 10]
        .into_iter()
        .find(|d| pix_fmt.contains(&format!("p{d}")))
        .unwrap_or(8);
    let (h, v) =
        if pix_fmt.contains("444") || pix_fmt.starts_with("rgb") || pix_fmt.starts_with("gbr") {
            (1, 1)
        } else if pix_fmt.contains("420") {
            (2, 2)
        } else {
            (2, 1)
        };
    (depth, h, v)
}

fn picture_descriptor(p: &Picture, url: &str) -> Object {
    let (depth, h, v) = pixel_layout(&p.pix_fmt);
    Object::new("0d010101-0101-2800-060e-2b3402060101")
        .data(0x3301, u32le(depth))
        .data(0x3302, u32le(h))
        .data(0x3308, u32le(v))
        .data(0x320c, vec![0]) // FullFrame
        .data(
            0x320d,
            [0i32, 0].iter().flat_map(|x| x.to_le_bytes()).collect(),
        )
        .data(0x320e, rational(p.width as i32, p.height.max(1) as i32))
        .data(0x3203, u32le(p.width))
        .data(0x3202, u32le(p.height))
        .data(0x3001, rational(p.rate.0, p.rate.1))
        .data(
            0x3201,
            auid("04010202-0000-0000-060e-2b3404010101").to_vec(),
        )
        .data(0x3002, i64le(p.frames))
        .vector(0x2f01, "Locator", vec![locator(url)])
}

fn sound_descriptor(s: &Sound, url: &str) -> Object {
    let block = s.channels.max(1) * s.bits.max(8).div_ceil(8);
    Object::new("0d010101-0101-4800-060e-2b3402060101")
        .data(0x3001, rational(s.sample_rate as i32, 1))
        .data(0x3d03, rational(s.sample_rate as i32, 1))
        .data(0x3d07, u32le(s.channels))
        .data(0x3d09, u32le(s.sample_rate * block))
        .data(0x3d01, u32le(s.bits))
        .data(0x3d0a, u16le(block as u16))
        .data(0x3002, i64le(s.length))
        .data(
            0x3d06,
            auid("04020202-0000-0000-060e-2b3404010101").to_vec(),
        )
        .vector(0x2f01, "Locator", vec![locator(url)])
}

/// Conteneur générique des liens AMA (QuickTime, MXF, MP4...).
const AMA_GENERIC: &str = "b22697a2-3442-44e8-bb8f-7a1cd290ebf1";
/// Pid de MultipleDescriptor::MediaContainerGUID (extension Avid du gabarit).
const MEDIA_CONTAINER_GUID: u16 = 0xff8d;

/// Mobs d'un fichier externe.
pub fn link(m: &Media) -> Linked {
    let url = &m.url;
    // Pistes : image, puis un canal par piste son.
    let mut tracks: Vec<(Kind, Rate, i64)> = Vec::new();
    if let Some(p) = &m.picture {
        tracks.push((Kind::Picture, p.rate, p.frames));
    }
    for s in &m.sounds {
        for _ in 0..s.channels {
            tracks.push((Kind::Sound, (s.sample_rate as i32, 1), s.length));
        }
    }

    // Source « bande » : origine du média, avec son timecode.
    let tape_id = mob_id(&format!("{}|tape", m.path.display()));
    let mut tape_slots: Vec<Object> = tracks
        .iter()
        .enumerate()
        .map(|(i, (kind, rate, len))| {
            let clip = source_clip(*kind, *len, 0, &NULL_MOB, 0);
            slot(i as u32 + 1, "", *rate, sequence(*kind, vec![clip]), None)
        })
        .collect();
    if let Some((start, rate, drop)) = m.timecode {
        let fps = (rate.0 as f64 / rate.1.max(1) as f64).round() as u16;
        let secs = tracks
            .first()
            .map(|(_, r, len)| *len as f64 * r.1 as f64 / r.0.max(1) as f64)
            .unwrap_or(0.0);
        let len = (secs * rate.0 as f64 / rate.1.max(1) as f64).round() as i64;
        tape_slots.push(slot(
            tracks.len() as u32 + 1,
            "",
            rate,
            timecode(len.max(1), start, fps.max(1), drop),
            Some(1),
        ));
    }
    let mut tape = mob(
        SOURCE,
        tape_id,
        &format!("{} <TAPE MOB>", m.name),
        tape_slots,
    );
    tape.obj = tape.obj.strong(
        0x4701,
        "EssenceDescription",
        Object::new("0d010101-0101-4a00-060e-2b3402060101"),
    );

    // Pistes qui renvoient, une à une, à celles d'un autre mob.
    let derived = |from: &MobId| -> Vec<Object> {
        let mut audio = 0;
        tracks
            .iter()
            .enumerate()
            .map(|(i, (kind, rate, len))| {
                let id = i as u32 + 1;
                let clip = source_clip(*kind, *len, 0, from, id);
                let track = (*kind == Kind::Sound).then(|| {
                    audio += 1;
                    audio
                });
                slot(id, "", *rate, sequence(*kind, vec![clip]), track)
            })
            .collect()
    };

    // Source fichier : descripteur et emplacement.
    let file_id = mob_id(&format!("{}|file", m.path.display()));
    let descriptor = match (&m.wave, &m.picture) {
        (Some(summary), None) => {
            let s = m.sounds.first();
            Object::new("0d010101-0101-2c00-060e-2b3402060101")
                .data(
                    0x3001,
                    rational(s.map_or(48_000, |s| s.sample_rate) as i32, 1),
                )
                .data(0x3801, summary.clone())
                .data(0x3002, i64le(s.map_or(0, |s| s.length)))
                .weak(
                    0x3004,
                    REF_CONTAINER_DEFS,
                    DEF_KEY_PID,
                    auid("4313b571-d8ba-11d2-809b-006008143e6f").to_vec(),
                )
                .vector(0x2f01, "Locator", vec![locator(url)])
        }
        _ => {
            let mut subs = Vec::new();
            if let Some(p) = &m.picture {
                subs.push(picture_descriptor(p, url));
            }
            subs.extend(m.sounds.iter().map(|s| sound_descriptor(s, url)));
            let (rate, len) = tracks
                .first()
                .map(|(_, r, l)| (*r, *l))
                .unwrap_or(((25, 1), 0));
            Object::new("0d010101-0101-4400-060e-2b3402060101")
                .data(0x3002, i64le(len))
                .data(0x3001, rational(rate.0, rate.1))
                .data(MEDIA_CONTAINER_GUID, auid(AMA_GENERIC).to_vec())
                .vector(0x2f01, "Locator", vec![locator(url)])
                .vector(0x3f01, "FileDescriptors", subs)
        }
    };
    let mut file = mob(
        SOURCE,
        file_id,
        &format!("{} <SOURCE MOB>", m.name),
        derived(&tape_id),
    );
    if let Some(p) = &m.picture {
        // Le module QuickTime de Media Composer attend le nom du codec.
        file.obj = file
            .obj
            .vector(0x4406, "UserComments", vec![comment("Video", &p.codec)]);
    }
    file.obj = file.obj.strong(0x4701, "EssenceDescription", descriptor);

    let master_id = mob_id(&format!("{}|master", m.path.display()));
    let master = mob(MASTER, master_id, &m.name, derived(&file_id));

    let picture = m.picture.as_ref().map(|_| 1);
    let sounds = tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.0 == Kind::Sound)
        .map(|(i, t)| (i as u32 + 1, t.1))
        .collect();
    Linked {
        mobs: vec![tape, file, master],
        master: master_id,
        picture,
        sounds,
    }
}

/// Résumé d'un WAV/BWF (WAVEDescriptor::Summary) : en-tête RIFF, bloc
/// « fmt » et en-tête du bloc « data » (sa taille), comme Pro Tools et Media
/// Composer. RF64/BW64 : taille lue dans « ds64 », plafonnée à 32 bits.
pub fn wave_summary(path: &Path) -> Option<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 12];
    f.read_exact(&mut head).ok()?;
    if !matches!(&head[..4], b"RIFF" | b"RF64" | b"BW64") || &head[8..12] != b"WAVE" {
        return None;
    }
    let mut fmt: Option<Vec<u8>> = None;
    let mut ds64_data: Option<u64> = None;
    loop {
        let mut chunk = [0u8; 8];
        f.read_exact(&mut chunk).ok()?;
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        match &chunk[..4] {
            b"fmt " => {
                let mut data = vec![0u8; size as usize];
                f.read_exact(&mut data).ok()?;
                fmt = Some(data);
                if size % 2 == 1 {
                    f.seek(SeekFrom::Current(1)).ok()?;
                }
                continue;
            }
            b"ds64" if size >= 16 => {
                let mut data = vec![0u8; size as usize];
                f.read_exact(&mut data).ok()?;
                ds64_data = Some(u64::from_le_bytes(data[8..16].try_into().ok()?));
                if size % 2 == 1 {
                    f.seek(SeekFrom::Current(1)).ok()?;
                }
                continue;
            }
            b"data" => {
                let fmt = fmt?;
                let data = match (size, ds64_data) {
                    (u32::MAX, Some(n)) => n.min(u64::from(u32::MAX - 1024)) as u32,
                    _ => size,
                };
                let riff = (4 + 8 + fmt.len() as u64 + 8 + u64::from(data)).min(u64::from(u32::MAX))
                    as u32;
                let mut out = b"RIFF".to_vec();
                out.extend(riff.to_le_bytes());
                out.extend(b"WAVEfmt ");
                out.extend((fmt.len() as u32).to_le_bytes());
                out.extend(fmt);
                out.extend(b"data");
                out.extend(data.to_le_bytes());
                return Some(out);
            }
            _ => {}
        }
        f.seek(SeekFrom::Current(i64::from(size) + i64::from(size % 2)))
            .ok()?;
    }
}

/// Objet Header complet : identification, contenu (mobs), gabarit.
pub fn header(mobs: Vec<Mob>) -> Object {
    let now = timestamp(&chrono::Local::now());
    let ident = Object::new("0d010101-0101-3000-060e-2b3402060101")
        .data(0x3c01, string("VERIFLOW"))
        .data(0x3c02, string("VERIFLOW"))
        .data(0x3c04, string(crate::VERSION))
        .data(0x3c03, version())
        .data(0x3c07, version())
        .data(
            0x3c05,
            auid("5b0e1a52-7c3d-4f8e-9a61-2d4c8b7e1f30").to_vec(),
        )
        .data(0x3c06, now.clone())
        .data(0x3c08, string(std::env::consts::OS))
        .data(0x3c09, new_auid("generation").to_vec());
    let content = Object::new("0d010101-0101-1800-060e-2b3402060101")
        .set(
            0x1901,
            "Mobs",
            (0x4401, 32),
            mobs.into_iter().map(|m| (m.id.to_vec(), m.obj)).collect(),
        )
        // Aucune essence copiée : ensemble vide, mais présent (requis).
        .set(0x1902, "EssenceData", (0x2701, 32), Vec::new());
    Object::new("0d010101-0101-2f00-060e-2b3402060101")
        .template(0x3b04, "Dictionary-3b04")
        .strong(0x3b03, "Content", content)
        .data(
            0x3b09,
            auid("0d011201-0100-0000-060e-2b3404010105").to_vec(),
        )
        .data(0x3b07, u32le(1))
        .data(0x3b05, vec![1, 2])
        .vector(0x3b06, "IdentificationList", vec![ident])
        .data(0x3b02, now)
        .data(0x3b01, u16le(0x4949))
}

/// Version de VERIFLOW (ProductVersion : majeure, mineure, tertiaire,
/// correctif, type « publiée »).
fn version() -> Vec<u8> {
    let mut parts = crate::VERSION
        .split(['.', '-'])
        .map(|p| p.parse::<u16>().unwrap_or(0));
    let mut out = Vec::new();
    for _ in 0..4 {
        out.extend(parts.next().unwrap_or(0).to_le_bytes());
    }
    out.push(1);
    out
}

/// Code d'usage « séquence de premier niveau » (CompositionMob::UsageCode).
pub fn usage_top_level() -> Vec<u8> {
    auid("0d010102-0101-0700-060e-2b3404010101").to_vec()
}
