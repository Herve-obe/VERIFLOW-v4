//! Fichier projet `.veriflow` (charte §5.3).
//!
//! Un projet = une production. Il est stocké dans un seul fichier, qui est une
//! base SQLite : pas de serveur, copiable et archivable avec les rushes.

use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

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
];

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
        )
        .unwrap();
        p.record_offload(&job, &src).unwrap();
        let prev = p.previous_offloads(&fp).unwrap();
        assert_eq!(prev.len(), 1);
        assert_eq!(prev[0].source_name, "A001");
        assert!(prev[0].destinations[0].ends_with("A001"));
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
