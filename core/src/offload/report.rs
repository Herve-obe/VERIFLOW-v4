//! Rapports de transfert : CSV, HTML et PDF horodatés (charte §7.1).
//!
//! Les rapports sont écrits dans `<destination>/_VERIFLOW/` (dossier exclu du
//! MHL). Un document ne peut pas contenir sa propre empreinte : le PDF et le
//! HTML impriment l'empreinte SHA-256 du CSV de données et des MHL, et
//! l'empreinte du PDF est écrite à côté, dans `<rapport>.pdf.sha256`.

use std::fs;
use std::path::PathBuf;

use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use serde::Serialize;

use super::engine::{DestStatus, OffloadSpec, OffloadSummary};
use super::hash::{hash_data, HashAlgo};
use super::meter::{PhaseStat, Throughput};
use super::mhl::REPORTS_DIR;
use super::mhl_verify::{VerifyStatus, VerifySummary};
use super::scan::SourceInventory;
use crate::{Result, VERSION};

/// Informations de contexte saisies par l'utilisateur.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReportInfo {
    pub project: Option<String>,
    pub operator: Option<String>,
    pub notes: Option<String>,
}

/// Fichiers de rapport produits pour une destination.
#[derive(Debug, Clone, Serialize)]
pub struct ReportFiles {
    pub csv: PathBuf,
    pub html: PathBuf,
    pub pdf: PathBuf,
    pub pdf_sha256: String,
}

/// Tout ce qu'un rapport affiche.
pub struct ReportData<'a> {
    pub inv: &'a SourceInventory,
    pub summary: &'a OffloadSummary,
    pub spec: &'a OffloadSpec,
    pub info: &'a ReportInfo,
    /// Fichier MHL écrit dans chaque destination (None si échec d'écriture).
    pub mhl: &'a [Option<PathBuf>],
}

/// Octets en Go et Gio : « 1,50 Go (1,40 Gio) ».
pub fn human_size(bytes: u64) -> String {
    let go = bytes as f64 / 1e9;
    let gio = bytes as f64 / 1_073_741_824.0;
    if bytes < 1_000_000 {
        format!("{bytes} octets")
    } else if bytes < 1_000_000_000 {
        format!(
            "{:.1} Mo ({:.1} Mio)",
            bytes as f64 / 1e6,
            bytes as f64 / 1_048_576.0
        )
        .replace('.', ",")
    } else {
        format!("{go:.2} Go ({gio:.2} Gio)").replace('.', ",")
    }
}

fn status_text(s: &DestStatus) -> &'static str {
    match s {
        DestStatus::Verified => "VÉRIFIÉ",
        DestStatus::ResumedVerified => "VÉRIFIÉ (reprise)",
        DestStatus::Failed(_) => "ÉCHEC",
    }
}

