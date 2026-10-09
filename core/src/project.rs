//! Fichier projet `.veriflow` (charte §5.3).
//!
//! Un projet = une production. Il est stocké dans un seul fichier, qui est une
//! base SQLite : pas de serveur, copiable et archivable avec les rushes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

use crate::player::logs::{Color, Marker};
use crate::report::{Report, ReportKind};
use crate::{Error, Result, VERSION};

/// Extension des fichiers projet.
pub const EXTENSION: &str = "veriflow";

/// Identifiant SQLite (`PRAGMA application_id`) propre à VERIFLOW : "VFLW".
const APPLICATION_ID: i64 = 0x5646_4C57;

/// Migrations successives du schéma. L'index + 1 est le numéro de version.
/// Ne jamais modifier une migration publiée : en ajouter une nouvelle.
const MIGRATIONS: &[&str] = &[
    // v1 : métadonnées du projet et journal d'activité.
    "CREATE TABLE meta (
         key   TEXT PRIMARY KEY,
         value TEXT NOT NULL
     );
     CREATE TABLE activity_log (
         id      INTEGER PRIMARY KEY,
         at      TEXT NOT NULL,
         kind    TEXT NOT NULL,
         message TEXT NOT NULL
     );",
    // v2 : historique des offloads (détection des cartes déjà copiées).
    "CREATE TABLE offloads (
         id           INTEGER PRIMARY KEY,
         started_at   TEXT NOT NULL,
         finished_at  TEXT NOT NULL,
         source_name  TEXT NOT NULL,
         source_path  TEXT NOT NULL,
         fingerprint  TEXT NOT NULL,
         files        INTEGER NOT NULL,
         bytes        INTEGER NOT NULL,
         failed_files INTEGER NOT NULL,
         cancelled    INTEGER NOT NULL,
         destinations TEXT NOT NULL,
         algorithms   TEXT NOT NULL,
         reports      TEXT NOT NULL
     );
     CREATE INDEX offloads_fingerprint ON offloads (fingerprint);
     CREATE TABLE offload_files (
         offload_id INTEGER NOT NULL REFERENCES offloads (id),
         rel        TEXT NOT NULL,
         size       INTEGER NOT NULL,
         modified   TEXT NOT NULL,
         hashes     TEXT NOT NULL,
         statuses   TEXT NOT NULL
     );",
    // v3 : métadonnées éditées par l'utilisateur (jamais écrites dans les originaux).
    "CREATE TABLE media_meta (
         path       TEXT NOT NULL,
         field      TEXT NOT NULL,
         value      TEXT NOT NULL,
         updated_at TEXT NOT NULL,
         PRIMARY KEY (path, field)
     );",
    // v4 : marqueurs et logs du PLAYER.
    "CREATE TABLE player_markers (
         id         INTEGER PRIMARY KEY,
         path       TEXT NOT NULL,
         frame      INTEGER NOT NULL,
         in_frame   INTEGER,
         out_frame  INTEGER,
         color      TEXT NOT NULL,
         comment    TEXT NOT NULL,
         scene      TEXT NOT NULL,
         take       TEXT NOT NULL,
         created_at TEXT NOT NULL,
         updated_at TEXT NOT NULL
     );
     CREATE INDEX player_markers_path ON player_markers (path, frame);",
    // v5 : rapports image et son (REPORT). Contenu en JSON (en-tête, lignes).
    "CREATE TABLE reports (
         id         INTEGER PRIMARY KEY,
         kind       TEXT NOT NULL,
         number     INTEGER NOT NULL,
         data       TEXT NOT NULL,
         created_at TEXT NOT NULL,
         updated_at TEXT NOT NULL
     );",
];

fn kind_id(kind: ReportKind) -> &'static str {
    match kind {
        ReportKind::Image => "image",
        ReportKind::Sound => "sound",
    }
}

/// Résumé d'un rapport pour la liste de l'onglet REPORT.
#[derive(Debug, Clone, Serialize)]
pub struct ReportSummary {
    pub id: i64,
    pub kind: ReportKind,
    pub number: u32,
    pub date: String,
    pub title: String,
    pub rows: usize,
    pub updated_at: String,
}

/// Version de schéma la plus récente connue de cette version de VERIFLOW.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Informations résumées d'un projet, envoyées à l'interface.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub created_at: String,
    pub created_with: String,
}

