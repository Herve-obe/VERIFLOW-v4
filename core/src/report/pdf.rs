//! Rapport PDF : feuillets calculés ici, mise en page par Typst (moteur
//! embarqué, sans dépendance au système). Les gabarits sont dans
//! `templates/reports/` : c'est là qu'on modifie l'apparence des rapports.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use serde::Serialize;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

use super::{columns, header_fields, Column, Lang, Report, ReportKind, Template, TRACKS_PER_SHEET};
use crate::{Error, Result};

const IMAGE_TEMPLATE: &str = include_str!("../../../templates/reports/image.typ");
const SOUND_TEMPLATE: &str = include_str!("../../../templates/reports/son.typ");
const COMMON_TEMPLATE: &str = include_str!("../../../templates/reports/common.typ");
const FONTS: &[&[u8]] = &[
    include_bytes!("../../../templates/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../templates/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../../../templates/fonts/Inter-Bold.ttf"),
    include_bytes!("../../../templates/fonts/Inter-Italic.ttf"),
];

/// Lignes par feuillet (format des rapports papier).
fn rows_per_sheet(kind: ReportKind, template: Template) -> usize {
    match (kind, template) {
        (ReportKind::Image, Template::School) => 22,
        (ReportKind::Image, Template::Pro) => 18,
        (ReportKind::Sound, _) => 15,
    }
}

/// Un feuillet imprimé : ses colonnes et ses lignes (texte des cellules).
#[derive(Debug, Clone, Serialize)]
pub struct Sheet {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
}

/// Autorise la coupure des longs noms sans espace (fichiers, chemins) après
/// « _ », « . » et « / », pour qu'ils ne débordent pas sur la colonne voisine.
fn breakable(s: &str) -> String {
    if s.split_whitespace().all(|w| w.chars().count() <= 14) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        out.push(c);
        if matches!(c, '_' | '.' | '/' | '\\' | '-') {
            out.push('\u{200B}');
        }
    }
    out
}

/// Découpe le rapport en feuillets. Rapport son de plus de 8 pistes : une
/// série de feuillets par groupe de 8 pistes. Modèle École : le dernier
/// feuillet est complété de lignes vides, à remplir à la main sur le plateau.
pub fn sheets(report: &Report, lang: Lang) -> Vec<Sheet> {
    let per = rows_per_sheet(report.kind, report.template);
    let groups: Vec<Vec<usize>> = match report.kind {
        ReportKind::Image => vec![Vec::new()],
        ReportKind::Sound => {
            let n = report.tracks.max(1) as usize;
            (0..n.div_ceil(TRACKS_PER_SHEET))
                .map(|g| {
                    let a = g * TRACKS_PER_SHEET + 1;
                    // Le modèle École garde toujours 8 colonnes de pistes.
                    let b = if report.template == Template::School {
                        a + TRACKS_PER_SHEET - 1
                    } else {
                        (a + TRACKS_PER_SHEET - 1).min(n)
                    };
                    (a..=b).collect()
                })
                .collect()
        }
    };
    let mut out = Vec::new();
    for g in groups {
        let cols = columns(report.kind, report.template, &g, lang);
        let cells: Vec<Vec<String>> = report
            .rows
            .iter()
            .map(|r| cols.iter().map(|c| breakable(r.get(&c.key))).collect())
            .collect();
        let mut chunks: Vec<Vec<Vec<String>>> = cells.chunks(per).map(|c| c.to_vec()).collect();
        if chunks.is_empty() {
            chunks.push(Vec::new());
        }
        if report.template == Template::School {
            let last = chunks.last_mut().expect("au moins un feuillet");
            while last.len() < per {
                last.push(vec![String::new(); cols.len()]);
            }
        }
        for rows in chunks {
            out.push(Sheet {
                columns: cols.clone(),
                rows,
            });
        }
    }
    out
}

/// Case à cocher d'un champ à choix.
#[derive(Debug, Clone, Serialize)]
struct OptionView {
    label: String,
    checked: bool,
}

/// Champ d'en-tête prêt à imprimer.
#[derive(Debug, Clone, Serialize)]
struct FieldView {
    label: String,
    value: String,
    options: Vec<OptionView>,
    /// Valeur hors des cases (« autre : ... »).
    other: String,
    unit: String,
}

