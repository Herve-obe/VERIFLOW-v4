//! Écriture de fichiers AAF (Advanced Authoring Format, AMWA AS-01/SMPTE 377
//! pour le modèle objet) : stockage structuré Microsoft (CFB, secteurs de
//! 4 Kio), objets rangés en arborescence, propriétés dans un flux
//! « properties » par objet.
//!
//! Le métadictionnaire (définitions des classes et des types) et le
//! dictionnaire (définitions des données, conteneurs, codecs) ne dépendent pas
//! du contenu : ils sont recopiés d'un gabarit généré par
//! `scripts/aaf-template.py` à partir de pyaaf2 (MIT), dont les fichiers
//! s'ouvrent dans Media Composer, Pro Tools et DaVinci Resolve.

pub mod mobs;

use std::fs;
use std::io::Write;
use std::path::Path;

use cfb::CompoundFile;
use uuid::Uuid;

use crate::{Error, Result};

/// Identifiant AAF (AUID) tel qu'enregistré : champs en petit-boutiste.
pub type Auid = [u8; 16];

/// « 0d010101-0101-2f00-060e-2b3402060101 » en octets enregistrés.
pub fn auid(text: &str) -> Auid {
    Uuid::parse_str(text)
        .map(|u| u.to_bytes_le())
        .unwrap_or_else(|_| panic!("AUID invalide : {text}"))
}

/// CLSID de la racine d'un fichier AAF.
const ROOT_CLSID: &str = "b3b398a5-1c90-11d4-8053-080036210804";

/// Formats de stockage des propriétés.
const SF_DATA: u16 = 0x82;
const SF_STRONG: u16 = 0x22;
const SF_STRONG_VECTOR: u16 = 0x32;
const SF_STRONG_SET: u16 = 0x3A;
const SF_WEAK: u16 = 0x02;

/// Chemins (pids) des ensembles visés par les références faibles.
pub const REF_DATA_DEFS: &[u16] = &[0x0002, 0x3b04, 0x2605];
pub const REF_CONTAINER_DEFS: &[u16] = &[0x0002, 0x3b04, 0x2608];

/// Pid de la clé des définitions (DefinitionObject::Identification).
pub const DEF_KEY_PID: u16 = 0x1b01;

