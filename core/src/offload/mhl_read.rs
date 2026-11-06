//! Lecture d'un historique ASC MHL v2 (dossier `ascmhl/`) : chaîne, générations
//! et empreinte la plus récente de chaque fichier, comme `ascmhl verify`.
//!
//! Les MHL écrits par VERIFLOW et ceux d'autres outils sont lus de la même
//! façon : plusieurs méthodes par fichier, attributs facultatifs, éléments
//! inconnus ignorés. Un MHL v1 ou un historique imbriqué est refusé avec un
//! message explicite : une vérification fausse est pire qu'aucune.
//!
//! Les chemins du XML sont en « / ». Les clés de l'historique sont normalisées
//! en NFC (macOS écrit les noms accentués en NFD, d'autres outils en NFC) ;
//! le chemin réel du disque reste utilisé pour la lecture.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event as Xml;
use quick_xml::Reader;
use serde::Serialize;
use walkdir::WalkDir;

use super::engine::Hashes;
use super::hash::{hash_data, HashAlgo};
use crate::{Error, Result};

/// Dossier d'historique ASC MHL.
pub const MHL_DIR: &str = "ascmhl";
/// Nom du fichier de chaîne.
pub const CHAIN_FILE: &str = "ascmhl_chain.xml";

/// Méthodes par ordre de préférence pour relire un disque.
pub const BEST_ORDER: [HashAlgo; 7] = [
    HashAlgo::Xxh128,
    HashAlgo::C4,
    HashAlgo::Xxh64,
    HashAlgo::Xxh3,
    HashAlgo::Md5,
    HashAlgo::Sha1,
    HashAlgo::Sha256,
];

/// Une génération inscrite dans la chaîne.
#[derive(Debug, Clone, Serialize)]
pub struct ChainEntry {
    pub seq: u32,
    /// Nom du fichier .mhl, relatif à `ascmhl/`.
    pub file: String,
    /// C4 inscrit dans la chaîne (vide si l'outil d'origine n'en a pas écrit).
    pub c4: String,
}

/// Anomalie de la chaîne.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum ChainIssue {
    /// Le C4 du fichier .mhl ne correspond plus à celui de la chaîne.
    Altered(String),
    /// Le fichier .mhl de la chaîne n'existe plus.
    Missing(String),
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Chain {
    pub entries: Vec<ChainEntry>,
    pub issues: Vec<ChainIssue>,
}

/// Un fichier décrit par une génération.
#[derive(Debug, Clone, Serialize)]
pub struct GenFile {
    /// Chemin relatif, séparateur « / ».
    pub rel: String,
    /// Taille (attribut facultatif dans la norme).
    pub size: Option<u64>,
    pub hashes: Hashes,
    /// Action de la première empreinte (`original`, `verified`, `failed`...).
    pub action: String,
    /// Méthodes d'empreinte non reconnues (éléments portant un `action`).
    pub unsupported: Vec<String>,
    /// Raison du refus du chemin (`..`, racine, lecteur), None s'il est sûr.
    pub invalid: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Generation {
    pub process: String,
    /// Date de création (`creationdate`, texte du fichier), vide si absente.
    pub date: String,
    pub files: Vec<GenFile>,
    /// Motifs `<ignore>` de la génération.
    pub ignore: Vec<String>,
}

/// Historique complet d'un disque.
#[derive(Debug, Clone, Serialize)]
pub struct History {
    pub chain: Chain,
    /// Dernière empreinte connue de chaque fichier : (taille, empreintes).
    pub files: HashMap<String, (Option<u64>, Hashes)>,
    /// Motifs d'exclusion de toutes les générations.
    pub ignore: Vec<String>,
    /// Fichiers dont la dernière entrée n'a que des méthodes non reconnues
    /// (clé NFC, noms des méthodes) : leurs empreintes sont vides dans `files`.
    pub unsupported: HashMap<String, Vec<String>>,
    /// Chemins refusés (chemin brut, raison) : signalés en écart.
    pub invalid: Vec<(String, String)>,
}

/// Raison de refus d'un chemin venu d'un MHL : jamais de sortie du disque.
fn unsafe_path(rel: &str) -> Option<&'static str> {
    let b = rel.as_bytes();
    if rel.starts_with('/') || rel.starts_with('\\') {
        Some("chemin absolu")
    } else if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        Some("lecteur dans le chemin")
    } else if rel.split(['/', '\\']).any(|c| c == "..") {
        Some("chemin sortant du disque (..)")
    } else {
        None
    }
}