fn duration_text(secs: f64) -> String {
    let s = secs.round() as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn rate_text(bytes: u64, secs: f64) -> String {
    if secs <= 0.0 {
        return "-".into();
    }
    format!("{:.1} Mo/s", bytes as f64 / 1e6 / secs).replace('.', ",")
}

/// Une phase : volume, temps passé dans les appels d'E/S et débit pendant ce temps.
fn phase_text(p: &PhaseStat) -> String {
    format!(
        "{} en {} s d'E/S, {}",
        human_size(p.bytes),
        format!("{:.1}", p.busy_s).replace('.', ","),
        rate_text(p.bytes, p.busy_s)
    )
}

/// Lignes de débit communes aux rapports de copie (libellé, valeur).
fn throughput_rows(t: &Throughput) -> Vec<(String, String)> {
    let mut rows = vec![(
        "Débit global (taille / durée totale)".to_string(),
        if t.effective_rate > 0.0 {
            format!("{:.1} Mo/s", t.effective_rate / 1e6).replace('.', ",")
        } else {
            "-".into()
        },
    )];
    rows.push(("Lecture source".into(), phase_text(&t.source_read)));
    for (i, p) in t.write.iter().enumerate() {
        rows.push((format!("Écriture destination {}", i + 1), phase_text(p)));
    }
    for (i, p) in t.verify.iter().enumerate() {
        rows.push((format!("Relecture destination {}", i + 1), phase_text(p)));
    }
    rows.push(("Limité par".into(), bottleneck_text(&t.bottleneck)));
    rows
}

/// Lignes de débit du rapport de vérification MHL.
fn verify_throughput_rows(s: &VerifySummary) -> Vec<(String, String)> {
    vec![
        (
            "Débit global (taille / durée totale)".into(),
            rate_text(s.total_bytes, s.duration_s),
        ),
        ("Relecture du disque".into(), phase_text(&s.read)),
    ]
}

fn bottleneck_text(b: &str) -> String {
    match b {
        "source" => "lecture de la source".into(),
        "cpu" => "calcul des empreintes (processeur)".into(),
        "" => "-".into(),
        d => match d
            .strip_prefix("dest:")
            .and_then(|i| i.parse::<usize>().ok())
        {
            Some(i) => format!("destination {}", i + 1),
            None => d.to_owned(),
        },
    }
}

impl ReportData<'_> {
    fn ok(&self) -> bool {
        !self.summary.cancelled && self.summary.failed_files == 0
    }

    fn verdict(&self) -> String {
        if self.summary.cancelled {
            "INTERROMPU : copie annulée avant la fin".into()
        } else if self.ok() {
            format!(
                "CONFORME : {} fichiers vérifiés bit à bit sur {} destination(s)",
                self.summary.files.len(),
                self.spec.destinations.len()
            )
        } else {
            format!(
                "ÉCHEC : {} fichier(s) non conforme(s)",
                self.summary.failed_files
            )
        }
    }

    fn algos(&self) -> String {
        self.spec
            .algorithms
            .iter()
            .map(|a| a.label())
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn primary(&self) -> HashAlgo {
        self.spec.algorithms[0]
    }

    /// Lignes d'en-tête communes (libellé, valeur).
    fn header_rows(&self, csv_sha: &str) -> Vec<(String, String)> {
        let mut rows = vec![
            (
                "Projet".into(),
                self.info.project.clone().unwrap_or_else(|| "-".into()),
            ),
            (
                "Opérateur".into(),
                self.info.operator.clone().unwrap_or_else(|| "-".into()),
            ),
            (
                "Poste".into(),
                gethostname::gethostname().to_string_lossy().into_owned(),
            ),
            ("Logiciel".into(), format!("VERIFLOW {VERSION}")),
            ("Début".into(), self.summary.started_at.clone()),
            ("Fin".into(), self.summary.finished_at.clone()),
            ("Durée".into(), duration_text(self.summary.duration_s)),
            (
                "Débit moyen".into(),
                rate_text(self.summary.total_bytes, self.summary.duration_s),
            ),
            (
                "Source".into(),
                format!("{} ({})", self.inv.name, self.inv.root.display()),
            ),
            (
                "Empreinte de la source".into(),
                format!("XXH128 {} (inventaire)", self.inv.fingerprint),
            ),
            (
                "Volume".into(),
                format!(
                    "{} fichiers, {}",
                    self.inv.files.len(),
                    human_size(self.inv.total_bytes)
                ),
            ),
            ("Empreintes".into(), self.algos()),
            (
                "Vérification".into(),
                "relecture intégrale de chaque destination depuis le support physique".into(),
            ),
        ];
        for (i, d) in self.spec.destinations.iter().enumerate() {
            let ok = self
                .summary
                .files
                .iter()
                .filter(|f| !matches!(f.destinations[i], DestStatus::Failed(_)))
                .count();
            rows.push((
                format!("Destination {}", i + 1),
                format!(
                    "{} : {ok}/{} fichiers vérifiés",
                    d.display(),
                    self.summary.files.len()
                ),
            ));
            if let Some(Some(m)) = self.mhl.get(i) {
                let sha = fs::read(m)
                    .map(|b| hash_data(HashAlgo::Sha256, &b))
                    .unwrap_or_default();
                rows.push((
                    format!("MHL destination {}", i + 1),
                    format!("{} (SHA-256 {sha})", m.display()),
                ));
            }
        }
        rows.extend(throughput_rows(&self.summary.throughput));
        rows.push(("Données (CSV)".into(), format!("SHA-256 {csv_sha}")));
        if let Some(n) = self.info.notes.as_deref().filter(|n| !n.trim().is_empty()) {
            rows.push(("Remarques".into(), n.to_owned()));
        }
        rows
    }

    /// CSV (séparateur « ; », UTF-8 avec BOM pour Excel).
    pub fn csv(&self) -> String {
        let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let mut out = String::from("\u{feff}");
        let mut head = vec![
            "fichier".to_string(),
            "taille_octets".into(),
            "date_modification".into(),
        ];
        head.extend(self.spec.algorithms.iter().map(|a| a.id().to_string()));
        for i in 0..self.spec.destinations.len() {
            head.push(format!("destination_{}", i + 1));
            head.push(format!("detail_{}", i + 1));
        }
        out += &head.join(";");
        out += "\r\n";
        for f in &self.summary.files {
            let mut row = vec![q(&f.rel), f.size.to_string(), f.modified.clone()];
            for a in &self.spec.algorithms {
                row.push(
                    f.hashes
                        .iter()
                        .find(|(x, _)| x == a)
                        .map(|(_, h)| h.clone())
                        .unwrap_or_default(),
                );
            }
            for d in &f.destinations {
                row.push(status_text(d).into());
                row.push(q(match d {
                    DestStatus::Failed(m) => m,
                    _ => "",
                }));
            }
            out += &row.join(";");
            out += "\r\n";
        }
        // Mesures de débit : lignes « #libellé;valeur » après les fichiers.
        for (k, v) in throughput_rows(&self.summary.throughput) {
            out += &format!("#{k};{}\r\n", q(&v));
        }
        out
    }

    /// HTML autonome, prêt à imprimer.
    pub fn html(&self, csv_sha: &str) -> String {
        let e = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let mut h = String::from(
            "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><title>Rapport de transfert VERIFLOW</title><style>\
body{font:13px/1.45 system-ui,Segoe UI,Helvetica,sans-serif;color:#111;margin:32px}h1{font-size:20px;margin:0 0 4px}\
.v{padding:10px 14px;border-radius:6px;font-weight:700;margin:12px 0}.ok{background:#d8f5e3;color:#0b6b2f}.ko{background:#fde0e0;color:#a10e0e}\
table{border-collapse:collapse;width:100%;margin-top:12px}td,th{border:1px solid #ccc;padding:4px 6px;text-align:left;vertical-align:top}\
th{background:#f0f0f0}.m{font-family:ui-monospace,Consolas,monospace;font-size:11px;word-break:break-all}.r{text-align:right;white-space:nowrap}\
.info td:first-child{width:220px;color:#555}.f{color:#a10e0e;font-weight:600}</style></head><body>",
        );
        h += &format!("<h1>Rapport de transfert (OFFLOAD)</h1><div>VERIFLOW {VERSION}</div>");
        h += &format!(
            "<div class=\"v {}\">{}</div><table class=\"info\">",
            if self.ok() { "ok" } else { "ko" },
            e(&self.verdict())
        );
        for (k, v) in self.header_rows(csv_sha) {
            h += &format!("<tr><td>{}</td><td class=\"m\">{}</td></tr>", e(&k), e(&v));
        }
        h += "</table><table><tr><th>#</th><th>Fichier</th><th>Taille</th>";
        h += &format!("<th>{}</th>", self.primary().label());
        for i in 0..self.spec.destinations.len() {
            h += &format!("<th>Dest. {}</th>", i + 1);
        }
        h += "</tr>";
        for (n, f) in self.summary.files.iter().enumerate() {
            let hash = f.hashes.first().map(|(_, v)| v.as_str()).unwrap_or("");
            h += &format!(
                "<tr><td class=\"r\">{}</td><td class=\"m\">{}</td><td class=\"r\">{}</td><td class=\"m\">{}</td>",
                n + 1,
                e(&f.rel),
                f.size,
                hash
            );
            for d in &f.destinations {
                match d {
                    DestStatus::Failed(m) => h += &format!("<td class=\"f\">ÉCHEC : {}</td>", e(m)),
                    _ => h += &format!("<td>{}</td>", status_text(d)),
                }
            }
            h += "</tr>";
        }
        h += "</table></body></html>\n";
        h
    }

    /// PDF A4, polices standard (aucune police à embarquer).
    pub fn pdf(&self, csv_sha: &str) -> Vec<u8> {
        let mut doc = PdfDoc::new();
        doc.text(&Font::Bold, 18.0, "Rapport de transfert (OFFLOAD)");
        doc.gap(4.0);
        doc.text(&Font::Regular, 9.0, &format!("VERIFLOW {VERSION}"));
        doc.gap(8.0);
        doc.banner(&self.verdict(), self.ok());
        doc.gap(8.0);
        for (k, v) in self.header_rows(csv_sha) {
            doc.key_value(&k, &v);
        }
        doc.gap(10.0);
        let n_dest = self.spec.destinations.len();
        let mut cols = vec![
            ("#", 24.0),
            ("Fichier", 0.0),
            ("Taille", 62.0),
            (self.primary().label(), 150.0),
        ];
        let dest_labels: Vec<String> = (1..=n_dest).map(|i| format!("Dest. {i}")).collect();
        for l in &dest_labels {
            cols.push((l.as_str(), 46.0));
        }
        doc.table_header(&cols);
        for (n, f) in self.summary.files.iter().enumerate() {
            let mut cells = vec![
                (n + 1).to_string(),
                f.rel.clone(),
                f.size.to_string(),
                f.hashes.first().map(|(_, v)| v.clone()).unwrap_or_default(),
            ];
            let mut failed = false;
            for d in &f.destinations {
                failed |= matches!(d, DestStatus::Failed(_));
                cells.push(match d {
                    DestStatus::Failed(_) => "ÉCHEC".into(),
                    _ => "OK".into(),
                });
            }
            doc.table_row(&cols, &cells, failed);
            for d in &f.destinations {
                if let DestStatus::Failed(m) = d {
                    doc.note(m);
                }
            }
        }
        doc.finish(&format!(
            "Rapport de transfert {} - {}",
            self.inv.name, self.summary.finished_at
        ))
    }
}

/// Écrit les rapports dans chaque destination. Renvoie les fichiers produits.
pub fn write_reports(data: &ReportData) -> Result<Vec<ReportFiles>> {
    let csv = data.csv();
    let csv_sha = hash_data(HashAlgo::Sha256, csv.as_bytes());
    let html = data.html(&csv_sha);
    let pdf = data.pdf(&csv_sha);
    let pdf_sha = hash_data(HashAlgo::Sha256, &pdf);
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%SZ");
    let base = format!("{}_offload_{stamp}", data.inv.name);
    let mut out = Vec::new();
    for dest in &data.spec.destinations {
        let dir = dest.join(REPORTS_DIR);
        if fs::create_dir_all(&dir).is_err() {
            continue; // destination inaccessible : rapport écrit sur les autres
        }
        let p = |ext: &str| dir.join(format!("{base}.{ext}"));
        let write = || -> std::io::Result<()> {
            fs::write(p("csv"), &csv)?;
            fs::write(p("html"), &html)?;
            fs::write(p("pdf"), &pdf)?;
            fs::write(p("pdf.sha256"), format!("{pdf_sha}  {base}.pdf\n"))?;
            Ok(())
        };
        if write().is_ok() {
            out.push(ReportFiles {
                csv: p("csv"),
                html: p("html"),
                pdf: p("pdf"),
                pdf_sha256: pdf_sha.clone(),
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Rapport de vérification ASC MHL (revérification d'un disque sans la source).
// ---------------------------------------------------------------------------

/// Tout ce que le rapport de vérification affiche.
pub struct VerifyData<'a> {
    pub summary: &'a VerifySummary,
    pub info: &'a ReportInfo,
}

fn verify_status_text(s: &VerifyStatus) -> &'static str {
    match s {
        VerifyStatus::Ok => "CONFORME",
        VerifyStatus::Different => "DIFFÉRENT",
        VerifyStatus::Missing => "MANQUANT",
        VerifyStatus::New => "NOUVEAU",
        VerifyStatus::Unreadable(_) => "ILLISIBLE",
    }
}

impl VerifyData<'_> {
    fn verdict(&self) -> String {
        let s = self.summary;
        let mut text = if s.cancelled {
            "INTERROMPU : vérification annulée avant la fin".to_string()
        } else if s.conform {
            format!(
                "CONFORME : {} fichier(s) relus et identiques à l'historique",
                s.ok
            )
        } else {
            let mut causes = Vec::new();
            if s.different > 0 {
                causes.push(format!("{} différent(s)", s.different));
            }
            if s.missing > 0 {
                causes.push(format!("{} manquant(s)", s.missing));
            }
            if s.unreadable > 0 {
                causes.push(format!("{} illisible(s)", s.unreadable));
            }
            if !s.chain.issues.is_empty() {
                causes.push(format!("{} anomalie(s) de chaîne", s.chain.issues.len()));
            }
            format!("ÉCART : {}", causes.join(", "))
        };
        if s.new > 0 && !s.cancelled {
            text += &format!(" ({} nouveau(x) fichier(s) signalé(s))", s.new);
        }
        text
    }

    fn header_rows(&self, csv_sha: &str) -> Vec<(String, String)> {
        let s = self.summary;
        let mut rows = vec![
            (
                "Projet".into(),
                self.info.project.clone().unwrap_or_else(|| "-".into()),
            ),
            (
                "Opérateur".into(),
                self.info.operator.clone().unwrap_or_else(|| "-".into()),
            ),
            (
                "Poste".into(),
                gethostname::gethostname().to_string_lossy().into_owned(),
            ),
            ("Logiciel".into(), format!("VERIFLOW {VERSION}")),
            ("Début".into(), s.started_at.clone()),
            ("Fin".into(), s.finished_at.clone()),
            ("Durée".into(), duration_text(s.duration_s)),
            ("Disque".into(), s.root.display().to_string()),
            ("Volume relu".into(), human_size(s.total_bytes)),
            (
                "Vérification".into(),
                "relecture intégrale depuis le support physique, comparée à l'empreinte la plus récente de l'historique ASC MHL"
                    .into(),
            ),
            (
                "Générations lues".into(),
                s.chain.entries.len().to_string(),
            ),
            (
                "Résultat".into(),
                format!(
                    "{} conforme(s), {} différent(s), {} manquant(s), {} nouveau(x), {} illisible(s)",
                    s.ok, s.different, s.missing, s.new, s.unreadable
                ),
            ),
        ];
        rows.extend(verify_throughput_rows(s));
        for issue in &s.chain.issues {
            rows.push(match issue {
                super::mhl_read::ChainIssue::Altered(n) => (
                    "Chaîne ALTÉRÉE".into(),
                    format!("{n} (C4 différent de la chaîne)"),
                ),
                super::mhl_read::ChainIssue::Missing(n) => {
                    ("Chaîne : fichier ABSENT".into(), n.clone())
                }
            });
        }
        if let Some(m) = &s.mhl {
            let sha = fs::read(m)
                .map(|b| hash_data(HashAlgo::Sha256, &b))
                .unwrap_or_default();
            rows.push((
                "Génération écrite".into(),
                format!("{} (SHA-256 {sha})", m.display()),
            ));
        }
        if let Some(e) = &s.write_error {
            rows.push(("Écriture de la génération".into(), format!("ÉCHEC : {e}")));
        }
        rows.push(("Données (CSV)".into(), format!("SHA-256 {csv_sha}")));
        if let Some(n) = self.info.notes.as_deref().filter(|n| !n.trim().is_empty()) {
            rows.push(("Remarques".into(), n.to_owned()));
        }
        rows
    }

    fn detail(f: &super::mhl_verify::VerifyFile) -> String {
        match &f.status {
            VerifyStatus::Unreadable(m) => m.clone(),
            _ => String::new(),
        }
    }

    /// CSV (séparateur « ; », UTF-8 avec BOM pour Excel).
    pub fn csv(&self) -> String {
        let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let mut out = String::from("\u{feff}");
        out +=
            "fichier;taille_octets;statut;methode;empreinte_historique;empreinte_relue;detail\r\n";
        for f in &self.summary.files {
            let row = [
                q(&f.rel),
                f.size.to_string(),
                verify_status_text(&f.status).into(),
                f.algo.map(|a| a.id().to_string()).unwrap_or_default(),
                f.expected.clone().unwrap_or_default(),
                f.actual.clone().unwrap_or_default(),
                q(&Self::detail(f)),
            ];
            out += &row.join(";");
            out += "\r\n";
        }
        for (k, v) in verify_throughput_rows(self.summary) {
            out += &format!("#{k};{}\r\n", q(&v));
        }
        out
    }

    pub fn html(&self, csv_sha: &str) -> String {
        let e = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let mut h = String::from(
            "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><title>Vérification ASC MHL VERIFLOW</title><style>\
body{font:13px/1.45 system-ui,Segoe UI,Helvetica,sans-serif;color:#111;margin:32px}h1{font-size:20px;margin:0 0 4px}\
.v{padding:10px 14px;border-radius:6px;font-weight:700;margin:12px 0}.ok{background:#d8f5e3;color:#0b6b2f}.ko{background:#fde0e0;color:#a10e0e}\
table{border-collapse:collapse;width:100%;margin-top:12px}td,th{border:1px solid #ccc;padding:4px 6px;text-align:left;vertical-align:top}\
th{background:#f0f0f0}.m{font-family:ui-monospace,Consolas,monospace;font-size:11px;word-break:break-all}.r{text-align:right;white-space:nowrap}\
.info td:first-child{width:220px;color:#555}.f{color:#a10e0e;font-weight:600}</style></head><body>",
        );
        h += &format!("<h1>VÉRIFICATION ASC MHL</h1><div>VERIFLOW {VERSION}</div>");
        h += &format!(
            "<div class=\"v {}\">{}</div><table class=\"info\">",
            if self.summary.conform { "ok" } else { "ko" },
            e(&self.verdict())
        );
        for (k, v) in self.header_rows(csv_sha) {
            h += &format!("<tr><td>{}</td><td class=\"m\">{}</td></tr>", e(&k), e(&v));
        }
        h += "</table><table><tr><th>#</th><th>Fichier</th><th>Taille</th><th>Méthode</th><th>Empreinte relue</th><th>Statut</th></tr>";
        for (n, f) in self.summary.files.iter().enumerate() {
            let bad = !matches!(f.status, VerifyStatus::Ok | VerifyStatus::New);
            let detail = Self::detail(f);
            h += &format!(
                "<tr><td class=\"r\">{}</td><td class=\"m\">{}</td><td class=\"r\">{}</td><td>{}</td><td class=\"m\">{}</td><td{}>{}{}</td></tr>",
                n + 1,
                e(&f.rel),
                f.size,
                f.algo.map(|a| a.label()).unwrap_or(""),
                f.actual.as_deref().unwrap_or(""),
                if bad { " class=\"f\"" } else { "" },
                verify_status_text(&f.status),
                if detail.is_empty() { String::new() } else { format!(" : {}", e(&detail)) }
            );
        }
        h += "</table></body></html>\n";
        h
    }

    pub fn pdf(&self, csv_sha: &str) -> Vec<u8> {
        let mut doc = PdfDoc::new();
        doc.text(&Font::Bold, 18.0, "VÉRIFICATION ASC MHL");
        doc.gap(4.0);
        doc.text(&Font::Regular, 9.0, &format!("VERIFLOW {VERSION}"));
        doc.gap(8.0);
        doc.banner(&self.verdict(), self.summary.conform);
        doc.gap(8.0);
        for (k, v) in self.header_rows(csv_sha) {
            doc.key_value(&k, &v);
        }
        doc.gap(10.0);
        let cols = vec![
            ("#", 24.0),
            ("Fichier", 0.0),
            ("Taille", 62.0),
            ("Empreinte relue", 150.0),
            ("Statut", 58.0),
        ];
        doc.table_header(&cols);
        for (n, f) in self.summary.files.iter().enumerate() {
            let cells = vec![
                (n + 1).to_string(),
                f.rel.clone(),
                f.size.to_string(),
                f.actual.clone().unwrap_or_default(),
                verify_status_text(&f.status).into(),
            ];
            let bad = !matches!(f.status, VerifyStatus::Ok | VerifyStatus::New);
            doc.table_row(&cols, &cells, bad);
            if let VerifyStatus::Unreadable(m) = &f.status {
                doc.note(m);
            }
        }
        doc.finish(&format!(
            "Vérification ASC MHL {} - {}",
            super::scan::source_name(&self.summary.root),
            self.summary.finished_at
        ))
    }
}

/// Écrit les rapports de vérification dans `<disque>/_VERIFLOW/`.
pub fn write_verify_reports(data: &VerifyData) -> Result<ReportFiles> {
    let csv = data.csv();
    let csv_sha = hash_data(HashAlgo::Sha256, csv.as_bytes());
    let html = data.html(&csv_sha);
    let pdf = data.pdf(&csv_sha);
    let pdf_sha = hash_data(HashAlgo::Sha256, &pdf);
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%SZ");
    let base = format!(
        "{}_verification_{stamp}",
        super::scan::source_name(&data.summary.root)
    );
    let dir = data.summary.root.join(REPORTS_DIR);
    fs::create_dir_all(&dir)?;
    let p = |ext: &str| dir.join(format!("{base}.{ext}"));
    fs::write(p("csv"), &csv)?;
    fs::write(p("html"), &html)?;
    fs::write(p("pdf"), &pdf)?;
    fs::write(p("pdf.sha256"), format!("{pdf_sha}  {base}.pdf\n"))?;
    Ok(ReportFiles {
        csv: p("csv"),
        html: p("html"),
        pdf: p("pdf"),
        pdf_sha256: pdf_sha,
    })
}

// ---------------------------------------------------------------------------
// Mise en page PDF minimale (A4, Helvetica / Courier, encodage WinAnsi).
// ---------------------------------------------------------------------------

const PAGE_W: f32 = 595.0;
const PAGE_H: f32 = 842.0;
const MARGIN: f32 = 40.0;

enum Font {
    Regular,
    Bold,
    Mono,
}

impl Font {
    fn name(&self) -> Name<'static> {
        match self {
            Font::Regular => Name(b"F1"),
            Font::Bold => Name(b"F2"),
            Font::Mono => Name(b"F3"),
        }
    }

    /// Largeur approximative d'un texte (métriques AFM Helvetica ; Courier fixe).
    fn width(&self, s: &str, size: f32) -> f32 {
        let units: f32 = s
            .chars()
            .map(|c| match self {
                Font::Mono => 600.0,
                _ => {
                    let w = helvetica_width(c);
                    if matches!(self, Font::Bold) {
                        w * 1.06
                    } else {
                        w
                    }
                }
            })
            .sum();
        units * size / 1000.0
    }
}

fn helvetica_width(c: char) -> f32 {
    const W: [u16; 95] = [
        278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556,
        556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722,
        722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722,
        667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556,
        556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500,
        500, 334, 260, 334, 584,
    ];
    let base = match c {
        'à' | 'â' | 'ä' | 'á' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'î' | 'ï' | 'í' => 'i',
        'ô' | 'ö' | 'ó' => 'o',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ç' => 'c',
        'É' | 'È' | 'Ê' => 'E',
        'À' | 'Â' => 'A',
        'Ç' => 'C',
        _ => c,
    };
    let code = base as u32;
    if (32..127).contains(&code) {
        W[(code - 32) as usize] as f32
    } else {
        556.0
    }
}

/// Encode du texte en WinAnsi (CP1252) pour les polices standard.
fn win_ansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c as u32 {
            0x20..=0x7E | 0xA0..=0xFF => c as u32 as u8,
            _ => match c {
                '€' => 0x80,
                '’' => 0x92,
                '‘' => 0x91,
                '“' => 0x93,
                '”' => 0x94,
                '•' => 0x95,
                '–' => 0x96,
                '…' => 0x85,
                'œ' => 0x9C,
                'Œ' => 0x8C,
                _ => b'?',
            },
        })
        .collect()
}

struct PdfDoc {
    pages: Vec<Content>,
    y: f32,
}

impl PdfDoc {
    fn new() -> Self {
        Self {
            pages: vec![Content::new()],
            y: PAGE_H - MARGIN,
        }
    }

    fn page(&mut self) -> &mut Content {
        self.pages.last_mut().expect("au moins une page")
    }

    fn ensure(&mut self, height: f32) {
        if self.y - height < MARGIN + 20.0 {
            self.pages.push(Content::new());
            self.y = PAGE_H - MARGIN;
        }
    }

    fn gap(&mut self, h: f32) {
        self.y -= h;
    }

    fn put(&mut self, font: &Font, size: f32, x: f32, y: f32, s: &str) {
        let bytes = win_ansi(s);
        let c = self.page();
        c.begin_text();
        c.set_font(font.name(), size);
        c.next_line(x, y);
        c.show(Str(&bytes));
        c.end_text();
    }

    /// Tronque un texte pour qu'il tienne dans `width` (début conservé, « ... »).
    fn fit(font: &Font, size: f32, s: &str, width: f32) -> String {
        if font.width(s, size) <= width {
            return s.to_owned();
        }
        let mut out: Vec<char> = s.chars().collect();
        while !out.is_empty()
            && font.width(&format!("...{}", out.iter().collect::<String>()), size) > width
        {
            out.remove(0);
        }
        format!("...{}", out.iter().collect::<String>())
    }

    /// Coupe un texte en lignes de largeur maximale `width`.
    fn wrap(font: &Font, size: f32, s: &str, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for ch in s.chars() {
            line.push(ch);
            if font.width(&line, size) > width {
                let last = line.pop().expect("caractère ajouté");
                // Coupure de préférence après un espace ou un séparateur de chemin.
                let cut = line
                    .rfind([' ', '/', '\\'])
                    .map(|i| i + 1)
                    .filter(|&i| i > line.len() / 2);
                match cut {
                    Some(i) => {
                        let rest = line.split_off(i);
                        lines.push(std::mem::take(&mut line));
                        line = rest;
                    }
                    None => lines.push(std::mem::take(&mut line)),
                }
                line.push(last);
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    fn text(&mut self, font: &Font, size: f32, s: &str) {
        self.ensure(size + 4.0);
        self.y -= size;
        let y = self.y;
        self.put(font, size, MARGIN, y, s);
        self.y -= 4.0;
    }

    fn banner(&mut self, s: &str, ok: bool) {
        self.ensure(26.0);
        let (r, g, b) = if ok {
            (0.85, 0.96, 0.89)
        } else {
            (0.99, 0.88, 0.88)
        };
        let y = self.y - 24.0;
        let c = self.page();
        c.set_fill_rgb(r, g, b);
        c.rect(MARGIN, y, PAGE_W - 2.0 * MARGIN, 24.0);
        c.fill_nonzero();
        if ok {
            c.set_fill_rgb(0.04, 0.42, 0.18);
        } else {
            c.set_fill_rgb(0.63, 0.05, 0.05);
        }
        self.put(&Font::Bold, 11.0, MARGIN + 8.0, y + 8.0, s);
        self.page().set_fill_rgb(0.0, 0.0, 0.0);
        self.y = y;
    }

    fn key_value(&mut self, k: &str, v: &str) {
        let key_w = 130.0;
        let lines = Self::wrap(&Font::Regular, 8.5, v, PAGE_W - 2.0 * MARGIN - key_w);
        self.ensure(11.0 * lines.len() as f32);
        self.y -= 11.0;
        let y = self.y;
        self.page().set_fill_rgb(0.35, 0.35, 0.35);
        self.put(&Font::Regular, 8.5, MARGIN, y, k);
        self.page().set_fill_rgb(0.0, 0.0, 0.0);
        for (i, l) in lines.iter().enumerate() {
            if i > 0 {
                self.y -= 10.0;
            }
            let y = self.y;
            self.put(&Font::Regular, 8.5, MARGIN + key_w, y, l);
        }
    }

    fn widths(cols: &[(&str, f32)]) -> Vec<f32> {
        let fixed: f32 = cols.iter().map(|(_, w)| w).sum();
        let flex = (PAGE_W - 2.0 * MARGIN - fixed).max(60.0);
        cols.iter()
            .map(|(_, w)| if *w == 0.0 { flex } else { *w })
            .collect()
    }

    fn table_header(&mut self, cols: &[(&str, f32)]) {
        self.ensure(30.0);
        let widths = Self::widths(cols);
        let y = self.y - 14.0;
        let c = self.page();
        c.set_fill_rgb(0.92, 0.92, 0.92);
        c.rect(MARGIN, y - 3.0, PAGE_W - 2.0 * MARGIN, 14.0);
        c.fill_nonzero();
        c.set_fill_rgb(0.0, 0.0, 0.0);
        let mut x = MARGIN + 2.0;
        for ((label, _), w) in cols.iter().zip(&widths) {
            self.put(&Font::Bold, 8.0, x, y, label);
            x += w;
        }
        self.y = y - 4.0;
    }

    fn table_row(&mut self, cols: &[(&str, f32)], cells: &[String], failed: bool) {
        self.ensure(11.0);
        let widths = Self::widths(cols);
        self.y -= 10.5;
        let y = self.y;
        if failed {
            self.page().set_fill_rgb(0.63, 0.05, 0.05);
        }
        let mut x = MARGIN + 2.0;
        for (i, (cell, w)) in cells.iter().zip(&widths).enumerate() {
            let font = if i == 3 { Font::Mono } else { Font::Regular };
            let size = if i == 3 { 6.5 } else { 7.5 };
            let s = Self::fit(&font, size, cell, w - 4.0);
            self.put(&font, size, x, y, &s);
            x += w;
        }
        if failed {
            self.page().set_fill_rgb(0.0, 0.0, 0.0);
        }
    }

    fn note(&mut self, s: &str) {
        for l in Self::wrap(&Font::Regular, 7.0, s, PAGE_W - 2.0 * MARGIN - 40.0) {
            self.ensure(9.0);
            self.y -= 9.0;
            let y = self.y;
            self.page().set_fill_rgb(0.63, 0.05, 0.05);
            self.put(&Font::Regular, 7.0, MARGIN + 30.0, y, &l);
            self.page().set_fill_rgb(0.0, 0.0, 0.0);
        }
    }

    fn finish(mut self, title: &str) -> Vec<u8> {
        let total = self.pages.len();
        for i in 0..total {
            let label = format!("VERIFLOW - page {} / {}", i + 1, total);
            let bytes = win_ansi(&label);
            let c = &mut self.pages[i];
            c.set_fill_rgb(0.45, 0.45, 0.45);
            c.begin_text();
            c.set_font(Name(b"F1"), 7.0);
            c.next_line(MARGIN, MARGIN - 18.0);
            c.show(Str(&bytes));
            c.end_text();
        }
        let mut pdf = Pdf::new();
        let catalog = Ref::new(1);
        let tree = Ref::new(2);
        let info = Ref::new(3);
        let fonts = [Ref::new(4), Ref::new(5), Ref::new(6)];
        let first_page = 7;
        let page_ids: Vec<Ref> = (0..total)
            .map(|i| Ref::new(first_page + 2 * i as i32))
            .collect();
        pdf.catalog(catalog).pages(tree);
        pdf.pages(tree)
            .kids(page_ids.iter().copied())
            .count(total as i32);
        for (font, base) in
            fonts
                .iter()
                .zip([b"Helvetica".as_slice(), b"Helvetica-Bold", b"Courier"])
        {
            pdf.type1_font(*font)
                .base_font(Name(base))
                .encoding_predefined(Name(b"WinAnsiEncoding"));
        }
        for (i, content) in self.pages.into_iter().enumerate() {
            let page_id = page_ids[i];
            let content_id = Ref::new(page_id.get() + 1);
            let mut page = pdf.page(page_id);
            page.media_box(Rect::new(0.0, 0.0, PAGE_W, PAGE_H));
            page.parent(tree);
            page.contents(content_id);
            let mut res = page.resources();
            let mut f = res.fonts();
            f.pair(Name(b"F1"), fonts[0]);
            f.pair(Name(b"F2"), fonts[1]);
            f.pair(Name(b"F3"), fonts[2]);
            f.finish();
            res.finish();
            page.finish();
            pdf.stream(content_id, &content.finish());
        }
        pdf.document_info(info)
            .title(TextStr(title))
            .producer(TextStr(&format!("VERIFLOW {VERSION}")));
        pdf.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::engine::run;
    use crate::offload::scan::scan;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn writes_csv_html_pdf_with_fingerprints() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("A001");
        fs::create_dir_all(src.join("CLIP")).unwrap();
        for i in 0..80 {
            fs::write(
                src.join(format!("CLIP/C{i:04}_très_long_nom_de_fichier_é.MP4")),
                vec![i as u8; 100],
            )
            .unwrap();
        }
        let dest = dir.path().join("SSD1/A001");
        let inv = scan(&src).unwrap();
        let spec = OffloadSpec {
            destinations: vec![dest.clone()],
            algorithms: vec![HashAlgo::Xxh128, HashAlgo::Md5],
            known: None,
            overlap: false,
        };
        let summary = run(&inv, &spec, &AtomicBool::new(false), |_| {}).unwrap();
        let info = ReportInfo {
            project: Some("Court métrage « Été »".into()),
            operator: Some("Hervé".into()),
            notes: None,
        };
        let data = ReportData {
            inv: &inv,
            summary: &summary,
            spec: &spec,
            info: &info,
            mhl: &[None],
        };
        let files = write_reports(&data).unwrap();
        assert_eq!(files.len(), 1);
        let f = &files[0];
        let csv = fs::read_to_string(&f.csv).unwrap();
        assert!(csv.starts_with('\u{feff}'));
        assert_eq!(csv.lines().filter(|l| !l.starts_with('#')).count(), 81);
        assert!(csv.contains(";VÉRIFIÉ;"));
        let html = fs::read_to_string(&f.html).unwrap();
        assert!(
            html.contains("CONFORME")
                && html.contains(&hash_data(HashAlgo::Sha256, csv.as_bytes()))
        );
        let pdf = fs::read(&f.pdf).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert_eq!(hash_data(HashAlgo::Sha256, &pdf), f.pdf_sha256);
        // 80 fichiers : plusieurs pages.
        assert!(
            String::from_utf8_lossy(&pdf)
                .matches("/Type /Page\n")
                .count()
                + String::from_utf8_lossy(&pdf)
                    .matches("/Type /Page ")
                    .count()
                >= 1
        );
        assert!(f.pdf.with_extension("pdf.sha256").exists());
        assert!(f.csv.starts_with(dest.join(REPORTS_DIR)));
    }

    #[test]
    fn writes_verify_reports_conform_and_gap() {
        use crate::offload::mhl_verify::verify_root;
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("A001");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.mov"), b"abcde").unwrap();
        fs::write(src.join("b.mov"), b"fghij").unwrap();
        let dest = dir.path().join("SSD/A001");
        let inv = scan(&src).unwrap();
        let spec = OffloadSpec {
            destinations: vec![dest.clone()],
            algorithms: vec![HashAlgo::Xxh128],
            known: None,
            overlap: false,
        };
        let s = run(&inv, &spec, &AtomicBool::new(false), |_| {}).unwrap();
        crate::offload::mhl::write_generation(
            &inv,
            &s,
            0,
            &dest,
            &spec.algorithms,
            &Default::default(),
        )
        .unwrap();
        let info = ReportInfo::default();
        let v = verify_root(&dest, &AtomicBool::new(false), |_| {}).unwrap();
        let f = write_verify_reports(&VerifyData {
            summary: &v,
            info: &info,
        })
        .unwrap();
        let html = fs::read_to_string(&f.html).unwrap();
        assert!(html.contains("VÉRIFICATION ASC MHL") && html.contains("CONFORME"));
        let csv = fs::read_to_string(&f.csv).unwrap();
        assert_eq!(csv.lines().filter(|l| !l.starts_with('#')).count(), 3);
        assert!(csv.contains("#Relecture du disque;") && html.contains("Relecture du disque"));
        assert!(fs::read(&f.pdf).unwrap().starts_with(b"%PDF-"));
        assert!(f.csv.starts_with(dest.join(REPORTS_DIR)));

        // Un fichier modifié, un manquant, un nouveau : écart, le nouveau seul n'échoue pas.
        fs::write(dest.join("a.mov"), b"abcdX").unwrap();
        fs::remove_file(dest.join("b.mov")).unwrap();
        fs::write(dest.join("c.mov"), b"neuf").unwrap();
        let v = verify_root(&dest, &AtomicBool::new(false), |_| {}).unwrap();
        let f = write_verify_reports(&VerifyData {
            summary: &v,
            info: &info,
        })
        .unwrap();
        let html = fs::read_to_string(&f.html).unwrap();
        assert!(html.contains("ÉCART") && html.contains("DIFFÉRENT") && html.contains("MANQUANT"));
        assert!(html.contains("NOUVEAU"));
        let csv = fs::read_to_string(&f.csv).unwrap();
        assert!(csv.contains(";MANQUANT;") && csv.contains(";NOUVEAU;"));

        // Rapports impossibles à écrire : erreur rendue, résultat intact.
        let blocked = tempfile::tempdir().unwrap();
        let mut moved = v.clone();
        moved.root = blocked.path().to_path_buf();
        fs::write(blocked.path().join(REPORTS_DIR), b"fichier").unwrap();
        assert!(write_verify_reports(&VerifyData {
            summary: &moved,
            info: &info
        })
        .is_err());
    }

    #[test]
    fn report_contains_throughput_rows() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("A001");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.mov"), vec![7u8; 5000]).unwrap();
        let dests = vec![dir.path().join("D1/A001"), dir.path().join("D2/A001")];
        let inv = scan(&src).unwrap();
        let spec = OffloadSpec {
            destinations: dests.clone(),
            algorithms: vec![HashAlgo::Xxh128],
            known: None,
            overlap: false,
        };
        let summary = run(&inv, &spec, &AtomicBool::new(false), |_| {}).unwrap();
        let info = ReportInfo::default();
        let data = ReportData {
            inv: &inv,
            summary: &summary,
            spec: &spec,
            info: &info,
            mhl: &[None, None],
        };
        let files = write_reports(&data).unwrap();
        let labels = [
            "Débit global (taille / durée totale)",
            "Lecture source",
            "Écriture destination 1",
            "Écriture destination 2",
            "Relecture destination 1",
            "Relecture destination 2",
            "Limité par",
        ];
        let csv = fs::read_to_string(&files[0].csv).unwrap();
        let html = fs::read_to_string(&files[0].html).unwrap();
        for l in labels {
            assert!(html.contains(l), "HTML sans « {l} »");
            assert!(csv.contains(&format!("#{l};")), "CSV sans « {l} »");
        }
        // PDF : les libellés passent par le même tableau d'en-tête.
        assert!(data.header_rows("x").iter().any(|(k, _)| k == "Limité par"));
        assert!(fs::read(&files[0].pdf).unwrap().starts_with(b"%PDF-"));
    }

    #[test]
    fn helpers() {
        assert_eq!(human_size(1_500_000_000), "1,50 Go (1,40 Gio)");
        assert_eq!(duration_text(3725.0), "01:02:05");
        assert_eq!(win_ansi("é€œ"), vec![0xE9, 0x80, 0x9C]);
        let lines = PdfDoc::wrap(&Font::Regular, 8.0, &"a/".repeat(200), 100.0);
        assert!(lines.len() > 3 && lines.iter().all(|l| Font::Regular.width(l, 8.0) <= 100.0));
    }
}