/// Date AAAA-MM-JJ affichée JJ / MM / AAAA (autre format : telle quelle).
pub fn display_date(s: &str) -> String {
    let p: Vec<&str> = s.trim().split('-').collect();
    match p[..] {
        [y, m, d] if y.len() == 4 && m.len() == 2 && d.len() == 2 => format!("{d} / {m} / {y}"),
        _ => s.trim().to_string(),
    }
}

fn header_view(report: &Report, lang: Lang) -> BTreeMap<String, FieldView> {
    header_fields(report.kind)
        .iter()
        .map(|f| {
            let raw = report.header(f.key).trim().to_string();
            let value = if f.key == "date" {
                display_date(&raw)
            } else {
                raw.clone()
            };
            let options: Vec<OptionView> = f
                .options
                .iter()
                .map(|o| OptionView {
                    label: (*o).into(),
                    checked: raw == *o,
                })
                .collect();
            let other = if !f.options.is_empty() && !f.options.contains(&raw.as_str()) {
                raw.clone()
            } else {
                String::new()
            };
            let label = if lang == Lang::Fr {
                f.label_fr
            } else {
                f.label_en
            };
            (
                f.key.to_string(),
                FieldView {
                    label: label.into(),
                    value,
                    options,
                    other,
                    unit: f.unit.into(),
                },
            )
        })
        .collect()
}

/// Données envoyées au gabarit Typst (fichier virtuel `data.json`).
#[derive(Debug, Serialize)]
struct Data {
    lang: Lang,
    template: Template,
    number: String,
    fields: BTreeMap<String, FieldView>,
    sheets: Vec<Sheet>,
    has_logo: bool,
    /// Ligne d'en-tête personnalisée (nom de l'école, de la production).
    organization: String,
    footer: String,
}

/// Options de mise en page communes à tous les rapports.
#[derive(Debug, Clone, Default)]
pub struct Branding {
    /// Logo (PNG, JPEG ou SVG) imprimé en haut à gauche.
    pub logo: Option<Vec<u8>>,
    /// Format du logo : « png », « jpg » ou « svg ».
    pub logo_ext: String,
    pub organization: String,
}

struct ReportWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: FileId,
    files: HashMap<FileId, Bytes>,
}

fn file_id(path: &str) -> FileId {
    RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(path).expect("chemin virtuel valide"),
    )
    .intern()
}

/// Polices chargées une seule fois : Inter (texte) et DejaVu Sans Mono
/// (timecodes), fournie avec Typst.
fn fonts() -> &'static (Vec<Font>, LazyHash<FontBook>) {
    static FONTS_CACHE: OnceLock<(Vec<Font>, LazyHash<FontBook>)> = OnceLock::new();
    FONTS_CACHE.get_or_init(|| {
        let mut fonts: Vec<Font> = FONTS
            .iter()
            .flat_map(|d| Font::iter(Bytes::new(d.to_vec())))
            .collect();
        for d in typst_assets::fonts() {
            for f in Font::iter(Bytes::new(d.to_vec())) {
                if f.info().family.starts_with("DejaVu Sans Mono") {
                    fonts.push(f);
                }
            }
        }
        let book = LazyHash::new(FontBook::from_fonts(&fonts));
        (fonts, book)
    })
}

impl World for ReportWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.main
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        let bytes = self.file(id)?;
        let text = std::str::from_utf8(bytes.as_slice())
            .map_err(|_| FileError::InvalidUtf8)?
            .to_string();
        Ok(Source::new(id, text))
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(id.get().vpath().get_without_slash().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        use chrono::Datelike;
        let d = chrono::Local::now().date_naive();
        Datetime::from_ymd(d.year(), d.month() as u8, d.day() as u8)
    }
}