/// Forme NFC d'un chemin, pour comparer des noms écrits en NFC ou en NFD.
pub fn nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect()
}

/// Chemin relatif « / » converti pour le système.
pub fn to_path(root: &Path, rel: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    p.extend(rel.split('/').filter(|s| !s.is_empty()));
    p
}

/// Meilleure méthode disponible parmi des empreintes.
pub fn best_algo(hashes: &Hashes) -> Option<HashAlgo> {
    BEST_ORDER
        .into_iter()
        .find(|a| hashes.iter().any(|(x, _)| x == a))
}

#[derive(PartialEq)]
enum Ev {
    Start,
    End,
}

/// Parcourt un XML en appelant `on(événement, chemin des éléments ouverts,
/// attributs de l'élément, texte)` ; le texte n'est renseigné qu'à la fermeture.
fn walk_xml(
    xml: &str,
    mut on: impl FnMut(Ev, &[String], &[(String, String)], &str) -> Result<()>,
) -> Result<()> {
    let mut reader = Reader::from_str(xml);
    let bad = |e: &dyn std::fmt::Display| Error::Report(format!("XML illisible : {e}"));
    let mut path: Vec<String> = Vec::new();
    let mut attrs: Vec<Vec<(String, String)>> = Vec::new();
    let mut text = String::new();
    loop {
        let ev = reader.read_event().map_err(|e| bad(&e))?;
        let (start, empty) = match &ev {
            Xml::Start(e) => (Some(e), false),
            Xml::Empty(e) => (Some(e), true),
            _ => (None, false),
        };
        if let Some(e) = start {
            let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
            let mut list = Vec::new();
            for a in e.attributes().flatten() {
                let key = String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned();
                let value = a
                    .decode_and_unescape_value(reader.decoder())
                    .map_err(|e| bad(&e))?
                    .into_owned();
                list.push((key, value));
            }
            path.push(name);
            text.clear();
            on(Ev::Start, &path, &list, "")?;
            if empty {
                on(Ev::End, &path, &list, "")?;
                path.pop();
            } else {
                attrs.push(list);
            }
            continue;
        }
        match ev {
            Xml::Text(t) => text += &t.decode().map_err(|e| bad(&e))?,
            Xml::CData(t) => text += &t.decode().map_err(|e| bad(&e))?,
            Xml::GeneralRef(r) => {
                if let Some(c) = r.resolve_char_ref().map_err(|e| bad(&e))? {
                    text.push(c);
                } else {
                    match r.decode().map_err(|e| bad(&e))?.as_ref() {
                        "amp" => text.push('&'),
                        "lt" => text.push('<'),
                        "gt" => text.push('>'),
                        "quot" => text.push('"'),
                        "apos" => text.push('\''),
                        autre => return Err(bad(&format!("entité inconnue &{autre};"))),
                    }
                }
            }
            Xml::End(_) => {
                let list = attrs.pop().unwrap_or_default();
                on(Ev::End, &path, &list, text.trim())?;
                path.pop();
                text.clear();
            }
            Xml::Eof => {
                if path.is_empty() {
                    return Ok(());
                }
                return Err(bad(&format!(
                    "fichier tronqué (élément « {} » non fermé)",
                    path.join("/")
                )));
            }
            _ => {}
        }
    }
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn names(path: &[String]) -> Vec<&str> {
    path.iter().map(String::as_str).collect()
}

/// Lit `ascmhl_chain.xml` (sans contrôle des fichiers).
pub(super) fn parse_chain_file(xml: &str) -> Result<Vec<ChainEntry>> {
    let mut out = Vec::new();
    let (mut seq, mut file, mut c4) = (0u32, String::new(), String::new());
    let mut root_ok = false;
    walk_xml(xml, |ev, path, attrs, text| {
        match (ev, names(path).as_slice()) {
            (Ev::Start, ["ascmhldirectory"]) => root_ok = true,
            (Ev::Start, ["ascmhldirectory", "hashlist"]) => {
                // Numéro absent : position dans la chaîne.
                seq = attr(attrs, "sequencenr")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(out.len() as u32 + 1);
                file.clear();
                c4.clear();
            }
            (Ev::End, ["ascmhldirectory", "hashlist", "path"]) => file = text.to_owned(),
            (Ev::End, ["ascmhldirectory", "hashlist", "c4"]) => c4 = text.to_owned(),
            (Ev::End, ["ascmhldirectory", "hashlist"]) if !file.is_empty() => {
                out.push(ChainEntry {
                    seq,
                    file: file.clone(),
                    c4: c4.clone(),
                });
            }
            _ => {}
        }
        Ok(())
    })?;
    if !root_ok {
        return Err(Error::Report(format!(
            "{CHAIN_FILE} n'est pas une chaîne ASC MHL v2"
        )));
    }
    out.sort_by_key(|e| e.seq);
    Ok(out)
}

fn mhl_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join(MHL_DIR);
    if !root.is_dir() {
        return Err(Error::NotFound(root.display().to_string()));
    }
    if !dir.is_dir() {
        return Err(Error::Report(format!(
            "aucun historique ASC MHL (dossier {MHL_DIR}) dans {}",
            root.display()
        )));
    }
    Ok(dir)
}