/// Offload déjà réalisé pour une carte.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviousOffload {
    pub finished_at: String,
    pub source_name: String,
    pub destinations: Vec<String>,
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// Projet ouvert.
pub struct Project {
    conn: Connection,
    path: PathBuf,
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Ajoute l'extension `.veriflow` si elle manque.
pub fn with_extension(path: &Path) -> PathBuf {
    match path.extension() {
        Some(ext) if ext.eq_ignore_ascii_case(EXTENSION) => path.to_path_buf(),
        _ => {
            let mut p = path.as_os_str().to_owned();
            p.push(".");
            p.push(EXTENSION);
            PathBuf::from(p)
        }
    }
}

impl Project {
    /// Crée un nouveau projet. Le nom par défaut est celui du fichier.
    pub fn create(path: &Path, name: Option<&str>) -> Result<Self> {
        let path = with_extension(path);
        if path.exists() {
            return Err(Error::AlreadyExists(path.display().to_string()));
        }
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "application_id", APPLICATION_ID)?;
        let project = Self { conn, path };
        project.migrate(0)?;

        let name = name
            .map(str::to_owned)
            .or_else(|| {
                project
                    .path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "Projet".to_owned());
        project.set_meta("name", &name)?;
        project.set_meta("created_at", &now())?;
        project.set_meta("created_with", VERSION)?;
        project.log("project", "Projet créé")?;
        Ok(project)
    }

    /// Ouvre un projet existant et met son schéma à jour si nécessaire.
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Err(Error::NotFound(path.display().to_string()));
        }
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let not_a_project = || Error::NotAProject(path.display().to_string());

        let app_id: i64 = conn
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(|_| not_a_project())?;
        if app_id != APPLICATION_ID {
            return Err(not_a_project());
        }
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(Error::SchemaTooNew {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        let project = Self {
            conn,
            path: path.to_path_buf(),
        };
        project.migrate(version)?;
        project.log("project", "Projet ouvert")?;
        Ok(project)
    }

    /// Applique les migrations manquantes dans une transaction.
    fn migrate(&self, from: i64) -> Result<()> {
        for (index, sql) in MIGRATIONS.iter().enumerate().skip(from as usize) {
            let version = index as i64 + 1;
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Ajoute une ligne au journal d'activité du projet.
    pub fn log(&self, kind: &str, message: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO activity_log (at, kind, message) VALUES (?1, ?2, ?3)",
            params![now(), kind, message],
        )?;
        Ok(())
    }

    /// Enregistre un offload terminé (ou interrompu) dans l'historique du projet.
    pub fn record_offload(
        &self,
        job: &crate::offload::job::JobResult,
        source: &Path,
    ) -> Result<i64> {
        let s = &job.summary;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO offloads (started_at, finished_at, source_name, source_path, fingerprint, files,
                 bytes, failed_files, cancelled, destinations, algorithms, reports)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                s.started_at,
                s.finished_at,
                job.source_name,
                source.display().to_string(),
                job.fingerprint,
                s.files.len() as i64,
                s.total_bytes as i64,
                s.failed_files as i64,
                s.cancelled,
                json(&job.roots),
                json(&s.files.first().map(|f| f.hashes.iter().map(|(a, _)| *a).collect::<Vec<_>>()).unwrap_or_default()),
                json(&job.reports),
            ],
        )?;
        let id = tx.last_insert_rowid();
        {
            let mut stmt = tx.prepare(
                "INSERT INTO offload_files (offload_id, rel, size, modified, hashes, statuses)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for f in &s.files {
                stmt.execute(params![
                    id,
                    f.rel,
                    f.size as i64,
                    f.modified,
                    json(&f.hashes),
                    json(&f.destinations)
                ])?;
            }
        }
        tx.commit()?;
        self.log(
            "offload",
            &format!(
                "{} : {} fichiers, {} en échec{}",
                job.source_name,
                s.files.len(),
                s.failed_files,
                if s.cancelled { ", interrompu" } else { "" }
            ),
        )?;
        Ok(id)
    }