/// Produit le PDF d'un rapport.
pub fn render(report: &Report, lang: Lang, branding: &Branding) -> Result<Vec<u8>> {
    let has_logo = branding.logo.is_some();
    let data = Data {
        lang,
        template: report.template,
        number: if report.number > 0 {
            report.number.to_string()
        } else {
            String::new()
        },
        fields: header_view(report, lang),
        sheets: sheets(report, lang),
        has_logo,
        organization: branding.organization.clone(),
        footer: format!("VERIFLOW {}", crate::VERSION),
    };
    let json = serde_json::to_string(&data).map_err(|e| Error::Report(e.to_string()))?;
    let template = match report.kind {
        ReportKind::Image => IMAGE_TEMPLATE,
        ReportKind::Sound => SOUND_TEMPLATE,
    };
    let main = file_id("/main.typ");
    let mut files = HashMap::new();
    files.insert(main, Bytes::from_string(template.to_string()));
    files.insert(file_id("/data.json"), Bytes::from_string(json));
    files.insert(
        file_id("/common.typ"),
        Bytes::from_string(COMMON_TEMPLATE.to_string()),
    );
    if let Some(logo) = &branding.logo {
        let ext = match branding.logo_ext.to_lowercase().as_str() {
            "jpeg" | "jpg" => "jpg",
            "svg" => "svg",
            _ => "png",
        };
        files.insert(file_id(&format!("/logo.{ext}")), Bytes::new(logo.clone()));
        files.insert(
            file_id("/logo.txt"),
            Bytes::from_string(format!("/logo.{ext}")),
        );
    }
    let (fonts, book) = fonts();
    let world = ReportWorld {
        library: LazyHash::new(Library::default()),
        book: book.clone(),
        fonts: fonts.clone(),
        main,
        files,
    };
    let doc = typst::compile::<PagedDocument>(&world)
        .output
        .map_err(|errors| {
            let msg: Vec<String> = errors.iter().map(|e| e.message.to_string()).collect();
            Error::Report(format!("mise en page : {}", msg.join(" ; ")))
        })?;
    typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).map_err(|errors| {
        let msg: Vec<String> = errors.iter().map(|e| e.message.to_string()).collect();
        Error::Report(format!("écriture du PDF : {}", msg.join(" ; ")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Row;

    fn sample(kind: ReportKind, template: Template, rows: usize) -> Report {
        let mut r = Report::new(kind, template);
        r.number = 3;
        r.header.insert("date".into(), "2026-10-09".into());
        r.header.insert("title".into(), "Le court".into());
        r.header.insert("fps".into(), "25".into());
        r.header.insert("definition".into(), "6K".into());
        for i in 0..rows {
            let mut row = Row::default();
            row.fields.insert("scene".into(), format!("{}", i + 1));
            row.fields.insert("take".into(), "1".into());
            row.fields.insert("tc_in".into(), "10:00:00:00".into());
            row.fields.insert("track_1".into(), "Boom".into());
            row.fields
                .insert("notes".into(), "Très bien, à garder".into());
            r.rows.push(row);
        }
        r
    }

    #[test]
    fn school_sheets_are_filled_with_blank_lines() {
        let s = sheets(&sample(ReportKind::Image, Template::School, 30), Lang::Fr);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].rows.len(), 22, "dernier feuillet complété");
        assert_eq!(s[1].rows[8][0], "", "lignes vides à la suite");
        let pro = sheets(&sample(ReportKind::Image, Template::Pro, 30), Lang::Fr);
        assert_eq!(pro[1].rows.len(), 12, "pas de lignes vides en Pro");
    }

    #[test]
    fn sound_reports_print_eight_tracks_per_sheet() {
        let mut r = sample(ReportKind::Sound, Template::School, 3);
        r.tracks = 12;
        let s = sheets(&r, Lang::Fr);
        assert_eq!(s.len(), 2, "pistes 1-8 puis 9-16");
        assert_eq!(s[1].columns[3].label, "Piste 9");
        assert_eq!(s[1].columns.len(), 3 + 8 + 1);
    }

    #[test]
    fn choices_become_check_boxes_or_other() {
        let r = sample(ReportKind::Image, Template::School, 0);
        let h = header_view(&r, Lang::Fr);
        assert!(h["fps"]
            .options
            .iter()
            .any(|o| o.label == "25" && o.checked));
        assert_eq!(h["definition"].other, "6K");
        assert!(h["definition"].options.iter().all(|o| !o.checked));
        assert_eq!(h["date"].value, "09 / 10 / 2026");
    }

    #[test]
    fn renders_both_reports_to_pdf() {
        for kind in [ReportKind::Image, ReportKind::Sound] {
            for t in [Template::School, Template::Pro] {
                let pdf = render(&sample(kind, t, 25), Lang::Fr, &Branding::default())
                    .unwrap_or_else(|e| panic!("{kind:?} {t:?} : {e}"));
                assert!(pdf.starts_with(b"%PDF"));
            }
        }
    }
}