/// Désécape les cinq entités XML prédéfinies (`&amp;` en dernier).
fn unescape_once(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Lit la chaîne et contrôle chaque génération (présence, puis C4).
pub fn read_chain(root: &Path) -> Result<Chain> {
    let dir = mhl_dir(root)?;
    let chain_path = dir.join(CHAIN_FILE);
    let xml = fs::read_to_string(&chain_path).map_err(|e| {
        Error::Report(format!(
            "chaîne ASC MHL illisible ({}) : {e}",
            chain_path.display()
        ))
    })?;
    let mut entries = parse_chain_file(&xml)?;
    let mut issues = Vec::new();
    for e in &mut entries {
        // Chaîne écrite par une ancienne version qui échappait deux fois les
        // noms (« A&amp;B ») : un seul essai avec le nom désécapé.
        if !to_path(&dir, &e.file).is_file() {
            let plain = unescape_once(&e.file);
            if plain != e.file && to_path(&dir, &plain).is_file() {
                e.file = plain;
            }
        }
        match fs::read(to_path(&dir, &e.file)) {
            Err(_) => issues.push(ChainIssue::Missing(e.file.clone())),
            Ok(bytes) => {
                if !e.c4.is_empty() && hash_data(HashAlgo::C4, &bytes) != e.c4 {
                    issues.push(ChainIssue::Altered(e.file.clone()));
                }
            }
        }
    }
    Ok(Chain { entries, issues })
}

/// Lit un fichier .mhl (v2 uniquement).
pub fn read_generation(path: &Path) -> Result<Generation> {
    let xml = fs::read_to_string(path)
        .map_err(|e| Error::Report(format!("MHL illisible ({}) : {e}", path.display())))?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let name = name.unwrap_or_default();
    let mut gen = Generation::default();
    let mut cur: Option<GenFile> = None;
    let mut root_seen = false;
    walk_xml(&xml, |ev, path, attrs, text| {
        match (ev, names(path).as_slice()) {
            (Ev::Start, ["hashlist"]) => {
                root_seen = true;
                let version = attr(attrs, "version").unwrap_or("");
                if !version.starts_with('2') {
                    return Err(Error::Unsupported(format!(
                        "{name} : ASC MHL v1 (version « {version} ») non pris en charge, \
                         seul l'ASC MHL v2 est vérifiable"
                    )));
                }
            }
            (Ev::Start, [_, ..]) if path[0] != "hashlist" => {
                return Err(Error::Report(format!(
                    "{name} n'est pas un fichier ASC MHL (élément racine « {} »)",
                    path[0]
                )));
            }
            (Ev::End, ["hashlist", "processinfo", "process"]) => gen.process = text.to_owned(),
            (Ev::End, ["hashlist", "creatorinfo", "creationdate"]) => gen.date = text.to_owned(),
            (Ev::End, ["hashlist", "processinfo", "ignore", "pattern"]) if !text.is_empty() => {
                gen.ignore.push(text.to_owned());
            }
            (Ev::Start, ["hashlist", "hashes", "hash"]) => {
                cur = Some(GenFile {
                    rel: String::new(),
                    size: None,
                    hashes: Vec::new(),
                    action: String::new(),
                    unsupported: Vec::new(),
                    invalid: None,
                });
            }
            (Ev::End, ["hashlist", "hashes", "hash", "path"]) => {
                if let Some(c) = cur.as_mut() {
                    c.rel = text.trim_start_matches("./").to_owned();
                    c.invalid = unsafe_path(&c.rel).map(str::to_owned);
                    c.size = attr(attrs, "size").and_then(|s| s.parse().ok());
                }
            }
            (Ev::End, ["hashlist", "hashes", "hash", method])
                if method.parse::<HashAlgo>().is_err() =>
            {
                // Une empreinte porte toujours un `action` : sans lui, c'est un
                // élément étranger (métadonnées...), ignoré.
                if let (Some(c), Some(_)) = (cur.as_mut(), attr(attrs, "action")) {
                    c.unsupported.push((*method).to_owned());
                }
            }
            (Ev::End, ["hashlist", "hashes", "hash", method]) => {
                if let (Some(c), Ok(algo)) = (cur.as_mut(), method.parse::<HashAlgo>()) {
                    // C4 sensible à la casse ; les autres formats sont en hexadécimal.
                    let value = if algo == HashAlgo::C4 {
                        text.to_owned()
                    } else {
                        text.to_ascii_lowercase()
                    };
                    if c.action.is_empty() {
                        c.action = attr(attrs, "action").unwrap_or("original").to_owned();
                    }
                    if !value.is_empty() {
                        c.hashes.push((algo, value));
                    }
                }
            }
            (Ev::End, ["hashlist", "hashes", "hash"]) => {
                if let Some(c) = cur.take().filter(|c| !c.rel.is_empty()) {
                    gen.files.push(c);
                }
            }
            _ => {}
        }
        Ok(())
    })
    .map_err(|e| match e {
        Error::Report(m) => Error::Report(format!("{name} : {m}")),
        autre => autre,
    })?;
    if !root_seen {
        return Err(Error::Report(format!("{name} : fichier MHL vide")));
    }
    Ok(gen)
}

/// Chaîne, empreintes les plus récentes et motifs d'exclusion d'un disque.
/// Les empreintes `failed` ne remplacent jamais une empreinte connue : elles
/// décrivent un fichier déjà altéré, pas la référence. Une entrée plus récente
/// remplace le jeu d'empreintes entier du fichier (jamais de fusion avec une
/// génération plus ancienne).
pub fn load_history(root: &Path) -> Result<History> {
    let chain = read_chain(root)?;
    let dir = root.join(MHL_DIR);
    let mut files: HashMap<String, (Option<u64>, Hashes)> = HashMap::new();
    let mut unsupported: HashMap<String, Vec<String>> = HashMap::new();
    let mut invalid: Vec<(String, String)> = Vec::new();
    let mut ignore: Vec<String> = Vec::new();
    for e in &chain.entries {
        let path = to_path(&dir, &e.file);
        if !path.is_file() {
            continue; // signalé par la chaîne
        }
        let gen = read_generation(&path)?;
        for p in gen.ignore {
            if !ignore.contains(&p) {
                ignore.push(p);
            }
        }
        for f in gen.files {
            if let Some(why) = f.invalid {
                let item = (f.rel, why);
                if !invalid.contains(&item) {
                    invalid.push(item);
                }
                continue;
            }
            if f.action == "failed" || (f.hashes.is_empty() && f.unsupported.is_empty()) {
                continue;
            }
            let key = nfc(&f.rel);
            if f.hashes.is_empty() {
                unsupported.insert(key.clone(), f.unsupported);
            } else {
                unsupported.remove(&key);
            }
            files.insert(key, (f.size, f.hashes));
        }
    }
    Ok(History {
        chain,
        files,
        ignore,
        unsupported,
        invalid,
    })
}

/// Empreinte la plus récente de chaque fichier, toutes générations confondues.
pub fn history(root: &Path) -> Result<HashMap<String, (Option<u64>, Hashes)>> {
    Ok(load_history(root)?.files)
}

/// Sous-dossiers de `dir` (jusqu'à `depth` niveaux) qui contiennent un historique.
pub fn find_histories(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut it = WalkDir::new(dir)
        .min_depth(1)
        .max_depth(depth)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter();
    while let Some(Ok(e)) = it.next() {
        if !e.file_type().is_dir() {
            continue;
        }
        if e.path().join(MHL_DIR).join(CHAIN_FILE).is_file() {
            out.push(e.path().to_path_buf());
            it.skip_current_dir();
        }
    }
    out
}

/// Refuse un disque dont un sous-dossier a son propre `ascmhl/` : sa
/// vérification serait fausse (historiques imbriqués non pris en charge).
pub fn check_not_nested(root: &Path) -> Result<()> {
    let mut it = WalkDir::new(root)
        .min_depth(1)
        .follow_links(false)
        .into_iter();
    while let Some(entry) = it.next() {
        // Un dossier illisible n'arrête pas la recherche.
        let Ok(e) = entry else { continue };
        if !e.file_type().is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy();
        if name == MHL_DIR {
            if e.depth() > 1 {
                return Err(nested_error(root, e.path()));
            }
            it.skip_current_dir();
        } else if (e.depth() == 1 && name == super::mhl::REPORTS_DIR)
            || super::scan::IGNORED.contains(&name.as_ref())
        {
            it.skip_current_dir();
        }
    }
    Ok(())
}

pub(super) fn nested_error(root: &Path, found: &Path) -> Error {
    let rel = found.strip_prefix(root).unwrap_or(found);
    Error::Unsupported(format!(
        "historique ASC MHL imbriqué ({}) : vérifie ce sous-dossier à part",
        rel.display()
    ))
}

/// Résumé d'un historique pour l'affichage avant vérification.
#[derive(Debug, Clone, Serialize)]
pub struct MhlInfo {
    pub root: PathBuf,
    /// Nombre de générations inscrites dans la chaîne.
    pub generations: usize,
    /// Date de la dernière génération lisible (texte du MHL).
    pub last_date: Option<String>,
    /// Fichiers suivis par l'historique.
    pub files: usize,
    /// Méthodes d'empreinte présentes, de la plus à la moins prioritaire du MHL.
    pub algos: Vec<HashAlgo>,
    pub chain_issues: Vec<ChainIssue>,
    /// Sous-dossiers qui ont leur propre historique, renseignés seulement quand
    /// `root` n'en a pas (l'utilisateur a choisi un dossier parent).
    pub histories: Vec<PathBuf>,
}

/// Profondeur de recherche des historiques dans les sous-dossiers.
const HISTORY_SEARCH_DEPTH: usize = 3;

/// Lit l'historique de `root` pour l'aperçu. Sans `ascmhl/`, renvoie un
/// résumé vide dont `histories` liste les sous-dossiers qui en ont ; l'erreur
/// « aucun historique » n'est rendue que s'il n'y en a nulle part.
pub fn info(root: &Path) -> Result<MhlInfo> {
    if root.is_dir() && !root.join(MHL_DIR).is_dir() {
        let histories = find_histories(root, HISTORY_SEARCH_DEPTH);
        if !histories.is_empty() {
            return Ok(MhlInfo {
                root: root.to_path_buf(),
                generations: 0,
                last_date: None,
                files: 0,
                algos: Vec::new(),
                chain_issues: Vec::new(),
                histories,
            });
        }
    }
    check_not_nested(root)?;
    let h = load_history(root)?;
    let dir = root.join(MHL_DIR);
    let last_date = h
        .chain
        .entries
        .iter()
        .rev()
        .filter_map(|e| read_generation(&to_path(&dir, &e.file)).ok())
        .map(|g| g.date)
        .find(|d| !d.is_empty());
    let mut algos: Vec<HashAlgo> = h
        .files
        .values()
        .flat_map(|(_, hashes)| hashes.iter().map(|(a, _)| *a))
        .collect();
    algos.sort_by_key(|a| a.mhl_order());
    algos.dedup();
    Ok(MhlInfo {
        root: root.to_path_buf(),
        generations: h.chain.entries.len(),
        last_date,
        files: h.files.len(),
        algos,
        chain_issues: h.chain.issues,
        histories: Vec::new(),
    })
}

/// Code stable d'une erreur de lecture ou de vérification d'historique, pour
/// la traduction côté interface : `no_history`, `mhl_v1` ou `nested_history`.
pub fn error_code(e: &Error) -> Option<&'static str> {
    match e {
        Error::Report(m) if m.starts_with("aucun historique ASC MHL") => Some("no_history"),
        Error::Unsupported(m) if m.contains("ASC MHL v1") => Some("mhl_v1"),
        Error::Unsupported(m) if m.starts_with("historique ASC MHL imbriqué") => {
            Some("nested_history")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::engine::{run, OffloadSpec};
    use crate::offload::mhl::{write_generation, MhlAuthor};
    use crate::offload::scan::scan;
    use std::sync::atomic::AtomicBool;

    /// Copie une carte de test et écrit sa génération 1.
    fn offloaded(dir: &Path) -> PathBuf {
        let src = dir.join("A001");
        fs::create_dir_all(src.join("CLIP")).unwrap();
        fs::write(src.join("CLIP/a.mov"), b"abcde").unwrap();
        fs::write(src.join("b & c.txt"), b"texte").unwrap();
        let dest = dir.join("SSD/A001");
        let inv = scan(&src).unwrap();
        let spec = OffloadSpec {
            destinations: vec![dest.clone()],
            algorithms: vec![HashAlgo::Xxh128, HashAlgo::Md5],
            known: None,
            overlap: false,
            expected: None,
        };
        let s = run(&inv, &spec, &AtomicBool::new(false), |_| {}).unwrap();
        write_generation(&inv, &s, 0, &dest, &spec.algorithms, &MhlAuthor::default()).unwrap();
        dest
    }

    #[test]
    fn reads_back_own_generation() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let chain = read_chain(&dest).unwrap();
        assert_eq!(chain.entries.len(), 1);
        assert!(chain.issues.is_empty());
        let gen = read_generation(&dest.join("ascmhl").join(&chain.entries[0].file)).unwrap();
        assert_eq!(gen.process, "transfer");
        assert_eq!(gen.files.len(), 2);
        assert!(gen.ignore.iter().any(|p| p == "_VERIFLOW"));
        let f = gen.files.iter().find(|f| f.rel == "b & c.txt").unwrap();
        assert_eq!(f.size, Some(5));
        assert_eq!(f.action, "original");
        assert_eq!(best_algo(&f.hashes), Some(HashAlgo::Xxh128));
        assert_eq!(f.hashes.len(), 2);
        let h = history(&dest).unwrap();
        assert_eq!(h["CLIP/a.mov"].0, Some(5));
        assert_eq!(
            to_path(&dest, "CLIP/a.mov"),
            dest.join("CLIP").join("a.mov")
        );
    }

    #[test]
    fn reads_foreign_mhl() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("DISQUE");
        let mhl_dir = root.join("ascmhl");
        fs::create_dir_all(&mhl_dir).unwrap();
        // Autre outil : espace de noms et commentaires, taille absente, deux
        // méthodes (hexadécimal en majuscules), élément inconnu, entité numérique.
        let mhl = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- écrit ailleurs -->
<hashlist version="2.0" xmlns="urn:ASC:MHL:v2.0">
  <creatorinfo><creationdate>2024-01-01T00:00:00+00:00</creationdate>
    <tool version="9">AutreOutil</tool></creatorinfo>
  <processinfo><process>in-place</process>
    <roothash><content><md5>00</md5></content></roothash>
    <ignore><pattern>*.tmp</pattern><pattern>ascmhl/</pattern></ignore></processinfo>
  <hashes>
    <hash><path>x/un &amp; deux&#233;.mov</path>
      <md5 action="original" hashdate="2024-01-01T00:00:00+00:00">ABCDEF0123456789ABCDEF0123456789</md5>
      <xxh64 action="original">00112233AABBCCDD</xxh64>
      <futur>x</futur></hash>
    <hash><path size="3" lastmodificationdate="2024-01-01T00:00:00+00:00">y.txt</path>
      <sha1 action="verified">0000000000000000000000000000000000000000</sha1></hash>
    <directoryhash><path>x</path><content><md5>11</md5></content></directoryhash>
  </hashes>
</hashlist>
"#;
        let name = "0001_DISQUE_2024-01-01_000000Z.mhl";
        fs::write(mhl_dir.join(name), mhl).unwrap();
        // Chaîne d'un autre outil : pas de c4 → aucun contrôle possible, aucun faux positif.
        fs::write(
            mhl_dir.join("ascmhl_chain.xml"),
            format!(
                "<?xml version=\"1.0\"?><ascmhldirectory xmlns=\"urn:ASC:MHL:DIRECTORY:v2.0\">\
                 <hashlist sequencenr=\"1\"><path>{name}</path></hashlist></ascmhldirectory>"
            ),
        )
        .unwrap();
        let h = load_history(&root).unwrap();
        assert!(h.chain.issues.is_empty());
        assert_eq!(h.ignore, ["*.tmp", "ascmhl/"]);
        assert_eq!(h.files.len(), 2);
        let (size, hashes) = &h.files["x/un & deuxé.mov"];
        assert_eq!(*size, None);
        assert_eq!(hashes.len(), 2);
        assert!(hashes.contains(&(HashAlgo::Md5, "abcdef0123456789abcdef0123456789".into())));
        assert_eq!(best_algo(hashes), Some(HashAlgo::Xxh64));
        assert_eq!(h.files["y.txt"].0, Some(3));
    }

    #[test]
    fn refuses_mhl_v1_and_missing_history() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("D");
        fs::create_dir_all(&root).unwrap();
        let err = read_chain(&root).unwrap_err().to_string();
        assert!(err.contains("aucun historique"), "{err}");
        let v1 = root.join("ascmhl/0001_D_x.mhl");
        fs::create_dir_all(v1.parent().unwrap()).unwrap();
        fs::write(
            &v1,
            "<hashlist version=\"1.0\"><hash><file>a</file></hash></hashlist>",
        )
        .unwrap();
        let err = read_generation(&v1).unwrap_err().to_string();
        assert!(err.contains("non pris en charge"), "{err}");
    }

    #[test]
    fn info_summarises_history_and_finds_children() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let i = info(&dest).unwrap();
        assert_eq!((i.generations, i.files), (1, 2));
        assert!(i.last_date.is_some_and(|d| d.contains('T')));
        assert!(i.algos.contains(&HashAlgo::Xxh128) && i.algos.contains(&HashAlgo::Md5));
        assert!(i.chain_issues.is_empty() && i.histories.is_empty());
        // Dossier parent sans historique : les sous-dossiers qui en ont sont listés.
        let parent = dir.path().join("SSD");
        let i = info(&parent).unwrap();
        assert_eq!(i.generations, 0);
        assert_eq!(i.histories, [dest]);
        // Nulle part : erreur au code stable.
        let empty = dir.path().join("vide");
        fs::create_dir_all(&empty).unwrap();
        let err = info(&empty).unwrap_err();
        assert_eq!(error_code(&err), Some("no_history"));
    }

    #[test]
    fn truncated_xml_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let chain = read_chain(&dest).unwrap();
        let mhl = dest.join("ascmhl").join(&chain.entries[0].file);
        let xml = fs::read_to_string(&mhl).unwrap();
        // Coupé juste après un élément fermé : aucun faux « complet ».
        let cut = xml.find("</hashes>").unwrap();
        fs::write(&mhl, &xml[..cut]).unwrap();
        let err = read_generation(&mhl).unwrap_err().to_string();
        assert!(err.contains("tronqué"), "{err}");
        fs::write(&mhl, "").unwrap();
        assert!(read_generation(&mhl).is_err());
    }

    #[test]
    fn info_refuses_nested_history() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        fs::create_dir_all(dest.join("CLIP/ascmhl")).unwrap();
        let err = info(&dest).unwrap_err();
        assert_eq!(error_code(&err), Some("nested_history"));
    }

    #[test]
    fn double_escaped_chain_name_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("D");
        fs::create_dir_all(root.join("ascmhl")).unwrap();
        // Ancienne version : « & » du nom de fichier écrit deux fois échappé.
        let real = "0001_A&B_x.mhl";
        fs::write(root.join("ascmhl").join(real), "x").unwrap();
        fs::write(
            root.join("ascmhl/ascmhl_chain.xml"),
            "<ascmhldirectory><hashlist sequencenr=\"1\"><path>0001_A&amp;amp;B_x.mhl</path></hashlist></ascmhldirectory>",
        )
        .unwrap();
        let chain = read_chain(&root).unwrap();
        assert_eq!(chain.entries[0].file, real);
        assert!(chain.issues.is_empty());
    }

    #[test]
    fn error_codes_are_stable() {
        let dir = tempfile::tempdir().unwrap();
        let v1 = dir.path().join("0001_D_x.mhl");
        fs::write(&v1, "<hashlist version=\"1.0\"></hashlist>").unwrap();
        let err = read_generation(&v1).unwrap_err();
        assert_eq!(error_code(&err), Some("mhl_v1"));
        assert_eq!(error_code(&Error::Cancelled), None);
    }
}