    /// Offloads réussis d'une même carte (même empreinte d'inventaire).
    pub fn previous_offloads(&self, fingerprint: &str) -> Result<Vec<PreviousOffload>> {
        let mut stmt = self.conn.prepare(
            "SELECT finished_at, source_name, destinations FROM offloads
             WHERE fingerprint = ?1 AND failed_files = 0 AND cancelled = 0 ORDER BY id",
        )?;
        let rows = stmt.query_map([fingerprint], |r| {
            Ok(PreviousOffload {
                finished_at: r.get(0)?,
                source_name: r.get(1)?,
                destinations: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Dossiers de destination des offloads du projet (plus récents d'abord, sans doublon).
    pub fn offload_roots(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT destinations FROM offloads ORDER BY id DESC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out: Vec<String> = Vec::new();
        for row in rows {
            for d in serde_json::from_str::<Vec<String>>(&row?).unwrap_or_default() {
                if !out.contains(&d) {
                    out.push(d);
                }
            }
        }
        Ok(out)
    }

    /// Modifie des champs sur un ou plusieurs médias (édition par lot).
    /// Une valeur vide efface le champ. Renvoie le nombre de champs écrits.
    pub fn set_media_meta(
        &self,
        paths: &[String],
        values: &BTreeMap<String, String>,
    ) -> Result<usize> {
        use crate::media::fields::{is_valid, normalize};
        let tx = self.conn.unchecked_transaction()?;
        let mut count = 0;
        let at = now();
        for path in paths {
            for (field, value) in values {
                if !is_valid(field) {
                    continue;
                }
                let v = normalize(field, value);
                if v.is_empty() {
                    tx.execute(
                        "DELETE FROM media_meta WHERE path = ?1 AND field = ?2",
                        params![path, field],
                    )?;
                } else {
                    tx.execute(
                        "INSERT INTO media_meta (path, field, value, updated_at) VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT(path, field) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                        params![path, field, v, at],
                    )?;
                }
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Champs édités d'un média.
    pub fn media_meta(&self, path: &str) -> Result<BTreeMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT field, value FROM media_meta WHERE path = ?1")?;
        let rows = stmt.query_map([path], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Champs édités de tous les médias d'un dossier (préfixe de chemin).
    pub fn media_meta_under(
        &self,
        dir: &str,
    ) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, field, value FROM media_meta WHERE substr(path, 1, length(?1)) = ?1",
        )?;
        let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let rows = stmt.query_map([dir], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (p, f, v) = row?;
            out.entry(p).or_default().insert(f, v);
        }
        Ok(out)
    }

    /// Enregistre un marqueur : création si `id` vaut 0, sinon mise à jour.
    /// Renvoie le marqueur avec son identifiant.
    pub fn save_marker(&self, marker: &Marker) -> Result<Marker> {
        let at = now();
        let mut m = marker.clone();
        if m.id == 0 {
            self.conn.execute(
                "INSERT INTO player_markers (path, frame, in_frame, out_frame, color, comment, scene, take, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
                params![m.path, m.frame, m.in_frame, m.out_frame, m.color.id(), m.comment, m.scene, m.take, at],
            )?;
            m.id = self.conn.last_insert_rowid();
        } else {
            let n = self.conn.execute(
                "UPDATE player_markers SET path = ?2, frame = ?3, in_frame = ?4, out_frame = ?5, color = ?6,
                     comment = ?7, scene = ?8, take = ?9, updated_at = ?10 WHERE id = ?1",
                params![m.id, m.path, m.frame, m.in_frame, m.out_frame, m.color.id(), m.comment, m.scene, m.take, at],
            )?;
            if n == 0 {
                return Err(Error::NotFound(format!("marqueur {}", m.id)));
            }
        }
        Ok(m)
    }

    pub fn delete_marker(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM player_markers WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Marqueurs d'un média (tous les médias si `path` vaut `None`), par position.
    pub fn markers(&self, path: Option<&str>) -> Result<Vec<Marker>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, frame, in_frame, out_frame, color, comment, scene, take FROM player_markers
             WHERE ?1 IS NULL OR path = ?1 ORDER BY path, frame, id",
        )?;
        let rows = stmt.query_map([path], |r| {
            Ok(Marker {
                id: r.get(0)?,
                path: r.get(1)?,
                frame: r.get(2)?,
                in_frame: r.get(3)?,
                out_frame: r.get(4)?,
                color: Color::from_id(&r.get::<_, String>(5)?).unwrap_or_default(),
                comment: r.get(6)?,
                scene: r.get(7)?,
                take: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Enregistre un rapport : création si `id` vaut 0 (numéro suivant du même
    /// type si `number` vaut 0), sinon mise à jour.
    pub fn save_report(&self, report: &Report) -> Result<Report> {
        let at = now();
        let mut r = report.clone();
        let kind = kind_id(r.kind);
        if r.number == 0 {
            let max: Option<i64> = self.conn.query_row(
                "SELECT MAX(number) FROM reports WHERE kind = ?1",
                [kind],
                |row| row.get(0),
            )?;
            r.number = max.unwrap_or(0) as u32 + 1;
        }
        let data = serde_json::to_string(&r).map_err(|e| Error::Report(e.to_string()))?;
        if r.id == 0 {
            self.conn.execute(
                "INSERT INTO reports (kind, number, data, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                params![kind, r.number, data, at],
            )?;
            r.id = self.conn.last_insert_rowid();
        } else {
            let n = self.conn.execute(
                "UPDATE reports SET kind = ?2, number = ?3, data = ?4, updated_at = ?5 WHERE id = ?1",
                params![r.id, kind, r.number, data, at],
            )?;
            if n == 0 {
                return Err(Error::NotFound(format!("rapport {}", r.id)));
            }
        }
        Ok(r)
    }

    pub fn report(&self, id: i64) -> Result<Report> {
        let data: Option<String> = self
            .conn
            .query_row("SELECT data FROM reports WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        let data = data.ok_or_else(|| Error::NotFound(format!("rapport {id}")))?;
        let mut r: Report =
            serde_json::from_str(&data).map_err(|e| Error::Report(e.to_string()))?;
        r.id = id;
        Ok(r)
    }

    pub fn delete_report(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM reports WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Liste des rapports, les plus récents d'abord.
    pub fn reports(&self) -> Result<Vec<ReportSummary>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, data, updated_at FROM reports ORDER BY kind, number DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, data, updated_at) = row?;
            if let Ok(rep) = serde_json::from_str::<Report>(&data) {
                out.push(ReportSummary {
                    id,
                    kind: rep.kind,
                    number: rep.number,
                    date: rep.header("date").to_string(),
                    title: rep.header("title").to_string(),
                    rows: rep.rows.len(),
                    updated_at,
                });
            }
        }
        Ok(out)
    }

    pub fn info(&self) -> Result<ProjectInfo> {
        Ok(ProjectInfo {
            path: self.path.display().to_string(),
            name: self.meta("name")?.unwrap_or_default(),
            created_at: self.meta("created_at")?.unwrap_or_default(),
            created_with: self.meta("created_with")?.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_are_numbered_per_kind_and_saved() {
        let dir = tempfile::tempdir().unwrap();
        let p = Project::create(&dir.path().join("r"), None).unwrap();
        let a = p.save_report(&Report::new(ReportKind::Image)).unwrap();
        let b = p.save_report(&Report::new(ReportKind::Image)).unwrap();
        let s = p.save_report(&Report::new(ReportKind::Sound)).unwrap();
        assert_eq!((a.number, b.number, s.number), (1, 2, 1));
        let mut b2 = p.report(b.id).unwrap();
        b2.header.insert("title".into(), "Film".into());
        p.save_report(&b2).unwrap();
        assert_eq!(p.report(b.id).unwrap().header("title"), "Film");
        let list = p.reports().unwrap();
        assert_eq!(list.len(), 3);
        assert!(list.iter().any(|r| r.title == "Film" && r.number == 2));
        p.delete_report(a.id).unwrap();
        assert_eq!(p.reports().unwrap().len(), 2);
        assert!(p.report(a.id).is_err());
    }

    #[test]
    fn markers_are_saved_updated_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let p = Project::create(&dir.path().join("m"), None).unwrap();
        let mut m = Marker {
            id: 0,
            path: "/r/a.mov".into(),
            frame: 50,
            in_frame: Some(25),
            out_frame: Some(74),
            color: Color::Green,
            comment: "Bonne".into(),
            scene: "12A".into(),
            take: "3".into(),
        };
        m = p.save_marker(&m).unwrap();
        assert!(m.id > 0);
        let other = p
            .save_marker(&Marker {
                id: 0,
                path: "/r/b.mov".into(),
                frame: 3,
                in_frame: None,
                out_frame: None,
                ..m.clone()
            })
            .unwrap();
        m.comment = "Très bonne".into();
        m.color = Color::Blue;
        p.save_marker(&m).unwrap();
        assert_eq!(p.markers(Some("/r/a.mov")).unwrap(), vec![m.clone()]);
        assert_eq!(p.markers(None).unwrap().len(), 2);
        p.delete_marker(other.id).unwrap();
        assert_eq!(p.markers(None).unwrap(), vec![m.clone()]);
        assert!(p.save_marker(&Marker { id: 999, ..m }).is_err());
    }

    #[test]
    fn create_then_open_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Tournage");
        let created = Project::create(&path, None).unwrap();
        assert_eq!(created.path().extension().unwrap(), EXTENSION);
        let info = created.info().unwrap();
        assert_eq!(info.name, "Tournage");
        assert_eq!(info.created_with, VERSION);
        drop(created);

        let opened = Project::open(&dir.path().join("Tournage.veriflow")).unwrap();
        assert_eq!(opened.info().unwrap(), info);
    }

    #[test]
    fn create_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.veriflow");
        Project::create(&path, Some("A")).unwrap();
        assert!(matches!(
            Project::create(&path, Some("B")),
            Err(Error::AlreadyExists(_))
        ));
    }

    #[test]
    fn open_rejects_foreign_sqlite_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("other.veriflow");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (x)")
            .unwrap();
        assert!(matches!(Project::open(&path), Err(Error::NotAProject(_))));
    }

    #[test]
    fn open_rejects_non_sqlite_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text.veriflow");
        std::fs::write(&path, b"ceci n'est pas une base").unwrap();
        assert!(Project::open(&path).is_err());
    }

    #[test]
    fn open_rejects_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("future.veriflow");
        let p = Project::create(&path, None).unwrap();
        p.conn
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(p);
        assert!(matches!(
            Project::open(&path),
            Err(Error::SchemaTooNew { .. })
        ));
    }

    #[test]
    fn records_offloads_and_finds_previous_copies() {
        use crate::offload::hash::HashAlgo;
        use crate::offload::job::{execute, preflight, OffloadRequest, TemplateVars};
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("A001");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("C0001.MP4"), b"video").unwrap();
        let req = OffloadRequest {
            source: src.clone(),
            destinations: vec![dir.path().join("SSD1")],
            template: "{carte}".into(),
            vars: TemplateVars::default(),
            algorithms: vec![HashAlgo::Xxh128],
            operator: None,
            notes: None,
            existing: Default::default(),
            date: None,
            roots: None,
        };
        let p = Project::create(&dir.path().join("p.veriflow"), None).unwrap();
        let pre = preflight(&req).unwrap();
        let fp = pre.fingerprint.clone();
        assert!(p.previous_offloads(&fp).unwrap().is_empty());
        let job = execute(
            &req,
            pre,
            &std::sync::atomic::AtomicBool::new(false),
            |_| {},
            None,
            None,
        )
        .unwrap();
        p.record_offload(&job, &src).unwrap();
        let prev = p.previous_offloads(&fp).unwrap();
        assert_eq!(prev.len(), 1);
        assert_eq!(p.offload_roots().unwrap(), prev[0].destinations);
        assert_eq!(prev[0].source_name, "A001");
        assert!(prev[0].destinations[0].ends_with("A001"));
    }

    #[test]
    fn edits_media_metadata_in_batch() {
        let dir = tempfile::tempdir().unwrap();
        let p = Project::create(&dir.path().join("p.veriflow"), None).unwrap();
        let paths = vec!["/r/A/C1.MOV".to_string(), "/r/A/C2.MOV".to_string()];
        let mut v = BTreeMap::new();
        v.insert("scene".into(), "12".into());
        v.insert("circled".into(), "oui".into());
        v.insert("inconnu".into(), "x".into());
        assert_eq!(p.set_media_meta(&paths, &v).unwrap(), 4);
        let m = p.media_meta("/r/A/C2.MOV").unwrap();
        assert_eq!(m.get("scene").map(String::as_str), Some("12"));
        assert_eq!(m.get("circled").map(String::as_str), Some("true"));
        assert!(!m.contains_key("inconnu"));
        // Valeur vide : champ effacé pour un seul média.
        let mut clear = BTreeMap::new();
        clear.insert("scene".into(), String::new());
        p.set_media_meta(&paths[..1], &clear).unwrap();
        let all = p.media_meta_under("/r/A/").unwrap();
        assert!(!all["/r/A/C1.MOV"].contains_key("scene"));
        assert_eq!(all["/r/A/C2.MOV"]["scene"], "12");
    }

    #[test]
    fn open_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Project::open(&dir.path().join("absent.veriflow")),
            Err(Error::NotFound(_))
        ));
    }
}