/// Valeur d'une propriété.
pub enum Value {
    Data(Vec<u8>),
    Strong(Box<Object>),
    Vector(Vec<Object>),
    /// Ensemble indexé par une clé unique : pid et taille de la clé, (clé, objet).
    Set(u16, u8, Vec<(Vec<u8>, Object)>),
    /// Référence faible : chemin de l'ensemble visé, pid et valeur de la clé.
    Weak(&'static [u16], u16, Vec<u8>),
    /// Objet déjà présent dans le gabarit (nom de son stockage).
    Template(&'static str),
}

pub struct Property {
    pub pid: u16,
    pub name: &'static str,
    pub value: Value,
}

/// Objet AAF : sa classe et ses propriétés.
pub struct Object {
    pub class: Auid,
    pub props: Vec<Property>,
}

impl Object {
    pub fn new(class: &str) -> Self {
        Self {
            class: auid(class),
            props: Vec::new(),
        }
    }

    fn with(mut self, pid: u16, name: &'static str, value: Value) -> Self {
        self.props.push(Property { pid, name, value });
        self
    }

    pub fn data(self, pid: u16, bytes: Vec<u8>) -> Self {
        self.with(pid, "", Value::Data(bytes))
    }

    pub fn strong(self, pid: u16, name: &'static str, obj: Object) -> Self {
        self.with(pid, name, Value::Strong(Box::new(obj)))
    }

    pub fn vector(self, pid: u16, name: &'static str, objs: Vec<Object>) -> Self {
        self.with(pid, name, Value::Vector(objs))
    }

    pub fn set(
        self,
        pid: u16,
        name: &'static str,
        key: (u16, u8),
        items: Vec<(Vec<u8>, Object)>,
    ) -> Self {
        self.with(pid, name, Value::Set(key.0, key.1, items))
    }

    pub fn weak(self, pid: u16, path: &'static [u16], key_pid: u16, key: Vec<u8>) -> Self {
        self.with(pid, "", Value::Weak(path, key_pid, key))
    }

    pub fn template(self, pid: u16, storage: &'static str) -> Self {
        self.with(pid, "", Value::Template(storage))
    }
}

// Encodage des valeurs (petit-boutiste, chaînes UTF-16 terminées par un nul).

pub fn string(s: &str) -> Vec<u8> {
    let mut out: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
    out.extend([0, 0]);
    out
}

pub fn u16le(v: u16) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

pub fn u32le(v: u32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

pub fn i64le(v: i64) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

pub fn rational(num: i32, den: i32) -> Vec<u8> {
    let mut out = num.to_le_bytes().to_vec();
    out.extend(den.to_le_bytes());
    out
}

/// Horodatage AAF : année, mois, jour, heure, minute, seconde, centièmes.
pub fn timestamp(t: &chrono::DateTime<chrono::Local>) -> Vec<u8> {
    use chrono::{Datelike, Timelike};
    let mut out = (t.year() as i16).to_le_bytes().to_vec();
    out.extend([
        t.month() as u8,
        t.day() as u8,
        t.hour() as u8,
        t.minute() as u8,
        t.second() as u8,
        (t.timestamp_subsec_millis() / 10) as u8,
    ]);
    out
}

/// Valeur indirecte de type chaîne (TaggedValue::Value).
pub fn indirect_string(s: &str) -> Vec<u8> {
    let mut out = vec![0x4c];
    out.extend(auid("01100200-0000-0000-060e-2b3401040101"));
    out.extend(string(s));
    out
}

/// Nom de stockage d'une propriété : « Nom-pid », raccourci au milieu pour
/// tenir dans `size` caractères (règle de pyaaf2 et du SDK AAF).
fn mangle(name: &str, pid: u16, size: usize) -> String {
    let p = format!("{pid:x}");
    let max = size - p.len() - 2;
    let chars: Vec<char> = name.chars().collect();
    let short: String = if chars.len() <= max {
        name.to_owned()
    } else {
        let half = max / 2;
        (0..max)
            .map(|i| match i {
                i if i < half => chars[i],
                i if i == half => '-',
                i => chars[chars.len() - (max - i)],
            })
            .collect()
    };
    format!("{short}-{p}")
}

/// Gabarit : table des références et sous-arbres recopiés.
struct Template {
    table: Vec<Vec<u16>>,
    /// (stockage ?, chemin, CLSID ou données)
    entries: Vec<(bool, String, Vec<u8>)>,
}

const TEMPLATE: &[u8] = include_bytes!("template.bin");

fn read_template() -> Result<Template> {
    let bad = || Error::Unsupported("gabarit AAF illisible".into());
    let mut pos = 0usize;
    let mut take = |n: usize| -> Result<&[u8]> {
        let s = TEMPLATE.get(pos..pos + n).ok_or_else(bad)?;
        pos += n;
        Ok(s)
    };
    if take(8)? != b"VFAAFT1\0" {
        return Err(bad());
    }
    let u16_at = |b: &[u8]| u16::from_le_bytes([b[0], b[1]]);
    let u32_at = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let mut table = Vec::new();
    for _ in 0..u32_at(take(4)?) {
        let n = u16_at(take(2)?) as usize;
        let pids = take(2 * n)?.chunks(2).map(u16_at).collect();
        table.push(pids);
    }
    let mut entries = Vec::new();
    for _ in 0..u32_at(take(4)?) {
        let kind = take(1)?[0];
        let len = u16_at(take(2)?) as usize;
        let path = String::from_utf8(take(len)?.to_vec()).map_err(|_| bad())?;
        let payload = if kind == 0 {
            take(16)?.to_vec()
        } else {
            let n = u32_at(take(4)?) as usize;
            take(n)?.to_vec()
        };
        entries.push((kind == 0, path, payload));
    }
    Ok(Template { table, entries })
}

struct Writer<F: std::io::Read + Write + std::io::Seek> {
    cf: CompoundFile<F>,
    table: Vec<Vec<u16>>,
}

impl<F: std::io::Read + Write + std::io::Seek> Writer<F> {
    fn table_index(&mut self, path: &[u16]) -> u16 {
        match self.table.iter().position(|p| p == path) {
            Some(i) => i as u16,
            None => {
                self.table.push(path.to_vec());
                (self.table.len() - 1) as u16
            }
        }
    }

    fn stream(&mut self, path: &str, data: &[u8]) -> Result<()> {
        let mut s = self.cf.create_stream(path)?;
        s.write_all(data)?;
        Ok(())
    }

    /// Écrit l'objet dans le stockage `dir` (déjà créé), puis ses enfants.
    fn object(&mut self, dir: &str, obj: &Object) -> Result<()> {
        self.cf
            .set_storage_clsid(dir, Uuid::from_bytes_le(obj.class))?;
        let mut entries: Vec<(u16, u16, Vec<u8>)> = Vec::new();
        for p in &obj.props {
            match &p.value {
                Value::Data(d) => entries.push((p.pid, SF_DATA, d.clone())),
                Value::Template(name) => entries.push((p.pid, SF_STRONG, string(name))),
                Value::Strong(child) => {
                    let name = mangle(p.name, p.pid, 32);
                    let path = format!("{dir}/{name}");
                    self.cf.create_storage(&path)?;
                    self.object(&path, child)?;
                    entries.push((p.pid, SF_STRONG, string(&name)));
                }
                Value::Vector(items) => {
                    let name = mangle(p.name, p.pid, 22);
                    let mut index = Vec::new();
                    index.extend((items.len() as u32).to_le_bytes());
                    index.extend((items.len() as u32).to_le_bytes());
                    index.extend(u32::MAX.to_le_bytes());
                    for (i, child) in items.iter().enumerate() {
                        let path = format!("{dir}/{name}{{{i:x}}}");
                        self.cf.create_storage(&path)?;
                        self.object(&path, child)?;
                        index.extend((i as u32).to_le_bytes());
                    }
                    self.stream(&format!("{dir}/{name} index"), &index)?;
                    entries.push((p.pid, SF_STRONG_VECTOR, string(&name)));
                }
                Value::Set(key_pid, key_size, items) => {
                    let name = mangle(p.name, p.pid, 22);
                    let mut index = Vec::new();
                    index.extend((items.len() as u32).to_le_bytes());
                    index.extend((items.len() as u32).to_le_bytes());
                    index.extend(u32::MAX.to_le_bytes());
                    index.extend(key_pid.to_le_bytes());
                    index.push(*key_size);
                    for (i, (key, child)) in items.iter().enumerate() {
                        let path = format!("{dir}/{name}{{{i:x}}}");
                        self.cf.create_storage(&path)?;
                        self.object(&path, child)?;
                        index.extend((i as u32).to_le_bytes());
                        index.extend(1u32.to_le_bytes());
                        index.extend(key);
                    }
                    self.stream(&format!("{dir}/{name} index"), &index)?;
                    entries.push((p.pid, SF_STRONG_SET, string(&name)));
                }
                Value::Weak(path, key_pid, key) => {
                    let mut d = self.table_index(path).to_le_bytes().to_vec();
                    d.extend(key_pid.to_le_bytes());
                    d.push(key.len() as u8);
                    d.extend(key);
                    entries.push((p.pid, SF_WEAK, d));
                }
            }
        }
        let mut s = vec![0x4c, 32];
        s.extend((entries.len() as u16).to_le_bytes());
        for (pid, format, data) in &entries {
            let len = u16::try_from(data.len())
                .map_err(|_| Error::Unsupported("propriété AAF trop longue".into()))?;
            s.extend(pid.to_le_bytes());
            s.extend(format.to_le_bytes());
            s.extend(len.to_le_bytes());
        }
        for (_, _, data) in &entries {
            s.extend(data);
        }
        self.stream(&format!("{dir}/properties"), &s)
    }
}

/// Écrit un fichier AAF : l'en-tête `header` (objet Header) et le gabarit.
pub fn write(path: &Path, header: &Object) -> Result<()> {
    let template = read_template()?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    let cf = CompoundFile::create_with_version(cfb::Version::V4, file)?;
    let mut w = Writer {
        cf,
        table: template.table,
    };
    w.cf.set_storage_clsid("/", Uuid::from_bytes_le(auid(ROOT_CLSID)))?;
    w.cf.create_storage("/Header-2")?;
    w.object("/Header-2", header)?;
    for (storage, p, payload) in &template.entries {
        let p = format!("/{p}");
        if *storage {
            w.cf.create_storage(&p)?;
            let clsid: [u8; 16] = payload[..].try_into().unwrap_or_default();
            w.cf.set_storage_clsid(&p, Uuid::from_bytes_le(clsid))?;
        } else {
            w.stream(&p, payload)?;
        }
    }
    // Racine : métadictionnaire et en-tête.
    let meta = string("MetaDictionary-1");
    let head = string("Header-2");
    let mut root = vec![0x4c, 32];
    root.extend(2u16.to_le_bytes());
    for (pid, d) in [(1u16, &meta), (2u16, &head)] {
        root.extend(pid.to_le_bytes());
        root.extend(SF_STRONG.to_le_bytes());
        root.extend((d.len() as u16).to_le_bytes());
    }
    root.extend(&meta);
    root.extend(&head);
    w.stream("/properties", &root)?;
    // Table des références faibles.
    let count: usize = w.table.iter().map(|p| p.len() + 1).sum();
    let mut refs = vec![0x4c];
    refs.extend((w.table.len() as u16).to_le_bytes());
    refs.extend((count as u32).to_le_bytes());
    for p in &w.table {
        for pid in p {
            refs.extend(pid.to_le_bytes());
        }
        refs.extend(0u16.to_le_bytes());
    }
    w.stream("/referenced properties", &refs)?;
    w.cf.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_squeezed_like_the_sdk() {
        assert_eq!(
            mangle("IdentificationList", 0x3b06, 22),
            "Identifi-ionList-3b06"
        );
        assert_eq!(
            mangle("ContainerDefinitions", 0x2608, 22),
            "Containe-nitions-2608"
        );
        assert_eq!(mangle("Mobs", 0x1901, 22), "Mobs-1901");
        assert_eq!(
            mangle("EssenceDescription", 0x4701, 32),
            "EssenceDescription-4701"
        );
    }

    #[test]
    fn auids_are_stored_little_endian() {
        assert_eq!(
            auid("0d011201-0100-0000-060e-2b3404010105").to_vec(),
            [0x01, 0x12, 0x01, 0x0d, 0x00, 0x01, 0x00, 0x00, 0x06, 0x0e, 0x2b, 0x34, 4, 1, 1, 5]
        );
    }

    #[test]
    fn template_reads() {
        let t = read_template().unwrap();
        assert_eq!(t.table[2], REF_DATA_DEFS);
        assert!(t
            .entries
            .iter()
            .any(|(_, p, _)| p == "MetaDictionary-1/properties"));
        assert!(t
            .entries
            .iter()
            .any(|(_, p, _)| p == "Header-2/Dictionary-3b04/properties"));
    }
}
